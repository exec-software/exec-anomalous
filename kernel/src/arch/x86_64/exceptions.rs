use core::arch::asm;
use core::fmt::Write;
use core::ptr;
#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
use core::sync::atomic::AtomicU64;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::console::serial_port;

use super::idt;

const architectural_exception_count: usize = 32;
#[cfg(all(
    feature = "circle_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub(super) const circle_report_vector: usize = 0xf0;
#[cfg(all(
    feature = "control_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub(super) const control_gate_vector: usize = 0xf1;
#[cfg(all(
    any(
        feature = "rite_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "display_vessel_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ),
    not(test)
))]
pub(super) const rite_gate_vector: usize = 0xf3;
#[cfg(all(
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ),
    not(test)
))]
pub(super) const scheduler_gate_vector: usize = 0xf2;
#[cfg(any(
    test,
    all(
        feature = "fault_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )
))]
const invalid_opcode_vector: u64 = 6;
const double_fault_vector: usize = 8;
const page_fault_vector: u64 = 14;
const nmi_vector: usize = 2;
#[cfg(any(
    test,
    all(
        feature = "fault_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )
))]
const no_expected_fault: u64 = u64::MAX;

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct exception_frame {
    pub(super) accumulator: u64,
    pub(super) base: u64,
    pub(super) counter: u64,
    pub(super) data: u64,
    pub(super) frame_pointer: u64,
    pub(super) source_index: u64,
    pub(super) destination_index: u64,
    pub(super) register_8: u64,
    pub(super) register_9: u64,
    pub(super) register_10: u64,
    pub(super) register_11: u64,
    pub(super) register_12: u64,
    pub(super) register_13: u64,
    pub(super) register_14: u64,
    pub(super) register_15: u64,
    pub(super) vector: u64,
    pub(super) error_code: u64,
    pub(super) instruction_pointer: u64,
    pub(super) code_segment: u64,
    pub(super) flags: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct page_fault_cause {
    protection_violation: bool,
    write: bool,
    user: bool,
    reserved_bit: bool,
    instruction_fetch: bool,
    protection_key: bool,
    shadow_stack: bool,
    enclave: bool,
}

impl page_fault_cause {
    fn decode(error_code: u64) -> Self {
        Self {
            protection_violation: error_code & (1 << 0) != 0,
            write: error_code & (1 << 1) != 0,
            user: error_code & (1 << 2) != 0,
            reserved_bit: error_code & (1 << 3) != 0,
            instruction_fetch: error_code & (1 << 4) != 0,
            protection_key: error_code & (1 << 5) != 0,
            shadow_stack: error_code & (1 << 6) != 0,
            enclave: error_code & (1 << 15) != 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct exception_snapshot {
    #[cfg_attr(
        not(any(feature = "circle_test", feature = "input_wait_test")),
        allow(dead_code)
    )]
    pub accumulator: u64,
    pub vector: u64,
    pub error_code: u64,
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub flags: u64,
    pub stack_pointer: u64,
    // Every hardware long-mode frame supplies SS, including same-CPL entries.
    // The optional shape remains available for manually assembled diagnostics.
    pub stack_segment: Option<u64>,
    pub fault_address: Option<u64>,
}

#[cfg(any(
    test,
    all(
        feature = "fault_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )
))]
#[derive(Clone, Copy)]
struct expected_fault {
    vector: u64,
    instruction_pointer: u64,
    code_segment: u64,
}

static exception_active: AtomicBool = AtomicBool::new(false);
#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
static expected_fault_vector: AtomicU64 = AtomicU64::new(no_expected_fault);
#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
static expected_fault_instruction_pointer: AtomicU64 = AtomicU64::new(0);
#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
static expected_fault_code_segment: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn x86_64_exception_stub_0();
    fn x86_64_exception_stub_1();
    fn x86_64_exception_stub_2();
    fn x86_64_exception_stub_3();
    fn x86_64_exception_stub_4();
    fn x86_64_exception_stub_5();
    fn x86_64_exception_stub_6();
    fn x86_64_exception_stub_7();
    fn x86_64_exception_stub_8();
    fn x86_64_exception_stub_9();
    fn x86_64_exception_stub_10();
    fn x86_64_exception_stub_11();
    fn x86_64_exception_stub_12();
    fn x86_64_exception_stub_13();
    fn x86_64_exception_stub_14();
    fn x86_64_exception_stub_15();
    fn x86_64_exception_stub_16();
    fn x86_64_exception_stub_17();
    fn x86_64_exception_stub_18();
    fn x86_64_exception_stub_19();
    fn x86_64_exception_stub_20();
    fn x86_64_exception_stub_21();
    fn x86_64_exception_stub_22();
    fn x86_64_exception_stub_23();
    fn x86_64_exception_stub_24();
    fn x86_64_exception_stub_25();
    fn x86_64_exception_stub_26();
    fn x86_64_exception_stub_27();
    fn x86_64_exception_stub_28();
    fn x86_64_exception_stub_29();
    fn x86_64_exception_stub_30();
    fn x86_64_exception_stub_31();
    #[cfg(all(
        feature = "fault_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    fn x86_64_fault_test_invalid_opcode();
    #[cfg(all(
        feature = "circle_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    fn x86_64_circle_report_entry();
    #[cfg(all(
        feature = "control_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_control_gate_entry();
    #[cfg(all(
        feature = "rite_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_rite_gate_entry();
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_shell_vessel_rite_gate_entry();
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_input_wait_rite_gate_entry();
    #[cfg(all(
        feature = "display_vessel_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_display_vessel_gate_entry();
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    fn x86_64_supervised_renderer_rite_gate_entry();
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    fn x86_64_semantic_surface_presentation_rite_gate_entry();
    #[cfg(all(
        feature = "semantic_surface_transport_test",
        not(feature = "semantic_surface_presentation_test"),
        not(test)
    ))]
    fn x86_64_semantic_surface_transport_rite_gate_entry();
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    fn x86_64_scheduler_gate_entry();
}

const exception_stubs: [unsafe extern "C" fn(); architectural_exception_count] = [
    x86_64_exception_stub_0,
    x86_64_exception_stub_1,
    x86_64_exception_stub_2,
    x86_64_exception_stub_3,
    x86_64_exception_stub_4,
    x86_64_exception_stub_5,
    x86_64_exception_stub_6,
    x86_64_exception_stub_7,
    x86_64_exception_stub_8,
    x86_64_exception_stub_9,
    x86_64_exception_stub_10,
    x86_64_exception_stub_11,
    x86_64_exception_stub_12,
    x86_64_exception_stub_13,
    x86_64_exception_stub_14,
    x86_64_exception_stub_15,
    x86_64_exception_stub_16,
    x86_64_exception_stub_17,
    x86_64_exception_stub_18,
    x86_64_exception_stub_19,
    x86_64_exception_stub_20,
    x86_64_exception_stub_21,
    x86_64_exception_stub_22,
    x86_64_exception_stub_23,
    x86_64_exception_stub_24,
    x86_64_exception_stub_25,
    x86_64_exception_stub_26,
    x86_64_exception_stub_27,
    x86_64_exception_stub_28,
    x86_64_exception_stub_29,
    x86_64_exception_stub_30,
    x86_64_exception_stub_31,
];

pub fn initialize(code_selector: u16, double_fault_ist: u8, nmi_ist: u8) {
    #[cfg(all(
        feature = "workspace_dev",
        not(feature = "dispatcher_test"),
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
    assert!(super::workspace_runtime::install_gate());
    for (vector, stub) in exception_stubs.iter().enumerate() {
        let interrupt_stack = match vector {
            double_fault_vector => double_fault_ist,
            nmi_vector => nmi_ist,
            _ => 0,
        };
        // SAFETY: Initialization is single-threaded with maskable interrupts
        // disabled and NMI masked. Every stub is resident for the whole boot.
        let installed = unsafe {
            idt::install_gate(
                vector,
                *stub as usize as u64,
                code_selector,
                interrupt_stack,
                idt::interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "circle_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    {
        // SAFETY: The feature-local vector lies within the fixed IDT, its
        // assembly entry is resident, and initialization still owns the table.
        let installed = unsafe {
            idt::install_gate(
                circle_report_vector,
                x86_64_circle_report_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "control_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The feature-local vector lies within the fixed IDT, its
        // assembly entry is resident, and initialization still owns the table.
        let installed = unsafe {
            idt::install_gate(
                control_gate_vector,
                x86_64_control_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "rite_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The feature-local vector lies outside architectural and PIC
        // vectors. The resident fixed-task entry owns its complete proof.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The composition feature has explicit all-features precedence
        // for this vector. Its entry returns only a runtime-validated scheduled
        // frame, and initialization still exclusively owns the IDT.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_shell_vessel_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The input-wait proof owns the highest-precedence Rite gate.
        // Its resident entry returns only a runtime-validated scheduled frame.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_input_wait_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "display_vessel_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The terminal display proof has explicit highest precedence
        // for this feature-local vector in combined-feature link checks.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_display_vessel_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    {
        // SAFETY: The supervised renderer is the sole terminal proof owner of
        // the Rite vector. Its resident entry returns only a runtime-validated
        // task frame, and initialization still exclusively owns the IDT.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_supervised_renderer_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    {
        // SAFETY: The highest-precedence presentation proof exclusively owns
        // the Rite vector and validates each selected task frame.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_semantic_surface_presentation_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "semantic_surface_transport_test",
        not(feature = "semantic_surface_presentation_test"),
        not(test)
    ))]
    {
        // SAFETY: This highest-precedence proof exclusively owns the fixed
        // Rite gate and its resident entry validates every returned task frame.
        let installed = unsafe {
            idt::install_gate(
                rite_gate_vector,
                x86_64_semantic_surface_transport_rite_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::user_interrupt_gate_attributes,
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    {
        // SAFETY: The feature-local vector is distinct from architectural,
        // PIC, and earlier proof gates. Its resident entry uses a DPL-3
        // interrupt gate and initialization still exclusively owns the IDT.
        let installed = unsafe {
            idt::install_gate(
                scheduler_gate_vector,
                x86_64_scheduler_gate_entry as *const () as usize as u64,
                code_selector,
                0,
                idt::interrupt_gate_attributes | (3 << 5),
            )
        };
        debug_assert!(installed);
    }

    #[cfg(all(
        feature = "dispatcher_test",
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
    assert!(super::dispatcher_runtime::install_gate());

    // SAFETY: Every architecturally deliverable gate is initialized, maskable
    // interrupts remain disabled, and NMI remains masked.
    unsafe { idt::load() };
}

#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn run_invalid_opcode_test(code_selector: u16) {
    let instruction_pointer = x86_64_fault_test_invalid_opcode as *const () as usize as u64;
    expected_fault_instruction_pointer.store(instruction_pointer, Ordering::Relaxed);
    expected_fault_code_segment.store(u64::from(code_selector), Ordering::Relaxed);
    expected_fault_vector.store(invalid_opcode_vector, Ordering::Release);
    // SAFETY: The test-only assembly entry contains `ud2`. The installed IDT
    // must transfer control to the fatal dispatcher and cannot return here.
    unsafe { x86_64_fault_test_invalid_opcode() };
    super::qemu_exit_failure();
    super::halt_forever();
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum exception_disposition {
    fatal,
    fault_test,
    circle_test,
}

impl exception_disposition {
    const fn level(self) -> &'static str {
        match self {
            Self::fatal => "fatal",
            Self::fault_test | Self::circle_test => "info",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::fatal => "halted",
            Self::fault_test => "expected_test",
            Self::circle_test => "contained_test",
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_exception_dispatch(
    frame: *mut exception_frame,
) -> *mut exception_frame {
    if exception_active.swap(true, Ordering::SeqCst) {
        write_nested_exception_record();
        super::qemu_exit_failure();
        super::halt_forever();
    }

    // SAFETY: All exception stubs save the same normalized prefix before the
    // processor-pushed SS:RSP tail. The snapshot does not outlive that frame.
    let snapshot = unsafe { snapshot_from_frame(frame) };
    #[cfg(all(
        feature = "owned_workspace_cpu_test",
        not(feature = "dispatcher_test"),
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
    if super::workspace_runtime::is_active()
        && snapshot.code_segment == u64::from(super::descriptors::user_code_selector)
    {
        super::workspace_runtime::capture_fault(frame);
    }

    #[cfg(all(
        feature = "memory_test",
        not(test),
        not(any(
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ))
    ))]
    // SAFETY: The live hardware frame and snapshot match. The feature admits
    // only a complete resident frame for its running task and exact phase.
    if let Some(terminal) = unsafe { super::user_mapping_test::contain(frame, snapshot) } {
        exception_active.store(false, Ordering::Release);
        if terminal {
            // SAFETY: Exact terminal admission consumed the task's user state;
            // the feature consumes its retained kernel continuation once.
            unsafe { super::user_mapping_test::resume_kernel() }
        }
        return frame;
    }
    #[cfg(all(
        feature = "memory_test",
        not(test),
        not(any(
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ))
    ))]
    // SAFETY: The common assembly entry owns this normalized hardware frame;
    // the feature helper bounds its complete saved SS:RSP tail and accepts
    // only the armed exact CPL0 page fault under the admitted resident root.
    if unsafe { super::resident_probe::contain(frame, snapshot) } {
        exception_active.store(false, Ordering::Release);
        return frame;
    }
    #[cfg(all(
        feature = "workspace_dev",
        not(feature = "dispatcher_test"),
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
    if super::workspace_runtime::is_active()
        && snapshot.code_segment == u64::from(super::descriptors::user_code_selector)
    {
        exception_active.store(false, Ordering::Release);
        super::workspace_runtime::fault(frame, snapshot);
    }
    #[cfg(all(
        feature = "application_profile_test",
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
    if super::dispatcher_runtime::is_active() {
        // The runtime proves current-task frame provenance separately from
        // selected survivor returnability. Clear the common exception latch
        // before a terminal nonlocal return to the retained bootstrap stack.
        if let Some(selected) = super::dispatcher_runtime::fault(frame, snapshot) {
            if exception_frame_address_is_valid(selected)
                && user_return_frame_is_valid(&unsafe { ptr::read(selected) })
            {
                exception_active.store(false, Ordering::Release);
                return selected;
            }
            super::scheduler_test::reject_unexpected_exception();
        }
        exception_active.store(false, Ordering::Release);
        super::dispatcher_runtime::resume_kernel_after_fault();
    }
    #[cfg(all(
        any(
            feature = "scheduler_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ),
        not(test)
    ))]
    if super::scheduler_test::scheduler_is_active() {
        if snapshot.code_segment == u64::from(super::descriptors::user_code_selector)
            && let Some(selected_frame) =
                super::scheduler_test::contain_expected_user_exception(frame, snapshot)
            && exception_frame_address_is_valid(selected_frame)
        {
            // SAFETY: The scheduler contract accepts only a complete live frame
            // owned by the selected task before returning its pointer.
            let selected_prefix = unsafe { ptr::read(selected_frame) };
            if user_return_frame_is_valid(&selected_prefix) {
                exception_active.store(false, Ordering::Release);
                return selected_frame;
            }
        }
        super::scheduler_test::reject_unexpected_exception();
    }
    #[cfg(all(
        feature = "circle_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    let expected_circle_fault = super::circle_test::matches_expected_fault(snapshot);
    #[cfg(not(all(
        feature = "circle_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )))]
    let expected_circle_fault = false;
    let expected_fault_test = consume_expected_fault(snapshot);
    let disposition = if expected_circle_fault {
        exception_disposition::circle_test
    } else if expected_fault_test {
        exception_disposition::fault_test
    } else {
        exception_disposition::fatal
    };
    let mut serial = serial_port::com1();
    serial.initialize();
    write_exception_record(&mut serial, snapshot, disposition);

    #[cfg(all(
        feature = "circle_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if expected_circle_fault {
        super::circle_test::contain_expected_fault(snapshot, &mut serial);
    }

    if expected_fault_test {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.fault_test.passed","level":"info","vector":{},"kind":"{}"}}"#,
            snapshot.vector,
            exception_name(snapshot.vector)
        );
        super::qemu_exit_success();
    } else {
        super::qemu_exit_failure();
    }
    super::halt_forever()
}

#[cfg(all(
    any(
        feature = "scheduler_test",
        feature = "supervision_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ),
    not(test)
))]
fn exception_frame_address_is_valid(frame: *const exception_frame) -> bool {
    !frame.is_null() && (frame as usize) & (core::mem::align_of::<exception_frame>() - 1) == 0
}

#[cfg(any(
    test,
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
fn user_return_frame_is_valid(frame: &exception_frame) -> bool {
    frame.error_code == 0
        && frame.flags & (1 << 1) != 0
        && frame.code_segment == u64::from(super::descriptors::user_code_selector)
}

/// Read the normalized prefix and processor-pushed long-mode SS:RSP tail.
///
/// # Safety
///
/// `frame_address` must be aligned and point to a readable, initialized complete
/// 22-word frame built by an Anomalous 64-bit exception or feature-local
/// interrupt entry. RSP and SS must follow the 20-word normalized prefix even
/// for same-CPL, non-IST entries; a prefix alone is insufficient. The storage
/// must remain live and unchanged throughout this read. For a page-fault frame,
/// the caller must execute at CPL0 before another fault changes CR2.
pub(super) unsafe fn snapshot_from_frame(
    frame_address: *const exception_frame,
) -> exception_snapshot {
    // SAFETY: The caller provides the assembly-created frame described above.
    let frame = unsafe { ptr::read(frame_address) };
    // SAFETY: Long mode always saves these words, regardless of saved CS or
    // IST selection (Intel SDM Vol. 3A, 6.14.2). The caller supplies both words.
    // In particular, hardware may realign a same-CPL stack, so the interrupted
    // RSP cannot be reconstructed by adding the frame size to its address.
    let stack_pointer = unsafe { ptr::read(frame_address.cast::<u64>().add(20)) };
    let stack_segment = Some(unsafe { ptr::read(frame_address.cast::<u64>().add(21)) });
    let fault_address = if frame.vector == page_fault_vector {
        Some(read_cr2())
    } else {
        None
    };

    exception_snapshot {
        accumulator: frame.accumulator,
        vector: frame.vector,
        error_code: frame.error_code,
        instruction_pointer: frame.instruction_pointer,
        code_segment: frame.code_segment,
        flags: frame.flags,
        stack_pointer,
        stack_segment,
        fault_address,
    }
}

#[cfg(all(
    feature = "circle_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub(super) fn reset_after_contained_fault() {
    exception_active.store(false, Ordering::Release);
}

#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
fn consume_expected_fault(snapshot: exception_snapshot) -> bool {
    let expected = expected_fault {
        vector: expected_fault_vector.swap(no_expected_fault, Ordering::AcqRel),
        instruction_pointer: expected_fault_instruction_pointer.load(Ordering::Relaxed),
        code_segment: expected_fault_code_segment.load(Ordering::Relaxed),
    };
    expected_fault_matches(snapshot, expected)
}

#[cfg(not(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
)))]
fn consume_expected_fault(_snapshot: exception_snapshot) -> bool {
    false
}

#[cfg(any(
    test,
    all(
        feature = "fault_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )
))]
fn expected_fault_matches(snapshot: exception_snapshot, expected: expected_fault) -> bool {
    expected.vector != no_expected_fault
        && snapshot.vector == expected.vector
        && snapshot.instruction_pointer == expected.instruction_pointer
        && snapshot.code_segment == expected.code_segment
}

fn write_exception_record(
    serial: &mut serial_port,
    snapshot: exception_snapshot,
    disposition: exception_disposition,
) {
    let level = disposition.level();
    let disposition = disposition.name();
    let origin = match snapshot.code_segment & 0x03 {
        0 => "kernel",
        3 => "user",
        _ => "other",
    };
    let stack_segment = snapshot.stack_segment.unwrap_or(0);
    let stack_switched = uses_separate_stack(snapshot.vector, snapshot.code_segment);
    if let Some(fault_address) = snapshot.fault_address {
        let cause = page_fault_cause::decode(snapshot.error_code);
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.exception","level":"{level}","vector":{},"kind":"{}","error_code":"0x{:016x}","instruction_pointer":"0x{:016x}","stack_pointer":"0x{:016x}","stack_segment":"0x{stack_segment:04x}","stack_switched":{stack_switched},"code_segment":"0x{:04x}","flags":"0x{:016x}","origin":"{origin}","disposition":"{disposition}","fault_address":"0x{fault_address:016x}","protection_violation":{},"write":{},"user":{},"reserved_bit":{},"instruction_fetch":{},"protection_key":{},"shadow_stack":{},"enclave":{}}}"#,
            snapshot.vector,
            exception_name(snapshot.vector),
            snapshot.error_code,
            snapshot.instruction_pointer,
            snapshot.stack_pointer,
            snapshot.code_segment,
            snapshot.flags,
            cause.protection_violation,
            cause.write,
            cause.user,
            cause.reserved_bit,
            cause.instruction_fetch,
            cause.protection_key,
            cause.shadow_stack,
            cause.enclave
        );
    } else {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.exception","level":"{level}","vector":{},"kind":"{}","error_code":"0x{:016x}","instruction_pointer":"0x{:016x}","stack_pointer":"0x{:016x}","stack_segment":"0x{stack_segment:04x}","stack_switched":{stack_switched},"code_segment":"0x{:04x}","flags":"0x{:016x}","origin":"{origin}","disposition":"{disposition}","fault_address":null}}"#,
            snapshot.vector,
            exception_name(snapshot.vector),
            snapshot.error_code,
            snapshot.instruction_pointer,
            snapshot.stack_pointer,
            snapshot.code_segment,
            snapshot.flags
        );
    }
}

fn uses_separate_stack(vector: u64, code_segment: u64) -> bool {
    // This is diagnostic classification for the current CPL0 IDT: only NMI
    // and double fault use IST. Hardware SS:RSP presence is unconditional and
    // is not evidence of a privilege or IST switch, nor return authorization.
    code_segment & 0x03 != 0 || vector == nmi_vector as u64 || vector == double_fault_vector as u64
}

fn write_nested_exception_record() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.exception.nested","level":"fatal","disposition":"halted"}}"#
    );
}

fn read_cr2() -> u64 {
    let address: u64;
    // SAFETY: CR2 is readable at ring 0 and records the current page-fault
    // linear address without changing processor state.
    unsafe {
        asm!(
            "mov {address}, cr2",
            address = out(reg) address,
            options(nomem, nostack, preserves_flags)
        );
    }
    address
}

fn exception_name(vector: u64) -> &'static str {
    match vector {
        0 => "divide_error",
        1 => "debug",
        2 => "non_maskable_interrupt",
        3 => "breakpoint",
        4 => "overflow",
        5 => "bound_range_exceeded",
        6 => "invalid_opcode",
        7 => "device_not_available",
        8 => "double_fault",
        9 => "coprocessor_segment_overrun",
        10 => "invalid_tss",
        11 => "segment_not_present",
        12 => "stack_segment_fault",
        13 => "general_protection_fault",
        14 => "page_fault",
        15 => "reserved",
        16 => "x87_floating_point",
        17 => "alignment_check",
        18 => "machine_check",
        19 => "simd_floating_point",
        20 => "virtualization",
        21 => "control_protection",
        22..=27 => "reserved",
        28 => "hypervisor_injection",
        29 => "vmm_communication",
        30 => "security",
        31 => "reserved",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use core::mem;

    use super::*;

    #[repr(C)]
    struct complete_frame {
        prefix: exception_frame,
        stack_pointer: u64,
        stack_segment: u64,
    }

    fn test_frame(code_segment: u64) -> exception_frame {
        exception_frame {
            accumulator: 0,
            base: 0,
            counter: 0,
            data: 0,
            frame_pointer: 0,
            source_index: 0,
            destination_index: 0,
            register_8: 0,
            register_9: 0,
            register_10: 0,
            register_11: 0,
            register_12: 0,
            register_13: 0,
            register_14: 0,
            register_15: 0,
            vector: invalid_opcode_vector,
            error_code: 0,
            instruction_pointer: 0x1000,
            code_segment,
            flags: 1 << 1,
        }
    }

    fn test_snapshot(vector: u64, instruction_pointer: u64) -> exception_snapshot {
        exception_snapshot {
            accumulator: 0,
            vector,
            error_code: 0,
            instruction_pointer,
            code_segment: 0x08,
            flags: 0x02,
            stack_pointer: 0x2000,
            stack_segment: None,
            fault_address: None,
        }
    }

    #[test]
    fn assembly_frame_prefix_matches_the_rust_layout() {
        assert_eq!(mem::size_of::<exception_frame>(), 20 * 8);
        assert_eq!(mem::align_of::<exception_frame>(), 8);
        assert_eq!(mem::offset_of!(exception_frame, accumulator), 0);
        assert_eq!(mem::offset_of!(exception_frame, register_15), 14 * 8);
        assert_eq!(mem::offset_of!(exception_frame, vector), 15 * 8);
        assert_eq!(mem::offset_of!(exception_frame, error_code), 16 * 8);
        assert_eq!(
            mem::offset_of!(exception_frame, instruction_pointer),
            17 * 8
        );
        assert_eq!(mem::offset_of!(exception_frame, code_segment), 18 * 8);
        assert_eq!(mem::offset_of!(exception_frame, flags), 19 * 8);
        assert_eq!(mem::size_of::<complete_frame>(), 22 * 8);
        assert_eq!(mem::align_of::<complete_frame>(), 8);
        assert_eq!(mem::offset_of!(complete_frame, stack_pointer), 20 * 8);
        assert_eq!(mem::offset_of!(complete_frame, stack_segment), 21 * 8);
    }

    #[test]
    fn user_return_frame_requires_the_exact_full_selector() {
        let mut frame = test_frame(u64::from(super::super::descriptors::user_code_selector));
        assert!(user_return_frame_is_valid(&frame));

        frame.code_segment |= 1_u64 << 32;
        assert!(!user_return_frame_is_valid(&frame));
        frame.code_segment = u64::from(super::super::descriptors::user_code_selector);
        frame.error_code = 1;
        assert!(!user_return_frame_is_valid(&frame));
        frame.error_code = 0;
        frame.flags = 0;
        assert!(!user_return_frame_is_valid(&frame));
    }

    #[test]
    fn snapshot_reads_hardware_stack_for_kernel_user_and_ist_entries() {
        let kernel_code = u64::from(super::super::descriptors::kernel_code_selector);
        let kernel_data = 0x10;
        let user_code = u64::from(super::super::descriptors::user_code_selector);
        let user_data = u64::from(super::super::descriptors::user_data_selector);
        for (vector, code_segment, stack_segment, switched) in [
            (invalid_opcode_vector, kernel_code, kernel_data, false),
            (invalid_opcode_vector, user_code, user_data, true),
            (double_fault_vector as u64, kernel_code, kernel_data, true),
            (nmi_vector as u64, kernel_code, kernel_data, true),
            (nmi_vector as u64, user_code, user_data, true),
        ] {
            let mut full = complete_frame {
                prefix: test_frame(code_segment),
                // Deliberately unaligned: the saved value must survive the
                // processor's independent alignment of its interrupt frame.
                stack_pointer: 0x1234_5678,
                stack_segment,
            };
            full.prefix.vector = vector;
            // SAFETY: `full` supplies every initialized word; none of these
            // non-page-fault fixtures accesses a privileged register.
            let snapshot = unsafe { snapshot_from_frame(ptr::addr_of!(full.prefix)) };
            assert_eq!(snapshot.stack_pointer, full.stack_pointer);
            assert_eq!(snapshot.stack_segment, Some(stack_segment));
            assert_eq!(snapshot.fault_address, None);
            assert_eq!(uses_separate_stack(vector, code_segment), switched);
        }
    }

    #[test]
    fn snapshot_never_uses_selector_validation_to_shorten_the_hardware_frame() {
        for code_segment in [
            0,
            0x09,
            (1_u64 << 32) | u64::from(super::super::descriptors::user_code_selector),
        ] {
            let full = complete_frame {
                prefix: test_frame(code_segment),
                stack_pointer: 0x8877_6655_4433_2211,
                stack_segment: 0,
            };
            // SAFETY: The complete initialized fixture is present even though
            // its raw selector would not authorize a return to execution.
            let snapshot = unsafe { snapshot_from_frame(ptr::addr_of!(full.prefix)) };
            assert_eq!(snapshot.code_segment, code_segment);
            assert_eq!(snapshot.stack_pointer, full.stack_pointer);
            assert_eq!(snapshot.stack_segment, Some(0));
            assert!(!user_return_frame_is_valid(&full.prefix));
        }
    }

    #[test]
    fn page_fault_error_code_is_decoded_without_losing_bits() {
        let cause = page_fault_cause::decode(0x806f);

        assert!(cause.protection_violation);
        assert!(cause.write);
        assert!(cause.user);
        assert!(cause.reserved_bit);
        assert!(!cause.instruction_fetch);
        assert!(cause.protection_key);
        assert!(cause.shadow_stack);
        assert!(cause.enclave);
    }

    #[test]
    fn expected_fault_requires_the_exact_vector_address_and_selector() {
        let expected = expected_fault {
            vector: invalid_opcode_vector,
            instruction_pointer: 0x1234,
            code_segment: 0x08,
        };

        assert!(expected_fault_matches(test_snapshot(6, 0x1234), expected));
        assert!(!expected_fault_matches(test_snapshot(13, 0x1234), expected));
        assert!(!expected_fault_matches(test_snapshot(6, 0x1235), expected));
        let mut user_fault = test_snapshot(6, 0x1234);
        user_fault.code_segment = 0x1b;
        assert!(!expected_fault_matches(user_fault, expected));
    }

    #[test]
    fn vector_names_are_static_and_total() {
        assert_eq!(exception_name(2), "non_maskable_interrupt");
        assert_eq!(exception_name(14), "page_fault");
        assert_eq!(exception_name(31), "reserved");
        assert_eq!(exception_name(255), "unknown");
    }
}
