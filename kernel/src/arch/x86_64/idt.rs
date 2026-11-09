use core::arch::asm;
use core::mem;
use core::ptr;

const idt_entry_count: usize = 256;
pub(super) const interrupt_gate_attributes: u8 = 0x8e;
#[cfg(any(
    test,
    feature = "circle_test",
    feature = "control_test",
    feature = "rite_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test",
    feature = "workspace_dev",
    feature = "dispatcher_test",
    feature = "ny_vessel_test"
))]
pub(super) const user_interrupt_gate_attributes: u8 = 0xee;

#[derive(Clone, Copy)]
#[repr(C)]
struct idt_gate {
    offset_low: u16,
    selector: u16,
    interrupt_stack: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl idt_gate {
    const fn missing() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            interrupt_stack: 0,
            attributes: 0,
            offset_middle: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    fn new(handler: u64, selector: u16, interrupt_stack: u8, attributes: u8) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            interrupt_stack: interrupt_stack & 0x07,
            attributes,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }

    #[cfg(test)]
    fn handler(&self) -> u64 {
        u64::from(self.offset_low)
            | (u64::from(self.offset_middle) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

#[derive(Clone, Copy)]
#[repr(C, packed)]
struct descriptor_table_pointer {
    limit: u16,
    base: u64,
}

static mut kernel_idt: [idt_gate; idt_entry_count] = [idt_gate::missing(); idt_entry_count];

/// Install one gate while no asynchronous observer can use the table.
///
/// # Safety
///
/// The caller must run on the bootstrap processor with maskable interrupts
/// disabled and NMI masked. It must also provide a resident handler using the
/// entry convention described by `attributes`.
pub(super) unsafe fn install_gate(
    vector: usize,
    handler: u64,
    selector: u16,
    interrupt_stack: u8,
    attributes: u8,
) -> bool {
    if vector >= idt_entry_count {
        return false;
    }
    let gate = idt_gate::new(handler, selector, interrupt_stack, attributes);
    let table = ptr::addr_of_mut!(kernel_idt).cast::<idt_gate>();
    // SAFETY: The caller owns mutation of the static table and `vector` was
    // checked against its exact capacity above.
    unsafe { ptr::write(table.add(vector), gate) };
    true
}

/// Load the kernel-owned IDT after its required gates have been populated.
///
/// # Safety
///
/// The caller must run on the bootstrap processor with maskable interrupts
/// disabled and NMI masked. All gates that can currently be delivered must
/// already name live handlers.
pub(super) unsafe fn load() {
    let pointer = descriptor_table_pointer {
        limit: (mem::size_of::<[idt_gate; idt_entry_count]>() - 1) as u16,
        base: ptr::addr_of!(kernel_idt) as usize as u64,
    };
    // SAFETY: The caller established the table lifetime and initialization
    // contract stated above.
    unsafe {
        asm!(
            "lidt [{pointer}]",
            pointer = in(reg) ptr::addr_of!(pointer),
            options(readonly, nostack, preserves_flags)
        );
    }
}

/// Verify the loaded IDT and every present handler against resident storage.
///
/// # Safety
/// The BSP owns the initialized IDT with IF clear and NMI masked. Kernel linker
/// symbols and this private table remain live and unmodified during inspection.
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
    unsafe extern "C" {
        static kernel_text_start: u8;
        static kernel_text_end: u8;
    }
    let base = ptr::addr_of!(kernel_idt) as u64;
    let length = mem::size_of::<[idt_gate; idt_entry_count]>() as u64;
    if start >= end || base < start || base.checked_add(length).is_none_or(|last| last > end) {
        return false;
    }
    let mut loaded = descriptor_table_pointer { limit: 0, base: 0 };
    // SAFETY: SIDT writes exactly this initialized local packed descriptor.
    unsafe {
        asm!("sidt [{}]", in(reg) ptr::addr_of_mut!(loaded), options(nostack, preserves_flags));
    }
    if loaded.base != base || u64::from(loaded.limit) + 1 != length {
        return false;
    }
    let text_start = ptr::addr_of!(kernel_text_start) as u64;
    let text_end = ptr::addr_of!(kernel_text_end) as u64;
    if text_start >= text_end {
        return false;
    }
    for vector in 0..idt_entry_count {
        // SAFETY: The full private table is resident and mutation is excluded.
        let gate = unsafe { ptr::read(ptr::addr_of!(kernel_idt).cast::<idt_gate>().add(vector)) };
        if gate.attributes & 0x80 == 0 {
            if vector < 32
                || gate.offset_low != 0
                || gate.offset_middle != 0
                || gate.offset_high != 0
                || gate.selector != 0
                || gate.interrupt_stack != 0
                || gate.attributes != 0
                || gate.reserved != 0
            {
                return false;
            }
            continue;
        }
        let handler = u64::from(gate.offset_low)
            | (u64::from(gate.offset_middle) << 16)
            | (u64::from(gate.offset_high) << 32);
        if handler < text_start
            || handler >= text_end
            || gate.selector != super::descriptors::kernel_code_selector
            || gate.reserved != 0
            || ![interrupt_gate_attributes, 0xee].contains(&gate.attributes)
            || gate.interrupt_stack & !7 != 0
        {
            return false;
        }
        if vector < 32 {
            let expected_ist = match vector {
                2 => 2,
                8 => 1,
                _ => 0,
            };
            if gate.interrupt_stack != expected_ist || gate.attributes != interrupt_gate_attributes
            {
                return false;
            }
        } else if gate.interrupt_stack != 0 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_encodes_handler_selector_and_ist() {
        let gate = idt_gate::new(0x1234_5678_9abc_def0, 0x08, 2, interrupt_gate_attributes);

        assert_eq!(mem::size_of::<idt_gate>(), 16);
        assert_eq!(gate.handler(), 0x1234_5678_9abc_def0);
        assert_eq!(gate.selector, 0x08);
        assert_eq!(gate.interrupt_stack, 2);
        assert_eq!(gate.attributes, interrupt_gate_attributes);
    }

    #[test]
    fn user_gate_is_present_at_dpl_three() {
        let gate = idt_gate::new(0x1234, 0x08, 0, user_interrupt_gate_attributes);

        assert_eq!(gate.attributes & 0x80, 0x80);
        assert_eq!((gate.attributes >> 5) & 0x03, 3);
        assert_eq!(gate.attributes & 0x0f, 0x0e);
    }
}
