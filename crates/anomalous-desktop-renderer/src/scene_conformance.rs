//! Optional host-side bridge from the canonical scene to the bounded capsule.
//!
//! This module is intentionally absent from the default dependency surface.
//! It constructs and retires one live scene locally, verifies human/machine
//! parity, and returns only the paint capsule. Protected source text is never
//! copied into an error or output field.

use core::fmt;

use anomalous_desktop_scene::{
    accessible_render_plan, action_identity, declared_public_text, declared_scene_state,
    desktop_scene, machine_action_count, machine_node_count, node_identity, paint_color_token,
    paint_command_kind, paint_shape_token, protected_text, scene_action_definition,
    scene_action_kind, scene_candidate, scene_depth, scene_disclosure, scene_focus, scene_identity,
    scene_label, scene_motion, scene_node_count, scene_node_definition, scene_role, scene_slot,
    standard_paint_command_count,
};

use crate::{
    capsule_color, capsule_command_kind, capsule_rect, capsule_shape, capsule_slot,
    fixture_command_count, fixture_state_ordinal, projection_command, render_capsule,
};

const fixture_scene_slot: u64 = 41;
const fixture_generation: u64 = 7;

const slots: [scene_slot; scene_node_count] = [
    scene_slot::root,
    scene_slot::masthead,
    scene_slot::navigation,
    scene_slot::primary_action,
    scene_slot::secondary_action,
    scene_slot::content,
    scene_slot::heading,
    scene_slot::status,
];

const roles: [scene_role; scene_node_count] = [
    scene_role::desktop,
    scene_role::region,
    scene_role::region,
    scene_role::control,
    scene_role::control,
    scene_role::region,
    scene_role::heading,
    scene_role::status,
];

const labels: [&str; scene_node_count] = [
    "Anomalous desktop",
    "Anomalous",
    "Navigation",
    "Open shell",
    "Inspect system",
    "Workspace",
    "System workspace",
    "Protected status",
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_conformance_error {
    protected_source_rejected,
    fixture_identity_rejected,
    fixture_label_rejected,
    scene_construction_rejected,
    projection_mismatch,
    capsule_rejected,
    scene_retirement_rejected,
}

impl scene_conformance_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::protected_source_rejected => "protected_source_rejected",
            Self::fixture_identity_rejected => "fixture_identity_rejected",
            Self::fixture_label_rejected => "fixture_label_rejected",
            Self::scene_construction_rejected => "scene_construction_rejected",
            Self::projection_mismatch => "projection_mismatch",
            Self::capsule_rejected => "capsule_rejected",
            Self::scene_retirement_rejected => "scene_retirement_rejected",
        }
    }
}

impl fmt::Debug for scene_conformance_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for scene_conformance_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Builds the exact protected revision-1 fixture and returns only its capsule.
///
/// `protected_source` remains owned by the canonical scene until explicit
/// retirement. Its bytes and length do not influence the returned capsule or
/// any rejection code.
pub fn build_fixture_capsule(
    protected_source: &str,
) -> Result<render_capsule, scene_conformance_error> {
    let candidate = fixture_candidate(protected_source)?;
    let (scene, control) = desktop_scene::new(candidate)
        .map_err(|_| scene_conformance_error::scene_construction_rejected)?;
    let capsule = capsule_from_scene(&scene);
    let retirement = scene.retire(control);
    if retirement.is_err() {
        return Err(scene_conformance_error::scene_retirement_rejected);
    }
    capsule
}

fn fixture_candidate(protected_source: &str) -> Result<scene_candidate, scene_conformance_error> {
    let identity = scene_identity::try_new(fixture_scene_slot, fixture_generation)
        .map_err(|_| scene_conformance_error::fixture_identity_rejected)?;
    let status = scene_label::protected(
        protected_text::try_from_str(protected_source)
            .map_err(|_| scene_conformance_error::protected_source_rejected)?,
    );
    let nodes = [
        fixture_node(identity, scene_slot::root, public_label(labels[0])?)?,
        fixture_node(identity, scene_slot::masthead, public_label(labels[1])?)?,
        fixture_node(identity, scene_slot::navigation, public_label(labels[2])?)?,
        fixture_node(
            identity,
            scene_slot::primary_action,
            public_label(labels[3])?,
        )?,
        fixture_node(
            identity,
            scene_slot::secondary_action,
            public_label(labels[4])?,
        )?,
        fixture_node(identity, scene_slot::content, public_label(labels[5])?)?,
        fixture_node(identity, scene_slot::heading, public_label(labels[6])?)?,
        fixture_node(identity, scene_slot::status, status)?,
    ];
    let state = declared_scene_state::new(
        true,
        false,
        scene_focus::primary,
        scene_disclosure::protected,
        scene_depth::standard,
        scene_motion::reduced,
    );
    if state.candidate_ordinal() != fixture_state_ordinal {
        return Err(scene_conformance_error::projection_mismatch);
    }
    Ok(scene_candidate::new(identity, nodes, state))
}

fn public_label(text: &str) -> Result<scene_label, scene_conformance_error> {
    declared_public_text::try_from_str(text)
        .map(scene_label::declared_public)
        .map_err(|_| scene_conformance_error::fixture_label_rejected)
}

fn fixture_node(
    identity: scene_identity,
    slot: scene_slot,
    label: scene_label,
) -> Result<scene_node_definition, scene_conformance_error> {
    let node = node_for(identity, slot)?;
    let parent = match slot.canonical_parent() {
        Some(parent) => Some(node_for(identity, parent)?),
        None => None,
    };
    let action = match slot {
        scene_slot::primary_action | scene_slot::secondary_action => Some(
            scene_action_definition::new(action_for(identity, slot)?, scene_action_kind::activate),
        ),
        _ => None,
    };
    Ok(scene_node_definition::new(
        slot,
        node,
        slot.canonical_role(),
        parent,
        slot.canonical_bounds(),
        label,
        action,
    ))
}

fn node_for(
    identity: scene_identity,
    slot: scene_slot,
) -> Result<node_identity, scene_conformance_error> {
    node_identity::try_new(slot.identity_slot(), identity.generation())
        .map_err(|_| scene_conformance_error::fixture_identity_rejected)
}

fn action_for(
    identity: scene_identity,
    slot: scene_slot,
) -> Result<action_identity, scene_conformance_error> {
    let action_slot = match slot {
        scene_slot::primary_action => 1,
        scene_slot::secondary_action => 2,
        _ => return Err(scene_conformance_error::fixture_identity_rejected),
    };
    action_identity::try_new(action_slot, identity.generation())
        .map_err(|_| scene_conformance_error::fixture_identity_rejected)
}

fn capsule_from_scene(scene: &desktop_scene) -> Result<render_capsule, scene_conformance_error> {
    let projections = scene.projections();
    let render = projections.render_plan();
    let machine = projections.machine_projection();
    if render.command_count() != standard_paint_command_count
        || render.command_count() != fixture_command_count
        || render.accessibility_node_count() != scene_node_count
        || machine.node_count() != machine_node_count
        || machine.action_count() != machine_action_count
        || render.command(fixture_command_count).is_some()
        || render.accessibility_node(scene_node_count).is_some()
        || machine.node(machine_node_count).is_some()
        || machine.action(machine_action_count).is_some()
    {
        return Err(scene_conformance_error::projection_mismatch);
    }

    for index in 0..scene_node_count {
        let accessible = render
            .accessibility_node(index)
            .ok_or(scene_conformance_error::projection_mismatch)?;
        let semantic = machine
            .node(index)
            .ok_or(scene_conformance_error::projection_mismatch)?;
        let human_label = render
            .label(index)
            .ok_or(scene_conformance_error::projection_mismatch)?;
        let machine_label = machine
            .label(index)
            .ok_or(scene_conformance_error::projection_mismatch)?;
        if accessible.slot() != slots[index]
            || accessible.role() != roles[index]
            || accessible.node() != semantic.node()
            || accessible.parent() != semantic.parent()
            || accessible.role() != semantic.role()
            || accessible.bounds() != semantic.bounds()
            || accessible.disclosure() != semantic.disclosure()
            || accessible.enabled() != semantic.enabled()
            || accessible.focused() != semantic.focused()
            || accessible.focus_order() != semantic.focus_order()
            || accessible.action() != semantic.action()
            || human_label.disclosure() != machine_label.disclosure()
            || human_label.expose_str() != labels[index]
            || machine_label.expose_str() != labels[index]
        {
            return Err(scene_conformance_error::projection_mismatch);
        }
    }

    for index in 0..machine_action_count {
        let descriptor = machine
            .action(index)
            .ok_or(scene_conformance_error::projection_mismatch)?;
        let slot = if index == 0 {
            scene_slot::primary_action
        } else {
            scene_slot::secondary_action
        };
        let node = machine
            .node(slot.index())
            .ok_or(scene_conformance_error::projection_mismatch)?;
        if Some(descriptor.reference()) != node.action()
            || descriptor.enabled() != node.enabled()
            || descriptor.focused() != node.focused()
        {
            return Err(scene_conformance_error::projection_mismatch);
        }
    }

    let first = mapped_command(&render, 0)?;
    let mut commands = [first; fixture_command_count];
    for (index, destination) in commands.iter_mut().enumerate() {
        *destination = mapped_command(&render, index)?;
    }
    let viewport = capsule_rect::try_new(
        render.viewport().x(),
        render.viewport().y(),
        render.viewport().width(),
        render.viewport().height(),
    )
    .map_err(|_| scene_conformance_error::capsule_rejected)?;
    render_capsule::try_encode(viewport, render.state().candidate_ordinal(), &commands)
        .map_err(|_| scene_conformance_error::capsule_rejected)
}

fn mapped_command<'a>(
    render: &'a accessible_render_plan<'_>,
    index: usize,
) -> Result<projection_command<'a>, scene_conformance_error> {
    let command = render
        .command(index)
        .ok_or(scene_conformance_error::projection_mismatch)?;
    let label = if command.kind() == paint_command_kind::label {
        Some(
            render
                .label(command.slot().index())
                .ok_or(scene_conformance_error::projection_mismatch)?
                .expose_str(),
        )
    } else {
        None
    };
    let bounds = capsule_rect::try_new(
        command.bounds().x(),
        command.bounds().y(),
        command.bounds().width(),
        command.bounds().height(),
    )
    .map_err(|_| scene_conformance_error::capsule_rejected)?;
    Ok(projection_command::new(
        map_kind(command.kind()),
        map_shape(command.shape()),
        map_color(command.color()),
        command.secondary_color().map(map_color),
        map_slot(command.slot()),
        bounds,
        command.state_active(),
        label,
    ))
}

fn map_kind(kind: paint_command_kind) -> capsule_command_kind {
    match kind {
        paint_command_kind::fill => capsule_command_kind::fill,
        paint_command_kind::outline => capsule_command_kind::outline,
        paint_command_kind::label => capsule_command_kind::label,
        paint_command_kind::enabled_marker => capsule_command_kind::enabled_marker,
        paint_command_kind::focus_outline => capsule_command_kind::focus_outline,
        paint_command_kind::shadow => capsule_command_kind::shadow,
        paint_command_kind::highlight => capsule_command_kind::highlight,
    }
}

fn map_shape(shape: paint_shape_token) -> capsule_shape {
    match shape {
        paint_shape_token::rectangle => capsule_shape::rectangle,
        paint_shape_token::text => capsule_shape::text,
        paint_shape_token::enabled => capsule_shape::enabled,
        paint_shape_token::disabled => capsule_shape::disabled,
        paint_shape_token::focus_absent => capsule_shape::focus_absent,
        paint_shape_token::focus_double_outline => capsule_shape::focus_double_outline,
    }
}

fn map_color(color: paint_color_token) -> capsule_color {
    match color {
        paint_color_token::canvas => capsule_color::canvas,
        paint_color_token::surface => capsule_color::surface,
        paint_color_token::primary_fill => capsule_color::primary_fill,
        paint_color_token::secondary_fill => capsule_color::secondary_fill,
        paint_color_token::disabled => capsule_color::disabled,
        paint_color_token::primary_text => capsule_color::primary_text,
        paint_color_token::secondary_text => capsule_color::secondary_text,
        paint_color_token::on_accent => capsule_color::on_accent,
        paint_color_token::outline => capsule_color::outline,
        paint_color_token::focus_outer => capsule_color::focus_outer,
        paint_color_token::focus_inner => capsule_color::focus_inner,
        paint_color_token::public_status => capsule_color::public_status,
        paint_color_token::protected_status => capsule_color::protected_status,
        paint_color_token::shadow => capsule_color::shadow,
        paint_color_token::highlight => capsule_color::highlight,
    }
}

fn map_slot(slot: scene_slot) -> capsule_slot {
    match slot {
        scene_slot::root => capsule_slot::root,
        scene_slot::masthead => capsule_slot::masthead,
        scene_slot::navigation => capsule_slot::navigation,
        scene_slot::primary_action => capsule_slot::primary_action,
        scene_slot::secondary_action => capsule_slot::secondary_action,
        scene_slot::content => capsule_slot::content,
        scene_slot::heading => capsule_slot::heading,
        scene_slot::status => capsule_slot::status,
    }
}
