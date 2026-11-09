use core::mem;
use core::ptr;
#[cfg(any(
    test,
    feature = "memory_test",
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "rite_test",
    feature = "ny_vessel_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
use core::sync::atomic::{Ordering, compiler_fence};

pub const kernel_code_selector: u16 = 0x08;
#[cfg_attr(
    not(any(
        test,
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    )),
    allow(dead_code)
)]
pub const user_data_selector: u16 = 0x2b;
#[cfg_attr(
    not(any(
        test,
        feature = "circle_test",
        feature = "control_test",
        feature = "rite_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    )),
    allow(dead_code)
)]
pub const user_code_selector: u16 = 0x33;

const kernel_data_selector: u16 = 0x10;
const kernel_tss_selector: u16 = 0x18;
const kernel_code_descriptor: u64 = 0x00af_9a00_0000_ffff;
const kernel_data_descriptor: u64 = 0x00cf_9200_0000_ffff;
const user_data_descriptor: u64 = 0x00cf_f200_0000_ffff;
const user_code_descriptor: u64 = 0x00af_fa00_0000_ffff;
const descriptor_count: usize = 7;
const exception_stack_size: usize = 16 * 1024;
#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub(super) const privilege_stack_size: usize = exception_stack_size;

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct task_state_segment {
    reserved_1: u32,
    privilege_stacks: [u64; 3],
    reserved_2: u64,
    interrupt_stacks: [u64; 7],
    reserved_3: u64,
    reserved_4: u16,
    io_map_base: u16,
}

impl task_state_segment {
    const fn empty() -> Self {
        Self {
            reserved_1: 0,
            privilege_stacks: [0; 3],
            reserved_2: 0,
            interrupt_stacks: [0; 7],
            reserved_3: 0,
            reserved_4: 0,
            io_map_base: mem::size_of::<Self>() as u16,
        }
    }

    fn with_stacks(
        privilege_stack_top: u64,
        double_fault_stack_top: u64,
        nmi_stack_top: u64,
    ) -> Self {
        Self {
            reserved_1: 0,
            privilege_stacks: [privilege_stack_top, 0, 0],
            reserved_2: 0,
            interrupt_stacks: [double_fault_stack_top, nmi_stack_top, 0, 0, 0, 0, 0],
            reserved_3: 0,
            reserved_4: 0,
            io_map_base: mem::size_of::<Self>() as u16,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct descriptor_table_pointer {
    limit: u16,
    base: u64,
}

#[repr(C, align(4096))]
struct aligned_stack([u8; exception_stack_size]);

pub struct descriptor_state {
    pub code_selector: u16,
    pub double_fault_ist: u8,
    pub nmi_ist: u8,
}

static mut kernel_gdt: [u64; descriptor_count] = [0; descriptor_count];
static mut kernel_tss: task_state_segment = task_state_segment::empty();
static mut privilege_stack: aligned_stack = aligned_stack([0; exception_stack_size]);
static mut double_fault_stack: aligned_stack = aligned_stack([0; exception_stack_size]);
static mut nmi_stack: aligned_stack = aligned_stack([0; exception_stack_size]);

unsafe extern "C" {
    fn x86_64_load_descriptor_tables(
        pointer: *const descriptor_table_pointer,
        code_selector: u64,
        data_selector: u64,
        tss_selector: u64,
    );
}

pub fn initialize() -> descriptor_state {
    let tss = task_state_segment::with_stacks(
        stack_top(ptr::addr_of!(privilege_stack)),
        stack_top(ptr::addr_of!(double_fault_stack)),
        stack_top(ptr::addr_of!(nmi_stack)),
    );
    // SAFETY: Initialization runs once on the bootstrap processor while
    // interrupts remain disabled. No other code can observe the tables until
    // the assembly routine loads GDTR and TR below.
    unsafe { ptr::write(ptr::addr_of_mut!(kernel_tss), tss) };

    let tss_address = ptr::addr_of!(kernel_tss) as usize as u64;
    let (tss_low, tss_high) = encode_tss_descriptor(tss_address);
    let entries = [
        0,
        kernel_code_descriptor,
        kernel_data_descriptor,
        tss_low,
        tss_high,
        user_data_descriptor,
        user_code_descriptor,
    ];
    // SAFETY: This is the same single-processor initialization window. The GDT
    // remains static and writable because the processor marks the TSS busy.
    unsafe { ptr::write(ptr::addr_of_mut!(kernel_gdt), entries) };

    let pointer = descriptor_table_pointer {
        limit: (mem::size_of::<[u64; descriptor_count]>() - 1) as u16,
        base: ptr::addr_of!(kernel_gdt) as usize as u64,
    };
    // SAFETY: The table contains present ring-0 code/data descriptors and an
    // available 64-bit TSS whose backing storage lives for the whole boot.
    unsafe {
        x86_64_load_descriptor_tables(
            ptr::addr_of!(pointer),
            u64::from(kernel_code_selector),
            u64::from(kernel_data_selector),
            u64::from(kernel_tss_selector),
        )
    };

    descriptor_state {
        code_selector: kernel_code_selector,
        double_fault_ist: 1,
        nmi_ist: 2,
    }
}

/// Check the actual descriptor state and all TSS-selected supervisor stacks.
///
/// # Safety
/// The initialized BSP exclusively owns descriptor state, with IF clear and
/// NMI masked. `start..end` is independently admitted supervisor RW resident
/// storage in both roots; no descriptor/stack update may race this inspection.
#[cfg(any(
    all(
        feature = "memory_test",
        not(test),
        not(any(
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ))
    ),
    all(
        any(feature = "owned_dispatcher_test", feature = "owned_workspace_test"),
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
    )
))]
pub(super) unsafe fn resident_state_is_valid(start: u64, end: u64) -> bool {
    use core::arch::asm;

    let gdt_base = ptr::addr_of!(kernel_gdt) as u64;
    let gdt_length = mem::size_of::<[u64; descriptor_count]>() as u64;
    let tss_base = ptr::addr_of!(kernel_tss) as u64;
    let stacks = [
        ptr::addr_of!(privilege_stack),
        ptr::addr_of!(double_fault_stack),
        ptr::addr_of!(nmi_stack),
    ];
    if !resident_range(start, end, gdt_base, gdt_length)
        || !resident_range(
            start,
            end,
            tss_base,
            mem::size_of::<task_state_segment>() as u64,
        )
        || stacks
            .iter()
            .any(|stack| !resident_range(start, end, *stack as u64, exception_stack_size as u64))
    {
        return false;
    }
    let mut loaded = descriptor_table_pointer { limit: 0, base: 0 };
    let tr: u16;
    let cs: u16;
    let ss: u16;
    let flags: u64;
    // SAFETY: These instructions only inspect CPL0 descriptor/control state;
    // SGDT writes exactly the local packed descriptor and PUSHFQ is balanced.
    unsafe {
        asm!("sgdt [{}]", in(reg) ptr::addr_of_mut!(loaded), options(nostack, preserves_flags));
        asm!("str {0:x}", out(reg) tr, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        asm!("mov {0:x}, ss", out(reg) ss, options(nomem, nostack, preserves_flags));
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
    }
    if loaded.base != gdt_base
        || u64::from(loaded.limit) + 1 != gdt_length
        || tr != kernel_tss_selector
        || cs != kernel_code_selector
        || ss != kernel_data_selector
        || flags & (1 << 9) != 0
    {
        return false;
    }
    let (tss_low, tss_high) = encode_tss_descriptor(tss_base);
    let expected = [
        0,
        kernel_code_descriptor,
        kernel_data_descriptor,
        tss_low | (1 << 41),
        tss_high,
        user_data_descriptor,
        user_code_descriptor,
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        // SAFETY: The full private GDT remains resident and exclusively held.
        let actual =
            unsafe { ptr::read_volatile(ptr::addr_of!(kernel_gdt).cast::<u64>().add(index)) };
        // Loading a code/data segment may set its architectural accessed bit;
        // LTR must have changed the exact TSS descriptor to the busy type.
        let permitted = if matches!(index, 1 | 2 | 5 | 6) {
            1 << 40
        } else {
            0
        };
        if actual & !permitted != expected {
            return false;
        }
    }
    // SAFETY: TSS is packed; copying it unaligned avoids references to fields.
    let current = unsafe { ptr::read_unaligned(ptr::addr_of!(kernel_tss)) };
    let privilege = current.privilege_stacks;
    let interrupt = current.interrupt_stacks;
    if privilege != [stack_top(stacks[0]), 0, 0]
        || interrupt != [stack_top(stacks[1]), stack_top(stacks[2]), 0, 0, 0, 0, 0]
        || current.reserved_1 != 0
        || current.reserved_2 != 0
        || current.reserved_3 != 0
        || current.reserved_4 != 0
        || usize::from(current.io_map_base) != mem::size_of::<task_state_segment>()
    {
        return false;
    }
    // SAFETY: The same residency/exclusion interval covers the loaded IDT.
    unsafe { super::idt::resident_state_is_valid(start, end) }
}

#[cfg(any(
    any(
        test,
        all(
            feature = "memory_test",
            not(any(
                feature = "input_wait_test",
                feature = "supervised_renderer_test",
                feature = "semantic_surface_transport_test"
            ))
        )
    ),
    all(
        any(feature = "owned_dispatcher_test", feature = "owned_workspace_test"),
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
    )
))]
fn resident_range(start: u64, end: u64, base: u64, length: u64) -> bool {
    start != 0
        && start < end
        && length != 0
        && base >= start
        && base.checked_add(length).is_some_and(|last| last <= end)
}

/// Read the privilege-transition stack selected in the live TSS.
///
/// # Safety
///
/// The caller must own the bootstrap processor with maskable interrupts and
/// NMI excluded so no privilege transition can observe a partial bytewise
/// update of the packed, naturally unaligned RSP0 field.
#[cfg(any(
    feature = "memory_test",
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "rite_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test",
    feature = "ny_vessel_test"
))]
pub(super) unsafe fn selected_privilege_stack_top() -> u64 {
    // SAFETY: The caller holds the complete privilege-transition exclusion
    // contract and the TSS remains resident for the lifetime of the kernel.
    unsafe { read_privilege_stack_top(ptr::addr_of!(kernel_tss)) }
}

/// Select one task-owned privilege-transition stack in the live TSS.
///
/// A valid top is canonical and 16-byte aligned. The function writes the
/// packed RSP0 field one byte at a time, verifies its exact readback, and does
/// not alter RSP1/RSP2 or any IST entry.
///
/// # Safety
///
/// The caller must own the bootstrap processor with maskable interrupts and NMI
/// excluded, and `stack_top` must identify live supervisor-writable storage
/// for every privilege transition until another stack is selected.
#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub(super) unsafe fn select_privilege_stack_top(stack_top: u64) -> bool {
    if !privilege_stack_top_is_valid(stack_top) {
        return false;
    }
    // SAFETY: The caller supplies exclusive access and a validated live top;
    // the helper touches only the eight packed bytes occupied by RSP0.
    unsafe { write_privilege_stack_top(ptr::addr_of_mut!(kernel_tss), stack_top) };
    // SAFETY: The same exclusion interval covers the verification read.
    unsafe { read_privilege_stack_top(ptr::addr_of!(kernel_tss)) == stack_top }
}

/// Restore the fixed bootstrap privilege-transition stack in the live TSS.
///
/// # Safety
///
/// The caller must meet [`select_privilege_stack_top`]'s exclusion contract
/// and must have stopped every task that could still enter through the old
/// stack selection.
#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub(super) unsafe fn restore_bootstrap_privilege_stack() -> bool {
    // SAFETY: The fixed stack remains resident and writable for the whole boot;
    // the caller excludes privilege transitions during the packed update.
    unsafe { select_privilege_stack_top(stack_top(ptr::addr_of!(privilege_stack))) }
}

#[cfg(any(
    feature = "memory_test",
    feature = "circle_test",
    feature = "control_test",
    feature = "rite_test",
    feature = "ny_vessel_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
pub fn wipe_privilege_stack() -> bool {
    let stack = ptr::addr_of_mut!(privilege_stack).cast::<u8>();
    for offset in 0..exception_stack_size {
        // SAFETY: The fixed prototype calls this only after it has abandoned
        // the TSS RSP0 stack and restored the original kernel stack.
        unsafe { ptr::write_volatile(stack.add(offset), 0) };
    }
    for offset in 0..exception_stack_size {
        // SAFETY: The static backing remains allocated for the whole boot.
        if unsafe { ptr::read_volatile(stack.add(offset)) } != 0 {
            return false;
        }
    }
    true
}

fn stack_top(stack: *const aligned_stack) -> u64 {
    stack as usize as u64 + exception_stack_size as u64
}

#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
fn privilege_stack_top_is_valid(stack_top: u64) -> bool {
    let Some(stack_base) = stack_top.checked_sub(exception_stack_size as u64) else {
        return false;
    };
    stack_top & 0x0f == 0
        && super::paging::is_canonical(stack_top)
        && super::paging::is_canonical(stack_base)
}

#[cfg(any(
    feature = "memory_test",
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "rite_test",
    feature = "ny_vessel_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
unsafe fn read_privilege_stack_top(tss: *const task_state_segment) -> u64 {
    let mut bytes = [0_u8; mem::size_of::<u64>()];
    let rsp0 = tss
        .cast::<u8>()
        .wrapping_add(mem::offset_of!(task_state_segment, privilege_stacks));
    compiler_fence(Ordering::SeqCst);
    for (offset, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: RSP0 occupies these exact eight bytes in the packed TSS. Byte
        // accesses impose no alignment requirement.
        *byte = unsafe { ptr::read_volatile(rsp0.add(offset)) };
    }
    compiler_fence(Ordering::SeqCst);
    u64::from_ne_bytes(bytes)
}

#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
unsafe fn write_privilege_stack_top(tss: *mut task_state_segment, stack_top: u64) {
    let rsp0 = tss
        .cast::<u8>()
        .wrapping_add(mem::offset_of!(task_state_segment, privilege_stacks));
    compiler_fence(Ordering::SeqCst);
    for (offset, byte) in stack_top.to_ne_bytes().iter().enumerate() {
        // SAFETY: RSP0 occupies these exact eight bytes in the packed TSS. Byte
        // accesses impose no alignment requirement.
        unsafe { ptr::write_volatile(rsp0.add(offset), *byte) };
    }
    compiler_fence(Ordering::SeqCst);
}

fn encode_tss_descriptor(base: u64) -> (u64, u64) {
    let limit = (mem::size_of::<task_state_segment>() - 1) as u64;
    let low = (limit & 0xffff)
        | ((base & 0x00ff_ffff) << 16)
        | (0x89_u64 << 40)
        | (((limit >> 16) & 0x0f) << 48)
        | (((base >> 24) & 0xff) << 56);
    (low, base >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residency_requires_the_complete_nonempty_extent_without_overflow() {
        assert!(resident_range(0x1000, 0x3000, 0x1000, 0x2000));
        assert!(resident_range(0x1000, 0x3000, 0x2800, 0x800));
        assert!(!resident_range(0x1000, 0x3000, 0x0fff, 1));
        assert!(!resident_range(0x1000, 0x3000, 0x2800, 0x801));
        assert!(!resident_range(0x1000, 0x3000, 0x3000, 0));
        assert!(!resident_range(0, 0x3000, 0x1000, 8));
        assert!(!resident_range(0x3000, 0x1000, 0x3000, 8));
        assert!(!resident_range(0x1000, u64::MAX, u64::MAX - 3, 8));
    }

    #[test]
    fn task_state_segment_layout_matches_x86_64() {
        assert_eq!(mem::size_of::<task_state_segment>(), 104);
        assert_eq!(mem::offset_of!(task_state_segment, privilege_stacks), 4);
        assert_eq!(mem::offset_of!(task_state_segment, interrupt_stacks), 36);
        assert_eq!(mem::offset_of!(task_state_segment, io_map_base), 102);
    }

    #[test]
    fn tss_descriptor_preserves_base_limit_and_type() {
        let base = 0x1234_5678_9abc_def0;
        let (low, high) = encode_tss_descriptor(base);
        let decoded_base = ((low >> 16) & 0x00ff_ffff) | ((low >> 32) & 0xff00_0000) | (high << 32);

        assert_eq!(decoded_base, base);
        assert_eq!(low & 0xffff, 103);
        assert_eq!((low >> 40) & 0xff, 0x89);
    }

    #[test]
    fn descriptor_selectors_match_the_kernel_gdt_slots() {
        assert_eq!(kernel_code_selector, 1 * 8);
        assert_eq!(kernel_data_selector, 2 * 8);
        assert_eq!(kernel_tss_selector, 3 * 8);
        assert_eq!(user_data_selector, 5 * 8 | 3);
        assert_eq!(user_code_selector, 6 * 8 | 3);
    }

    #[test]
    fn user_descriptors_are_present_at_dpl_three() {
        assert_eq!((user_data_descriptor >> 45) & 0x03, 3);
        assert_eq!((user_code_descriptor >> 45) & 0x03, 3);
        assert_eq!((user_data_descriptor >> 47) & 0x01, 1);
        assert_eq!((user_code_descriptor >> 47) & 0x01, 1);
    }

    #[test]
    fn packed_rsp0_updates_are_bytewise_exact_and_preserve_every_other_field() {
        let original = task_state_segment::with_stacks(
            0x1111_2222_3333_4440,
            0x5555_6666_7777_8880,
            0x9999_aaaa_bbbb_ccc0,
        );
        let mut updated = original;
        let original_bytes = unsafe {
            core::slice::from_raw_parts(
                ptr::addr_of!(original).cast::<u8>(),
                mem::size_of::<task_state_segment>(),
            )
        };

        // SAFETY: `updated` is an exclusively owned local TSS value.
        unsafe { write_privilege_stack_top(ptr::addr_of_mut!(updated), 0xfedc_ba98_7654_3210) };
        // SAFETY: The same local value remains live for this exact readback.
        assert_eq!(
            unsafe { read_privilege_stack_top(ptr::addr_of!(updated)) },
            0xfedc_ba98_7654_3210
        );
        let updated_bytes = unsafe {
            core::slice::from_raw_parts(
                ptr::addr_of!(updated).cast::<u8>(),
                mem::size_of::<task_state_segment>(),
            )
        };
        let rsp0_start = mem::offset_of!(task_state_segment, privilege_stacks);
        let rsp0_end = rsp0_start + mem::size_of::<u64>();
        assert_eq!(&updated_bytes[..rsp0_start], &original_bytes[..rsp0_start]);
        assert_eq!(&updated_bytes[rsp0_end..], &original_bytes[rsp0_end..]);
    }

    #[test]
    fn privilege_stack_selection_requires_a_complete_canonical_aligned_extent() {
        assert!(privilege_stack_top_is_valid(0x0000_0000_0020_0000));
        assert!(!privilege_stack_top_is_valid(0));
        assert!(!privilege_stack_top_is_valid(
            exception_stack_size as u64 - 16
        ));
        assert!(!privilege_stack_top_is_valid(0x0000_0000_0020_0008));
        assert!(!privilege_stack_top_is_valid(0x0000_8000_0000_0000));
    }
}

/// Immutable extent of the separately reserved bootstrap privilege stack.
#[cfg(all(
    feature = "memory_test",
    not(test),
    not(any(
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))
))]
pub(super) fn bootstrap_privilege_stack_range() -> (u64, u64) {
    let base = ptr::addr_of!(privilege_stack) as u64;
    (base, base + exception_stack_size as u64)
}
