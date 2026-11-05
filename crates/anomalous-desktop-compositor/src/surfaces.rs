use crate::{chrome, composition_error, frame_height, frame_width, paint, render, row_byte_count};
use anomalous_desktop_workspace::{render_view, window_capacity, window_id, window_view};
use anomalous_window_surface::frame;

/// A local immutable pixel borrow for one exact window incarnation. Public
/// identities select content; this value conveys no mutation or input grant.
#[derive(Clone, Copy)]
pub struct surface_binding<'pixels> {
    window: window_id,
    pixels: &'pixels frame<'pixels>,
}
impl<'pixels> surface_binding<'pixels> {
    pub const fn new(window: window_id, pixels: &'pixels frame<'pixels>) -> Self {
        Self { window, pixels }
    }
}

/// The exact client rectangle shared by composition and future local client
/// input routing. It excludes decoration and the theme's content padding.
/// Empty rectangles are legal when a small window leaves no body space.
pub fn client_viewport(view: &render_view<'_>, window: window_view) -> crate::chrome_rect {
    chrome::content_bounds(view, window)
}

/// One canonical view and its complete immutable pixel set. Construction
/// validates every attached window, including hidden/minimized ones, before
/// any output can change. The borrowed view and buffers stay frozen throughout
/// all rows; callers must keep this frame until complete presentation.
pub struct composition_frame<'view, 'pixels> {
    view: render_view<'view>,
    bindings: [Option<surface_binding<'pixels>>; window_capacity],
}
impl<'view, 'pixels> composition_frame<'view, 'pixels> {
    pub fn try_new(
        view: &render_view<'view>,
        bindings: &[surface_binding<'pixels>],
    ) -> Result<Self, composition_error> {
        if view.width() != frame_width || view.height() != frame_height {
            return Err(composition_error::unsupported_viewport);
        }
        if bindings.len() > window_capacity {
            return Err(composition_error::surface_capacity_exceeded);
        }
        let mut retained = [None; window_capacity];
        for (index, binding) in bindings.iter().enumerate() {
            if bindings[..index]
                .iter()
                .any(|prior| prior.window == binding.window)
            {
                return Err(composition_error::duplicate_surface_binding);
            }
            let expected = view
                .window(binding.window)
                .and_then(window_view::surface)
                .ok_or(composition_error::extraneous_surface_binding)?;
            if binding.pixels.descriptor() != expected {
                return Err(composition_error::surface_descriptor_mismatch);
            }
            retained[index] = Some(*binding);
        }
        for window in view.all_windows() {
            if window.surface().is_some()
                && !bindings.iter().any(|binding| binding.window == window.id())
            {
                return Err(composition_error::missing_surface_binding);
            }
        }
        Ok(Self {
            view: *view,
            bindings: retained,
        })
    }

    /// Compose exactly one complete RGB8 row. All fallible checks precede the
    /// first output mutation; no surface lookup or decoding can fail mid-row.
    pub fn compose_row(&self, row: u32, output: &mut [u8]) -> Result<(), composition_error> {
        if row >= frame_height {
            return Err(composition_error::row_out_of_bounds);
        }
        if output.len() != row_byte_count {
            return Err(composition_error::row_size_mismatch);
        }
        render::compose(self, &mut paint::row_canvas::new(row, output));
        Ok(())
    }

    pub(crate) const fn view(&self) -> &render_view<'view> {
        &self.view
    }

    pub(crate) fn surface(&self, window: window_id) -> Option<&'pixels frame<'pixels>> {
        self.bindings
            .iter()
            .flatten()
            .find(|binding| binding.window == window)
            .map(|binding| binding.pixels)
    }
}
