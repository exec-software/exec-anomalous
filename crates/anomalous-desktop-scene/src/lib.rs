#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Allocation-free proof of one canonical Anomalous desktop-scene revision.
//!
//! This crate models one exact eight-node scene. A committed revision owns the
//! canonical definition and precomputed human, accessibility, and machine
//! projections together. It does not render pixels, route input, authenticate
//! a caller, execute an action, define a wire format, or grant authority.
//!
//! Public scene, node, action, revision, and projection references are
//! correlation data only. Local action and focus operations additionally
//! require move-only handles bound to an unexposed process-local scene brand.
//!
//! Declared-public text is a caller assertion, not a privacy-policy decision.
//! Protected text never enters a projection; projections expose only one fixed
//! redaction placeholder. Explicit text getters are local disclosure
//! boundaries and authorize no logging, persistence, transport, or capture.
//! A prepared revision may expose one borrowed, explicitly uncommitted render
//! preview. That preview contains only public render fields and already
//! projected labels; it cannot mint handles or expose canonical semantic
//! projection owners.
#![doc = include_str!("privacy_compile_fail.md")]

mod identity;
mod observation;
mod projection;
mod scene;
mod text;

pub use identity::{
    action_identity, identity_error, node_identity, scene_identity, scene_revision,
};
pub use observation::{
    scene_observation_event, scene_observation_result, scene_privacy_observation,
};
pub use projection::{
    accessibility_node, accessibility_node_count, accessible_render_plan, flat_paint_command_count,
    machine_action_count, machine_action_descriptor, machine_node, machine_node_count,
    machine_scene_projection, paint_color_token, paint_command, paint_command_kind,
    paint_shape_token, prepared_paint_command, prepared_render_preview, projection_label, rgb8,
    scene_action_reference, scene_focus_reference, scene_projections, standard_paint_command_count,
};
pub use scene::{
    action_resolution, action_resolution_rejection, commit_rejection, declared_scene_state,
    desktop_scene, focus_direction, focus_resolution, focus_resolution_rejection,
    local_action_handle, local_focus_handle, prepare_rejection, prepared_revision,
    retirement_rejection, scene_action_count, scene_action_definition, scene_action_kind,
    scene_candidate, scene_construction_rejection, scene_control, scene_depth, scene_disclosure,
    scene_error, scene_focus, scene_motion, scene_node_count, scene_node_definition, scene_rect,
    scene_resolution_source, scene_retirement, scene_role, scene_slot,
};
pub use text::{
    declared_public_text, protected_text, scene_label, scene_text_error, scene_text_limit,
};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod scene_tests;
