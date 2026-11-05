use std::format;
use std::vec::Vec;

use crate::identity::{
    action_identity, identity_error, node_identity, scene_identity, scene_revision,
};
use crate::observation::{
    scene_observation_event, scene_observation_result, scene_privacy_observation,
};
use crate::projection::{
    accessibility_node_count, flat_paint_command_count, machine_action_count, machine_node_count,
    paint_color_token, paint_command, paint_command_kind, paint_shape_token, rgb8,
    standard_paint_command_count,
};
use crate::scene::{
    declared_scene_state, desktop_scene, focus_direction, scene_action_count,
    scene_action_definition, scene_action_kind, scene_candidate, scene_control, scene_depth,
    scene_disclosure, scene_error, scene_focus, scene_motion, scene_node_count,
    scene_node_definition, scene_rect, scene_resolution_source, scene_role, scene_slot,
};
use crate::text::{
    declared_public_text, protected_text, scene_label, scene_text_error, scene_text_limit,
};

const fixture_scene_slot: u64 = 41;
const fixture_generation: u64 = 7;
const protected_placeholder: &str = "Protected status";

const _: [(); 8] = [(); scene_node_count];
const _: [(); 8] = [(); accessibility_node_count];
const _: [(); 8] = [(); machine_node_count];
const _: [(); 2] = [(); scene_action_count];
const _: [(); 2] = [(); machine_action_count];
const _: [(); 25] = [(); standard_paint_command_count];
const _: [(); 21] = [(); flat_paint_command_count];

const oracle_slots: [scene_slot; scene_node_count] = [
    scene_slot::root,
    scene_slot::masthead,
    scene_slot::navigation,
    scene_slot::primary_action,
    scene_slot::secondary_action,
    scene_slot::content,
    scene_slot::heading,
    scene_slot::status,
];

const oracle_roles: [scene_role; scene_node_count] = [
    scene_role::desktop,
    scene_role::region,
    scene_role::region,
    scene_role::control,
    scene_role::control,
    scene_role::region,
    scene_role::heading,
    scene_role::status,
];

const oracle_parent_indices: [Option<usize>; scene_node_count] = [
    None,
    Some(0),
    Some(0),
    Some(2),
    Some(2),
    Some(0),
    Some(5),
    Some(5),
];

const oracle_bounds: [(u32, u32, u32, u32); scene_node_count] = [
    (0, 0, 1024, 768),
    (32, 24, 960, 88),
    (32, 136, 256, 600),
    (56, 184, 208, 64),
    (56, 272, 208, 64),
    (312, 136, 680, 600),
    (352, 176, 600, 48),
    (352, 248, 600, 96),
];

const oracle_labels: [&str; scene_node_count] = [
    "Anomalous desktop",
    "Anomalous",
    "Navigation",
    "Open shell",
    "Inspect system",
    "Workspace",
    "System workspace",
    "System ready",
];

const valid_ordinals: [u8; 64] = [
    0, 1, 2, 3, 4, 5, 6, 7, 24, 25, 26, 27, 28, 29, 30, 31, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81,
    82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95,
];

const invalid_ordinals: [u8; 32] = [
    8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 32, 33, 34, 35, 36, 37, 38, 39,
    64, 65, 66, 67, 68, 69, 70, 71,
];

const standard_kinds: [paint_command_kind; standard_paint_command_count] = [
    paint_command_kind::fill,
    paint_command_kind::fill,
    paint_command_kind::outline,
    paint_command_kind::label,
    paint_command_kind::shadow,
    paint_command_kind::fill,
    paint_command_kind::highlight,
    paint_command_kind::outline,
    paint_command_kind::label,
    paint_command_kind::fill,
    paint_command_kind::outline,
    paint_command_kind::label,
    paint_command_kind::enabled_marker,
    paint_command_kind::focus_outline,
    paint_command_kind::fill,
    paint_command_kind::outline,
    paint_command_kind::label,
    paint_command_kind::enabled_marker,
    paint_command_kind::focus_outline,
    paint_command_kind::shadow,
    paint_command_kind::fill,
    paint_command_kind::highlight,
    paint_command_kind::outline,
    paint_command_kind::label,
    paint_command_kind::label,
];

const standard_slots: [scene_slot; standard_paint_command_count] = [
    scene_slot::root,
    scene_slot::masthead,
    scene_slot::masthead,
    scene_slot::masthead,
    scene_slot::navigation,
    scene_slot::navigation,
    scene_slot::navigation,
    scene_slot::navigation,
    scene_slot::navigation,
    scene_slot::primary_action,
    scene_slot::primary_action,
    scene_slot::primary_action,
    scene_slot::primary_action,
    scene_slot::primary_action,
    scene_slot::secondary_action,
    scene_slot::secondary_action,
    scene_slot::secondary_action,
    scene_slot::secondary_action,
    scene_slot::secondary_action,
    scene_slot::content,
    scene_slot::content,
    scene_slot::content,
    scene_slot::content,
    scene_slot::heading,
    scene_slot::status,
];

const standard_shapes: [paint_shape_token; standard_paint_command_count] = [
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::text,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::text,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::text,
    paint_shape_token::enabled,
    paint_shape_token::focus_double_outline,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::text,
    paint_shape_token::enabled,
    paint_shape_token::focus_absent,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::rectangle,
    paint_shape_token::text,
    paint_shape_token::text,
];

const standard_colors: [paint_color_token; standard_paint_command_count] = [
    paint_color_token::canvas,
    paint_color_token::surface,
    paint_color_token::outline,
    paint_color_token::primary_text,
    paint_color_token::shadow,
    paint_color_token::surface,
    paint_color_token::highlight,
    paint_color_token::outline,
    paint_color_token::primary_text,
    paint_color_token::primary_fill,
    paint_color_token::outline,
    paint_color_token::on_accent,
    paint_color_token::on_accent,
    paint_color_token::focus_outer,
    paint_color_token::secondary_fill,
    paint_color_token::outline,
    paint_color_token::primary_text,
    paint_color_token::public_status,
    paint_color_token::outline,
    paint_color_token::shadow,
    paint_color_token::surface,
    paint_color_token::highlight,
    paint_color_token::outline,
    paint_color_token::primary_text,
    paint_color_token::public_status,
];

const flat_removed_standard_indices: [usize; 4] = [4, 6, 19, 21];

fn must_succeed<T, E>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("{context}"),
    }
}

fn must_reject<T, E>(result: Result<T, E>, context: &str) -> E {
    match result {
        Ok(_) => panic!("{context}"),
        Err(rejection) => rejection,
    }
}

fn fixture_identity() -> scene_identity {
    scene_identity::try_new(fixture_scene_slot, fixture_generation).expect("fixture scene identity")
}

fn oracle_rect(index: usize) -> scene_rect {
    let (x, y, width, height) = oracle_bounds[index];
    scene_rect::try_new(x, y, width, height).expect("oracle rectangle")
}

fn oracle_node_identity(identity: scene_identity, slot: scene_slot) -> node_identity {
    node_identity::try_new(slot.identity_slot(), identity.generation()).expect("node identity")
}

fn oracle_action_identity(identity: scene_identity, slot: scene_slot) -> action_identity {
    let action_slot = match slot {
        scene_slot::primary_action => 1,
        scene_slot::secondary_action => 2,
        _ => panic!("non-control slot has no action identity"),
    };
    action_identity::try_new(action_slot, identity.generation()).expect("action identity")
}

fn public_label(text: &str) -> scene_label {
    scene_label::declared_public(
        declared_public_text::try_from_str(text).expect("declared-public fixture label"),
    )
}

fn status_label(disclosure: scene_disclosure, text: &str) -> scene_label {
    match disclosure {
        scene_disclosure::declared_public => public_label(text),
        scene_disclosure::protected => scene_label::protected(
            protected_text::try_from_str(text).expect("protected fixture label"),
        ),
    }
}

fn canonical_nodes(
    identity: scene_identity,
    disclosure: scene_disclosure,
    status: &str,
) -> [scene_node_definition; scene_node_count] {
    core::array::from_fn(|index| {
        let slot = oracle_slots[index];
        let parent = oracle_parent_indices[index]
            .map(|parent_index| oracle_node_identity(identity, oracle_slots[parent_index]));
        let action = match slot {
            scene_slot::primary_action | scene_slot::secondary_action => {
                Some(scene_action_definition::new(
                    oracle_action_identity(identity, slot),
                    scene_action_kind::activate,
                ))
            }
            _ => None,
        };
        let label = if slot == scene_slot::status {
            status_label(disclosure, status)
        } else {
            public_label(oracle_labels[index])
        };
        scene_node_definition::new(
            slot,
            oracle_node_identity(identity, slot),
            oracle_roles[index],
            parent,
            oracle_rect(index),
            label,
            action,
        )
    })
}

fn candidate(
    identity: scene_identity,
    state: declared_scene_state,
    status: &str,
) -> scene_candidate {
    scene_candidate::new(
        identity,
        canonical_nodes(identity, state.status_disclosure(), status),
        state,
    )
}

fn ordinary_state() -> declared_scene_state {
    declared_scene_state::new(
        true,
        true,
        scene_focus::primary,
        scene_disclosure::declared_public,
        scene_depth::standard,
        scene_motion::standard,
    )
}

fn state_from_ordinal(ordinal: u8) -> declared_scene_state {
    assert!(ordinal < 96);
    let motion = if ordinal.is_multiple_of(2) {
        scene_motion::standard
    } else {
        scene_motion::reduced
    };
    let depth = if (ordinal / 2).is_multiple_of(2) {
        scene_depth::standard
    } else {
        scene_depth::flat
    };
    let disclosure = if (ordinal / 4).is_multiple_of(2) {
        scene_disclosure::declared_public
    } else {
        scene_disclosure::protected
    };
    let packed = ordinal / 8;
    let focus = match packed % 3 {
        0 => scene_focus::none,
        1 => scene_focus::primary,
        2 => scene_focus::secondary,
        _ => unreachable!(),
    };
    let enabled = packed / 3;
    declared_scene_state::new(
        enabled & 2 != 0,
        enabled & 1 != 0,
        focus,
        disclosure,
        depth,
        motion,
    )
}

fn construct(candidate: scene_candidate) -> (desktop_scene, scene_control) {
    match desktop_scene::new(candidate) {
        Ok(value) => value,
        Err(rejection) => panic!("scene construction failed: {}", rejection.error().code()),
    }
}

fn retire(scene: desktop_scene, control: scene_control) {
    match scene.retire(control) {
        Ok(receipt) => {
            assert!(receipt.identity().slot() != 0);
            assert!(receipt.final_revision().get() != 0);
        }
        Err(rejection) => panic!("scene retirement failed: {}", rejection.error().code()),
    }
}

fn command_fields_equal(left: paint_command, right: paint_command) -> bool {
    left.kind() == right.kind()
        && left.shape() == right.shape()
        && left.color() == right.color()
        && left.secondary_color() == right.secondary_color()
        && left.slot() == right.slot()
        && left.node() == right.node()
        && left.bounds() == right.bounds()
        && left.state_active() == right.state_active()
        && left.action() == right.action()
}

fn standard_index_for_plan_index(depth: scene_depth, plan_index: usize) -> usize {
    if depth == scene_depth::standard {
        return plan_index;
    }

    let mut flat_index = 0;
    for standard_index in 0..standard_paint_command_count {
        if flat_removed_standard_indices.contains(&standard_index) {
            continue;
        }
        if flat_index == plan_index {
            return standard_index;
        }
        flat_index += 1;
    }
    panic!("flat paint index outside the oracle budget");
}

fn oracle_paint_appearance(
    standard_index: usize,
    state: declared_scene_state,
) -> (
    paint_shape_token,
    paint_color_token,
    Option<paint_color_token>,
    bool,
) {
    let mut shape = standard_shapes[standard_index];
    let mut color = standard_colors[standard_index];
    let mut secondary = None;
    let mut state_active = true;

    match standard_index {
        9 => {
            color = if state.primary_enabled() {
                paint_color_token::primary_fill
            } else {
                paint_color_token::disabled
            };
        }
        11 => {
            color = if state.primary_enabled() {
                paint_color_token::on_accent
            } else {
                paint_color_token::secondary_text
            };
        }
        12 => {
            shape = if state.primary_enabled() {
                paint_shape_token::enabled
            } else {
                paint_shape_token::disabled
            };
            color = if state.primary_enabled() {
                paint_color_token::on_accent
            } else {
                paint_color_token::outline
            };
            state_active = state.primary_enabled();
        }
        13 => {
            let focused = state.focus() == scene_focus::primary;
            shape = if focused {
                paint_shape_token::focus_double_outline
            } else {
                paint_shape_token::focus_absent
            };
            color = if focused {
                paint_color_token::focus_outer
            } else {
                paint_color_token::outline
            };
            secondary = if focused {
                Some(paint_color_token::focus_inner)
            } else {
                None
            };
            state_active = focused;
        }
        14 => {
            color = if state.secondary_enabled() {
                paint_color_token::secondary_fill
            } else {
                paint_color_token::disabled
            };
        }
        17 => {
            shape = if state.secondary_enabled() {
                paint_shape_token::enabled
            } else {
                paint_shape_token::disabled
            };
            color = if state.secondary_enabled() {
                paint_color_token::public_status
            } else {
                paint_color_token::outline
            };
            state_active = state.secondary_enabled();
        }
        18 => {
            let focused = state.focus() == scene_focus::secondary;
            shape = if focused {
                paint_shape_token::focus_double_outline
            } else {
                paint_shape_token::focus_absent
            };
            color = if focused {
                paint_color_token::focus_outer
            } else {
                paint_color_token::outline
            };
            secondary = if focused {
                Some(paint_color_token::focus_inner)
            } else {
                None
            };
            state_active = focused;
        }
        24 => {
            color = if state.status_disclosure() == scene_disclosure::declared_public {
                paint_color_token::public_status
            } else {
                paint_color_token::protected_status
            };
        }
        _ => {}
    }

    (shape, color, secondary, state_active)
}

#[test]
fn exact_topology_geometry_preorder_and_semantic_parity_match_the_independent_oracle() {
    let identity = fixture_identity();
    let state = ordinary_state();
    let (scene, control) = construct(candidate(identity, state, oracle_labels[7]));
    let projections = scene.projections();
    let render = projections.render_plan();
    let machine = projections.machine_projection();

    assert!(projections.identity() == identity);
    assert!(projections.revision().get() == 1);
    assert!(render.viewport() == oracle_rect(0));
    assert!(machine.viewport() == oracle_rect(0));
    assert!(render.command_count() == 25);
    assert!(render.accessibility_node_count() == accessibility_node_count);
    assert!(machine.node_count() == machine_node_count);
    assert!(machine.action_count() == machine_action_count);
    assert!(render.command(25).is_none());
    assert!(render.accessibility_node(8).is_none());
    assert!(render.label(8).is_none());
    assert!(machine.node(8).is_none());
    assert!(machine.label(8).is_none());
    assert!(machine.action(2).is_none());

    for index in 0..scene_node_count {
        let slot = oracle_slots[index];
        let expected_node = oracle_node_identity(identity, slot);
        let expected_parent = oracle_parent_indices[index]
            .map(|parent_index| oracle_node_identity(identity, oracle_slots[parent_index]));
        let accessible = render
            .accessibility_node(index)
            .expect("complete accessibility tree");
        let semantic = machine.node(index).expect("complete machine tree");

        assert!(slot.index() == index);
        assert!(slot.canonical_bounds() == oracle_rect(index));
        assert!(slot.canonical_role() == oracle_roles[index]);
        assert!(slot.canonical_parent() == oracle_parent_indices[index].map(|i| oracle_slots[i]));
        assert!(accessible.slot() == slot);
        assert!(semantic.slot() == slot);
        assert!(accessible.node() == expected_node);
        assert!(semantic.node() == expected_node);
        assert!(accessible.parent() == expected_parent);
        assert!(semantic.parent() == expected_parent);
        assert!(accessible.role() == oracle_roles[index]);
        assert!(semantic.role() == oracle_roles[index]);
        assert!(accessible.bounds() == oracle_rect(index));
        assert!(semantic.bounds() == oracle_rect(index));
        assert!(accessible.disclosure() == semantic.disclosure());
        assert!(accessible.enabled() == semantic.enabled());
        assert!(accessible.focused() == semantic.focused());
        assert!(accessible.focus_order() == semantic.focus_order());
        assert!(accessible.action() == semantic.action());

        let human_label = render.label(index).expect("human label");
        let machine_label = machine.label(index).expect("machine label");
        assert!(human_label.disclosure() == machine_label.disclosure());
        assert!(human_label.expose_str() == oracle_labels[index]);
        assert!(machine_label.expose_str() == oracle_labels[index]);

        let is_primary = slot == scene_slot::primary_action;
        let is_secondary = slot == scene_slot::secondary_action;
        if is_primary || is_secondary {
            let reference = accessible.action().expect("control action reference");
            assert!(reference.scene() == identity);
            assert!(reference.revision().get() == 1);
            assert!(reference.node() == expected_node);
            assert!(reference.action() == oracle_action_identity(identity, slot));
            assert!(reference.kind() == scene_action_kind::activate);
            assert!(accessible.focus_order() == Some(if is_primary { 0 } else { 1 }));
            assert!(accessible.focused() == is_primary);
        } else {
            assert!(accessible.action().is_none());
            assert!(accessible.focus_order().is_none());
            assert!(!accessible.focused());
        }
    }

    for action_index in 0..machine_action_count {
        let descriptor = machine.action(action_index).expect("machine action");
        let slot = if action_index == 0 {
            scene_slot::primary_action
        } else {
            scene_slot::secondary_action
        };
        let node = machine.node(slot.index()).expect("control node");
        assert!(descriptor.reference() == node.action().expect("control action"));
        assert!(descriptor.enabled() == node.enabled());
        assert!(descriptor.focused() == node.focused());
    }

    retire(scene, control);
}

#[test]
fn complete_state_matrix_has_the_exact_valid_and_invalid_ordinal_sets() {
    let identity = fixture_identity();
    let mut observed_valid = Vec::new();
    let mut observed_invalid = Vec::new();

    for ordinal in 0..96 {
        let state = state_from_ordinal(ordinal);
        assert!(state.candidate_ordinal() == ordinal);
        let result = desktop_scene::new(candidate(identity, state, "matrix status"));
        match result {
            Ok((scene, control)) => {
                observed_valid.push(ordinal);
                let projections = scene.projections();
                let plan = projections.render_plan();
                let machine = projections.machine_projection();
                let expected_commands = if state.depth() == scene_depth::standard {
                    standard_paint_command_count
                } else {
                    flat_paint_command_count
                };

                assert!(projections.identity() == identity);
                assert!(projections.revision().get() == 1);
                assert!(plan.command_count() == expected_commands);
                assert!(plan.state() == state);
                assert!(machine.state() == state);
                assert!(plan.accessibility_node_count() == accessibility_node_count);
                assert!(machine.node_count() == machine_node_count);
                assert!(machine.action_count() == machine_action_count);
                assert!(plan.command(expected_commands).is_none());
                assert!(plan.accessibility_node(accessibility_node_count).is_none());
                assert!(plan.label(accessibility_node_count).is_none());
                assert!(machine.node(machine_node_count).is_none());
                assert!(machine.label(machine_node_count).is_none());
                assert!(machine.action(machine_action_count).is_none());

                for plan_index in 0..expected_commands {
                    let standard_index = standard_index_for_plan_index(state.depth(), plan_index);
                    let command = plan.command(plan_index).expect("matrix paint command");
                    let slot = standard_slots[standard_index];
                    let (shape, color, secondary_color, state_active) =
                        oracle_paint_appearance(standard_index, state);
                    assert!(command.kind() == standard_kinds[standard_index]);
                    assert!(command.shape() == shape);
                    assert!(command.color() == color);
                    assert!(command.secondary_color() == secondary_color);
                    assert!(command.state_active() == state_active);
                    assert!(command.slot() == slot);
                    assert!(command.node() == oracle_node_identity(identity, slot));
                    assert!(command.bounds() == oracle_rect(slot.index()));
                    if matches!(
                        slot,
                        scene_slot::primary_action | scene_slot::secondary_action
                    ) {
                        let reference = command.action().expect("control paint action");
                        assert!(reference.scene() == identity);
                        assert!(reference.revision().get() == 1);
                        assert!(reference.node() == oracle_node_identity(identity, slot));
                        assert!(reference.action() == oracle_action_identity(identity, slot));
                        assert!(reference.kind() == scene_action_kind::activate);
                    } else {
                        assert!(command.action().is_none());
                    }
                }

                for index in 0..scene_node_count {
                    let slot = oracle_slots[index];
                    let accessible = plan
                        .accessibility_node(index)
                        .expect("matrix accessibility node");
                    let semantic = machine.node(index).expect("matrix machine node");
                    let expected_parent = oracle_parent_indices[index].map(|parent_index| {
                        oracle_node_identity(identity, oracle_slots[parent_index])
                    });
                    let expected_disclosure = if slot == scene_slot::status {
                        state.status_disclosure()
                    } else {
                        scene_disclosure::declared_public
                    };
                    let expected_enabled = match slot {
                        scene_slot::primary_action => state.primary_enabled(),
                        scene_slot::secondary_action => state.secondary_enabled(),
                        _ => true,
                    };
                    let expected_focused = match slot {
                        scene_slot::primary_action => state.focus() == scene_focus::primary,
                        scene_slot::secondary_action => state.focus() == scene_focus::secondary,
                        _ => false,
                    };
                    let expected_focus_order = match slot {
                        scene_slot::primary_action => Some(0),
                        scene_slot::secondary_action => Some(1),
                        _ => None,
                    };

                    assert!(accessible.slot() == slot);
                    assert!(semantic.slot() == slot);
                    assert!(accessible.node() == oracle_node_identity(identity, slot));
                    assert!(semantic.node() == accessible.node());
                    assert!(accessible.parent() == expected_parent);
                    assert!(semantic.parent() == expected_parent);
                    assert!(accessible.role() == oracle_roles[index]);
                    assert!(semantic.role() == oracle_roles[index]);
                    assert!(accessible.bounds() == oracle_rect(index));
                    assert!(semantic.bounds() == oracle_rect(index));
                    assert!(accessible.disclosure() == expected_disclosure);
                    assert!(semantic.disclosure() == expected_disclosure);
                    assert!(accessible.enabled() == expected_enabled);
                    assert!(semantic.enabled() == expected_enabled);
                    assert!(accessible.focused() == expected_focused);
                    assert!(semantic.focused() == expected_focused);
                    assert!(accessible.focus_order() == expected_focus_order);
                    assert!(semantic.focus_order() == expected_focus_order);
                    assert!(accessible.action() == semantic.action());

                    let human_label = plan.label(index).expect("matrix human label");
                    let machine_label = machine.label(index).expect("matrix machine label");
                    let expected_label = if slot == scene_slot::status {
                        if expected_disclosure == scene_disclosure::protected {
                            protected_placeholder
                        } else {
                            "matrix status"
                        }
                    } else {
                        oracle_labels[index]
                    };
                    assert!(human_label.disclosure() == expected_disclosure);
                    assert!(machine_label.disclosure() == expected_disclosure);
                    assert!(human_label.expose_str() == expected_label);
                    assert!(machine_label.expose_str() == expected_label);

                    if matches!(
                        slot,
                        scene_slot::primary_action | scene_slot::secondary_action
                    ) {
                        let reference = accessible.action().expect("matrix control action");
                        assert!(reference.scene() == identity);
                        assert!(reference.revision().get() == 1);
                        assert!(reference.node() == oracle_node_identity(identity, slot));
                        assert!(reference.action() == oracle_action_identity(identity, slot));
                        assert!(reference.kind() == scene_action_kind::activate);
                    } else {
                        assert!(accessible.action().is_none());
                    }
                }

                for (action_index, slot) in
                    [scene_slot::primary_action, scene_slot::secondary_action]
                        .into_iter()
                        .enumerate()
                {
                    let descriptor = machine.action(action_index).expect("matrix action");
                    let semantic = machine.node(slot.index()).expect("matrix control node");
                    let expected_enabled = if slot == scene_slot::primary_action {
                        state.primary_enabled()
                    } else {
                        state.secondary_enabled()
                    };
                    let expected_focused = state.focus()
                        == if slot == scene_slot::primary_action {
                            scene_focus::primary
                        } else {
                            scene_focus::secondary
                        };
                    assert!(descriptor.reference() == semantic.action().expect("control ref"));
                    assert!(descriptor.enabled() == expected_enabled);
                    assert!(descriptor.focused() == expected_focused);
                    assert!(semantic.enabled() == descriptor.enabled());
                    assert!(semantic.focused() == descriptor.focused());
                }

                retire(scene, control);
            }
            Err(rejection) => {
                observed_invalid.push(ordinal);
                assert!(rejection.error() == scene_error::focus_disabled);
                assert!(rejection.candidate().state().candidate_ordinal() == ordinal);
            }
        }
    }

    assert!(observed_valid.as_slice() == valid_ordinals);
    assert!(observed_invalid.as_slice() == invalid_ordinals);
}

#[test]
fn standard_and_flat_paint_streams_match_the_exact_order_and_removal_oracle() {
    let identity = fixture_identity();
    let standard_state = ordinary_state();
    let flat_state = declared_scene_state::new(
        true,
        true,
        scene_focus::primary,
        scene_disclosure::declared_public,
        scene_depth::flat,
        scene_motion::standard,
    );
    let (standard_scene, standard_control) =
        construct(candidate(identity, standard_state, oracle_labels[7]));
    let (flat_scene, flat_control) = construct(candidate(identity, flat_state, oracle_labels[7]));
    let standard = standard_scene.projections().render_plan();
    let flat = flat_scene.projections().render_plan();

    assert!(standard.command_count() == standard_paint_command_count);
    assert!(flat.command_count() == flat_paint_command_count);
    assert!(standard.command_count() == 25);
    assert!(flat.command_count() == 21);
    assert!(standard.command(25).is_none());
    assert!(flat.command(21).is_none());

    for index in 0..standard_paint_command_count {
        let command = standard.command(index).expect("standard command");
        let slot = standard_slots[index];
        assert!(command.kind() == standard_kinds[index]);
        assert!(command.slot() == slot);
        assert!(command.node() == oracle_node_identity(identity, slot));
        assert!(command.bounds() == slot.canonical_bounds());
        assert!(
            command.action().is_some()
                == matches!(
                    slot,
                    scene_slot::primary_action | scene_slot::secondary_action
                )
        );
    }

    let expected_shapes = [
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::text,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::text,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::text,
        paint_shape_token::enabled,
        paint_shape_token::focus_double_outline,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::text,
        paint_shape_token::enabled,
        paint_shape_token::focus_absent,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::rectangle,
        paint_shape_token::text,
        paint_shape_token::text,
    ];
    let expected_colors = [
        paint_color_token::canvas,
        paint_color_token::surface,
        paint_color_token::outline,
        paint_color_token::primary_text,
        paint_color_token::shadow,
        paint_color_token::surface,
        paint_color_token::highlight,
        paint_color_token::outline,
        paint_color_token::primary_text,
        paint_color_token::primary_fill,
        paint_color_token::outline,
        paint_color_token::on_accent,
        paint_color_token::on_accent,
        paint_color_token::focus_outer,
        paint_color_token::secondary_fill,
        paint_color_token::outline,
        paint_color_token::primary_text,
        paint_color_token::public_status,
        paint_color_token::outline,
        paint_color_token::shadow,
        paint_color_token::surface,
        paint_color_token::highlight,
        paint_color_token::outline,
        paint_color_token::primary_text,
        paint_color_token::public_status,
    ];
    for index in 0..standard_paint_command_count {
        let command = standard.command(index).expect("standard command");
        assert!(command.shape() == expected_shapes[index]);
        assert!(command.color() == expected_colors[index]);
        assert!(command.state_active() == (index != 18));
        assert!(
            command.secondary_color()
                == if index == 13 {
                    Some(paint_color_token::focus_inner)
                } else {
                    None
                }
        );
    }

    let mut flat_index = 0;
    for standard_index in 0..standard_paint_command_count {
        if flat_removed_standard_indices.contains(&standard_index) {
            continue;
        }
        let standard_command = standard.command(standard_index).expect("standard command");
        let flat_command = flat.command(flat_index).expect("flat command");
        assert!(command_fields_equal(standard_command, flat_command));
        flat_index += 1;
    }
    assert!(flat_index == flat_paint_command_count);
    assert!(standard.command(8).expect("navigation label").kind() == paint_command_kind::label);
    assert!(standard.command(24).expect("status label").kind() == paint_command_kind::label);

    retire(flat_scene, flat_control);
    retire(standard_scene, standard_control);
}

#[test]
fn projections_are_repeatable_and_motion_changes_only_the_context() {
    let identity = fixture_identity();
    let standard_state = ordinary_state();
    let reduced_state = declared_scene_state::new(
        true,
        true,
        scene_focus::primary,
        scene_disclosure::declared_public,
        scene_depth::standard,
        scene_motion::reduced,
    );
    let (standard_scene, standard_control) =
        construct(candidate(identity, standard_state, oracle_labels[7]));
    let (reduced_scene, reduced_control) =
        construct(candidate(identity, reduced_state, oracle_labels[7]));
    let standard = standard_scene.projections();
    let repeated = standard_scene.projections();
    let reduced = reduced_scene.projections();

    for index in 0..standard_paint_command_count {
        let first = standard
            .render_plan()
            .command(index)
            .expect("standard command");
        let second = repeated
            .render_plan()
            .command(index)
            .expect("repeated command");
        let motion_variant = reduced
            .render_plan()
            .command(index)
            .expect("reduced command");
        assert!(command_fields_equal(first, second));
        assert!(command_fields_equal(first, motion_variant));
    }
    for index in 0..scene_node_count {
        let first_accessible = standard
            .render_plan()
            .accessibility_node(index)
            .expect("accessible node");
        let second_accessible = reduced
            .render_plan()
            .accessibility_node(index)
            .expect("accessible node");
        let first_machine = standard
            .machine_projection()
            .node(index)
            .expect("machine node");
        let second_machine = reduced
            .machine_projection()
            .node(index)
            .expect("machine node");
        assert!(first_accessible == second_accessible);
        assert!(first_machine == second_machine);
        assert!(
            standard
                .render_plan()
                .label(index)
                .expect("label")
                .expose_str()
                == reduced
                    .render_plan()
                    .label(index)
                    .expect("label")
                    .expose_str()
        );
    }
    for index in 0..machine_action_count {
        assert!(
            standard.machine_projection().action(index)
                == reduced.machine_projection().action(index)
        );
    }
    assert!(standard.render_plan().state().motion() == scene_motion::standard);
    assert!(standard.machine_projection().state().motion() == scene_motion::standard);
    assert!(reduced.render_plan().state().motion() == scene_motion::reduced);
    assert!(reduced.machine_projection().state().motion() == scene_motion::reduced);

    retire(reduced_scene, reduced_control);
    retire(standard_scene, standard_control);
}

#[test]
fn focus_traversal_is_brand_bound_ordered_wrapping_and_nonmutating() {
    let identity = fixture_identity();
    let state = declared_scene_state::new(
        true,
        true,
        scene_focus::none,
        scene_disclosure::declared_public,
        scene_depth::standard,
        scene_motion::standard,
    );
    let (scene, control) = construct(candidate(identity, state, oracle_labels[7]));

    let forward = must_succeed(
        scene.traverse_focus(
            scene.projections().render_plan().mint_focus_handle(),
            focus_direction::forward,
        ),
        "first forward focus",
    );
    assert!(forward.source() == scene_resolution_source::accessible_projection);
    assert!(
        forward.reference().node() == oracle_node_identity(identity, scene_slot::primary_action)
    );
    assert!(!forward.scene_focus_changed());
    assert!(scene.state().focus() == scene_focus::none);

    let second = must_succeed(
        scene.traverse_focus(forward.into_handle(), focus_direction::forward),
        "second forward focus",
    );
    assert!(
        second.reference().node() == oracle_node_identity(identity, scene_slot::secondary_action)
    );
    let wrapped = must_succeed(
        scene.traverse_focus(second.into_handle(), focus_direction::forward),
        "wrapped forward focus",
    );
    assert!(
        wrapped.reference().node() == oracle_node_identity(identity, scene_slot::primary_action)
    );

    let reverse = must_succeed(
        scene.traverse_focus(
            scene.projections().machine_projection().mint_focus_handle(),
            focus_direction::reverse,
        ),
        "first reverse focus",
    );
    assert!(reverse.source() == scene_resolution_source::machine_projection);
    assert!(
        reverse.reference().node() == oracle_node_identity(identity, scene_slot::secondary_action)
    );
    let reverse_second = must_succeed(
        scene.traverse_focus(reverse.into_handle(), focus_direction::reverse),
        "second reverse focus",
    );
    assert!(
        reverse_second.reference().node()
            == oracle_node_identity(identity, scene_slot::primary_action)
    );
    let reverse_wrapped = must_succeed(
        scene.traverse_focus(reverse_second.into_handle(), focus_direction::reverse),
        "wrapped reverse focus",
    );
    assert!(
        reverse_wrapped.reference().node()
            == oracle_node_identity(identity, scene_slot::secondary_action)
    );

    retire(scene, control);

    for (primary, secondary, expected) in [
        (true, false, scene_slot::primary_action),
        (false, true, scene_slot::secondary_action),
    ] {
        let state = declared_scene_state::new(
            primary,
            secondary,
            scene_focus::none,
            scene_disclosure::declared_public,
            scene_depth::flat,
            scene_motion::reduced,
        );
        let (scene, control) = construct(candidate(identity, state, oracle_labels[7]));
        for direction in [focus_direction::forward, focus_direction::reverse] {
            let first = must_succeed(
                scene.traverse_focus(
                    scene.projections().render_plan().mint_focus_handle(),
                    direction,
                ),
                "single enabled focus",
            );
            assert!(first.reference().node() == oracle_node_identity(identity, expected));
            let again = must_succeed(
                scene.traverse_focus(first.into_handle(), direction),
                "single enabled wrap",
            );
            assert!(again.reference().node() == oracle_node_identity(identity, expected));
        }
        retire(scene, control);
    }

    let disabled_state = declared_scene_state::new(
        false,
        false,
        scene_focus::none,
        scene_disclosure::declared_public,
        scene_depth::standard,
        scene_motion::standard,
    );
    let (scene, control) = construct(candidate(identity, disabled_state, oracle_labels[7]));
    let rejection = must_reject(
        scene.traverse_focus(
            scene.projections().render_plan().mint_focus_handle(),
            focus_direction::forward,
        ),
        "no enabled focus must reject",
    );
    assert!(rejection.error() == scene_error::no_enabled_focus);
    assert!(rejection.into_handle().cursor() == scene_focus::none);
    retire(scene, control);
}

#[test]
fn protected_labels_are_projection_noninterfering_and_actions_require_public_labels() {
    let identity = fixture_identity();
    let state = declared_scene_state::new(
        true,
        true,
        scene_focus::secondary,
        scene_disclosure::protected,
        scene_depth::standard,
        scene_motion::standard,
    );
    let first_secret = "private-alpha-sentinel";
    let second_secret = "a much longer private beta sentinel";
    let (first_scene, first_control) = construct(candidate(identity, state, first_secret));
    let (second_scene, second_control) = construct(candidate(identity, state, second_secret));
    let first = first_scene.projections();
    let second = second_scene.projections();
    let first_render = first.render_plan();
    let second_render = second.render_plan();
    let first_machine = first.machine_projection();
    let second_machine = second.machine_projection();

    for index in 0..standard_paint_command_count {
        assert!(command_fields_equal(
            first_render.command(index).expect("first command"),
            second_render.command(index).expect("second command")
        ));
    }
    for index in 0..scene_node_count {
        assert!(first_render.accessibility_node(index) == second_render.accessibility_node(index));
        assert!(first_machine.node(index) == second_machine.node(index));
        let first_human = first_render.label(index).expect("first human label");
        let second_human = second_render.label(index).expect("second human label");
        let first_explicit = first_machine.label(index).expect("first machine label");
        let second_explicit = second_machine.label(index).expect("second machine label");
        assert!(first_human.disclosure() == second_human.disclosure());
        assert!(first_explicit.disclosure() == second_explicit.disclosure());
        assert!(first_human.expose_str() == second_human.expose_str());
        assert!(first_explicit.expose_str() == second_explicit.expose_str());
        for projected in [
            first_human.expose_str(),
            second_human.expose_str(),
            first_explicit.expose_str(),
            second_explicit.expose_str(),
        ] {
            assert!(!projected.contains(first_secret));
            assert!(!projected.contains(second_secret));
        }
        if index == scene_slot::status.index() {
            assert!(first_human.disclosure() == scene_disclosure::protected);
            assert!(first_human.expose_str() == protected_placeholder);
            assert!(second_human.expose_str() == protected_placeholder);
            assert!(first_explicit.expose_str() == protected_placeholder);
            assert!(second_explicit.expose_str() == protected_placeholder);
        }
    }
    for index in 0..machine_action_count {
        assert!(first_machine.action(index) == second_machine.action(index));
    }

    let action_reference = first_machine
        .action(0)
        .expect("protected first action")
        .reference();
    let first_action = must_succeed(
        first_machine.mint_action_handle(action_reference),
        "first protected action handle",
    );
    let second_action = must_succeed(
        second_machine.mint_action_handle(action_reference),
        "second protected action handle",
    );
    let first_resolution = must_succeed(
        first_scene.resolve_action(first_action),
        "first protected action resolution",
    );
    let second_resolution = must_succeed(
        second_scene.resolve_action(second_action),
        "second protected action resolution",
    );
    assert!(first_resolution.reference() == second_resolution.reference());
    assert!(first_resolution.source() == second_resolution.source());
    assert!(first_resolution.effect_performed() == second_resolution.effect_performed());

    let first_focus = must_succeed(
        first_scene.traverse_focus(first_render.mint_focus_handle(), focus_direction::forward),
        "first protected focus traversal",
    );
    let second_focus = must_succeed(
        second_scene.traverse_focus(second_render.mint_focus_handle(), focus_direction::forward),
        "second protected focus traversal",
    );
    assert!(first_focus.reference() == second_focus.reference());
    assert!(first_focus.source() == second_focus.source());
    assert!(first_focus.handle().cursor() == second_focus.handle().cursor());
    assert!(first_focus.scene_focus_changed() == second_focus.scene_focus_changed());

    let observation = scene_privacy_observation::new(
        scene_observation_event::construction,
        scene_observation_result::accepted,
    );
    let formatted = format!("{observation:?}");
    assert!(!formatted.contains(first_secret));
    assert!(!formatted.contains(second_secret));

    retire(second_scene, second_control);
    retire(first_scene, first_control);

    let public_state = ordinary_state();
    let mut nodes = canonical_nodes(identity, public_state.status_disclosure(), oracle_labels[7]);
    let primary = scene_slot::primary_action;
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        scene_label::protected(
            protected_text::try_from_str("hidden action").expect("protected action label"),
        ),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, primary),
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, public_state)),
        "protected actionable label must reject",
    );
    let first_error = rejection.error();
    assert!(first_error == scene_error::protected_label_not_allowed);
    let returned = rejection.into_candidate().into_nodes();
    assert!(returned[primary.index()].label().expose_protected() == Some("hidden action"));

    let mut nodes = canonical_nodes(identity, public_state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        scene_label::protected(
            protected_text::try_from_str("a much longer hidden primary action")
                .expect("long protected action label"),
        ),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, primary),
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, public_state)),
        "different protected action bytes must reject identically",
    );
    assert!(rejection.error() == first_error);
    let returned = rejection.into_candidate().into_nodes();
    assert!(
        returned[primary.index()].label().expose_protected()
            == Some("a much longer hidden primary action")
    );
    let first_rejected_observation = scene_privacy_observation::new(
        scene_observation_event::construction,
        scene_observation_result::rejected,
    );
    let second_rejected_observation = scene_privacy_observation::new(
        scene_observation_event::construction,
        scene_observation_result::rejected,
    );
    assert!(
        format!("{first_rejected_observation:?}") == format!("{second_rejected_observation:?}")
    );

    let secondary_state = declared_scene_state::new(
        true,
        true,
        scene_focus::secondary,
        scene_disclosure::declared_public,
        scene_depth::flat,
        scene_motion::reduced,
    );
    let mut nodes = canonical_nodes(
        identity,
        secondary_state.status_disclosure(),
        oracle_labels[7],
    );
    let secondary = scene_slot::secondary_action;
    nodes[secondary.index()] = scene_node_definition::new(
        secondary,
        oracle_node_identity(identity, secondary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(secondary.index()),
        scene_label::protected(
            protected_text::try_from_str("longer hidden secondary action")
                .expect("protected secondary label"),
        ),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, secondary),
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, secondary_state)),
        "protected secondary focus target must reject",
    );
    assert!(rejection.error() == scene_error::protected_label_not_allowed);
    let returned = rejection.into_candidate().into_nodes();
    assert!(
        returned[secondary.index()].label().expose_protected()
            == Some("longer hidden secondary action")
    );
}

#[test]
fn transactions_are_brand_bound_failure_atomic_stale_and_replay_safe() {
    let identity = fixture_identity();
    let (mut first, first_control) =
        construct(candidate(identity, ordinary_state(), "revision one"));
    let (second, second_control) = construct(candidate(identity, ordinary_state(), "other brand"));

    let wrong_control = must_reject(
        first.prepare_revision(
            &second_control,
            first.revision(),
            candidate(identity, ordinary_state(), "wrong control"),
        ),
        "equal public identity cannot cross private brands",
    );
    assert!(wrong_control.error() == scene_error::control_denied);
    assert!(
        wrong_control
            .into_candidate()
            .node(scene_slot::status.index())
            .expect("returned status")
            .label()
            .expose_declared_public()
            == Some("wrong control")
    );

    let foreign_prepared = must_succeed(
        second.prepare_revision(
            &second_control,
            second.revision(),
            candidate(identity, ordinary_state(), "foreign prepared"),
        ),
        "foreign owner preparation",
    );
    let foreign_commit = must_reject(
        first.commit_revision(foreign_prepared),
        "foreign prepared revision must not commit",
    );
    assert!(foreign_commit.error() == scene_error::control_denied);
    drop(foreign_commit.into_prepared());
    assert!(first.revision().get() == 1);
    assert!(first.state() == ordinary_state());
    assert!(second.revision().get() == 1);
    assert!(
        first
            .projections()
            .render_plan()
            .label(scene_slot::status.index())
            .expect("unchanged first status")
            .expose_str()
            == "revision one"
    );

    let old_action_reference = first
        .projections()
        .machine_projection()
        .action(0)
        .expect("primary action")
        .reference();
    let old_action_handle = must_succeed(
        first
            .projections()
            .machine_projection()
            .mint_action_handle(old_action_reference),
        "old action handle",
    );
    let old_focus_handle = first.projections().render_plan().mint_focus_handle();
    let base = first.revision();
    let replacement_state = declared_scene_state::new(
        true,
        true,
        scene_focus::secondary,
        scene_disclosure::declared_public,
        scene_depth::flat,
        scene_motion::reduced,
    );
    let first_prepared = must_succeed(
        first.prepare_revision(
            &first_control,
            base,
            candidate(identity, replacement_state, "revision two"),
        ),
        "first preparation",
    );
    let competing = must_succeed(
        first.prepare_revision(
            &first_control,
            base,
            candidate(identity, replacement_state, "competing revision"),
        ),
        "competing preparation",
    );
    assert!(first.revision() == base);
    assert!(first.state() == ordinary_state());

    let next = must_succeed(first.commit_revision(first_prepared), "first commit");
    assert!(next.get() == 2);
    assert!(first.state() == replacement_state);
    let stale = must_reject(
        first.commit_revision(competing),
        "competing preparation must become stale",
    );
    assert!(stale.error() == scene_error::stale_revision);
    let stale_again = must_reject(
        first.commit_revision(stale.into_prepared()),
        "stale preparation replay must remain stale",
    );
    assert!(stale_again.error() == scene_error::stale_revision);
    drop(stale_again);
    assert!(first.revision() == next);
    assert!(first.state() == replacement_state);
    assert!(
        first
            .projections()
            .render_plan()
            .label(scene_slot::status.index())
            .expect("committed status label")
            .expose_str()
            == "revision two"
    );

    let stale_action = must_reject(
        first.resolve_action(old_action_handle),
        "old action handle must be stale",
    );
    assert!(stale_action.error() == scene_error::stale_revision);
    let returned_action = stale_action.into_handle();
    assert!(returned_action.reference() == old_action_reference);
    assert!(returned_action.source() == scene_resolution_source::machine_projection);
    let stale_focus = must_reject(
        first.traverse_focus(old_focus_handle, focus_direction::forward),
        "old focus handle must be stale",
    );
    assert!(stale_focus.error() == scene_error::stale_revision);
    let returned_focus = stale_focus.into_handle();
    assert!(returned_focus.cursor() == scene_focus::primary);
    assert!(returned_focus.source() == scene_resolution_source::accessible_projection);
    assert!(matches!(
        first
            .projections()
            .machine_projection()
            .mint_action_handle(old_action_reference),
        Err(scene_error::invalid_action_reference)
    ));

    let foreign_handle = must_succeed(
        second
            .projections()
            .machine_projection()
            .mint_action_handle(
                second
                    .projections()
                    .machine_projection()
                    .action(0)
                    .expect("foreign action")
                    .reference(),
            ),
        "foreign handle",
    );
    let foreign = must_reject(
        first.resolve_action(foreign_handle),
        "private brand must reject foreign handle",
    );
    assert!(foreign.error() == scene_error::control_denied);
    let foreign = foreign.into_handle();
    assert!(foreign.reference() == old_action_reference);
    assert!(foreign.source() == scene_resolution_source::machine_projection);
    let foreign_focus = must_reject(
        first.traverse_focus(
            second.projections().render_plan().mint_focus_handle(),
            focus_direction::forward,
        ),
        "private brand must reject foreign focus handle",
    );
    assert!(foreign_focus.error() == scene_error::control_denied);
    let foreign_focus = foreign_focus.into_handle();
    assert!(foreign_focus.cursor() == scene_focus::primary);
    assert!(foreign_focus.source() == scene_resolution_source::accessible_projection);

    retire(first, first_control);
    retire(second, second_control);
}

#[test]
fn prepared_render_preview_is_redacted_revision_bound_and_candidate_recoverable() {
    let identity = fixture_identity();
    let initial_state = ordinary_state();
    let (mut scene, control) = construct(candidate(identity, initial_state, "visible baseline"));
    let base = scene.revision();
    let replacement_state = declared_scene_state::new(
        true,
        true,
        scene_focus::secondary,
        scene_disclosure::protected,
        scene_depth::flat,
        scene_motion::reduced,
    );
    let protected_source = "hidden prepared status";
    let prepared = must_succeed(
        scene.prepare_revision(
            &control,
            base,
            candidate(identity, replacement_state, protected_source),
        ),
        "prepared render preview",
    );

    {
        let preview = prepared.preview();
        assert!(preview.identity() == identity);
        assert!(preview.base_revision() == base);
        assert!(preview.next_revision() == base.next().expect("next revision"));
        assert!(preview.state() == replacement_state);
        assert!(preview.viewport() == oracle_rect(scene_slot::root.index()));
        assert!(preview.command_count() == flat_paint_command_count);
        assert!(preview.command(flat_paint_command_count).is_none());

        for index in 0..preview.command_count() {
            let command = preview.command(index).expect("prepared paint command");
            let standard_index = standard_index_for_plan_index(scene_depth::flat, index);
            let slot = standard_slots[standard_index];
            let (shape, color, secondary, state_active) =
                oracle_paint_appearance(standard_index, replacement_state);
            assert!(command.kind() == standard_kinds[standard_index]);
            assert!(command.shape() == shape);
            assert!(command.color() == color);
            assert!(command.secondary_color() == secondary);
            assert!(command.slot() == slot);
            assert!(command.node() == oracle_node_identity(identity, slot));
            assert!(command.bounds() == oracle_rect(slot.index()));
            assert!(command.state_active() == state_active);
        }

        for (index, expected_label) in oracle_labels.iter().enumerate() {
            let label = preview.label(index).expect("prepared projected label");
            if index == scene_slot::status.index() {
                assert!(label.disclosure() == scene_disclosure::protected);
                assert!(label.expose_str() == protected_placeholder);
                assert!(label.expose_str() != protected_source);
            } else {
                assert!(label.disclosure() == scene_disclosure::declared_public);
                assert!(label.expose_str() == *expected_label);
            }
        }
        assert!(preview.label(scene_node_count).is_none());
    }

    assert!(scene.revision() == base);
    assert!(scene.state() == initial_state);
    assert!(
        scene
            .projections()
            .render_plan()
            .label(scene_slot::status.index())
            .expect("still committed status")
            .expose_str()
            == "visible baseline"
    );

    let recovered = prepared.into_candidate();
    assert!(recovered.identity() == identity);
    assert!(recovered.state() == replacement_state);
    assert!(
        recovered
            .node(scene_slot::status.index())
            .expect("recovered protected status")
            .label()
            .expose_protected()
            == Some(protected_source)
    );

    let prepared = must_succeed(
        scene.prepare_revision(&control, base, recovered),
        "recovered candidate preparation",
    );
    let next = must_succeed(
        scene.commit_revision(prepared),
        "recovered candidate commit",
    );
    assert!(next == base.next().expect("committed next revision"));
    assert!(scene.state() == replacement_state);
    assert!(
        scene
            .projections()
            .render_plan()
            .label(scene_slot::status.index())
            .expect("committed protected status")
            .expose_str()
            == protected_placeholder
    );

    retire(scene, control);
}

#[test]
fn malformed_and_invalid_replacement_preparation_is_failure_atomic() {
    let identity = fixture_identity();
    let (scene, control) = construct(candidate(identity, ordinary_state(), "atomic baseline"));
    let before = scene.projections();
    let before_render = before.render_plan();
    let before_machine = before.machine_projection();

    let foreign_identity = scene_identity::try_new(fixture_scene_slot + 1, identity.generation())
        .expect("foreign replacement identity");
    let foreign = must_reject(
        scene.prepare_revision(
            &control,
            scene.revision(),
            candidate(foreign_identity, ordinary_state(), "foreign replacement"),
        ),
        "wrong scene identity replacement must reject",
    );
    assert!(foreign.error() == scene_error::wrong_scene_identity);
    assert!(foreign.into_candidate().identity() == foreign_identity);

    let invalid_state = declared_scene_state::new(
        false,
        true,
        scene_focus::primary,
        scene_disclosure::declared_public,
        scene_depth::flat,
        scene_motion::reduced,
    );
    let rejected = must_reject(
        scene.prepare_revision(
            &control,
            scene.revision(),
            candidate(identity, invalid_state, "invalid replacement returned"),
        ),
        "focus-disabled replacement must reject",
    );
    assert!(rejected.error() == scene_error::focus_disabled);
    let returned = rejected.into_candidate();
    assert!(returned.state() == invalid_state);
    assert!(
        returned
            .node(scene_slot::status.index())
            .expect("returned invalid replacement status")
            .label()
            .expose_declared_public()
            == Some("invalid replacement returned")
    );

    assert!(scene.revision().get() == 1);
    assert!(scene.state() == ordinary_state());
    let after = scene.projections();
    let render = after.render_plan();
    let machine = after.machine_projection();
    assert!(render.command_count() == 25);
    for index in 0..25 {
        let expected_state = oracle_paint_appearance(index, ordinary_state());
        let command = render.command(index).expect("unchanged paint command");
        assert!(command_fields_equal(
            before_render
                .command(index)
                .expect("baseline paint command"),
            command,
        ));
        assert!(command.kind() == standard_kinds[index]);
        assert!(command.slot() == standard_slots[index]);
        assert!(command.shape() == expected_state.0);
        assert!(command.color() == expected_state.1);
        assert!(command.secondary_color() == expected_state.2);
        assert!(command.state_active() == expected_state.3);
    }
    for (index, oracle_label) in oracle_labels.iter().enumerate() {
        let accessible = render
            .accessibility_node(index)
            .expect("unchanged accessibility node");
        let semantic = machine.node(index).expect("unchanged machine node");
        assert!(before_render.accessibility_node(index) == Some(accessible));
        assert!(before_machine.node(index) == Some(semantic));
        assert!(accessible.node() == semantic.node());
        assert!(accessible.parent() == semantic.parent());
        assert!(accessible.role() == semantic.role());
        assert!(accessible.bounds() == semantic.bounds());
        assert!(accessible.disclosure() == semantic.disclosure());
        assert!(accessible.enabled() == semantic.enabled());
        assert!(accessible.focused() == semantic.focused());
        assert!(accessible.focus_order() == semantic.focus_order());
        assert!(accessible.action() == semantic.action());
        let expected_label = if index == scene_slot::status.index() {
            "atomic baseline"
        } else {
            *oracle_label
        };
        let human_label = render.label(index).expect("unchanged human label");
        let machine_label = machine.label(index).expect("unchanged machine label");
        assert!(
            before_render
                .label(index)
                .expect("baseline human label")
                .expose_str()
                == human_label.expose_str()
        );
        assert!(
            before_machine
                .label(index)
                .expect("baseline machine label")
                .expose_str()
                == machine_label.expose_str()
        );
        assert!(human_label.expose_str() == expected_label);
        assert!(machine_label.expose_str() == expected_label);
    }
    assert!(
        render
            .label(scene_slot::status.index())
            .expect("unchanged committed status")
            .expose_str()
            == "atomic baseline"
    );
    for index in 0..2 {
        let descriptor = machine.action(index).expect("unchanged action descriptor");
        assert!(before_machine.action(index) == Some(descriptor));
        let slot = if index == 0 {
            scene_slot::primary_action
        } else {
            scene_slot::secondary_action
        };
        assert!(
            descriptor.reference()
                == machine
                    .node(slot.index())
                    .expect("unchanged action node")
                    .action()
                    .expect("unchanged action reference")
        );
    }

    retire(scene, control);
}

#[test]
fn maximum_revision_can_commit_once_then_retire_and_fresh_owner_rejects_old_handles() {
    let identity = fixture_identity();
    let penultimate = scene_revision::try_new(u64::MAX - 1).expect("penultimate revision");
    let (mut scene, control) = desktop_scene::new_at_revision_for_test(
        candidate(identity, ordinary_state(), "penultimate"),
        penultimate,
    )
    .unwrap_or_else(|rejection| panic!("max-scene construction: {}", rejection.error().code()));
    let old_action = must_succeed(
        scene.projections().render_plan().mint_action_handle(
            scene
                .projections()
                .machine_projection()
                .action(0)
                .expect("action")
                .reference(),
        ),
        "old action handle",
    );
    let old_focus = scene.projections().machine_projection().mint_focus_handle();
    let prepared = must_succeed(
        scene.prepare_revision(
            &control,
            penultimate,
            candidate(identity, ordinary_state(), "maximum"),
        ),
        "final preparation",
    );
    let maximum = must_succeed(scene.commit_revision(prepared), "final commit");
    assert!(maximum.get() == u64::MAX);
    let exhausted = must_reject(
        scene.prepare_revision(
            &control,
            maximum,
            candidate(identity, ordinary_state(), "must be returned"),
        ),
        "revision exhaustion must reject",
    );
    assert!(exhausted.error() == scene_error::revision_exhausted);
    assert!(
        exhausted
            .into_candidate()
            .node(scene_slot::status.index())
            .expect("returned status")
            .label()
            .expose_declared_public()
            == Some("must be returned")
    );
    let retirement = must_succeed(scene.retire(control), "retirement at maximum revision");
    assert!(retirement.final_revision() == maximum);

    let fresh_identity = identity.next_generation().expect("fresh scene generation");
    let (fresh, fresh_control) = construct(candidate(fresh_identity, ordinary_state(), "fresh"));
    let stale_action = must_reject(
        fresh.resolve_action(old_action),
        "retired owner action handle must not cross replacement",
    );
    assert!(stale_action.error() == scene_error::control_denied);
    let stale_focus = must_reject(
        fresh.traverse_focus(old_focus, focus_direction::forward),
        "retired owner focus handle must not cross replacement",
    );
    assert!(stale_focus.error() == scene_error::control_denied);
    retire(fresh, fresh_control);
}

#[test]
fn action_resolution_is_enabled_brand_bound_effect_free_and_machine_focus_independent() {
    let identity = fixture_identity();
    let (scene, control) = construct(candidate(identity, ordinary_state(), oracle_labels[7]));
    let machine = scene.projections().machine_projection();
    let human = scene.projections().render_plan();
    let primary = machine.action(0).expect("primary action");
    let secondary = machine.action(1).expect("secondary action");

    let human_handle = must_succeed(
        human.mint_action_handle(primary.reference()),
        "human action handle",
    );
    let human_resolution = must_succeed(
        scene.resolve_action(human_handle),
        "human action resolution",
    );
    assert!(human_resolution.reference() == primary.reference());
    assert!(human_resolution.source() == scene_resolution_source::accessible_projection);
    assert!(!human_resolution.effect_performed());

    let machine_handle = must_succeed(
        machine.mint_action_handle(secondary.reference()),
        "machine action handle",
    );
    let machine_resolution = must_succeed(
        scene.resolve_action(machine_handle),
        "machine action resolution independent of human focus",
    );
    assert!(machine_resolution.reference() == secondary.reference());
    assert!(machine_resolution.source() == scene_resolution_source::machine_projection);
    assert!(!machine_resolution.effect_performed());
    assert!(scene.state().focus() == scene_focus::primary);

    retire(scene, control);

    let disabled_state = declared_scene_state::new(
        false,
        true,
        scene_focus::secondary,
        scene_disclosure::declared_public,
        scene_depth::standard,
        scene_motion::standard,
    );
    let (scene, control) = construct(candidate(identity, disabled_state, oracle_labels[7]));
    let machine = scene.projections().machine_projection();
    let disabled = machine.action(0).expect("disabled action descriptor");
    assert!(!disabled.enabled());
    let disabled_handle = must_succeed(
        machine.mint_action_handle(disabled.reference()),
        "disabled action handle",
    );
    let rejection = must_reject(
        scene.resolve_action(disabled_handle),
        "disabled action resolution must reject",
    );
    assert!(rejection.error() == scene_error::disabled_action);
    assert!(rejection.into_handle().reference() == disabled.reference());
    retire(scene, control);
}

#[test]
fn malformed_definitions_are_rejected_with_owned_labels_returned() {
    let identity = fixture_identity();
    let state = ordinary_state();

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[scene_slot::masthead.index()] = scene_node_definition::new(
        scene_slot::navigation,
        oracle_node_identity(identity, scene_slot::masthead),
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::root)),
        oracle_rect(scene_slot::masthead.index()),
        public_label("wrong slot"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "wrong slot must reject",
    );
    assert!(rejection.error() == scene_error::wrong_slot);

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[scene_slot::masthead.index()] = scene_node_definition::new(
        scene_slot::masthead,
        oracle_node_identity(identity, scene_slot::masthead),
        scene_role::heading,
        Some(oracle_node_identity(identity, scene_slot::root)),
        oracle_rect(scene_slot::masthead.index()),
        public_label("wrong role"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "wrong role must reject",
    );
    assert!(rejection.error() == scene_error::wrong_role);

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[scene_slot::masthead.index()] = scene_node_definition::new(
        scene_slot::masthead,
        oracle_node_identity(identity, scene_slot::masthead),
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(scene_slot::masthead.index()),
        public_label("wrong parent"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "wrong parent must reject",
    );
    assert!(rejection.error() == scene_error::wrong_parent);

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[scene_slot::masthead.index()] = scene_node_definition::new(
        scene_slot::masthead,
        oracle_node_identity(identity, scene_slot::masthead),
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::root)),
        scene_rect::try_new(33, 24, 960, 88).expect("wrong geometry value"),
        public_label("wrong geometry"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "wrong geometry must reject",
    );
    assert!(rejection.error() == scene_error::wrong_geometry);

    let masthead_slot = scene_slot::masthead;
    let wrong_node_slot =
        node_identity::try_new(99, identity.generation()).expect("nonduplicate wrong node slot");
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[masthead_slot.index()] = scene_node_definition::new(
        masthead_slot,
        wrong_node_slot,
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::root)),
        oracle_rect(masthead_slot.index()),
        public_label("wrong node identity slot"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "nonduplicate wrong node slot must reject",
    );
    assert!(rejection.error() == scene_error::wrong_node_identity);
    assert!(
        rejection
            .into_candidate()
            .node(masthead_slot.index())
            .expect("returned wrong-node candidate")
            .identity()
            == wrong_node_slot
    );

    let primary = scene_slot::primary_action;
    let wrong_action_slot = action_identity::try_new(99, identity.generation())
        .expect("nonduplicate wrong action slot");
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        public_label("wrong action identity slot"),
        Some(scene_action_definition::new(
            wrong_action_slot,
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "nonduplicate wrong action slot must reject",
    );
    assert!(rejection.error() == scene_error::wrong_action_identity);
    assert!(
        rejection
            .into_candidate()
            .node(primary.index())
            .expect("returned wrong-action candidate")
            .action()
            .expect("returned wrong action")
            .identity()
            == wrong_action_slot
    );

    let wrong_node_generation =
        node_identity::try_new(masthead_slot.identity_slot(), identity.generation() + 1)
            .expect("mismatched node generation");
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[masthead_slot.index()] = scene_node_definition::new(
        masthead_slot,
        wrong_node_generation,
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::root)),
        oracle_rect(masthead_slot.index()),
        public_label("wrong node generation"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "mismatched node generation must reject",
    );
    assert!(rejection.error() == scene_error::wrong_identity_generation);
    assert!(
        rejection
            .into_candidate()
            .node(masthead_slot.index())
            .expect("returned node-generation candidate")
            .identity()
            == wrong_node_generation
    );

    let wrong_action_generation = action_identity::try_new(1, identity.generation() + 1)
        .expect("mismatched action generation");
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        public_label("wrong action generation"),
        Some(scene_action_definition::new(
            wrong_action_generation,
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "mismatched action generation must reject",
    );
    assert!(rejection.error() == scene_error::wrong_identity_generation);
    assert!(
        rejection
            .into_candidate()
            .node(primary.index())
            .expect("returned action-generation candidate")
            .action()
            .expect("returned generation-mismatched action")
            .identity()
            == wrong_action_generation
    );

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        public_label("dismiss is not fixture action"),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, primary),
            scene_action_kind::dismiss,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "dismiss action must reject",
    );
    assert!(rejection.error() == scene_error::wrong_action);
    assert!(
        rejection
            .into_candidate()
            .node(primary.index())
            .expect("returned dismiss candidate")
            .action()
            .expect("returned dismiss action")
            .kind()
            == scene_action_kind::dismiss
    );

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, primary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        public_label("missing action"),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "missing control action must reject",
    );
    assert!(rejection.error() == scene_error::wrong_action);
    assert!(
        rejection
            .into_candidate()
            .node(primary.index())
            .expect("returned missing-action candidate")
            .action()
            .is_none()
    );

    let extra_action = scene_action_definition::new(
        action_identity::try_new(99, identity.generation()).expect("extra action identity"),
        scene_action_kind::activate,
    );
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[masthead_slot.index()] = scene_node_definition::new(
        masthead_slot,
        oracle_node_identity(identity, masthead_slot),
        scene_role::region,
        Some(oracle_node_identity(identity, scene_slot::root)),
        oracle_rect(masthead_slot.index()),
        public_label("extra noncontrol action"),
        Some(extra_action),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "extra noncontrol action must reject",
    );
    assert!(rejection.error() == scene_error::wrong_action);
    assert!(
        rejection
            .into_candidate()
            .node(masthead_slot.index())
            .expect("returned extra-action candidate")
            .action()
            == Some(extra_action)
    );

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    nodes[primary.index()] = scene_node_definition::new(
        primary,
        oracle_node_identity(identity, scene_slot::navigation),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(primary.index()),
        public_label("duplicate node"),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, primary),
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "duplicate node identity must reject",
    );
    assert!(rejection.error() == scene_error::duplicate_node_identity);

    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    let secondary = scene_slot::secondary_action;
    nodes[secondary.index()] = scene_node_definition::new(
        secondary,
        oracle_node_identity(identity, secondary),
        scene_role::control,
        Some(oracle_node_identity(identity, scene_slot::navigation)),
        oracle_rect(secondary.index()),
        public_label("duplicate action"),
        Some(scene_action_definition::new(
            oracle_action_identity(identity, scene_slot::primary_action),
            scene_action_kind::activate,
        )),
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "duplicate action identity must reject",
    );
    assert!(rejection.error() == scene_error::duplicate_action_identity);

    let protected_state = declared_scene_state::new(
        true,
        true,
        scene_focus::primary,
        scene_disclosure::protected,
        scene_depth::standard,
        scene_motion::standard,
    );
    let rejection = must_succeed(
        desktop_scene::new(candidate(identity, protected_state, "returned secret")),
        "valid protected status",
    );
    retire(rejection.0, rejection.1);

    let mismatched_state = declared_scene_state::new(
        true,
        true,
        scene_focus::primary,
        scene_disclosure::protected,
        scene_depth::standard,
        scene_motion::standard,
    );
    let nodes = canonical_nodes(identity, scene_disclosure::declared_public, "owned status");
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, mismatched_state)),
        "status disclosure mismatch must reject",
    );
    assert!(rejection.error() == scene_error::wrong_disclosure);
    assert!(
        rejection
            .into_candidate()
            .node(scene_slot::status.index())
            .expect("returned status owner")
            .label()
            .expose_declared_public()
            == Some("owned status")
    );

    let mut cleared = declared_public_text::try_from_str("cleared before move")
        .expect("declared-public text to clear");
    cleared.clear();
    let mut nodes = canonical_nodes(identity, state.status_disclosure(), oracle_labels[7]);
    let heading_slot = scene_slot::heading;
    nodes[heading_slot.index()] = scene_node_definition::new(
        heading_slot,
        oracle_node_identity(identity, heading_slot),
        scene_role::heading,
        Some(oracle_node_identity(identity, scene_slot::content)),
        oracle_rect(heading_slot.index()),
        scene_label::declared_public(cleared),
        None,
    );
    let rejection = must_reject(
        desktop_scene::new(scene_candidate::new(identity, nodes, state)),
        "cleared label must reject",
    );
    assert!(rejection.error() == scene_error::empty_label);
    assert!(
        rejection
            .into_candidate()
            .node(heading_slot.index())
            .expect("returned cleared label owner")
            .label()
            .storage_is_clear()
    );
}

#[test]
fn identities_rectangles_errors_and_observations_are_bounded_and_content_free() {
    assert!(matches!(
        scene_identity::try_new(0, 1),
        Err(identity_error::zero_slot)
    ));
    assert!(matches!(
        scene_identity::try_new(1, 0),
        Err(identity_error::zero_generation)
    ));
    assert!(matches!(
        node_identity::try_new(0, 1),
        Err(identity_error::zero_slot)
    ));
    assert!(matches!(
        node_identity::try_new(1, 0),
        Err(identity_error::zero_generation)
    ));
    assert!(matches!(
        action_identity::try_new(0, 1),
        Err(identity_error::zero_slot)
    ));
    assert!(matches!(
        action_identity::try_new(1, 0),
        Err(identity_error::zero_generation)
    ));
    assert!(matches!(
        scene_revision::try_new(0),
        Err(identity_error::zero_revision)
    ));
    assert!(matches!(
        scene_identity::try_new(1, u64::MAX)
            .expect("maximum generation")
            .next_generation(),
        Err(identity_error::generation_exhausted)
    ));
    assert!(matches!(
        node_identity::try_new(1, u64::MAX)
            .expect("maximum node generation")
            .next_generation(),
        Err(identity_error::generation_exhausted)
    ));
    assert!(matches!(
        action_identity::try_new(1, u64::MAX)
            .expect("maximum action generation")
            .next_generation(),
        Err(identity_error::generation_exhausted)
    ));
    assert!(matches!(
        scene_revision::try_new(u64::MAX)
            .expect("maximum revision")
            .next(),
        Err(identity_error::revision_exhausted)
    ));
    assert!(matches!(
        scene_rect::try_new(0, 0, 0, 1),
        Err(scene_error::invalid_geometry)
    ));
    assert!(matches!(
        scene_rect::try_new(u32::MAX, 0, 2, 1),
        Err(scene_error::geometry_overflow)
    ));

    let private_sentinel = "never-format-private-scene-content";
    for error in [
        identity_error::zero_slot,
        identity_error::zero_generation,
        identity_error::generation_exhausted,
        identity_error::zero_revision,
        identity_error::revision_exhausted,
    ] {
        let display = format!("{error}");
        let debug = format!("{error:?}");
        assert!(display == error.code());
        assert!(debug == error.code());
        assert!(!display.contains(private_sentinel));
    }
    let errors = [
        scene_error::scene_instance_exhausted,
        scene_error::control_denied,
        scene_error::scene_not_live,
        scene_error::wrong_scene_identity,
        scene_error::stale_revision,
        scene_error::revision_exhausted,
        scene_error::invalid_geometry,
        scene_error::geometry_overflow,
        scene_error::wrong_slot,
        scene_error::duplicate_node_identity,
        scene_error::wrong_node_identity,
        scene_error::duplicate_action_identity,
        scene_error::wrong_action_identity,
        scene_error::wrong_identity_generation,
        scene_error::wrong_role,
        scene_error::wrong_parent,
        scene_error::wrong_geometry,
        scene_error::wrong_action,
        scene_error::wrong_disclosure,
        scene_error::empty_label,
        scene_error::protected_label_not_allowed,
        scene_error::focus_disabled,
        scene_error::projection_mismatch,
        scene_error::invalid_action_reference,
        scene_error::invalid_action_handle,
        scene_error::disabled_action,
        scene_error::invalid_focus_handle,
        scene_error::no_enabled_focus,
        scene_error::invariant_violation,
    ];
    for error in errors {
        let display = format!("{error}");
        let debug = format!("{error:?}");
        assert!(display == error.code());
        assert!(debug == error.code());
        assert!(!display.contains(private_sentinel));
    }

    for event in [
        scene_observation_event::construction,
        scene_observation_event::preparation,
        scene_observation_event::commit,
        scene_observation_event::action_resolution,
        scene_observation_event::focus_traversal,
        scene_observation_event::retirement,
    ] {
        for result in [
            scene_observation_result::accepted,
            scene_observation_result::rejected,
            scene_observation_result::resolved,
            scene_observation_result::retired,
        ] {
            let observation = scene_privacy_observation::new(event, result);
            assert!(observation.event() == event);
            assert!(observation.result() == result);
            assert!(observation.content_redacted());
            assert!(!observation.history_persisted());
            let formatted = format!("{observation}");
            assert!(!formatted.contains(private_sentinel));
            for forbidden_field in [
                "label",
                "disclosure",
                "identity",
                "revision",
                "count",
                "geometry",
                "length",
                "excerpt",
                "digest",
                "timestamp",
            ] {
                assert!(!formatted.contains(forbidden_field));
            }
        }
    }
}

#[test]
fn palette_contrast_and_decorative_nonstate_roles_match_an_independent_formula() {
    let palette = [
        (paint_color_token::canvas, (0xe7, 0xec, 0xf3)),
        (paint_color_token::surface, (0xf5, 0xf7, 0xfb)),
        (paint_color_token::primary_fill, (0x1e, 0x40, 0xaf)),
        (paint_color_token::secondary_fill, (0xe2, 0xe8, 0xf0)),
        (paint_color_token::disabled, (0xcb, 0xd5, 0xe1)),
        (paint_color_token::primary_text, (0x17, 0x20, 0x33)),
        (paint_color_token::secondary_text, (0x40, 0x51, 0x68)),
        (paint_color_token::on_accent, (0xff, 0xff, 0xff)),
        (paint_color_token::outline, (0x52, 0x65, 0x7a)),
        (paint_color_token::focus_outer, (0x11, 0x18, 0x27)),
        (paint_color_token::focus_inner, (0xff, 0xff, 0xff)),
        (paint_color_token::public_status, (0x14, 0x53, 0x2d)),
        (paint_color_token::protected_status, (0x6b, 0x21, 0xa8)),
        (paint_color_token::shadow, (0xa9, 0xb4, 0xc4)),
        (paint_color_token::highlight, (0xff, 0xff, 0xff)),
    ];
    for (token, expected) in palette {
        let observed = token.rgb();
        assert!((observed.red(), observed.green(), observed.blue()) == expected);
    }

    for (foreground, background) in [
        (paint_color_token::primary_text, paint_color_token::surface),
        (
            paint_color_token::primary_text,
            paint_color_token::secondary_fill,
        ),
        (paint_color_token::primary_text, paint_color_token::disabled),
        (
            paint_color_token::secondary_text,
            paint_color_token::disabled,
        ),
        (
            paint_color_token::on_accent,
            paint_color_token::primary_fill,
        ),
        (paint_color_token::public_status, paint_color_token::surface),
        (
            paint_color_token::protected_status,
            paint_color_token::surface,
        ),
    ] {
        assert!(contrast_ratio(foreground.rgb(), background.rgb()) >= 4.5);
    }
    for (foreground, background) in [
        (
            paint_color_token::on_accent,
            paint_color_token::primary_fill,
        ),
        (
            paint_color_token::public_status,
            paint_color_token::secondary_fill,
        ),
        (paint_color_token::outline, paint_color_token::disabled),
        (paint_color_token::focus_inner, paint_color_token::outline),
        (
            paint_color_token::focus_outer,
            paint_color_token::focus_inner,
        ),
        (paint_color_token::focus_outer, paint_color_token::surface),
    ] {
        assert!(contrast_ratio(foreground.rgb(), background.rgb()) >= 3.0);
    }

    for (fill, parent) in [
        (paint_color_token::surface, paint_color_token::canvas),
        (paint_color_token::primary_fill, paint_color_token::surface),
        (
            paint_color_token::secondary_fill,
            paint_color_token::surface,
        ),
        (paint_color_token::disabled, paint_color_token::surface),
    ] {
        let direct = contrast_ratio(fill.rgb(), parent.rgb()) >= 3.0;
        let outlined = contrast_ratio(paint_color_token::outline.rgb(), fill.rgb()) >= 3.0
            && contrast_ratio(paint_color_token::outline.rgb(), parent.rgb()) >= 3.0;
        assert!(direct || outlined);
    }

    let identity = fixture_identity();
    for ordinal in valid_ordinals {
        let state = state_from_ordinal(ordinal);
        let (scene, control) = construct(candidate(identity, state, "palette state"));
        let plan = scene.projections().render_plan();
        for index in 0..plan.command_count() {
            let command = plan.command(index).expect("paint command");
            if matches!(
                command.kind(),
                paint_command_kind::enabled_marker | paint_command_kind::focus_outline
            ) {
                assert!(command.color() != paint_color_token::shadow);
                assert!(command.color() != paint_color_token::highlight);
                assert!(command.secondary_color() != Some(paint_color_token::shadow));
                assert!(command.secondary_color() != Some(paint_color_token::highlight));
            }
            if matches!(
                command.color(),
                paint_color_token::shadow | paint_color_token::highlight
            ) {
                assert!(matches!(
                    command.kind(),
                    paint_command_kind::shadow | paint_command_kind::highlight
                ));
                assert!(matches!(
                    command.slot(),
                    scene_slot::navigation | scene_slot::content
                ));
            }
        }
        retire(scene, control);
    }
}

fn linearized_channel(value: u8) -> f64 {
    let component = f64::from(value) / 255.0;
    if component <= 0.04045 {
        component / 12.92
    } else {
        ((component + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(color: rgb8) -> f64 {
    0.2126 * linearized_channel(color.red())
        + 0.7152 * linearized_channel(color.green())
        + 0.0722 * linearized_channel(color.blue())
}

fn contrast_ratio(first: rgb8, second: rgb8) -> f64 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn text_policy_matches_the_frozen_scalar_oracle_and_explicit_clear_zeroes_storage() {
    assert!(matches!(
        declared_public_text::try_from_bytes(&[]),
        Err(scene_text_error::empty)
    ));
    assert!(matches!(
        protected_text::try_from_bytes(&[]),
        Err(scene_text_error::empty)
    ));
    assert!(matches!(
        declared_public_text::try_from_bytes(&[0xf0, 0x28, 0x8c, 0x28]),
        Err(scene_text_error::invalid_utf8)
    ));
    assert!(matches!(
        protected_text::try_from_bytes(&[0xf0, 0x28, 0x8c, 0x28]),
        Err(scene_text_error::invalid_utf8)
    ));
    let exact = [b'x'; scene_text_limit];
    let exact_text = declared_public_text::try_from_bytes(&exact).expect("exact-limit text");
    let exact_protected =
        protected_text::try_from_bytes(&exact).expect("exact-limit protected text");
    assert!(exact_text.expose_byte_len() == scene_text_limit);
    assert!(exact_protected.expose_byte_len() == scene_text_limit);
    assert!(exact_text.unused_tail_is_clear());
    assert!(exact_protected.unused_tail_is_clear());
    let over = [b'x'; scene_text_limit + 1];
    assert!(matches!(
        declared_public_text::try_from_bytes(&over),
        Err(scene_text_error::limit_exceeded)
    ));
    assert!(matches!(
        protected_text::try_from_bytes(&over),
        Err(scene_text_error::limit_exceeded)
    ));

    for error in [
        scene_text_error::empty,
        scene_text_error::invalid_utf8,
        scene_text_error::limit_exceeded,
        scene_text_error::forbidden_scalar,
    ] {
        assert!(format!("{error}") == error.code());
        assert!(format!("{error:?}") == error.code());
    }

    let mut forbidden = Vec::new();
    forbidden.extend(0x0000..=0x001f);
    forbidden.extend(0x007f..=0x009f);
    forbidden.extend([
        0x00ad, 0x034f, 0x061c, 0x180e, 0x200b, 0x200c, 0x200d, 0x200e, 0x200f, 0x2028, 0x2029,
        0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2060, 0x2061, 0x2062, 0x2063, 0x2064, 0x2065,
        0x2066, 0x2067, 0x2068, 0x2069, 0x206a, 0x206b, 0x206c, 0x206d, 0x206e, 0x206f, 0xfeff,
        0xfff9, 0xfffa, 0xfffb,
    ]);
    forbidden.sort_unstable();
    forbidden.dedup();
    for value in forbidden {
        let scalar = char::from_u32(value).expect("valid forbidden scalar");
        let mut encoded = [0; 4];
        let text = scalar.encode_utf8(&mut encoded);
        assert!(matches!(
            declared_public_text::try_from_str(text),
            Err(scene_text_error::forbidden_scalar)
        ));
        assert!(matches!(
            protected_text::try_from_str(text),
            Err(scene_text_error::forbidden_scalar)
        ));
    }

    for scalar in [
        ' ', '\u{00ac}', '\u{034e}', '\u{2010}', '\u{202f}', '\u{2070}', '\u{fff8}', '\u{fffc}',
    ] {
        let mut encoded = [0; 4];
        let text = scalar.encode_utf8(&mut encoded);
        assert!(declared_public_text::try_from_str(text).is_ok());
        assert!(protected_text::try_from_str(text).is_ok());
    }

    let mut public = declared_public_text::try_from_str("public clear sentinel").expect("text");
    assert!(public.unused_tail_is_clear());
    public.clear();
    assert!(public.storage_is_clear());
    let mut protected =
        protected_text::try_from_str("protected clear sentinel").expect("protected text");
    assert!(protected.unused_tail_is_clear());
    protected.clear();
    assert!(protected.storage_is_clear());
}
