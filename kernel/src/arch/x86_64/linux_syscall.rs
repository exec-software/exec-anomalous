#![allow(dead_code)]

//! Feature-private long-mode SYSCALL entry ownership, not Linux service policy.
//! The sole BSP owns the MSRs and two supervisor scratch words with IRQ/NMI
//! excluded. Kernel checks require all FS/GS/KERNEL_GS bases zero. In the opt-in
//! TLS composition, the separate TLS owner captures and scrubs user FS before
//! those checks. No SWAPGS or user-stack write is needed. The inspected entry
//! builds the existing frame and returns by IRETQ.
//!
//! ISA references: Intel SDM Vol.2, SYSCALL; AMD APM Vol.2 section6.1.1.
//! Linux's arch/x86/entry/entry_64.S likewise saves user RSP and selects trusted
//! storage before pushes. This bounded entry does not adopt Linux's general
//! per-CPU, signal, tracing, compatibility-mode or speculation policy.

use core::{
    arch::asm,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use super::scheduler_test::scheduler_frame;
use super::{descriptors, paging};

pub(super) const syscall_vector: u64 = 0x100;
const efer_msr: u32 = 0xc000_0080;
const star_msr: u32 = 0xc000_0081;
const lstar_msr: u32 = 0xc000_0082;
const fmask_msr: u32 = 0xc000_0084;
const sce: u64 = 1;
const stack_size: u64 = 16 * 1024;
// Mask arithmetic state as well as IF/DF/TF/AC and privilege-sensitive flags.
// Saved user flags stay in R11 for the normalized frame; these are kernel flags.
const entry_flag_mask: u64 = (1 << 0)
    | (1 << 2)
    | (1 << 4)
    | (1 << 6)
    | (1 << 7)
    | (1 << 8)
    | (1 << 9)
    | (1 << 10)
    | (1 << 11)
    | (3 << 12)
    | (1 << 14)
    | (1 << 16)
    | (1 << 18)
    | (1 << 21);
// SYSCALL CS=0x08, SS=CS+8=0x10. The unused SYSRET selectors would be
// CS=0x23+16=0x33, SS=0x23+8=0x2b; all actual returns use checked IRETQ.
const entry_star: u64 = (0x23 << 48) | (0x08 << 32);

static storage_owned: AtomicBool = AtomicBool::new(false);

// SAFETY: Only the retained session writes the stack word, and only the entry
// assembly writes the scratch word, under the sole-BSP nonnested contract.
// These exact u64 objects are supervisor-only in every admitted resident root.
#[unsafe(no_mangle)]
static mut x86_64_linux_syscall_stack_top: u64 = 0;
#[unsafe(no_mangle)]
static mut x86_64_linux_syscall_user_stack: u64 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct msr_snapshot {
    efer: u64,
    star: u64,
    lstar: u64,
    fmask: u64,
}

impl msr_snapshot {
    const fn zero() -> Self {
        Self {
            efer: 0,
            star: 0,
            lstar: 0,
            fmask: 0,
        }
    }

    fn read() -> Self {
        Self {
            efer: read_msr(efer_msr),
            star: read_msr(star_msr),
            lstar: read_msr(lstar_msr),
            fmask: read_msr(fmask_msr),
        }
    }

    fn operating(self, target: u64, enabled: bool) -> Self {
        Self {
            efer: (self.efer & !sce) | u64::from(enabled),
            star: entry_star,
            lstar: target,
            fmask: entry_flag_mask,
        }
    }
}

#[must_use = "retain the MSR owner through explicit finish, including errors"]
pub(super) struct syscall_session {
    original: msr_snapshot,
    target: u64,
    selected_stack: u64,
    enabled: bool,
    active: bool,
    restored: bool,
}

impl syscall_session {
    pub(super) const fn new() -> Self {
        Self {
            original: msr_snapshot::zero(),
            target: 0,
            selected_stack: 0,
            enabled: false,
            active: false,
            restored: false,
        }
    }

    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// # Safety
    /// Sole BSP, IF clear/NMI masked; complete kernel text/data and this owner
    /// stay supervisor-resident until finish. CPU legacy-state preparation has
    /// already established operating_efer with SCE clear. Retain on every Err.
    pub(super) unsafe fn prepare(&mut self, operating_efer: u64) -> Result<(), &'static str> {
        use core::arch::x86_64::__cpuid;

        if self.active || !interrupts_are_clear() || !zero_bases() || !self.storage_is_cleared() {
            return Err("linux_syscall_prepare_state");
        }
        let maximum = __cpuid(0x8000_0000).eax;
        if maximum < 0x8000_0001 || __cpuid(0x8000_0001).edx & (1 << 11) == 0 {
            return Err("linux_syscall_capability");
        }
        let target = entry_address();
        if !entry_is_resident(target) {
            return Err("linux_syscall_target");
        }
        let original = msr_snapshot::read();
        if original.efer != operating_efer || original.efer & sce != 0 {
            return Err("linux_syscall_efer");
        }
        if storage_owned
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("linux_syscall_owner");
        }
        // Retain the original complete MSR values before the first mutation.
        self.original = original;
        self.target = target;
        self.selected_stack = 0;
        self.enabled = false;
        self.restored = false;
        self.active = true;
        let values = original.operating(target, false);
        unsafe {
            write_msr(efer_msr, values.efer);
            write_msr(star_msr, values.star);
            write_msr(lstar_msr, values.lstar);
            write_msr(fmask_msr, values.fmask);
        }
        if !self.controls_are_active(operating_efer) {
            return Err("linux_syscall_install_readback");
        }
        Ok(())
    }

    pub(super) fn controls_are_active(&self, operating_efer: u64) -> bool {
        self.active
            && interrupts_are_clear()
            && zero_bases()
            && self.original.efer == operating_efer
            && storage_owned.load(Ordering::Acquire)
            && msr_snapshot::read() == self.original.operating(self.target, self.enabled)
            && shared_stack() == self.selected_stack
            && shared_scratch() == 0
    }

    /// # Safety
    /// IF clear/NMI masked, previous user FP state captured. stack_top is the
    /// newly selected task's retained 16KiB supervisor stack, identically mapped
    /// in both roots, and is already the exact current TSS.RSP0. No untrusted
    /// pointer may supply it. SCE is enabled only for the selected Linux task.
    pub(super) unsafe fn select(&mut self, stack_top: u64, enabled: bool) -> bool {
        if !self.controls_are_active(self.original.efer)
            || !stack_is_valid(stack_top)
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
        {
            return false;
        }
        // Disable the instruction before changing its stack dependency. No user
        // or nested IRQ/NMI can observe these transitional supervisor writes.
        unsafe { write_msr(efer_msr, self.original.efer & !sce) };
        let selected = if enabled { stack_top } else { 0 };
        unsafe { ptr::write_volatile(ptr::addr_of_mut!(x86_64_linux_syscall_stack_top), selected) };
        self.selected_stack = selected;
        self.enabled = enabled;
        unsafe { write_msr(efer_msr, self.original.operating(self.target, enabled).efer) };
        self.controls_are_active(self.original.efer)
    }

    pub(super) fn selection_matches(&self, stack_top: u64, enabled: bool) -> bool {
        self.enabled == enabled
            && self.selected_stack == if enabled { stack_top } else { 0 }
            && self.controls_are_active(self.original.efer)
    }

    /// # Safety
    /// Last user state captured, no user may resume, IF clear/NMI masked. All
    /// stacks and this owner remain retained. On false retain the failed owner.
    pub(super) unsafe fn finish(&mut self) -> bool {
        if !self.controls_are_active(self.original.efer) {
            return false;
        }
        unsafe {
            write_msr(efer_msr, self.original.efer & !sce);
            write_msr(star_msr, self.original.star);
            write_msr(lstar_msr, self.original.lstar);
            write_msr(fmask_msr, self.original.fmask);
            write_msr(efer_msr, self.original.efer);
            ptr::write_volatile(ptr::addr_of_mut!(x86_64_linux_syscall_stack_top), 0);
            ptr::write_volatile(ptr::addr_of_mut!(x86_64_linux_syscall_user_stack), 0);
        }
        if msr_snapshot::read() != self.original || !self.storage_is_cleared() {
            return false;
        }
        self.active = false;
        self.enabled = false;
        self.selected_stack = 0;
        self.restored = true;
        storage_owned.store(false, Ordering::Release);
        true
    }

    pub(super) fn storage_is_cleared(&self) -> bool {
        shared_stack() == 0 && shared_scratch() == 0
    }

    // cpu_session independently restores its pre-FFXSR EFER after our finish.
    // Only the three entry MSRs are compared here to avoid conflating owners.
    pub(super) fn restoration_is_verified(&self) -> bool {
        !self.active
            && self.restored
            && !storage_owned.load(Ordering::Acquire)
            && self.storage_is_cleared()
            && read_msr(star_msr) == self.original.star
            && read_msr(lstar_msr) == self.original.lstar
            && read_msr(fmask_msr) == self.original.fmask
    }
}

/// Check fresh hardware-derived fields before the dispatcher modifies any reply.
/// User RIP/RSP/code ownership and acceptable return flags require its separate
/// retained-frame admission. This function supplies no pointer dereference.
pub(super) fn entry_frame_matches_hardware(frame: scheduler_frame) -> bool {
    frame.vector == syscall_vector
        && frame.error_code == 0
        && frame.code_segment == u64::from(descriptors::user_code_selector)
        && frame.stack_segment == u64::from(descriptors::user_data_selector)
        && frame.counter == frame.instruction_pointer
        && frame.register_11 == frame.flags
}

fn stack_is_valid(top: u64) -> bool {
    top.checked_sub(stack_size).is_some_and(|base| {
        base != 0
            && top.is_multiple_of(16)
            && paging::is_canonical(base)
            && paging::is_canonical(top)
    })
}

fn shared_stack() -> u64 {
    // SAFETY: Sole-BSP ownership excludes concurrent entry installation.
    unsafe { ptr::read_volatile(ptr::addr_of!(x86_64_linux_syscall_stack_top)) }
}

fn shared_scratch() -> u64 {
    // SAFETY: Sole-BSP checks run after entry constructed its frame and cleared
    // this word, never during the few entry instructions that temporarily use it.
    unsafe { ptr::read_volatile(ptr::addr_of!(x86_64_linux_syscall_user_stack)) }
}

fn interrupts_are_clear() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags)) };
    flags & (1 << 9) == 0
}

fn zero_bases() -> bool {
    [0xc000_0100, 0xc000_0101, 0xc000_0102]
        .into_iter()
        .all(|msr| read_msr(msr) == 0)
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    // SAFETY: Only the admitted CPL0 CPU/session path invokes hardware helpers.
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high,
            options(nomem, nostack, preserves_flags));
    }
    (u64::from(high) << 32) | u64::from(low)
}

unsafe fn write_msr(msr: u32, value: u64) {
    // SAFETY: Caller owns this exact MSR for the excluded sole-BSP session.
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32,
            options(nostack, preserves_flags));
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    fn x86_64_linux_syscall_entry();
    static kernel_text_start: u8;
    static kernel_text_end: u8;
}

#[cfg(not(test))]
fn entry_address() -> u64 {
    x86_64_linux_syscall_entry as *const () as u64
}

#[cfg(test)]
fn entry_address() -> u64 {
    0
}

#[cfg(not(test))]
fn entry_is_resident(target: u64) -> bool {
    let start: u64;
    let end: u64;
    unsafe {
        asm!("lea {start}, [rip + {text_start}]", "lea {end}, [rip + {text_end}]",
            start = out(reg) start, end = out(reg) end,
            text_start = sym kernel_text_start, text_end = sym kernel_text_end,
            options(nomem, nostack, preserves_flags));
    }
    start <= target && target < end && paging::is_canonical(target)
}

#[cfg(test)]
fn entry_is_resident(_: u64) -> bool {
    false
}

#[cfg(all(
    feature = "linux_application_test",
    not(test),
    not(any(
        feature = "fault_test",
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "memory_test",
        feature = "interrupt_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test",
        feature = "semantic_surface_presentation_test",
        feature = "desktop_dev"
    ))
))]
core::arch::global_asm!(
    include_str!("linux_syscall_entry.S"),
    owned_tls_enabled = const cfg!(all(not(test), any(feature = "owned_tls_test", feature = "linux_tls_test"))) as usize,
    options(att_syntax)
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_entry_controls_preserve_all_efer_bits_except_owned_sce() {
        let original = msr_snapshot {
            efer: 0xd00,
            star: 0x11223344,
            lstar: 0x55667788,
            fmask: 0xff,
        };
        for enabled in [false, true] {
            let selected = original.operating(0x123456, enabled);
            assert_eq!(selected.efer, original.efer | u64::from(enabled));
            assert_eq!(selected.star, entry_star);
            assert_eq!(selected.lstar, 0x123456);
            assert_eq!(selected.fmask, entry_flag_mask);
        }
        assert_eq!(
            original,
            msr_snapshot {
                efer: 0xd00,
                star: 0x11223344,
                lstar: 0x55667788,
                fmask: 0xff
            }
        );
        for bit in 1..64 {
            let source = msr_snapshot {
                efer: 1 << bit,
                ..original
            };
            assert_eq!(source.operating(0x100000, false).efer, 1 << bit);
            assert_eq!(source.operating(0x100000, true).efer, (1 << bit) | 1);
        }
    }

    #[test]
    fn entry_mask_and_selector_layout_match_the_installed_64_bit_gdt() {
        for bit in [8, 9, 10, 12, 13, 14, 16, 18] {
            assert_ne!(entry_flag_mask & (1 << bit), 0);
        }
        assert_eq!(entry_flag_mask & 2, 0);
        assert_eq!(
            (entry_star >> 32) & 0xffff,
            u64::from(descriptors::kernel_code_selector)
        );
        assert_eq!(
            ((entry_star >> 48) + 16) as u16,
            descriptors::user_code_selector
        );
        assert_eq!(
            ((entry_star >> 48) + 8) as u16,
            descriptors::user_data_selector
        );
        assert_eq!(core::mem::size_of::<scheduler_frame>(), 176);
        assert_eq!(core::mem::offset_of!(scheduler_frame, counter), 16);
        assert_eq!(core::mem::offset_of!(scheduler_frame, register_11), 80);
        assert_eq!(
            core::mem::offset_of!(scheduler_frame, instruction_pointer),
            136
        );
        assert_eq!(core::mem::offset_of!(scheduler_frame, flags), 152);
        assert_eq!(core::mem::offset_of!(scheduler_frame, stack_pointer), 160);
    }

    #[test]
    fn kernel_stack_geometry_rejects_overflow_holes_and_misalignment() {
        assert!(stack_is_valid(0xffff_9000_0040_0000));
        assert!(stack_is_valid(0x200000));
        for top in [
            0,
            stack_size - 1,
            stack_size,
            0x200008,
            0x0000_8000_0000_0000,
            0xffff_8000_0000_0010,
            u64::MAX,
        ] {
            assert!(!stack_is_valid(top), "{top:#x}");
        }
    }

    #[test]
    fn fresh_frame_requires_exact_hardware_clobbers_without_constraining_arguments() {
        let mut frame = scheduler_frame::initial(0x400020);
        frame.vector = syscall_vector;
        frame.flags = 0x202;
        frame.counter = frame.instruction_pointer;
        frame.register_11 = frame.flags;
        frame.accumulator = 999;
        frame.destination_index = u64::MAX;
        frame.register_10 = 0x123456;
        assert!(entry_frame_matches_hardware(frame));
        for changed in [
            scheduler_frame {
                vector: 0xf4,
                ..frame
            },
            scheduler_frame {
                error_code: 1,
                ..frame
            },
            scheduler_frame {
                code_segment: 8,
                ..frame
            },
            scheduler_frame {
                stack_segment: 16,
                ..frame
            },
            scheduler_frame {
                counter: frame.counter + 1,
                ..frame
            },
            scheduler_frame {
                register_11: frame.register_11 ^ 1,
                ..frame
            },
        ] {
            assert!(!entry_frame_matches_hardware(changed));
        }
    }
}
