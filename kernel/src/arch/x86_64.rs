#[cfg(any(
    test,
    all(
        feature = "native_durable_test",
        any(
            not(feature = "native_interactive_editor_test"),
            feature = "native_durable_failure_test"
        ),
        not(any(feature = "native_relaunch_test", feature = "elf_application_test"))
    )
))]
mod native_durable_fixture;
#[cfg(any(
    test,
    feature = "native_editor_workspace_test",
    all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    )
))]
mod native_interactive_fixture;
#[cfg(any(
    test,
    feature = "native_editor_workspace_test",
    all(
        feature = "native_durable_test",
        not(any(feature = "native_relaunch_test", feature = "elf_application_test"))
    )
))]
mod native_storage_device;
use core::arch::{asm, global_asm};

#[cfg(all(
    feature = "memory_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod active_mapping_test;
mod address_space;
#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_workspace_cpu_test"
))]
mod application_cpu;
#[cfg(any(test, feature = "application_profile_test"))]
#[cfg_attr(feature = "linux_loader_entry_test", allow(dead_code))]
mod application_fault;
// The same gate its consumers carry: `owned_execution` is itself excluded
// from default and several test builds, and `prepared_launch`'s retained
// arena kind uses exactly this condition. `test` keeps the pure golden tests
// reachable under `cargo test --lib` with no features.
// Available wherever `owned_execution` is, which is what consumes it, plus
// `test` so the pure golden tests are reachable under `cargo test --lib`.
// `owned_execution` itself is `not(test)`, so the two never overlap.
#[cfg(any(
    test,
    feature = "owned_dispatcher_test",
    feature = "owned_workspace_test"
))]
mod arena_profile;
#[cfg(all(
    feature = "circle_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
mod circle_test;
#[cfg(all(
    feature = "control_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod control_test;
mod descriptors;
#[cfg(all(
    feature = "dispatcher_test",
    not(feature = "owned_dispatcher_test"),
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
mod dispatcher_runtime;
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(not(feature = "owned_multi_region_test"))]
#[path = "x86_64/owned_dispatcher_runtime.rs"]
mod dispatcher_runtime;
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(feature = "owned_multi_region_test")]
#[path = "x86_64/owned_multi_region_runtime.rs"]
mod dispatcher_runtime;
#[cfg(all(
    feature = "application_profile_test",
    not(feature = "ny_object_test"),
    not(feature = "native_application_test"),
    not(feature = "mixed_workspace_test"),
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
#[path = "x86_64/application_profile_runtime.rs"]
mod dispatcher_runtime;
#[cfg(all(
    feature = "mixed_workspace_test",
    not(feature = "ny_object_test"),
    not(feature = "native_application_test"),
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
#[cfg_attr(
    feature = "native_editor_workspace_test",
    path = "x86_64/native_editor_workspace_runtime.rs"
)]
#[cfg_attr(
    not(feature = "native_editor_workspace_test"),
    path = "x86_64/mixed_workspace_runtime.rs"
)]
mod dispatcher_runtime;
#[cfg(all(
    feature = "ny_object_test",
    not(feature = "native_application_test"),
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
#[path = "x86_64/ny_object_runtime.rs"]
mod dispatcher_runtime;
#[cfg(all(
    feature = "native_application_test",
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
#[cfg_attr(
    feature = "linux_loader_entry_test",
    path = "x86_64/linux_loader_entry_runtime.rs"
)]
#[cfg_attr(
    not(feature = "linux_loader_entry_test"),
    path = "x86_64/native_application_runtime.rs"
)]
mod dispatcher_runtime;
#[cfg(all(
    feature = "display_vessel_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod display_vessel_test;
mod exceptions;
mod idt;
#[cfg(all(
    feature = "input_wait_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod input_wait_test;
mod interrupts;
#[cfg(test)]
mod linux_application_calls;
#[cfg(any(test, feature = "linux_loader_entry_test"))]
mod linux_loader_plan;
#[cfg(feature = "linux_loader_entry_test")]
mod linux_loader_reply;
#[cfg(any(test, feature = "linux_loader_entry_test"))]
mod linux_loader_source;
#[cfg(feature = "linux_application_test")]
mod linux_syscall;
#[cfg(any(
    test,
    all(feature = "linux_tls_test", not(feature = "linux_loader_entry_test"))
))]
mod linux_tls_fixture;
mod mapping_plan;
#[cfg(all(
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
))]
#[cfg_attr(feature = "owned_workspace_test", allow(dead_code))]
mod owned_execution;
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(feature = "owned_source_test")]
#[path = "x86_64/owned_source_runtime.rs"]
mod owned_source_runtime;
#[cfg(any(
    test,
    all(feature = "owned_tls_test", not(feature = "owned_source_test"))
))]
mod owned_tls_fixture;
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
#[path = "x86_64/owned_tls_runtime.rs"]
mod owned_tls_runtime;
#[cfg(any(test, feature = "owned_workspace_test"))]
mod owned_workspace_fault;
#[cfg(any(test, feature = "owned_tls_test", feature = "linux_tls_test"))]
mod task_tls;
mod user_copy;
mod user_entry_plan;
#[cfg(any(
    test,
    all(
        feature = "memory_test",
        not(any(
            feature = "input_wait_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test"
        ))
    )
))]
mod user_fault_gate;
#[cfg(all(
    feature = "memory_test",
    not(test),
    not(any(
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))
))]
mod user_mapping_test;
#[cfg(any(test, feature = "owned_workspace_cpu_test"))]
mod workspace_cpu;
#[cfg(all(
    feature = "memory_test",
    not(test),
    not(any(
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))
))]
global_asm!(
    include_str!("x86_64/user_mapping_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "memory_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod memory_test;
#[cfg(any(test, feature = "owned_multi_region_test"))]
mod multi_region_plan;
#[cfg(all(feature = "ny_vessel_test", not(test)))]
#[path = "x86_64/ny_vessel_test.rs"]
mod ny_vessel_test;
mod owned_mapping;
#[cfg(all(
    feature = "memory_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod owned_mapping_test;
#[cfg(any(test, feature = "owned_source_test"))]
mod owned_source;
#[cfg(any(test, feature = "owned_source_test"))]
mod owned_source_plan;
mod paging;
#[cfg(any(
    test,
    feature = "memory_test",
    feature = "owned_dispatcher_test",
    feature = "owned_workspace_test"
))]
mod physical_access;
mod physical_window;
mod prepared_launch;
mod resident_plan;
#[cfg(any(
    test,
    all(
        feature = "memory_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    )
))]
mod resident_probe;
#[cfg(all(
    feature = "rite_test",
    not(feature = "ny_vessel_test"),
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod rite_test;
#[cfg(any(
    feature = "workspace_dev",
    feature = "rite_test",
    feature = "ny_vessel_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "display_vessel_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
// One attribute for both, because two `cfg_attr`s naming the same `allow`
// apply it twice when both features are on, and clippy rejects that under
// `-D warnings` in the all-features build. Each of these fixtures uses part
// of this module and leaves another part unused -- the vessel reads session
// input but never touches the semantic surface.
#[cfg_attr(
    any(feature = "workspace_dev", feature = "ny_vessel_test"),
    allow(dead_code)
)]
mod rite_transport;
mod runtime_mapping;
#[cfg(any(test, feature = "linux_loader_entry_test"))]
mod runtime_source_mapping;
#[cfg(any(
    feature = "scheduler_test",
    feature = "supervision_test",
    feature = "shell_vessel_test",
    feature = "input_wait_test",
    feature = "supervised_renderer_test",
    feature = "semantic_surface_transport_test"
))]
#[cfg_attr(
    any(
        feature = "workspace_dev",
        feature = "dispatcher_test",
        feature = "shell_vessel_test",
        feature = "input_wait_test",
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ),
    allow(dead_code)
)]
mod scheduler_test;
#[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
mod semantic_surface_presentation_test;
#[cfg(all(
    feature = "semantic_surface_transport_test",
    not(feature = "semantic_surface_presentation_test"),
    not(test)
))]
mod semantic_surface_transport_test;
#[cfg(all(
    feature = "shell_vessel_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
mod shell_vessel_test;
#[cfg(all(
    feature = "supervised_renderer_test",
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
mod supervised_renderer_test;
#[cfg(all(
    feature = "supervision_test",
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
mod supervision_test;
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
#[cfg_attr(
    feature = "owned_workspace_test",
    path = "x86_64/owned_workspace_runtime.rs"
)]
mod workspace_runtime;

#[cfg(test)]
mod workspace_mailbox;

#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub use interrupts::external_interrupt_health;
pub use interrupts::{external_interrupt_error, external_interrupt_state};

global_asm!(
    include_str!("x86_64/exception_entry.S"),
    owned_tls_enabled = const cfg!(all(any(feature = "owned_tls_test", feature = "linux_tls_test"), not(test))) as usize,
    options(att_syntax)
);
global_asm!(
    include_str!("x86_64/interrupt_entry.S"),
    owned_tls_enabled = const cfg!(all(any(feature = "owned_tls_test", feature = "linux_tls_test"), not(test))) as usize,
    options(att_syntax)
);
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
global_asm!(
    include_str!("x86_64/interrupt_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
global_asm!(
    include_str!("x86_64/fault_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "circle_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
global_asm!(
    include_str!("x86_64/circle_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "control_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/control_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "display_vessel_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/display_vessel_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "rite_test",
    not(feature = "ny_vessel_test"),
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/rite_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(feature = "ny_vessel_test", not(test)))]
global_asm!(
    include_str!("x86_64/ny_vessel_entry.S"),
    options(att_syntax)
);
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
global_asm!(
    include_str!("x86_64/scheduler_test_entry.S"),
    owned_tls_enabled = const cfg!(all(any(feature = "owned_tls_test", feature = "linux_tls_test"), not(test))) as usize,
    options(att_syntax)
);
#[cfg(all(
    feature = "supervision_test",
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/supervision_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "shell_vessel_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/shell_vessel_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "input_wait_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/input_wait_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "supervised_renderer_test",
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/supervised_renderer_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
global_asm!(
    include_str!("x86_64/semantic_surface_presentation_test_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "semantic_surface_transport_test",
    not(feature = "semantic_surface_presentation_test"),
    not(test)
))]
global_asm!(
    include_str!("x86_64/semantic_surface_transport_test_entry.S"),
    options(att_syntax)
);

#[cfg(feature = "smoke_test")]
const qemu_exit_port: u16 = 0xf4;
#[cfg(feature = "smoke_test")]
const qemu_exit_success_value: u32 = 0x10;
#[cfg(feature = "smoke_test")]
const qemu_exit_failure_value: u32 = 0x11;

pub fn halt_forever() -> ! {
    // SAFETY: Every caller has reached a terminal kernel path. Clearing IF
    // prevents initialized device IRQs from re-entering failed kernel state;
    // NMI remains independently masked by Threshold during this milestone.
    unsafe {
        asm!(
            "cli",
            "2:",
            "hlt",
            "jmp 2b",
            options(nomem, nostack, noreturn)
        )
    }
}

pub fn qemu_exit_success() {
    #[cfg(feature = "smoke_test")]
    write_debug_exit(qemu_exit_success_value);
}

pub fn qemu_exit_failure() {
    #[cfg(feature = "smoke_test")]
    write_debug_exit(qemu_exit_failure_value);
}

pub struct fault_path_state {
    pub vector_count: usize,
    pub double_fault_ist: u8,
    pub nmi_ist: u8,
    pub nmi_masked: bool,
}

pub fn initialize_fault_path() -> fault_path_state {
    let descriptors = descriptors::initialize();
    exceptions::initialize(
        descriptors.code_selector,
        descriptors.double_fault_ist,
        descriptors.nmi_ist,
    );
    fault_path_state {
        vector_count: 32,
        double_fault_ist: descriptors.double_fault_ist,
        nmi_ist: descriptors.nmi_ist,
        nmi_masked: true,
    }
}

pub fn initialize_memory_protections() -> bool {
    paging::enable_and_verify_protections()
}

pub fn initialize_external_interrupts() -> Result<external_interrupt_state, external_interrupt_error>
{
    interrupts::initialize_external_interrupts()
}

pub fn enable_external_interrupts() -> bool {
    interrupts::enable_external_interrupts()
}

pub fn disable_external_interrupts() {
    interrupts::disable_external_interrupts()
}

pub fn with_external_interrupts_disabled<T>(operation: impl FnOnce() -> T) -> Option<T> {
    interrupts::with_external_interrupts_disabled(operation)
}

pub fn rearm_keyboard_input_after_loss() -> Option<u32> {
    interrupts::rearm_keyboard_input_after_loss()
}

#[must_use]
pub fn external_interrupts_enabled() -> bool {
    interrupts::interrupts_enabled()
}

pub fn wait_for_interrupt_if_idle(work_ready_if_disabled: impl FnOnce() -> bool) -> bool {
    interrupts::wait_for_interrupt_if_idle(work_ready_if_disabled)
}

#[must_use]
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn read_external_interrupt_health() -> external_interrupt_health {
    interrupts::read_external_interrupt_health()
}

#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn run_interrupt_return_probe() -> bool {
    interrupts::run_interrupt_return_probe()
}

#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn interrupt_test_force_loss() -> bool {
    interrupts::interrupt_test_force_loss()
}

#[cfg(all(
    feature = "fault_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn run_fault_test() {
    exceptions::run_invalid_opcode_test(descriptors::kernel_code_selector)
}

#[cfg(all(
    feature = "circle_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub fn run_circle_test() {
    circle_test::run()
}

#[cfg(all(
    feature = "control_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_control_test() {
    control_test::run()
}

#[cfg(all(
    feature = "memory_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_memory_test() {
    memory_test::run()
}

#[cfg(all(
    feature = "display_vessel_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_display_vessel_test(framebuffer: crate::boot_omen::framebuffer_info) -> ! {
    display_vessel_test::run(framebuffer)
}

#[cfg(all(
    feature = "rite_test",
    not(feature = "ny_vessel_test"),
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_rite_test(screen: crate::console::screen_console) {
    rite_test::run(screen)
}

#[cfg(all(feature = "ny_vessel_test", not(test)))]
pub fn run_ny_vessel_test(screen: crate::console::screen_console) {
    ny_vessel_test::run(screen)
}

#[cfg(all(
    feature = "scheduler_test",
    not(feature = "dispatcher_test"),
    not(feature = "supervision_test"),
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_scheduler_test() {
    scheduler_test::run()
}

#[cfg(all(
    feature = "supervision_test",
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_supervision_test() {
    supervision_test::run()
}

#[cfg(all(
    feature = "shell_vessel_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_shell_vessel_test(screen: crate::console::screen_console) {
    shell_vessel_test::run(screen)
}

#[cfg(all(
    feature = "input_wait_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_input_wait_test() -> ! {
    input_wait_test::run()
}

#[cfg(all(
    feature = "supervised_renderer_test",
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub fn run_supervised_renderer_test(framebuffer: crate::boot_omen::framebuffer_info) -> ! {
    supervised_renderer_test::run(framebuffer)
}

#[cfg(all(
    feature = "semantic_surface_transport_test",
    not(feature = "semantic_surface_presentation_test"),
    not(test)
))]
pub fn run_semantic_surface_transport_test() -> ! {
    semantic_surface_transport_test::run()
}

#[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
pub fn run_semantic_surface_presentation_test(
    framebuffer: crate::boot_omen::framebuffer_info,
) -> ! {
    semantic_surface_presentation_test::run(framebuffer)
}

#[cfg(all(feature = "desktop_dev", not(test)))]
pub fn run_bootstrap_presentation(framebuffer: crate::boot_omen::framebuffer_info) {
    semantic_surface_presentation_test::run_bootstrap_presentation(framebuffer);
}

#[cfg(feature = "smoke_test")]
fn write_debug_exit(value: u32) {
    // SAFETY: This function only exists in the smoke-test kernel, whose QEMU
    // machine contract reserves port 0xf4 for isa-debug-exit.
    unsafe {
        asm!(
            "out dx, eax",
            in("dx") qemu_exit_port,
            in("eax") value,
            options(nomem, nostack, preserves_flags)
        );
    }
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
pub fn run_workspace(framebuffer: crate::boot_omen::framebuffer_info) -> ! {
    workspace_runtime::run(framebuffer)
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
global_asm!(
    include_str!("x86_64/workspace_runtime_entry.S"),
    options(att_syntax)
);

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
pub fn run_dispatcher_test() {
    dispatcher_runtime::run()
}

#[cfg(all(
    feature = "dispatcher_test",
    not(feature = "owned_dispatcher_test"),
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
global_asm!(
    include_str!("x86_64/dispatcher_runtime_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(not(feature = "owned_multi_region_test"))]
global_asm!(
    include_str!("x86_64/owned_dispatcher_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "owned_dispatcher_test",
    not(feature = "application_profile_test"),
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
#[cfg(feature = "owned_multi_region_test")]
global_asm!(
    include_str!("x86_64/owned_multi_region_entry.S"),
    owned_tls_enabled = const cfg!(all(any(feature = "owned_tls_test", feature = "linux_tls_test"), not(test))) as usize,
    owned_source_enabled = const cfg!(feature = "owned_source_test") as usize,
    owned_tls_proof_enabled = const cfg!(all(feature = "owned_tls_test", not(feature = "owned_source_test"))) as usize,
    options(att_syntax)
);
#[cfg(all(
    feature = "application_profile_test",
    not(feature = "ny_object_test"),
    not(feature = "native_application_test"),
    not(feature = "mixed_workspace_test"),
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
global_asm!(
    include_str!("x86_64/application_profile_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "ny_object_test",
    not(feature = "native_application_test"),
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
global_asm!(
    include_str!("x86_64/ny_object_entry.S"),
    options(att_syntax)
);
#[cfg(all(
    feature = "native_application_test",
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
global_asm!(
    include_str!("x86_64/native_application_entry.S"),
    owned_tls_enabled = const cfg!(all(any(feature = "owned_tls_test", feature = "linux_tls_test"), not(test))) as usize,
    options(att_syntax)
);

#[cfg(all(
    feature = "mixed_workspace_test",
    not(feature = "ny_object_test"),
    not(feature = "native_application_test"),
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
#[cfg(not(feature = "native_editor_workspace_test"))]
global_asm!(
    include_str!("x86_64/mixed_workspace_entry.S"),
    options(att_syntax)
);

#[cfg(all(
    feature = "mixed_workspace_test",
    not(feature = "ny_object_test"),
    not(feature = "native_application_test"),
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
#[cfg(feature = "native_editor_workspace_test")]
global_asm!(
    include_str!("x86_64/native_editor_workspace_entry.S"),
    options(att_syntax)
);
