//! Temporary keyboard launcher for the native-rendered development image.
//!
//! This owns a byte-exact cache of the one proved framebuffer. It is not a
//! semantic service or a renderer: the isolated native renderer produced every
//! launcher pixel before external interrupts were initialized.

use crate::boot_omen::framebuffer_info;
use crate::console::{screen_console, serial_port};
use crate::input::{
    bootstrap_input_decoder, bootstrap_input_delivery, key_event, key_state,
    physical_input_delivery,
};
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

const frame_width: usize = 1024;
const frame_height: usize = 768;
const maximum_pitch: usize = frame_width * 4;
const cache_capacity: usize = maximum_pitch * frame_height;

/// Unrelated test features keep the accepted terminal-proof precedence.
pub const fn selected() -> bool {
    !cfg!(any(
        feature = "workspace_dev",
        feature = "fault_test",
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "memory_test",
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test"
    ))
}

struct frame_cache_storage(UnsafeCell<[u8; cache_capacity]>);

// SAFETY: A one-time atomic claim grants access to exactly one non-cloneable
// desktop_frame. The sole BSP owns it for the image lifetime. IRQs never access
// this storage, and no user address space maps the kernel's BSS.
unsafe impl Sync for frame_cache_storage {}

static frame_cache: frame_cache_storage = frame_cache_storage(UnsafeCell::new([0; cache_capacity]));
static frame_cache_claimed: AtomicBool = AtomicBool::new(false);

pub struct desktop_frame {
    info: framebuffer_info,
    length: usize,
}

struct enter_activation_gate {
    armed: bool,
}

impl enter_activation_gate {
    const fn new() -> Self {
        Self { armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn observe(&mut self, enter: bool, released: bool) -> bool {
        if !enter {
            return false;
        }
        if released {
            self.armed = true;
            return false;
        }
        let activate = self.armed;
        self.armed = false;
        activate
    }
}

impl desktop_frame {
    /// Capture only after the native presenter's complete readback and teardown.
    ///
    /// # Safety
    /// `info` is the validated exclusively owned framebuffer, all native tasks
    /// and display writers are destroyed, and IF is still clear on the sole BSP.
    pub unsafe fn capture(info: framebuffer_info) -> Result<Self, &'static str> {
        let length = cache_length(info).ok_or("framebuffer_cache_bounds")?;
        if crate::arch::external_interrupts_enabled() {
            return Err("framebuffer_cache_interrupt_state");
        }
        if frame_cache_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("framebuffer_cache_already_owned");
        }
        let source = info.address as *const u8;
        let target = frame_cache.0.get().cast::<u8>();
        for offset in 0..length {
            // SAFETY: Complete aperture and cache extents were checked; the
            // atomic claim and caller exclude every other reader and writer.
            unsafe { ptr::write(target.add(offset), ptr::read_volatile(source.add(offset))) };
        }
        Ok(Self { info, length })
    }

    /// Restore only after the temporary screen_console has been dropped.
    fn restore(&mut self) -> bool {
        let source = frame_cache.0.get().cast::<u8>();
        let target = self.info.address as *mut u8;
        for offset in 0..self.length {
            // SAFETY: This non-cloneable owner retains the checked extents.
            // The shell has returned and dropped its console; normal IRQs do
            // not draw. A terminal diagnostic stops this context permanently.
            unsafe { ptr::write_volatile(target.add(offset), ptr::read(source.add(offset))) };
        }
        for offset in 0..self.length {
            // SAFETY: Same exact uniquely owned extents as the copy above.
            if unsafe { ptr::read_volatile(target.add(offset)) != ptr::read(source.add(offset)) } {
                return false;
            }
        }
        true
    }

    pub fn run(mut self, serial: &mut serial_port, input_available: bool) -> ! {
        if !input_available {
            fail(serial, "input_unavailable");
        }
        let mut input = bootstrap_input_decoder::new();
        let mut activation = enter_activation_gate::new();
        let mut sequence = 0_u64;
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"desktop_dev.ready","level":"info","input_available":true,"activation":"enter","return_command":"desktop","width":{frame_width},"height":{frame_height}}}"#
        );
        loop {
            let delivery = crate::arch::with_external_interrupts_disabled(|| {
                // SAFETY: The architecture critical section excludes IRQs on
                // the sole BSP; NMI remains masked throughout this image.
                unsafe { crate::input::poll_physical_input_if_disabled() }
            })
            .unwrap_or_else(|| fail(serial, "critical_section_unavailable"));
            let Some(delivery) = delivery else {
                if !crate::arch::wait_for_interrupt_if_idle(|| {
                    // SAFETY: The idle primitive runs this predicate with IF
                    // clear under the same single-BSP exclusion contract.
                    unsafe { crate::input::ps2_delivery_ready_if_disabled() }
                }) {
                    fail(serial, "interrupt_wait_unavailable");
                }
                continue;
            };
            let fresh_enter = match &delivery {
                physical_input_delivery::transition(record) => {
                    let transition = record.payload();
                    activation.observe(
                        transition.key().make_code() == 0x1c && !transition.key().is_extended(),
                        transition.state() == key_state::released,
                    )
                }
                physical_input_delivery::loss(_) => {
                    activation.disarm();
                    false
                }
            };
            let Some(delivery) = input.consume(delivery) else {
                continue;
            };
            let key = match delivery {
                bootstrap_input_delivery::key(record) => record.into_payload(),
                bootstrap_input_delivery::loss(loss) => {
                    if loss.terminal_reason().is_some() {
                        fail(serial, "source_identity_exhausted");
                    }
                    let generation = crate::arch::with_external_interrupts_disabled(|| {
                        crate::arch::rearm_keyboard_input_after_loss()
                    })
                    .flatten()
                    .unwrap_or_else(|| fail(serial, "source_rearm_failed"));
                    let _ = writeln!(
                        serial,
                        r#"{{"schema":1,"event":"desktop_dev.input.loss","level":"warn","source_generation":{},"lost_count":{},"rearmed":true,"new_source_generation":{generation}}}"#,
                        loss.source().generation(),
                        loss.lost_count()
                    );
                    continue;
                }
            };
            if !fresh_enter || !opens_shell(key) {
                continue;
            }
            sequence =
                next_sequence(sequence).unwrap_or_else(|| fail(serial, "sequence_exhausted"));
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"desktop_dev.shell_opened","level":"info","sequence":{sequence}}}"#
            );
            {
                // SAFETY: The cache is inert while this scoped console owns
                // the aperture. Its returning session drops the console before
                // restoration; no native presenter or user task remains.
                let screen = unsafe { screen_console::with_framebuffer(self.info) };
                crate::shell::run_desktop_session(screen, serial, &mut input);
            }
            // The Enter that submitted `desktop` may still be held. Its
            // remaining repeats cannot activate the newly restored launcher;
            // require the physical release before accepting another press.
            activation.disarm();
            if !self.restore() {
                fail(serial, "framebuffer_restore_mismatch");
            }
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"desktop_dev.restored","level":"info","sequence":{sequence}}}"#
            );
        }
    }
}

fn cache_length(info: framebuffer_info) -> Option<usize> {
    if info.width != frame_width
        || info.height != frame_height
        || !matches!(info.bits_per_pixel, 24 | 32)
        || info.address == 0
    {
        return None;
    }
    let minimum_pitch = info
        .width
        .checked_mul(usize::from(info.bits_per_pixel) / 8)?;
    let length = info.pitch.checked_mul(info.height)?;
    let end = info.address.checked_add(length as u64)?;
    if info.pitch < minimum_pitch
        || info.pitch > maximum_pitch
        || length > cache_capacity
        || end > 4 * 1024 * 1024 * 1024
    {
        return None;
    }
    Some(length)
}

fn opens_shell(key: key_event) -> bool {
    matches!(key, key_event::enter)
}

fn next_sequence(sequence: u64) -> Option<u64> {
    sequence.checked_add(1)
}

pub fn fail(serial: &mut serial_port, reason: &'static str) -> ! {
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"desktop_dev.failed","level":"fatal","reason":"{reason}"}}"#
    );
    crate::arch::disable_external_interrupts();
    crate::arch::qemu_exit_failure();
    crate::arch::halt_forever()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_omen::color_channel;

    fn framebuffer() -> framebuffer_info {
        framebuffer_info {
            address: 0xe000_0000,
            pitch: 3072,
            width: 1024,
            height: 768,
            bits_per_pixel: 24,
            red: color_channel {
                position: 16,
                size: 8,
            },
            green: color_channel {
                position: 8,
                size: 8,
            },
            blue: color_channel {
                position: 0,
                size: 8,
            },
        }
    }

    #[test]
    fn cache_covers_complete_24_and_32_bit_apertures_including_padding() {
        let mut info = framebuffer();
        assert_eq!(cache_length(info), Some(3072 * 768));
        info.pitch = 4096;
        assert_eq!(cache_length(info), Some(cache_capacity));
        info.bits_per_pixel = 32;
        assert_eq!(cache_length(info), Some(cache_capacity));
    }

    #[test]
    fn cache_rejects_short_oversize_overflowing_and_foreign_geometry() {
        let original = framebuffer();
        for pitch in [0, 3071, 4097, usize::MAX] {
            assert_eq!(cache_length(framebuffer_info { pitch, ..original }), None);
        }
        for address in [0, u64::MAX, 0xffff_ffff] {
            assert_eq!(
                cache_length(framebuffer_info {
                    address,
                    ..original
                }),
                None
            );
        }
        for bits_per_pixel in [0, 16, 25, 33] {
            assert_eq!(
                cache_length(framebuffer_info {
                    bits_per_pixel,
                    ..original
                }),
                None
            );
        }
        assert_eq!(
            cache_length(framebuffer_info {
                width: 1023,
                ..original
            }),
            None
        );
        assert_eq!(
            cache_length(framebuffer_info {
                height: 767,
                ..original
            }),
            None
        );
        assert_eq!(
            cache_length(framebuffer_info {
                bits_per_pixel: 32,
                ..original
            }),
            None
        );
    }

    #[test]
    fn only_enter_activates_the_single_enabled_launcher_control() {
        assert!(opens_shell(key_event::enter));
        assert!(!opens_shell(key_event::backspace));
        for byte in 0..=u8::MAX {
            assert!(!opens_shell(key_event::character(byte)));
        }
    }

    #[test]
    fn session_identity_exhaustion_never_wraps() {
        assert_eq!(next_sequence(0), Some(1));
        assert_eq!(next_sequence(u64::MAX - 1), Some(u64::MAX));
        assert_eq!(next_sequence(u64::MAX), None);
    }

    #[test]
    fn returning_to_desktop_requires_enter_release_before_another_activation() {
        let mut gate = enter_activation_gate::new();
        assert!(gate.observe(true, false));
        gate.disarm();
        assert!(!gate.observe(true, false));
        assert!(!gate.observe(true, false));
        assert!(!gate.observe(false, true));
        assert!(!gate.observe(true, false));
        assert!(!gate.observe(true, true));
        assert!(gate.observe(true, false));
        assert!(!gate.observe(true, false));
        gate.disarm();
        assert!(!gate.observe(true, false));
    }

    #[cfg(not(any(
        feature = "fault_test",
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "memory_test",
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test"
    )))]
    #[test]
    fn desktop_development_build_selects_interactive_entry() {
        assert!(selected());
    }

    #[cfg(any(
        feature = "fault_test",
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "memory_test",
        feature = "interrupt_test",
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test"
    ))]
    #[test]
    fn combined_terminal_features_keep_the_terminal_proof_entry() {
        assert!(!selected());
    }
}
