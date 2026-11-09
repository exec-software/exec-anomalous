mod x86_64;

#[cfg(all(feature = "desktop_dev", not(test)))]
pub use x86_64::run_bootstrap_presentation;

#[allow(unused_imports)]
pub use x86_64::qemu_exit_success;
#[cfg(all(
    feature = "circle_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub use x86_64::run_circle_test;
#[cfg(all(
    feature = "control_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_control_test;
#[cfg(all(
    feature = "display_vessel_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_display_vessel_test;
#[cfg(all(
    feature = "fault_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub use x86_64::run_fault_test;
#[cfg(all(
    feature = "input_wait_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_input_wait_test;
#[cfg(all(
    feature = "memory_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_memory_test;
#[cfg(all(feature = "ny_vessel_test", not(test)))]
pub use x86_64::run_ny_vessel_test;
#[cfg(all(
    feature = "rite_test",
    not(feature = "ny_vessel_test"),
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_rite_test;
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
pub use x86_64::run_scheduler_test;
#[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
pub use x86_64::run_semantic_surface_presentation_test;
#[cfg(all(
    feature = "semantic_surface_transport_test",
    not(feature = "semantic_surface_presentation_test"),
    not(test)
))]
pub use x86_64::run_semantic_surface_transport_test;
#[cfg(all(
    feature = "shell_vessel_test",
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_shell_vessel_test;
#[cfg(all(
    feature = "supervised_renderer_test",
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_supervised_renderer_test;
#[cfg(all(
    feature = "supervision_test",
    not(feature = "shell_vessel_test"),
    not(feature = "input_wait_test"),
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test"),
    not(test)
))]
pub use x86_64::run_supervision_test;
pub use x86_64::{
    disable_external_interrupts, enable_external_interrupts, external_interrupts_enabled,
    halt_forever, initialize_external_interrupts, initialize_fault_path,
    initialize_memory_protections, qemu_exit_failure, rearm_keyboard_input_after_loss,
    wait_for_interrupt_if_idle, with_external_interrupts_disabled,
};
#[cfg(all(
    feature = "interrupt_test",
    not(feature = "supervised_renderer_test"),
    not(feature = "semantic_surface_transport_test")
))]
pub use x86_64::{
    interrupt_test_force_loss, read_external_interrupt_health, run_interrupt_return_probe,
};

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
pub use x86_64::run_workspace;

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
pub use x86_64::run_dispatcher_test;
