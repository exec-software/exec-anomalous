#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Deterministic CPU composition and shared decoration hit testing for one
//! immutable canonical desktop view. Every control resolves to a typed action;
//! colors, font scale, cursor, geometry and static depth come from theme tokens.
//! No framebuffer, input authority, external asset, allocation, or publication
//! state is owned here. Native complete-frame presentation remains the caller's
//! job. Immutable RGB8 content is admitted as a complete, prevalidated frame
//! set; local surface bindings grant no application or device authority.

mod appearance;
mod chrome;
mod editor;
mod font;
mod launcher;
mod paint;
mod render;
mod surfaces;

use anomalous_desktop_workspace::render_view;
pub use appearance::appearance_controls_visible;
pub use chrome::{chrome_bounds, chrome_rect, chrome_target, hit_test_chrome};
pub use surfaces::{client_viewport, composition_frame, surface_binding};

/// Physical editing requires the same readable line geometry as painting.
pub fn editor_input_visible(
    view: &render_view<'_>,
    window: anomalous_desktop_workspace::window_view,
) -> bool {
    window.editor().is_some()
        && editor::layout(
            chrome::content_bounds(view, window).paint(),
            i64::from(view.theme_definition().font_scale),
        )
        .is_some()
}

pub const frame_width: u32 = 1024;
pub const frame_height: u32 = 768;
pub const row_byte_count: usize = frame_width as usize * 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum composition_error {
    unsupported_viewport,
    row_out_of_bounds,
    row_size_mismatch,
    surface_capacity_exceeded,
    duplicate_surface_binding,
    extraneous_surface_binding,
    missing_surface_binding,
    surface_descriptor_mismatch,
}
impl composition_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::unsupported_viewport => "unsupported_viewport",
            Self::row_out_of_bounds => "row_out_of_bounds",
            Self::row_size_mismatch => "row_size_mismatch",
            Self::surface_capacity_exceeded => "surface_capacity_exceeded",
            Self::duplicate_surface_binding => "duplicate_surface_binding",
            Self::extraneous_surface_binding => "extraneous_surface_binding",
            Self::missing_surface_binding => "missing_surface_binding",
            Self::surface_descriptor_mismatch => "surface_descriptor_mismatch",
        }
    }
}

/// Compose exactly one complete RGB8 row. Invalid viewport, row or output size
/// leaves every destination byte unchanged. Cursor is composed last from the
/// same canonical view as the windows; there is no independent cursor state.
/// Scenes with attached content require `composition_frame` and its exact
/// immutable bindings; this built-in-only convenience rejects missing frames.
pub fn compose_row(
    view: &render_view<'_>,
    row: u32,
    output: &mut [u8],
) -> Result<(), composition_error> {
    composition_frame::try_new(view, &[])?.compose_row(row, output)
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod frame_tests;

#[cfg(test)]
mod appearance_tests;

#[cfg(test)]
mod launcher_tests;

#[cfg(test)]
mod paint_equivalence_tests;

#[cfg(test)]
mod surface_tests;

#[cfg(test)]
mod editor_tests;
