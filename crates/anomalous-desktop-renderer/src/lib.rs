#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Allocation-free renderer for one bounded semantic desktop proof.
//!
//! The input capsule is a feature-private, revision-1 test artifact. It is not
//! a stable wire format, binary ABI, scene serialization, or authority token.
//! It carries only the exact paint fields and fixed-label selectors needed to
//! rasterize the accepted desktop fixture. In particular, it has no fields for
//! scene brands, scene/node/action identities, action or focus handles,
//! machine projections, or protected source text.
//!
//! A validated capsule always denotes the same 1024 by 768, 25-command paint
//! stream: standard depth, reduced motion, primary enabled and focused,
//! secondary disabled, and the fixed `Protected status` projection. Rendering
//! is deterministic and row-at-a-time into caller-owned RGB8 storage.

mod capsule;
mod font;
mod raster;

#[cfg(feature = "transport")]
mod transport;

#[cfg(feature = "scene_conformance")]
pub mod scene_conformance;

pub use capsule::{
    capsule_byte_count, capsule_color, capsule_command_kind, capsule_rect, capsule_shape,
    capsule_slot, fixture_command_count, fixture_state_ordinal, projection_command, render_capsule,
};
pub use font::glyph_table_fingerprint;
pub use raster::{frame_height, frame_width, render_row, row_byte_count};
#[cfg(feature = "transport")]
pub use transport::transport_render_snapshot;

use core::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum renderer_error {
    wrong_capsule_length,
    wrong_capsule_magic,
    wrong_viewport,
    wrong_command_count,
    wrong_state_ordinal,
    wrong_label_count,
    nonzero_reserved,
    invalid_command_kind,
    invalid_shape,
    invalid_color,
    invalid_secondary_color,
    invalid_slot,
    invalid_label_selector,
    invalid_state_flag,
    capsule_mismatch,
    invalid_geometry,
    geometry_overflow,
    outside_viewport,
    missing_label,
    unexpected_label,
    wrong_label,
    unsupported_glyph,
    wrong_row,
    wrong_row_buffer_length,
}

impl renderer_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::wrong_capsule_length => "wrong_capsule_length",
            Self::wrong_capsule_magic => "wrong_capsule_magic",
            Self::wrong_viewport => "wrong_viewport",
            Self::wrong_command_count => "wrong_command_count",
            Self::wrong_state_ordinal => "wrong_state_ordinal",
            Self::wrong_label_count => "wrong_label_count",
            Self::nonzero_reserved => "nonzero_reserved",
            Self::invalid_command_kind => "invalid_command_kind",
            Self::invalid_shape => "invalid_shape",
            Self::invalid_color => "invalid_color",
            Self::invalid_secondary_color => "invalid_secondary_color",
            Self::invalid_slot => "invalid_slot",
            Self::invalid_label_selector => "invalid_label_selector",
            Self::invalid_state_flag => "invalid_state_flag",
            Self::capsule_mismatch => "capsule_mismatch",
            Self::invalid_geometry => "invalid_geometry",
            Self::geometry_overflow => "geometry_overflow",
            Self::outside_viewport => "outside_viewport",
            Self::missing_label => "missing_label",
            Self::unexpected_label => "unexpected_label",
            Self::wrong_label => "wrong_label",
            Self::unsupported_glyph => "unsupported_glyph",
            Self::wrong_row => "wrong_row",
            Self::wrong_row_buffer_length => "wrong_row_buffer_length",
        }
    }
}

impl fmt::Debug for renderer_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for renderer_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
