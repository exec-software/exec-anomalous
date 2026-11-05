mod framebuffer;
mod serial;

pub use framebuffer::draw_boot_surface;
pub use framebuffer::draw_interrupt_failure;
pub use framebuffer::draw_interrupt_failure_surface;
pub use framebuffer::draw_memory_failure;
pub use framebuffer::draw_memory_failure_surface;
pub use framebuffer::draw_registered_interrupt_failure;
pub use framebuffer::draw_text_fallback;
pub use framebuffer::register_interrupt_failure_surface;
#[cfg(all(feature = "desktop_dev", not(test)))]
pub use framebuffer::restore_bootstrap_interrupt_failure_surface;
#[cfg(all(feature = "workspace_dev", not(test)))]
pub use framebuffer::restore_retired_interrupt_failure_surface;
pub use framebuffer::screen_console;
#[cfg(all(
    any(
        feature = "supervised_renderer_test",
        feature = "workspace_dev",
        feature = "native_application_test"
    ),
    not(test)
))]
pub use framebuffer::withdraw_interrupt_failure_surface;
pub use serial::serial_port;
