#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Allocation-free host proof of transactional semantic-surface publication.
//!
//! One [`desktop_surface`] owns one canonical desktop scene and keeps exactly
//! one committed revision public. At most one successor may be queued, leased
//! to the exact renderer incarnation, or ready for a trusted presentation
//! assertion. The committed human and machine projections remain paired at the
//! old revision until that assertion atomically commits the successor.
//!
//! Renderer replacement is bounded to one restart. The same immutable,
//! redacted render snapshot is reoffered under fresh renderer and delivery
//! authority; stale leases and completions cannot publish. Strict backpressure
//! returns an unaccepted scene candidate intact and never overwrites or
//! coalesces the pending revision.
//!
//! This crate does not render or inspect pixels, authenticate a presenter,
//! isolate a renderer process, define a wire format, route input, execute an
//! action effect, or define a compositor/window protocol. Calling publication
//! is the trusted host's assertion that the complete matching surface is ready.
//! Protected source bytes and exact lengths never enter a render snapshot, but
//! revision and operation cadence are observable and are not hidden here.
#![doc = include_str!("privacy_compile_fail.md")]

mod identity;
mod presented;
mod service;
mod snapshot;

pub use identity::{
    delivery_attempt, identity_error, publication_sequence, renderer_identity, surface_identity,
};
pub use presented::presented_scene_view;
pub use service::{
    completion_rejection, desktop_surface, prepared_presentation, presentation_control,
    presentation_receipt, presentation_rejection, publication_phase, publication_receipt,
    publication_rejection, queued_revision, render_lease, renderer_endpoint, restart_receipt,
    submission_rejection, surface_action_handle, surface_action_rejection,
    surface_construction_rejection, surface_control, surface_error, surface_focus_handle,
    surface_focus_rejection, surface_focus_resolution, surface_retirement,
    surface_retirement_rejection,
};
pub use snapshot::{render_command, render_label_selector, render_rect, render_snapshot_view};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod service_tests;
