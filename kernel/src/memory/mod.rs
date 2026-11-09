pub(crate) mod physical;

#[cfg(not(test))]
use crate::boot_omen::framebuffer_info;
use crate::boot_omen::memory_region;
#[cfg(all(
    not(test),
    any(
        feature = "memory_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "ny_vessel_test",
        feature = "display_vessel_test",
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        )
    )
))]
use core::sync::atomic::compiler_fence;
#[cfg(not(test))]
use core::{
    mem::MaybeUninit,
    ptr,
    sync::atomic::{AtomicU8, Ordering},
};
#[cfg(any(
    not(test),
    all(
        test,
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        )
    )
))]
use physical::frame_allocator;
use physical::{frame_accounting, frame_error, physical_range};

pub(crate) const address_limit: u64 = 4 * 1024 * 1024 * 1024;
pub(crate) const low_memory_limit: u64 = 1024 * 1024;
#[cfg(not(test))]
const ledger_byte_count: usize =
    (address_limit as usize / physical::frame_size as usize) / physical::frames_per_ledger_byte;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum memory_error {
    invalid_kernel_extent,
    kernel_frame_not_usable,
    invalid_framebuffer_extent,
    framebuffer_conflict,
    no_free_frames,
    already_initialized,
    #[cfg(any(
        all(
            test,
            any(
                feature = "scheduler_test",
                feature = "supervision_test",
                feature = "shell_vessel_test",
                feature = "input_wait_test",
                feature = "supervised_renderer_test",
                feature = "semantic_surface_transport_test"
            )
        ),
        all(
            not(test),
            any(
                feature = "memory_test",
                feature = "control_test",
                feature = "rite_test",
                feature = "ny_vessel_test",
                feature = "display_vessel_test",
                any(
                    feature = "scheduler_test",
                    feature = "supervision_test",
                    feature = "shell_vessel_test",
                    feature = "input_wait_test",
                    feature = "supervised_renderer_test",
                    feature = "semantic_surface_transport_test"
                )
            )
        )
    ))]
    allocator_busy,
    frame(frame_error),
}

impl memory_error {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::invalid_kernel_extent => "invalid_kernel_extent",
            Self::kernel_frame_not_usable => "kernel_frame_not_usable",
            Self::invalid_framebuffer_extent => "invalid_framebuffer_extent",
            Self::framebuffer_conflict => "framebuffer_conflict",
            Self::no_free_frames => "no_free_frames",
            Self::already_initialized => "already_initialized",
            #[cfg(any(
                all(
                    test,
                    any(
                        feature = "scheduler_test",
                        feature = "supervision_test",
                        feature = "shell_vessel_test",
                        feature = "input_wait_test",
                        feature = "supervised_renderer_test",
                        feature = "semantic_surface_transport_test"
                    )
                ),
                all(
                    not(test),
                    any(
                        feature = "memory_test",
                        feature = "control_test",
                        feature = "rite_test",
                        feature = "ny_vessel_test",
                        feature = "display_vessel_test",
                        any(
                            feature = "scheduler_test",
                            feature = "supervision_test",
                            feature = "shell_vessel_test",
                            feature = "input_wait_test",
                            feature = "supervised_renderer_test",
                            feature = "semantic_surface_transport_test"
                        )
                    )
                )
            ))]
            Self::allocator_busy => "allocator_busy",
            Self::frame(_) => "frame_normalization_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct memory_state {
    pub region_count: usize,
    pub frame_size: u64,
    pub address_limit: u64,
    pub total_frames: usize,
    pub accounting: frame_accounting,
}

// Unit tests exercise normalization through the pure helpers below; they never
// enter `kernel_main` or publish a linker-backed global allocator.
#[cfg(test)]
pub(crate) fn initialize(
    _regions: &[memory_region],
    _framebuffer: Option<crate::boot_omen::framebuffer_info>,
) -> Result<memory_state, memory_error> {
    Err(memory_error::already_initialized)
}

#[cfg(not(test))]
#[repr(C, align(4096))]
struct aligned_ledger([u8; ledger_byte_count]);

#[cfg(not(test))]
const allocator_uninitialized: u8 = 0;
#[cfg(not(test))]
const allocator_initializing: u8 = 1;
#[cfg(not(test))]
const allocator_ready: u8 = 2;
#[cfg(not(test))]
#[cfg(any(
    feature = "memory_test",
    feature = "control_test",
    feature = "rite_test",
    feature = "ny_vessel_test",
    feature = "display_vessel_test",
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    )
))]
const allocator_borrowed: u8 = 3;

#[cfg(not(test))]
static allocator_state: AtomicU8 = AtomicU8::new(allocator_uninitialized);

#[cfg(not(test))]
#[used]
#[unsafe(link_section = ".bss.memory.frame_ledger")]
static mut frame_ledger: aligned_ledger = aligned_ledger([0; ledger_byte_count]);

#[cfg(not(test))]
static mut published_allocator: MaybeUninit<frame_allocator<'static>> = MaybeUninit::uninit();

#[cfg(not(test))]
unsafe extern "C" {
    static kernel_image_start: u8;
    static kernel_memory_end: u8;
}

/// Validate and publish the bootstrap processor's bounded frame allocator.
///
/// Publication is one-shot. Threshold enters with maskable interrupts disabled,
/// and the kernel keeps NMI masked, so no second execution context can observe
/// the temporary `initializing` state in this milestone.
#[cfg(not(test))]
pub(crate) fn initialize(
    regions: &[memory_region],
    framebuffer: Option<framebuffer_info>,
) -> Result<memory_state, memory_error> {
    if allocator_state
        .compare_exchange(
            allocator_uninitialized,
            allocator_initializing,
            Ordering::Acquire,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err(memory_error::already_initialized);
    }

    let result = initialize_inner(regions, framebuffer);
    if result.is_err() {
        allocator_state.store(allocator_uninitialized, Ordering::Release);
    }
    result
}

#[cfg(not(test))]
fn initialize_inner(
    regions: &[memory_region],
    framebuffer: Option<framebuffer_info>,
) -> Result<memory_state, memory_error> {
    let kernel_start = ptr::addr_of!(kernel_image_start) as u64;
    let kernel_end = ptr::addr_of!(kernel_memory_end) as u64;
    let kernel_length = kernel_end
        .checked_sub(kernel_start)
        .ok_or(memory_error::invalid_kernel_extent)?;
    let kernel = physical_range::new(kernel_start, kernel_length);
    let framebuffer = framebuffer.map(framebuffer_range).transpose()?;
    let reservations = validate_bootstrap_layout(regions, kernel, framebuffer)?;

    // SAFETY: This is the one successful transition from uninitialized state.
    // No reference to the ledger exists before publication, and its storage has
    // static lifetime and exact four-GiB coverage.
    let ledger = unsafe {
        let ledger_pointer = ptr::addr_of_mut!(frame_ledger).cast::<u8>();
        core::slice::from_raw_parts_mut(ledger_pointer, ledger_byte_count)
    };
    let allocator =
        frame_allocator::initialize(ledger, regions, &reservations).map_err(memory_error::frame)?;
    let accounting = allocator.accounting();
    if accounting.free == 0 {
        return Err(memory_error::no_free_frames);
    }
    let represented_address_limit = allocator.maximum_physical_extent();
    let state = memory_state {
        region_count: regions.len(),
        frame_size: physical::frame_size,
        address_limit: represented_address_limit,
        total_frames: represented_address_limit as usize / physical::frame_size as usize,
        accounting,
    };

    // SAFETY: `published_allocator` is written once before the release store
    // makes it visible. Its borrowed ledger has static lifetime.
    unsafe { ptr::write(ptr::addr_of_mut!(published_allocator).cast(), allocator) };
    allocator_state.store(allocator_ready, Ordering::Release);
    Ok(state)
}

#[cfg(not(test))]
fn framebuffer_range(framebuffer: framebuffer_info) -> Result<physical_range, memory_error> {
    let length = framebuffer
        .pitch
        .checked_mul(framebuffer.height)
        .and_then(|length| u64::try_from(length).ok())
        .ok_or(memory_error::invalid_framebuffer_extent)?;
    let range = physical_range::new(framebuffer.address, length);
    if length == 0 || range.end().is_none_or(|end| end > address_limit) {
        return Err(memory_error::invalid_framebuffer_extent);
    }
    Ok(range)
}

/// Decide whether an already structure-validated framebuffer is safe enough
/// for a last-resort human-visible diagnostic before allocator publication.
#[cfg(not(test))]
pub(crate) fn framebuffer_is_safe_for_diagnostics(framebuffer: framebuffer_info) -> bool {
    let kernel_start = ptr::addr_of!(kernel_image_start) as u64;
    let kernel_end = ptr::addr_of!(kernel_memory_end) as u64;
    let Some(kernel_length) = kernel_end.checked_sub(kernel_start) else {
        return false;
    };
    if kernel_length == 0 || kernel_start < low_memory_limit || kernel_end > address_limit {
        return false;
    }
    let Ok(framebuffer) = framebuffer_range(framebuffer) else {
        return false;
    };
    !ranges_overlap(framebuffer, physical_range::new(0, low_memory_limit))
        && !ranges_overlap(
            framebuffer,
            physical_range::new(kernel_start, kernel_length),
        )
}

#[cfg(test)]
pub(crate) const fn framebuffer_is_safe_for_diagnostics(
    _framebuffer: crate::boot_omen::framebuffer_info,
) -> bool {
    false
}

fn validate_bootstrap_layout(
    regions: &[memory_region],
    kernel: physical_range,
    framebuffer: Option<physical_range>,
) -> Result<[physical_range; 3], memory_error> {
    let Some(kernel_end) = kernel.end() else {
        return Err(memory_error::invalid_kernel_extent);
    };
    if kernel.length == 0
        || kernel.base < low_memory_limit
        || kernel.base & (physical::frame_size - 1) != 0
        || kernel.length & (physical::frame_size - 1) != 0
        || kernel_end > address_limit
    {
        return Err(memory_error::invalid_kernel_extent);
    }

    let mut frame = kernel.base;
    while frame < kernel_end {
        if !frame_is_usable(regions, frame) {
            return Err(memory_error::kernel_frame_not_usable);
        }
        frame += physical::frame_size;
    }

    let framebuffer = framebuffer.unwrap_or(physical_range::new(0, 0));
    if framebuffer.length != 0 {
        let Some(framebuffer_end) = framebuffer.end() else {
            return Err(memory_error::invalid_framebuffer_extent);
        };
        if framebuffer_end > address_limit
            || ranges_overlap(framebuffer, physical_range::new(0, low_memory_limit))
            || ranges_overlap(framebuffer, kernel)
        {
            return Err(memory_error::framebuffer_conflict);
        }
    }

    Ok([
        physical_range::new(0, low_memory_limit),
        kernel,
        framebuffer,
    ])
}

fn frame_is_usable(regions: &[memory_region], frame: u64) -> bool {
    let frame_range = physical_range::new(frame, physical::frame_size);
    let frame_end = frame + physical::frame_size;
    let granted = regions.iter().any(|region| {
        region.kind() == 1
            && region.attributes() == 1
            && region.length() != 0
            && region.base() <= frame
            && region.end().is_some_and(|end| end >= frame_end)
    });
    let blocked = regions.iter().any(|region| {
        region.attributes() & 1 != 0
            && region.length() != 0
            && !(region.kind() == 1 && region.attributes() == 1)
            && region.end().is_some_and(|end| {
                ranges_overlap(
                    frame_range,
                    physical_range::new(region.base(), end - region.base()),
                )
            })
    });
    granted && !blocked
}

const fn ranges_overlap(left: physical_range, right: physical_range) -> bool {
    let Some(left_end) = left.end() else {
        return true;
    };
    let Some(right_end) = right.end() else {
        return true;
    };
    left.length != 0 && right.length != 0 && left.base < right_end && right.base < left_end
}

/// Temporarily lend the published allocator to one feature-only proof.
#[cfg(all(
    not(test),
    any(
        feature = "memory_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "display_vessel_test",
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "ny_vessel_test"
        )
    )
))]
pub(crate) fn with_frame_allocator<R>(
    operation: impl FnOnce(&mut frame_allocator<'static>) -> R,
) -> Result<R, memory_error> {
    if allocator_state
        .compare_exchange(
            allocator_ready,
            allocator_borrowed,
            Ordering::Acquire,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err(memory_error::allocator_busy);
    }
    // SAFETY: The state transition grants this call the only mutable borrow of
    // the initialized slot, and the borrow ends before state returns to ready.
    let result = unsafe { operation(&mut *ptr::addr_of_mut!(published_allocator).cast()) };
    allocator_state.store(allocator_ready, Ordering::Release);
    Ok(result)
}

#[cfg(all(
    test,
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    )
))]
pub(crate) fn with_frame_allocator<R>(
    _operation: impl FnOnce(&mut frame_allocator<'static>) -> R,
) -> Result<R, memory_error> {
    Err(memory_error::allocator_busy)
}

/// Clear and verify an allocator-authorized identity-mapped physical extent.
///
/// # Safety
///
/// `range` must be an exclusive extent supplied synchronously by the published
/// allocator's `allocate_with` or `release_with` callback. No hardware,
/// mapping, or hidden caller may access it for the duration of this operation.
#[cfg(all(
    not(test),
    any(
        feature = "memory_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "display_vessel_test",
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "ny_vessel_test"
        )
    )
))]
pub(crate) unsafe fn clear_and_verify(range: physical_range) -> bool {
    let Some(end) = range.end() else {
        return false;
    };
    if range.length == 0
        || range.base & (physical::frame_size - 1) != 0
        || range.length & (physical::frame_size - 1) != 0
        || end > address_limit
    {
        return false;
    }
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    let Ok(length) = usize::try_from(range.length) else {
        return false;
    };
    let word_count = length / core::mem::size_of::<u64>();
    // SAFETY: The live allocator granted this whole-frame extent below the
    // identity-map limit exclusively to the caller. Every address is aligned.
    unsafe {
        let words = base as *mut u64;
        for index in 0..word_count {
            ptr::write_volatile(words.add(index), 0);
        }
        compiler_fence(Ordering::SeqCst);
        for index in 0..word_count {
            if ptr::read_volatile(words.add(index)) != 0 {
                return false;
            }
        }
    }
    true
}

#[cfg(all(
    test,
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    )
))]
pub(crate) unsafe fn clear_and_verify(_range: physical_range) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const page: u64 = physical::frame_size;

    fn region(base: u64, length: u64, kind: u32, attributes: u32) -> memory_region {
        memory_region {
            base,
            length,
            kind,
            attributes,
        }
    }

    #[test]
    fn linked_kernel_requires_complete_volatile_usable_pages() {
        let kernel = physical_range::new(low_memory_limit, 3 * page);
        let usable = region(low_memory_limit, 3 * page, 1, 1);
        assert!(validate_bootstrap_layout(&[usable], kernel, None).is_ok());

        let partial = region(low_memory_limit + 1, 3 * page - 1, 1, 1);
        assert_eq!(
            validate_bootstrap_layout(&[partial], kernel, None),
            Err(memory_error::kernel_frame_not_usable)
        );

        let split = [
            region(low_memory_limit, 2 * page, 1, 1),
            region(low_memory_limit + 2 * page, page, 1, 1),
        ];
        assert!(validate_bootstrap_layout(&split, kernel, None).is_ok());
    }

    #[test]
    fn enabled_blocker_overrides_kernel_usable_coverage() {
        let kernel = physical_range::new(low_memory_limit, 2 * page);
        let regions = [
            region(low_memory_limit, 2 * page, 1, 1),
            region(low_memory_limit + page + 1, 1, 2, 1),
        ];
        assert_eq!(
            validate_bootstrap_layout(&regions, kernel, None),
            Err(memory_error::kernel_frame_not_usable)
        );
    }

    #[test]
    fn framebuffer_cannot_overlap_low_or_kernel_memory() {
        let kernel = physical_range::new(low_memory_limit, 2 * page);
        let regions = [region(low_memory_limit, 2 * page, 1, 1)];
        for framebuffer in [
            physical_range::new(low_memory_limit - 1, 2),
            physical_range::new(low_memory_limit + page, page),
        ] {
            assert_eq!(
                validate_bootstrap_layout(&regions, kernel, Some(framebuffer)),
                Err(memory_error::framebuffer_conflict)
            );
        }

        let separate = physical_range::new(8 * low_memory_limit, page);
        assert!(validate_bootstrap_layout(&regions, kernel, Some(separate)).is_ok());
    }

    #[test]
    fn invalid_layouts_fail_before_reservations_are_returned() {
        let usable = [region(low_memory_limit, 4 * page, 1, 1)];
        for kernel in [
            physical_range::new(low_memory_limit + 1, page),
            physical_range::new(low_memory_limit, page - 1),
            physical_range::new(low_memory_limit, 0),
            physical_range::new(address_limit, page),
        ] {
            assert_eq!(
                validate_bootstrap_layout(&usable, kernel, None),
                Err(memory_error::invalid_kernel_extent)
            );
        }
    }
}
