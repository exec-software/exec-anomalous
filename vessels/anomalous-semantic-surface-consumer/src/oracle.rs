use super::{binding_oracle, command_oracle, state_oracle};

// This is a hand-maintained oracle for the ADR 0027 ordinal-60 protected
// surface. It intentionally does not invoke the scene service, transport
// adapter, or transport encoder. Raw discriminants make the expected wire
// contract independent from the decoder that the consumer exercises.
const command_fill: u8 = 1;
const command_outline: u8 = 2;
const command_label: u8 = 3;
const command_enabled_marker: u8 = 4;
const command_focus_outline: u8 = 5;
const command_shadow: u8 = 6;
const command_highlight: u8 = 7;

const shape_rectangle: u8 = 1;
const shape_text: u8 = 2;
const shape_enabled: u8 = 3;
const shape_disabled: u8 = 4;
const shape_focus_absent: u8 = 5;
const shape_focus_double_outline: u8 = 6;

const color_canvas: u8 = 1;
const color_surface: u8 = 2;
const color_primary_fill: u8 = 3;
const color_disabled: u8 = 5;
const color_primary_text: u8 = 6;
const color_on_accent: u8 = 8;
const color_outline: u8 = 9;
const color_focus_outer: u8 = 10;
const color_focus_inner: u8 = 11;
const color_protected_status: u8 = 13;
const color_shadow: u8 = 14;
const color_highlight: u8 = 15;

const slot_root: u8 = 1;
const slot_masthead: u8 = 2;
const slot_navigation: u8 = 3;
const slot_primary_action: u8 = 4;
const slot_secondary_action: u8 = 5;
const slot_content: u8 = 6;
const slot_heading: u8 = 7;
const slot_status: u8 = 8;

pub(super) const expected_binding: binding_oracle = binding_oracle {
    surface_slot: 51,
    surface_generation: 3,
    scene_slot: 41,
    scene_generation: 7,
    base_revision: 1,
    revision: 2,
    renderer_slot: 61,
    renderer_generation: 1,
    attempt: 1,
};

pub(super) const expected_state: state_oracle = state_oracle {
    primary_enabled: true,
    secondary_enabled: false,
    focus: 1,
    disclosure: 1,
    depth: 0,
    motion: 0,
};

pub(super) const expected_labels: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 9];
pub(super) const expected_command_count: usize = 25;

pub(super) const expected_commands: [command_oracle; expected_command_count] = [
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_canvas,
        secondary_color: 0,
        slot: slot_root,
        state_active: true,
        x: 0,
        y: 0,
        width: 1_024,
        height: 768,
    },
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_surface,
        secondary_color: 0,
        slot: slot_masthead,
        state_active: true,
        x: 32,
        y: 24,
        width: 960,
        height: 88,
    },
    command_oracle {
        kind: command_outline,
        shape: shape_rectangle,
        color: color_outline,
        secondary_color: 0,
        slot: slot_masthead,
        state_active: true,
        x: 32,
        y: 24,
        width: 960,
        height: 88,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_primary_text,
        secondary_color: 0,
        slot: slot_masthead,
        state_active: true,
        x: 32,
        y: 24,
        width: 960,
        height: 88,
    },
    command_oracle {
        kind: command_shadow,
        shape: shape_rectangle,
        color: color_shadow,
        secondary_color: 0,
        slot: slot_navigation,
        state_active: true,
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_surface,
        secondary_color: 0,
        slot: slot_navigation,
        state_active: true,
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    command_oracle {
        kind: command_highlight,
        shape: shape_rectangle,
        color: color_highlight,
        secondary_color: 0,
        slot: slot_navigation,
        state_active: true,
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    command_oracle {
        kind: command_outline,
        shape: shape_rectangle,
        color: color_outline,
        secondary_color: 0,
        slot: slot_navigation,
        state_active: true,
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_primary_text,
        secondary_color: 0,
        slot: slot_navigation,
        state_active: true,
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_primary_fill,
        secondary_color: 0,
        slot: slot_primary_action,
        state_active: true,
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_outline,
        shape: shape_rectangle,
        color: color_outline,
        secondary_color: 0,
        slot: slot_primary_action,
        state_active: true,
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_on_accent,
        secondary_color: 0,
        slot: slot_primary_action,
        state_active: true,
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_enabled_marker,
        shape: shape_enabled,
        color: color_on_accent,
        secondary_color: 0,
        slot: slot_primary_action,
        state_active: true,
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_focus_outline,
        shape: shape_focus_double_outline,
        color: color_focus_outer,
        secondary_color: color_focus_inner,
        slot: slot_primary_action,
        state_active: true,
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_disabled,
        secondary_color: 0,
        slot: slot_secondary_action,
        state_active: true,
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_outline,
        shape: shape_rectangle,
        color: color_outline,
        secondary_color: 0,
        slot: slot_secondary_action,
        state_active: true,
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_primary_text,
        secondary_color: 0,
        slot: slot_secondary_action,
        state_active: true,
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_enabled_marker,
        shape: shape_disabled,
        color: color_outline,
        secondary_color: 0,
        slot: slot_secondary_action,
        state_active: false,
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_focus_outline,
        shape: shape_focus_absent,
        color: color_outline,
        secondary_color: 0,
        slot: slot_secondary_action,
        state_active: false,
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    command_oracle {
        kind: command_shadow,
        shape: shape_rectangle,
        color: color_shadow,
        secondary_color: 0,
        slot: slot_content,
        state_active: true,
        x: 312,
        y: 136,
        width: 680,
        height: 600,
    },
    command_oracle {
        kind: command_fill,
        shape: shape_rectangle,
        color: color_surface,
        secondary_color: 0,
        slot: slot_content,
        state_active: true,
        x: 312,
        y: 136,
        width: 680,
        height: 600,
    },
    command_oracle {
        kind: command_highlight,
        shape: shape_rectangle,
        color: color_highlight,
        secondary_color: 0,
        slot: slot_content,
        state_active: true,
        x: 312,
        y: 136,
        width: 680,
        height: 600,
    },
    command_oracle {
        kind: command_outline,
        shape: shape_rectangle,
        color: color_outline,
        secondary_color: 0,
        slot: slot_content,
        state_active: true,
        x: 312,
        y: 136,
        width: 680,
        height: 600,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_primary_text,
        secondary_color: 0,
        slot: slot_heading,
        state_active: true,
        x: 352,
        y: 176,
        width: 600,
        height: 48,
    },
    command_oracle {
        kind: command_label,
        shape: shape_text,
        color: color_protected_status,
        secondary_color: 0,
        slot: slot_status,
        state_active: true,
        x: 352,
        y: 248,
        width: 600,
        height: 96,
    },
];
