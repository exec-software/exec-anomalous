use anomalous_desktop_scene::{
    accessible_render_plan, action_identity, declared_public_text, declared_scene_state,
    desktop_scene, machine_action_count, machine_node_count, node_identity, paint_color_token,
    paint_command_kind, paint_shape_token, protected_text, scene_action_definition,
    scene_action_kind, scene_candidate, scene_control, scene_depth, scene_disclosure, scene_focus,
    scene_identity, scene_label, scene_motion, scene_node_count, scene_node_definition, scene_rect,
    scene_role, scene_slot, standard_paint_command_count,
};
use std::vec;

use crate::{
    capsule_byte_count, capsule_color, capsule_command_kind, capsule_rect, capsule_shape,
    capsule_slot, fixture_command_count, fixture_state_ordinal, frame_height, frame_width,
    projection_command, render_capsule, render_row, renderer_error, row_byte_count,
};

const fixture_scene_slot: u64 = 41;
const fixture_generation: u64 = 7;
const _: [(); 1024] = [(); frame_width as usize];
const _: [(); 768] = [(); frame_height as usize];
const _: [(); 3072] = [(); row_byte_count];

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

const parent_indices: [Option<usize>; scene_node_count] = [
    None,
    Some(0),
    Some(0),
    Some(2),
    Some(2),
    Some(0),
    Some(5),
    Some(5),
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

fn fixture_identity() -> scene_identity {
    scene_identity::try_new(fixture_scene_slot, fixture_generation).expect("fixture identity")
}

fn node_for(identity: scene_identity, slot: scene_slot) -> node_identity {
    node_identity::try_new(slot.identity_slot(), identity.generation()).expect("node identity")
}

fn action_for(identity: scene_identity, slot: scene_slot) -> action_identity {
    let action_slot = match slot {
        scene_slot::primary_action => 1,
        scene_slot::secondary_action => 2,
        _ => panic!("non-control slot has no action"),
    };
    action_identity::try_new(action_slot, identity.generation()).expect("action identity")
}

fn public_label(text: &str) -> scene_label {
    scene_label::declared_public(
        declared_public_text::try_from_str(text).expect("declared-public label"),
    )
}

fn fixture_candidate(secret: &str) -> scene_candidate {
    let identity = fixture_identity();
    let nodes = core::array::from_fn(|index| {
        let slot = slots[index];
        let parent = parent_indices[index].map(|parent| node_for(identity, slots[parent]));
        let label = if slot == scene_slot::status {
            scene_label::protected(protected_text::try_from_str(secret).expect("protected label"))
        } else {
            public_label(labels[index])
        };
        let action = match slot {
            scene_slot::primary_action | scene_slot::secondary_action => {
                Some(scene_action_definition::new(
                    action_for(identity, slot),
                    scene_action_kind::activate,
                ))
            }
            _ => None,
        };
        scene_node_definition::new(
            slot,
            node_for(identity, slot),
            roles[index],
            parent,
            slot.canonical_bounds(),
            label,
            action,
        )
    });
    let state = declared_scene_state::new(
        true,
        false,
        scene_focus::primary,
        scene_disclosure::protected,
        scene_depth::standard,
        scene_motion::reduced,
    );
    assert!(state.candidate_ordinal() == fixture_state_ordinal);
    scene_candidate::new(identity, nodes, state)
}

fn construct(secret: &str) -> (desktop_scene, scene_control) {
    match desktop_scene::new(fixture_candidate(secret)) {
        Ok(value) => value,
        Err(rejection) => panic!("scene rejected: {}", rejection.error().code()),
    }
}

fn retire(scene: desktop_scene, control: scene_control) {
    if let Err(rejection) = scene.retire(control) {
        panic!("scene retirement failed: {}", rejection.error().code());
    }
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

fn map_rect(rect: scene_rect) -> capsule_rect {
    capsule_rect::try_new(rect.x(), rect.y(), rect.width(), rect.height())
        .expect("scene rectangle is renderable")
}

fn projection_commands<'a>(
    render: &'a accessible_render_plan<'_>,
    label_override: Option<(usize, Option<&'a str>)>,
) -> [projection_command<'a>; fixture_command_count] {
    core::array::from_fn(|index| {
        let command = render.command(index).expect("complete paint plan");
        let mut label = if command.kind() == paint_command_kind::label {
            Some(
                render
                    .label(command.slot().index())
                    .expect("label command node label")
                    .expose_str(),
            )
        } else {
            None
        };
        if let Some((override_index, override_label)) = label_override
            && override_index == index
        {
            label = override_label;
        }
        projection_command::new(
            map_kind(command.kind()),
            map_shape(command.shape()),
            map_color(command.color()),
            command.secondary_color().map(map_color),
            map_slot(command.slot()),
            map_rect(command.bounds()),
            command.state_active(),
            label,
        )
    })
}

fn capsule_from_scene(scene: &desktop_scene) -> render_capsule {
    let projections = scene.projections();
    let render = projections.render_plan();
    let machine = projections.machine_projection();

    assert!(render.command_count() == standard_paint_command_count);
    assert!(render.command_count() == fixture_command_count);
    assert!(render.accessibility_node_count() == scene_node_count);
    assert!(machine.node_count() == machine_node_count);
    assert!(machine.action_count() == machine_action_count);
    assert!(render.command(fixture_command_count).is_none());
    assert!(render.accessibility_node(scene_node_count).is_none());
    assert!(machine.node(machine_node_count).is_none());
    assert!(machine.action(machine_action_count).is_none());

    for index in 0..scene_node_count {
        let accessible = render.accessibility_node(index).expect("accessible node");
        let semantic = machine.node(index).expect("machine node");
        assert!(accessible.slot() == semantic.slot());
        assert!(accessible.node() == semantic.node());
        assert!(accessible.parent() == semantic.parent());
        assert!(accessible.role() == semantic.role());
        assert!(accessible.bounds() == semantic.bounds());
        assert!(accessible.disclosure() == semantic.disclosure());
        assert!(accessible.enabled() == semantic.enabled());
        assert!(accessible.focused() == semantic.focused());
        assert!(accessible.focus_order() == semantic.focus_order());
        assert!(accessible.action() == semantic.action());
        assert!(
            render.label(index).expect("accessible label").expose_str()
                == machine.label(index).expect("machine label").expose_str()
        );
    }
    for index in 0..machine_action_count {
        let action = machine.action(index).expect("machine action");
        let node_index = if index == 0 {
            scene_slot::primary_action.index()
        } else {
            scene_slot::secondary_action.index()
        };
        let node = machine.node(node_index).expect("action node");
        assert!(Some(action.reference()) == node.action());
        assert!(action.enabled() == node.enabled());
        assert!(action.focused() == node.focused());
    }

    let commands = projection_commands(&render, None);
    render_capsule::try_encode(
        map_rect(render.viewport()),
        render.state().candidate_ordinal(),
        &commands,
    )
    .expect("accepted projection encodes")
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn hash_frame(capsule: &render_capsule) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut first = [0x5a_u8; row_byte_count];
    let mut repeated = [0xa5_u8; row_byte_count];
    for row in 0..frame_height {
        render_row(capsule, row, &mut first).expect("render row");
        render_row(capsule, row, &mut repeated).expect("repeat row");
        assert!(first == repeated, "row {row} changed between renders");
        for byte in first {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

#[test]
fn accepted_scene_projection_builds_the_exact_private_capsule() {
    let (scene, control) = construct("private fixture status");
    let capsule = capsule_from_scene(&scene);
    assert!(capsule == render_capsule::fixture());
    assert!(capsule.as_bytes().len() == capsule_byte_count);
    retire(scene, control);
}

#[test]
fn renderer_palette_matches_every_canonical_scene_color() {
    let pairs = [
        (capsule_color::canvas, paint_color_token::canvas),
        (capsule_color::surface, paint_color_token::surface),
        (capsule_color::primary_fill, paint_color_token::primary_fill),
        (
            capsule_color::secondary_fill,
            paint_color_token::secondary_fill,
        ),
        (capsule_color::disabled, paint_color_token::disabled),
        (capsule_color::primary_text, paint_color_token::primary_text),
        (
            capsule_color::secondary_text,
            paint_color_token::secondary_text,
        ),
        (capsule_color::on_accent, paint_color_token::on_accent),
        (capsule_color::outline, paint_color_token::outline),
        (capsule_color::focus_outer, paint_color_token::focus_outer),
        (capsule_color::focus_inner, paint_color_token::focus_inner),
        (
            capsule_color::public_status,
            paint_color_token::public_status,
        ),
        (
            capsule_color::protected_status,
            paint_color_token::protected_status,
        ),
        (capsule_color::shadow, paint_color_token::shadow),
        (capsule_color::highlight, paint_color_token::highlight),
    ];
    for (renderer, scene) in pairs {
        let scene = scene.rgb();
        assert!(renderer.rgb() == [scene.red(), scene.green(), scene.blue()]);
    }
}

#[test]
fn every_frame_byte_is_repeatable_and_matches_the_frozen_oracle() {
    let capsule = render_capsule::fixture();
    let observed = hash_frame(&capsule);
    assert!(observed == 0x6998_951a_a0fa_83e3);
}

#[test]
fn protected_sources_are_capsule_and_frame_noninterfering() {
    let first_secret = "private-alpha-sentinel";
    let second_secret = "a much longer private beta sentinel that changes source length";
    let (first_scene, first_control) = construct(first_secret);
    let (second_scene, second_control) = construct(second_secret);
    let first = capsule_from_scene(&first_scene);
    let second = capsule_from_scene(&second_scene);

    assert!(first.as_bytes() == second.as_bytes());
    assert!(!contains_subslice(
        first.as_bytes(),
        first_secret.as_bytes()
    ));
    assert!(!contains_subslice(
        first.as_bytes(),
        second_secret.as_bytes()
    ));

    let mut first_row = [0_u8; row_byte_count];
    let mut second_row = [0_u8; row_byte_count];
    for row in 0..frame_height {
        render_row(&first, row, &mut first_row).expect("first private row");
        render_row(&second, row, &mut second_row).expect("second private row");
        assert!(first_row == second_row, "private source changed row {row}");
    }

    retire(second_scene, second_control);
    retire(first_scene, first_control);
}

#[test]
fn every_capsule_byte_is_semantically_checked() {
    let canonical = render_capsule::fixture();
    for index in 0..capsule_byte_count {
        let mut mutated = *canonical.as_bytes();
        mutated[index] ^= 1;
        assert!(
            render_capsule::try_from_bytes(mutated).is_err(),
            "mutation at byte {index} was accepted"
        );
    }
    assert!(
        render_capsule::try_from_slice(&canonical.as_bytes()[..capsule_byte_count - 1])
            == Err(renderer_error::wrong_capsule_length)
    );
}

#[test]
fn labels_must_be_exact_and_use_only_the_bounded_glyph_set() {
    let (scene, control) = construct("private status");
    let projections = scene.projections();
    let render = projections.render_plan();
    let unsupported = projection_commands(&render, Some((24, Some("Protected status?"))));
    assert!(
        render_capsule::try_encode(
            capsule_rect::try_new(0, 0, frame_width, frame_height).expect("viewport"),
            fixture_state_ordinal,
            &unsupported,
        ) == Err(renderer_error::unsupported_glyph)
    );

    let wrong = projection_commands(&render, Some((24, Some("Protected state"))));
    assert!(
        render_capsule::try_encode(
            capsule_rect::try_new(0, 0, frame_width, frame_height).expect("viewport"),
            fixture_state_ordinal,
            &wrong,
        ) == Err(renderer_error::wrong_label)
    );

    let missing = projection_commands(&render, Some((24, None)));
    assert!(
        render_capsule::try_encode(
            capsule_rect::try_new(0, 0, frame_width, frame_height).expect("viewport"),
            fixture_state_ordinal,
            &missing,
        ) == Err(renderer_error::missing_label)
    );
    retire(scene, control);
}

#[test]
fn geometry_state_command_count_and_rows_are_bounded() {
    assert!(capsule_rect::try_new(0, 0, 0, 1) == Err(renderer_error::invalid_geometry));
    assert!(capsule_rect::try_new(u32::MAX, 0, 1, 1) == Err(renderer_error::geometry_overflow));
    assert!(capsule_rect::try_new(frame_width, 0, 1, 1) == Err(renderer_error::outside_viewport));
    assert!(capsule_rect::try_new(0, frame_height, 1, 1) == Err(renderer_error::outside_viewport));
    assert!(capsule_rect::try_new(0, 0, frame_width, frame_height).is_ok());

    let root = capsule_rect::try_new(0, 0, frame_width, frame_height).expect("root");
    assert!(
        render_capsule::try_encode(root, fixture_state_ordinal, &[])
            == Err(renderer_error::wrong_command_count)
    );
    assert!(
        render_capsule::try_encode(root, fixture_state_ordinal - 1, &[])
            == Err(renderer_error::wrong_state_ordinal)
    );
    let tiny = capsule_rect::try_new(0, 0, 1, 1).expect("tiny rectangle");
    assert!(
        render_capsule::try_encode(tiny, fixture_state_ordinal, &[])
            == Err(renderer_error::wrong_viewport)
    );

    let capsule = render_capsule::fixture();
    let mut short = vec![0_u8; row_byte_count - 1];
    let mut long = vec![0_u8; row_byte_count + 1];
    let mut exact = [0_u8; row_byte_count];
    assert!(render_row(&capsule, 0, &mut short) == Err(renderer_error::wrong_row_buffer_length));
    assert!(render_row(&capsule, 0, &mut long) == Err(renderer_error::wrong_row_buffer_length));
    assert!(render_row(&capsule, frame_height, &mut exact) == Err(renderer_error::wrong_row));
}

#[cfg(feature = "scene_conformance")]
#[test]
fn optional_scene_conformance_builder_is_protected_source_noninterfering() {
    let first = crate::scene_conformance::build_fixture_capsule("feature private alpha")
        .expect("first conformance capsule");
    let second = crate::scene_conformance::build_fixture_capsule(
        "feature private beta source with another length",
    )
    .expect("second conformance capsule");
    assert!(first == render_capsule::fixture());
    assert!(first == second);
    assert!(
        crate::scene_conformance::build_fixture_capsule("")
            == Err(crate::scene_conformance::scene_conformance_error::protected_source_rejected)
    );
}
