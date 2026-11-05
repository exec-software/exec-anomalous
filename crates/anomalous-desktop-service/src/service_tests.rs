use std::format;

use anomalous_desktop_scene::{
    accessibility_node, action_identity, declared_public_text, declared_scene_state,
    focus_direction, machine_action_descriptor, machine_node, node_identity, paint_command,
    protected_text, scene_action_definition, scene_action_kind, scene_candidate, scene_depth,
    scene_disclosure, scene_error, scene_focus, scene_identity, scene_label, scene_motion,
    scene_node_count, scene_node_definition, scene_rect, scene_resolution_source, scene_revision,
    scene_role, scene_slot,
};

use crate::identity::{
    delivery_attempt, identity_error, publication_sequence, renderer_identity, surface_identity,
};
use crate::service::{desktop_surface, publication_phase, surface_error};
use crate::snapshot::{
    maximum_snapshot_bytes, render_command, render_label_selector, render_rect,
    render_snapshot_view, snapshot_size,
};

const scene_slot_value: u64 = 41;
const scene_generation: u64 = 7;
const surface_slot_value: u64 = 51;
const surface_generation: u64 = 3;
const renderer_slot_value: u64 = 61;
const renderer_generation: u64 = 1;

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

const parents: [Option<usize>; scene_node_count] = [
    None,
    Some(0),
    Some(0),
    Some(2),
    Some(2),
    Some(0),
    Some(5),
    Some(5),
];

const bounds: [(u32, u32, u32, u32); scene_node_count] = [
    (0, 0, 1024, 768),
    (32, 24, 960, 88),
    (32, 136, 256, 600),
    (56, 184, 208, 64),
    (56, 272, 208, 64),
    (312, 136, 680, 600),
    (352, 176, 600, 48),
    (352, 248, 600, 96),
];

const labels: [&str; scene_node_count] = [
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

trait privacy_result<T, E> {
    fn privacy_expect(self, message: &str) -> T;
    fn privacy_expect_err(self, message: &str) -> E;
}

impl<T, E> privacy_result<T, E> for Result<T, E> {
    fn privacy_expect(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(_) => panic!("{message}"),
        }
    }

    fn privacy_expect_err(self, message: &str) -> E {
        match self {
            Ok(_) => panic!("{message}"),
            Err(error) => error,
        }
    }
}

trait privacy_option<T> {
    fn privacy_expect(self, message: &str) -> T;
}

impl<T> privacy_option<T> for Option<T> {
    fn privacy_expect(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

fn scene_id() -> scene_identity {
    scene_identity::try_new(scene_slot_value, scene_generation).privacy_expect("scene identity")
}

fn surface_id() -> surface_identity {
    surface_identity::try_new(surface_slot_value, surface_generation)
        .privacy_expect("surface identity")
}

fn renderer_id() -> renderer_identity {
    renderer_identity::try_new(renderer_slot_value, renderer_generation)
        .privacy_expect("renderer identity")
}

fn rect(index: usize) -> scene_rect {
    let (x, y, width, height) = bounds[index];
    scene_rect::try_new(x, y, width, height).privacy_expect("canonical rectangle")
}

fn node_id(identity: scene_identity, slot: scene_slot) -> node_identity {
    node_identity::try_new(slot.identity_slot(), identity.generation())
        .privacy_expect("node identity")
}

fn action_id(identity: scene_identity, slot: scene_slot) -> action_identity {
    let value = match slot {
        scene_slot::primary_action => 1,
        scene_slot::secondary_action => 2,
        _ => panic!("non-action slot"),
    };
    action_identity::try_new(value, identity.generation()).privacy_expect("action identity")
}

fn public_label(text: &str) -> scene_label {
    scene_label::declared_public(
        declared_public_text::try_from_str(text).privacy_expect("public label"),
    )
}

fn status_label(disclosure: scene_disclosure, text: &str) -> scene_label {
    match disclosure {
        scene_disclosure::declared_public => public_label(text),
        scene_disclosure::protected => scene_label::protected(
            protected_text::try_from_str(text).privacy_expect("protected label"),
        ),
    }
}

fn canonical_nodes(
    identity: scene_identity,
    state: declared_scene_state,
    status: &str,
    root_label: &str,
) -> [scene_node_definition; scene_node_count] {
    core::array::from_fn(|index| {
        let slot = slots[index];
        let parent = parents[index].map(|parent| node_id(identity, slots[parent]));
        let action = match slot {
            scene_slot::primary_action | scene_slot::secondary_action => {
                Some(scene_action_definition::new(
                    action_id(identity, slot),
                    scene_action_kind::activate,
                ))
            }
            _ => None,
        };
        let label = if slot == scene_slot::root {
            public_label(root_label)
        } else if slot == scene_slot::status {
            status_label(state.status_disclosure(), status)
        } else {
            public_label(labels[index])
        };
        scene_node_definition::new(
            slot,
            node_id(identity, slot),
            roles[index],
            parent,
            rect(index),
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
        canonical_nodes(identity, state, status, labels[0]),
        state,
    )
}

fn candidate_with_root(
    identity: scene_identity,
    state: declared_scene_state,
    status: &str,
    root: &str,
) -> scene_candidate {
    scene_candidate::new(
        identity,
        canonical_nodes(identity, state, status, root),
        state,
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

fn status_for(state: declared_scene_state) -> &'static str {
    match state.status_disclosure() {
        scene_disclosure::declared_public => labels[7],
        scene_disclosure::protected => "private fixture value",
    }
}

struct fixture {
    surface: desktop_surface,
    control: crate::surface_control,
    presentation: crate::presentation_control,
    endpoint: crate::renderer_endpoint,
}

fn construct(state: declared_scene_state, status: &str) -> fixture {
    let (surface, control, presentation, endpoint) = desktop_surface::new_presented(
        surface_id(),
        renderer_id(),
        candidate(scene_id(), state, status),
    )
    .privacy_expect("surface construction");
    fixture {
        surface,
        control,
        presentation,
        endpoint,
    }
}

fn retire(state: fixture) -> crate::surface_retirement {
    state
        .surface
        .retire(state.control)
        .privacy_expect("surface retirement")
}

fn assert_presented_parity(surface: &desktop_surface, state: declared_scene_state) {
    let view = surface.presented();
    assert!(view.state() == state);
    assert!(view.viewport() == rect(0));
    assert!(view.accessibility_node_count() == scene_node_count);
    assert!(view.machine_node_count() == scene_node_count);
    assert!(view.machine_action_count() == 2);
    for index in 0..scene_node_count {
        let accessible = view
            .accessibility_node(index)
            .privacy_expect("accessibility node");
        let machine = view.machine_node(index).privacy_expect("machine node");
        assert!(accessible.slot() == machine.slot());
        assert!(accessible.node() == machine.node());
        assert!(accessible.parent() == machine.parent());
        assert!(accessible.role() == machine.role());
        assert!(accessible.bounds() == machine.bounds());
        assert!(accessible.disclosure() == machine.disclosure());
        assert!(accessible.enabled() == machine.enabled());
        assert!(accessible.focused() == machine.focused());
        assert!(accessible.focus_order() == machine.focus_order());
        assert!(accessible.action() == machine.action());
        let human_label = view
            .accessibility_label(index)
            .privacy_expect("accessibility label");
        let machine_label = view.machine_label(index).privacy_expect("machine label");
        assert!(human_label.disclosure() == machine_label.disclosure());
        assert!(human_label.expose_str() == machine_label.expose_str());
    }
}

#[derive(Eq, PartialEq)]
struct presented_observation {
    scene: scene_identity,
    revision: scene_revision,
    state: declared_scene_state,
    viewport: scene_rect,
    command_count: usize,
    commands: [Option<paint_command>; 25],
    accessibility: [Option<accessibility_node>; scene_node_count],
    machine: [Option<machine_node>; scene_node_count],
    actions: [Option<machine_action_descriptor>; 2],
    labels: [Option<render_label_selector>; scene_node_count],
}

fn fixed_label_selector(
    index: usize,
    disclosure: scene_disclosure,
    text: &str,
) -> render_label_selector {
    match (index, disclosure, text) {
        (0, scene_disclosure::declared_public, "Anomalous desktop") => {
            render_label_selector::anomalous_desktop
        }
        (1, scene_disclosure::declared_public, "Anomalous") => render_label_selector::anomalous,
        (2, scene_disclosure::declared_public, "Navigation") => render_label_selector::navigation,
        (3, scene_disclosure::declared_public, "Open shell") => render_label_selector::open_shell,
        (4, scene_disclosure::declared_public, "Inspect system") => {
            render_label_selector::inspect_system
        }
        (5, scene_disclosure::declared_public, "Workspace") => render_label_selector::workspace,
        (6, scene_disclosure::declared_public, "System workspace") => {
            render_label_selector::system_workspace
        }
        (7, scene_disclosure::declared_public, "System ready") => {
            render_label_selector::system_ready
        }
        (7, scene_disclosure::protected, "Protected status") => {
            render_label_selector::protected_status
        }
        _ => panic!("noncanonical projected label"),
    }
}

fn observe_presented(surface: &desktop_surface) -> presented_observation {
    let view = surface.presented();
    let mut commands = [None; 25];
    for (index, command) in commands.iter_mut().enumerate() {
        *command = view.render_command(index);
    }
    let mut accessibility = [None; scene_node_count];
    let mut machine = [None; scene_node_count];
    let mut observed_labels = [None; scene_node_count];
    for index in 0..scene_node_count {
        accessibility[index] = view.accessibility_node(index);
        machine[index] = view.machine_node(index);
        let label = view
            .machine_label(index)
            .privacy_expect("complete presented label");
        observed_labels[index] = Some(fixed_label_selector(
            index,
            label.disclosure(),
            label.expose_str(),
        ));
    }
    let mut actions = [None; 2];
    for (index, action) in actions.iter_mut().enumerate() {
        *action = view.machine_action(index);
    }
    presented_observation {
        scene: view.identity(),
        revision: view.revision(),
        state: view.state(),
        viewport: view.viewport(),
        command_count: view.render_command_count(),
        commands,
        accessibility,
        machine,
        actions,
        labels: observed_labels,
    }
}

#[derive(Eq, PartialEq)]
struct snapshot_observation {
    surface: surface_identity,
    scene: scene_identity,
    base: scene_revision,
    revision: scene_revision,
    state: declared_scene_state,
    viewport: render_rect,
    command_count: usize,
    commands: [Option<render_command>; 25],
    labels: [Option<render_label_selector>; scene_node_count],
}

fn observe(snapshot: &render_snapshot_view<'_>) -> snapshot_observation {
    let mut commands = [None; 25];
    for (index, command) in commands.iter_mut().enumerate() {
        *command = snapshot.command(index);
    }
    let mut observed_labels = [None; scene_node_count];
    for (index, label) in observed_labels.iter_mut().enumerate() {
        *label = snapshot.label(index);
    }
    snapshot_observation {
        surface: snapshot.surface(),
        scene: snapshot.scene(),
        base: snapshot.base_revision(),
        revision: snapshot.revision(),
        state: snapshot.state(),
        viewport: snapshot.viewport(),
        command_count: snapshot.command_count(),
        commands,
        labels: observed_labels,
    }
}

fn publish_pending(state: &mut fixture) -> crate::publication_receipt {
    let lease = state
        .surface
        .acquire_render(&state.endpoint)
        .privacy_expect("render lease");
    let ready = state
        .surface
        .complete_render(lease)
        .privacy_expect("render completion");
    let receipt = state
        .surface
        .attest_presented(&state.presentation, ready)
        .privacy_expect("presentation receipt");
    state
        .surface
        .publish(&state.control, receipt)
        .privacy_expect("semantic publication")
}

#[test]
fn complete_constructor_matrix_accepts_64_states_and_returns_32_invalid_owners() {
    for ordinal in valid_ordinals {
        let state = state_from_ordinal(ordinal);
        let instance = construct(state, status_for(state));
        assert!(instance.surface.phase() == publication_phase::steady);
        assert!(instance.surface.presented_revision().get() == 1);
        assert!(instance.surface.pending_revision().is_none());
        assert_presented_parity(&instance.surface, state);
        let retirement = retire(instance);
        assert!(retirement.final_revision().get() == 1);
        assert!(!retirement.discarded_pending_revision());
    }

    for ordinal in invalid_ordinals {
        let state = state_from_ordinal(ordinal);
        let rejection = desktop_surface::new_presented(
            surface_id(),
            renderer_id(),
            candidate(scene_id(), state, status_for(state)),
        )
        .privacy_expect_err("invalid focus state must reject");
        assert!(rejection.error() == surface_error::scene_failure(scene_error::focus_disabled));
        assert!(rejection.candidate().state().candidate_ordinal() == ordinal);
        let returned = rejection.into_candidate();
        assert!(returned.state() == state);
    }
}

#[test]
fn invalid_malformed_and_foreign_successors_return_owners_without_mutation() {
    let initial = state_from_ordinal(80);
    let mut instance = construct(initial, labels[7]);
    let baseline = observe_presented(&instance.surface);

    for ordinal in invalid_ordinals {
        let state = state_from_ordinal(ordinal);
        let rejection = instance
            .surface
            .submit_revision(
                &instance.control,
                instance.surface.presented_revision(),
                candidate(scene_id(), state, status_for(state)),
            )
            .privacy_expect_err("invalid successor must reject");
        assert!(rejection.error() == surface_error::scene_failure(scene_error::focus_disabled));
        let returned = rejection.into_candidate();
        assert!(returned.identity() == scene_id());
        assert!(returned.state().candidate_ordinal() == ordinal);
        assert!(instance.surface.phase() == publication_phase::steady);
        assert!(observe_presented(&instance.surface) == baseline);
    }

    let malformed = instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate_with_root(scene_id(), initial, labels[7], "Wrong desktop"),
        )
        .privacy_expect_err("unsupported service profile");
    assert!(malformed.error() == surface_error::unsupported_profile);
    assert!(
        malformed
            .candidate()
            .node(scene_slot::root.index())
            .privacy_expect("returned malformed root")
            .label()
            .expose_declared_public()
            == Some("Wrong desktop")
    );

    let foreign_identity = scene_identity::try_new(scene_slot_value + 1, scene_generation)
        .privacy_expect("foreign scene identity");
    let foreign = instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate(foreign_identity, initial, labels[7]),
        )
        .privacy_expect_err("foreign scene successor");
    assert!(foreign.error() == surface_error::scene_failure(scene_error::wrong_scene_identity));
    assert!(foreign.into_candidate().identity() == foreign_identity);
    assert!(instance.surface.phase() == publication_phase::steady);
    assert!(observe_presented(&instance.surface) == baseline);
    retire(instance);
}

#[test]
fn every_valid_state_transition_publishes_only_at_the_receipt_boundary() {
    for initial_ordinal in valid_ordinals {
        let initial = state_from_ordinal(initial_ordinal);
        for next_ordinal in valid_ordinals {
            let next = state_from_ordinal(next_ordinal);
            let mut instance = construct(initial, status_for(initial));
            let initial_observation = observe_presented(&instance.surface);
            let queued = instance
                .surface
                .submit_revision(
                    &instance.control,
                    instance.surface.presented_revision(),
                    candidate(scene_id(), next, status_for(next)),
                )
                .privacy_expect("valid successor queued");
            assert!(queued.base_revision().get() == 1);
            assert!(queued.revision().get() == 2);
            assert!(instance.surface.phase() == publication_phase::queued);
            assert!(observe_presented(&instance.surface) == initial_observation);
            assert_presented_parity(&instance.surface, initial);

            let lease = instance
                .surface
                .acquire_render(&instance.endpoint)
                .privacy_expect("successor leased");
            assert!(instance.surface.phase() == publication_phase::leased);
            let snapshot = instance
                .surface
                .render_candidate(&lease)
                .privacy_expect("snapshot visible to exact lease");
            assert!(snapshot.state() == next);
            assert!(snapshot.base_revision().get() == 1);
            assert!(snapshot.revision().get() == 2);
            assert!(
                snapshot.command_count()
                    == if next.depth() == scene_depth::standard {
                        25
                    } else {
                        21
                    }
            );
            assert!(snapshot.command(snapshot.command_count()).is_none());
            assert!(snapshot.command(25).is_none());
            assert!(snapshot.label(scene_node_count).is_none());
            assert!(observe_presented(&instance.surface) == initial_observation);
            assert_presented_parity(&instance.surface, initial);

            let ready = instance
                .surface
                .complete_render(lease)
                .privacy_expect("successor ready");
            assert!(instance.surface.phase() == publication_phase::ready);
            assert!(observe_presented(&instance.surface) == initial_observation);
            assert_presented_parity(&instance.surface, initial);
            let receipt = instance
                .surface
                .attest_presented(&instance.presentation, ready)
                .privacy_expect("trusted host receipt");
            assert!(instance.surface.phase() == publication_phase::presented);
            assert!(observe_presented(&instance.surface) == initial_observation);
            assert_presented_parity(&instance.surface, initial);
            let publication = instance
                .surface
                .publish(&instance.control, receipt)
                .privacy_expect("successor published");
            assert!(publication.revision().get() == 2);
            assert!(publication.sequence().get() == 2);
            assert!(instance.surface.phase() == publication_phase::steady);
            assert!(instance.surface.pending_revision().is_none());
            assert_presented_parity(&instance.surface, next);
            retire(instance);
        }
    }
}

#[test]
fn strict_backpressure_returns_the_complete_candidate_in_every_pending_phase() {
    let initial = state_from_ordinal(80);
    let pending_state = state_from_ordinal(84);
    let competing_state = state_from_ordinal(92);
    let sentinel = "private returned owner sentinel";
    let mut instance = construct(initial, labels[7]);
    instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate(scene_id(), pending_state, "first protected value"),
        )
        .privacy_expect("first pending revision");

    let reject_competing = |surface: &mut desktop_surface,
                            control: &crate::surface_control,
                            competing: scene_candidate|
     -> scene_candidate {
        let rejection = surface
            .submit_revision(control, surface.presented_revision(), competing)
            .privacy_expect_err("full slot must reject");
        assert!(rejection.error() == surface_error::pending_full);
        let returned = rejection.into_candidate();
        assert!(returned.identity() == scene_id());
        assert!(returned.state() == competing_state);
        assert!(
            returned
                .node(scene_slot::status.index())
                .privacy_expect("status node")
                .label()
                .expose_protected()
                == Some(sentinel)
        );
        assert!(surface.presented_revision().get() == 1);
        assert_presented_parity(surface, initial);
        returned
    };

    let mut competing = candidate(scene_id(), competing_state, sentinel);
    assert!(instance.surface.phase() == publication_phase::queued);
    competing = reject_competing(&mut instance.surface, &instance.control, competing);
    let lease = instance
        .surface
        .acquire_render(&instance.endpoint)
        .privacy_expect("pending lease");
    assert!(instance.surface.phase() == publication_phase::leased);
    competing = reject_competing(&mut instance.surface, &instance.control, competing);
    let ready = instance
        .surface
        .complete_render(lease)
        .privacy_expect("pending ready");
    assert!(instance.surface.phase() == publication_phase::ready);
    competing = reject_competing(&mut instance.surface, &instance.control, competing);
    let receipt = instance
        .surface
        .attest_presented(&instance.presentation, ready)
        .privacy_expect("pending receipt");
    assert!(instance.surface.phase() == publication_phase::presented);
    competing = reject_competing(&mut instance.surface, &instance.control, competing);
    instance
        .surface
        .publish(&instance.control, receipt)
        .privacy_expect("first pending publication");

    instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            competing,
        )
        .privacy_expect("returned owner can retry after capacity release");
    let retirement = retire(instance);
    assert!(retirement.discarded_pending_revision());
}

#[test]
fn one_restart_reoffers_identical_snapshot_and_stales_every_old_owner() {
    let initial = state_from_ordinal(80);
    let next = state_from_ordinal(93);
    let mut instance = construct(initial, labels[7]);
    instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate(scene_id(), next, "replacement private value"),
        )
        .privacy_expect("pending revision");
    let old_lease = instance
        .surface
        .acquire_render(&instance.endpoint)
        .privacy_expect("first lease");
    let first_snapshot = observe(
        &instance
            .surface
            .render_candidate(&old_lease)
            .privacy_expect("first snapshot"),
    );
    let repeated_snapshot = observe(
        &instance
            .surface
            .render_candidate(&old_lease)
            .privacy_expect("repeated immutable snapshot"),
    );
    assert!(first_snapshot == repeated_snapshot);
    let old_renderer = instance.endpoint.renderer();
    let restart = instance
        .surface
        .restart_renderer(&instance.control)
        .privacy_expect("one restart");
    assert!(restart.previous_renderer() == old_renderer);
    assert!(restart.renderer().generation() == old_renderer.generation() + 1);
    assert!(instance.surface.phase() == publication_phase::queued);
    assert_presented_parity(&instance.surface, initial);
    assert!(matches!(
        instance.surface.acquire_render(&instance.endpoint),
        Err(surface_error::renderer_denied)
    ));
    assert!(matches!(
        instance.surface.render_candidate(&old_lease),
        Err(surface_error::stale_delivery)
    ));
    let old_lease = instance
        .surface
        .complete_render(old_lease)
        .privacy_expect_err("old lease cannot complete")
        .into_lease();
    assert!(old_lease.attempt().get() == 1);

    let replacement = restart.into_endpoint();
    let replacement_lease = instance
        .surface
        .acquire_render(&replacement)
        .privacy_expect("replacement lease");
    assert!(replacement_lease.attempt().get() == 2);
    let second_snapshot = observe(
        &instance
            .surface
            .render_candidate(&replacement_lease)
            .privacy_expect("replacement snapshot"),
    );
    assert!(first_snapshot == second_snapshot);
    let ready = instance
        .surface
        .complete_render(replacement_lease)
        .privacy_expect("replacement ready");
    assert!(matches!(
        instance.surface.restart_renderer(&instance.control),
        Err(surface_error::restart_limit_reached)
    ));
    assert!(instance.surface.phase() == publication_phase::ready);
    let receipt = instance
        .surface
        .attest_presented(&instance.presentation, ready)
        .privacy_expect("replacement presentation");
    let publication = instance
        .surface
        .publish(&instance.control, receipt)
        .privacy_expect("replacement publication");
    assert!(publication.renderer() == replacement.renderer());
    assert!(publication.attempt().get() == 2);
    assert_presented_parity(&instance.surface, next);
    retire(instance);
}

#[test]
fn stale_ready_cannot_cross_replacement_and_attested_receipt_blocks_restart() {
    let initial = state_from_ordinal(80);
    let next = state_from_ordinal(88);

    let mut stale_ready_instance = construct(initial, labels[7]);
    stale_ready_instance
        .surface
        .submit_revision(
            &stale_ready_instance.control,
            stale_ready_instance.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("stale-ready pending");
    assert!(matches!(
        stale_ready_instance
            .surface
            .restart_renderer(&stale_ready_instance.control),
        Err(surface_error::wrong_phase)
    ));
    let lease = stale_ready_instance
        .surface
        .acquire_render(&stale_ready_instance.endpoint)
        .privacy_expect("stale-ready lease");
    let ready = stale_ready_instance
        .surface
        .complete_render(lease)
        .privacy_expect("stale-ready completion");
    let replacement = stale_ready_instance
        .surface
        .restart_renderer(&stale_ready_instance.control)
        .privacy_expect("stale-ready restart")
        .into_endpoint();
    let ready = stale_ready_instance
        .surface
        .attest_presented(&stale_ready_instance.presentation, ready)
        .privacy_expect_err("old ready owner must be stale")
        .into_ready();
    assert!(ready.attempt().get() == 1);
    let lease = stale_ready_instance
        .surface
        .acquire_render(&replacement)
        .privacy_expect("stale-ready replacement lease");
    let ready = stale_ready_instance
        .surface
        .complete_render(lease)
        .privacy_expect("stale-ready replacement completion");
    let receipt = stale_ready_instance
        .surface
        .attest_presented(&stale_ready_instance.presentation, ready)
        .privacy_expect("stale-ready replacement receipt");
    stale_ready_instance
        .surface
        .publish(&stale_ready_instance.control, receipt)
        .privacy_expect("stale-ready replacement publication");
    retire(stale_ready_instance);

    let mut instance = construct(initial, labels[7]);
    instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("pending");
    let lease = instance
        .surface
        .acquire_render(&instance.endpoint)
        .privacy_expect("lease");
    let ready = instance
        .surface
        .complete_render(lease)
        .privacy_expect("ready");
    let receipt = instance
        .surface
        .attest_presented(&instance.presentation, ready)
        .privacy_expect("receipt");
    assert!(instance.surface.phase() == publication_phase::presented);
    assert!(matches!(
        instance.surface.restart_renderer(&instance.control),
        Err(surface_error::wrong_phase)
    ));
    assert!(receipt.attempt().get() == 1);
    assert_presented_parity(&instance.surface, initial);
    instance
        .surface
        .publish(&instance.control, receipt)
        .privacy_expect("attested publication");
    assert_presented_parity(&instance.surface, next);
    retire(instance);
}

#[test]
fn protected_values_and_lengths_do_not_change_renderer_or_public_content() {
    let initial = state_from_ordinal(80);
    let protected = state_from_ordinal(84);
    let first_secret = "amber";
    let second_secret = "violet sentinel with a deliberately different byte length";
    let mut first = construct(initial, labels[7]);
    let mut second = construct(initial, labels[7]);
    first
        .surface
        .submit_revision(
            &first.control,
            first.surface.presented_revision(),
            candidate(scene_id(), protected, first_secret),
        )
        .privacy_expect("first protected candidate");
    second
        .surface
        .submit_revision(
            &second.control,
            second.surface.presented_revision(),
            candidate(scene_id(), protected, second_secret),
        )
        .privacy_expect("second protected candidate");
    let first_lease = first
        .surface
        .acquire_render(&first.endpoint)
        .privacy_expect("first protected lease");
    let second_lease = second
        .surface
        .acquire_render(&second.endpoint)
        .privacy_expect("second protected lease");
    let first_backpressure = first
        .surface
        .submit_revision(
            &first.control,
            first.surface.presented_revision(),
            candidate(scene_id(), protected, "first competing private value"),
        )
        .privacy_expect_err("first protected backpressure");
    let second_backpressure = second
        .surface
        .submit_revision(
            &second.control,
            second.surface.presented_revision(),
            candidate(
                scene_id(),
                protected,
                "second competing private value of another length",
            ),
        )
        .privacy_expect_err("second protected backpressure");
    assert!(first_backpressure.error() == second_backpressure.error());
    assert!(first_backpressure.error() == surface_error::pending_full);
    drop(first_backpressure.into_candidate());
    drop(second_backpressure.into_candidate());
    let first_snapshot = observe(
        &first
            .surface
            .render_candidate(&first_lease)
            .privacy_expect("first protected snapshot"),
    );
    let second_snapshot = observe(
        &second
            .surface
            .render_candidate(&second_lease)
            .privacy_expect("second protected snapshot"),
    );
    assert!(first_snapshot == second_snapshot);
    assert!(
        first_snapshot.labels[scene_slot::status.index()]
            == Some(render_label_selector::protected_status)
    );
    for selector in first_snapshot.labels.into_iter().flatten() {
        assert!(!selector.text().contains(first_secret));
        assert!(!selector.text().contains(second_secret));
    }

    let first_ready = first
        .surface
        .complete_render(first_lease)
        .privacy_expect("first protected ready");
    let second_ready = second
        .surface
        .complete_render(second_lease)
        .privacy_expect("second protected ready");
    let first_receipt = first
        .surface
        .attest_presented(&first.presentation, first_ready)
        .privacy_expect("first protected receipt");
    let second_receipt = second
        .surface
        .attest_presented(&second.presentation, second_ready)
        .privacy_expect("second protected receipt");
    first
        .surface
        .publish(&first.control, first_receipt)
        .privacy_expect("first protected publication");
    second
        .surface
        .publish(&second.control, second_receipt)
        .privacy_expect("second protected publication");
    assert!(observe_presented(&first.surface) == observe_presented(&second.surface));
    let first_view = first.surface.presented();
    let second_view = second.surface.presented();
    let first_label = first_view
        .machine_label(scene_slot::status.index())
        .privacy_expect("first redaction");
    let second_label = second_view
        .machine_label(scene_slot::status.index())
        .privacy_expect("second redaction");
    assert!(first_label.expose_str() == "Protected status");
    assert!(first_label.expose_str() == second_label.expose_str());

    let steady = construct(initial, labels[7]);
    assert!(first.surface.publication_sequence().get() == 2);
    assert!(steady.surface.publication_sequence().get() == 1);
    assert!(first.surface.presented_revision().get() == 2);
    assert!(steady.surface.presented_revision().get() == 1);
    retire(first);
    retire(second);
    retire(steady);
}

#[test]
fn equal_public_identifiers_do_not_cross_private_service_brands() {
    let state = state_from_ordinal(80);
    let next = state_from_ordinal(88);
    let mut first = construct(state, labels[7]);
    let mut second = construct(state, labels[7]);

    let reference = first
        .surface
        .presented()
        .machine_action(0)
        .privacy_expect("action")
        .reference();
    let action = first
        .surface
        .mint_machine_action(reference)
        .privacy_expect("first action handle");
    let action = second
        .surface
        .resolve_action(action)
        .privacy_expect_err("foreign action handle")
        .into_handle();
    first
        .surface
        .resolve_action(action)
        .privacy_expect("returned action handle remains valid");
    let focus = first.surface.mint_machine_focus();
    let focus = second
        .surface
        .traverse_focus(focus, focus_direction::forward)
        .privacy_expect_err("foreign focus handle")
        .into_handle();
    first
        .surface
        .traverse_focus(focus, focus_direction::forward)
        .privacy_expect("returned focus remains valid");

    let first_candidate = first
        .surface
        .submit_revision(
            &second.control,
            first.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect_err("foreign surface control cannot submit")
        .into_candidate();
    assert!(first.surface.phase() == publication_phase::steady);
    first
        .surface
        .submit_revision(
            &first.control,
            first.surface.presented_revision(),
            first_candidate,
        )
        .privacy_expect("first pending");
    let second_candidate = second
        .surface
        .submit_revision(
            &second.control,
            scene_revision::try_new(2).privacy_expect("stale base"),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect_err("stale base cannot submit");
    assert!(second_candidate.error() == surface_error::scene_failure(scene_error::stale_revision));
    second
        .surface
        .submit_revision(
            &second.control,
            second.surface.presented_revision(),
            second_candidate.into_candidate(),
        )
        .privacy_expect("second pending");
    assert!(matches!(
        second.surface.acquire_render(&first.endpoint),
        Err(surface_error::renderer_denied)
    ));
    let first_lease = first
        .surface
        .acquire_render(&first.endpoint)
        .privacy_expect("first lease");
    let first_ready = first
        .surface
        .complete_render(first_lease)
        .privacy_expect("first ready");
    let second_lease = second
        .surface
        .acquire_render(&second.endpoint)
        .privacy_expect("second lease");
    let second_ready = second
        .surface
        .complete_render(second_lease)
        .privacy_expect("second ready");
    let first_ready = first
        .surface
        .attest_presented(&second.presentation, first_ready)
        .privacy_expect_err("foreign presentation control")
        .into_ready();
    assert!(first.surface.phase() == publication_phase::ready);
    let second_ready = first
        .surface
        .attest_presented(&first.presentation, second_ready)
        .privacy_expect_err("foreign ready owner")
        .into_ready();
    let first_receipt = first
        .surface
        .attest_presented(&first.presentation, first_ready)
        .privacy_expect("first receipt");
    let second_receipt = second
        .surface
        .attest_presented(&second.presentation, second_ready)
        .privacy_expect("second receipt");
    let first_receipt = first
        .surface
        .publish(&second.control, first_receipt)
        .privacy_expect_err("foreign surface control cannot publish")
        .into_receipt();
    assert!(first.surface.phase() == publication_phase::presented);
    let first_receipt = second
        .surface
        .publish(&second.control, first_receipt)
        .privacy_expect_err("foreign presentation receipt")
        .into_receipt();
    first
        .surface
        .publish(&first.control, first_receipt)
        .privacy_expect("returned first receipt");
    second
        .surface
        .publish(&second.control, second_receipt)
        .privacy_expect("second receipt");
    retire(first);
    retire(second);
}

#[test]
fn old_action_and_focus_authority_stays_live_until_publication_then_stales() {
    let initial = state_from_ordinal(80);
    let next = state_from_ordinal(88);
    let mut instance = construct(initial, labels[7]);
    let reference = instance
        .surface
        .presented()
        .machine_action(0)
        .privacy_expect("primary action")
        .reference();
    let accessible_probe = instance
        .surface
        .mint_accessible_action(reference)
        .privacy_expect("accessible action");
    let held_action = instance
        .surface
        .mint_machine_action(reference)
        .privacy_expect("machine action");
    let focus = instance.surface.mint_accessible_focus();
    instance
        .surface
        .submit_revision(
            &instance.control,
            instance.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("next revision");
    let action = instance
        .surface
        .resolve_action(accessible_probe)
        .privacy_expect("old action valid while pending");
    assert!(action.reference().revision().get() == 1);
    assert!(action.source() == scene_resolution_source::accessible_projection);
    assert!(!action.effect_performed());
    let focus = instance
        .surface
        .traverse_focus(focus, focus_direction::forward)
        .privacy_expect("old focus valid while pending");
    assert!(focus.reference().revision().get() == 1);
    assert!(!focus.scene_focus_changed());
    let held_focus = focus.into_handle();

    publish_pending(&mut instance);
    let held_action = instance
        .surface
        .resolve_action(held_action)
        .privacy_expect_err("old action stale after publication")
        .into_handle();
    let action_replay = instance
        .surface
        .resolve_action(held_action)
        .privacy_expect_err("old action remains stale");
    assert!(action_replay.error() == surface_error::scene_failure(scene_error::stale_revision));
    let held_focus = instance
        .surface
        .traverse_focus(held_focus, focus_direction::forward)
        .privacy_expect_err("old focus stale after publication")
        .into_handle();
    let focus_replay = instance
        .surface
        .traverse_focus(held_focus, focus_direction::forward)
        .privacy_expect_err("old focus remains stale");
    assert!(focus_replay.error() == surface_error::scene_failure(scene_error::stale_revision));
    assert!(matches!(
        instance.surface.mint_machine_action(reference),
        Err(surface_error::scene_failure(
            scene_error::invalid_action_reference
        ))
    ));
    let new_reference = instance
        .surface
        .presented()
        .machine_action(0)
        .privacy_expect("new primary action")
        .reference();
    let new_handle = instance
        .surface
        .mint_machine_action(new_reference)
        .privacy_expect("new action handle");
    instance
        .surface
        .resolve_action(new_handle)
        .privacy_expect("new action resolves");
    retire(instance);
}

#[test]
fn exhaustion_and_profile_rejections_are_failure_atomic_and_return_owners() {
    let state = state_from_ordinal(80);
    let next = state_from_ordinal(88);
    let wrong_profile = desktop_surface::new_presented(
        surface_id(),
        renderer_id(),
        candidate_with_root(scene_id(), state, labels[7], "Wrong desktop"),
    )
    .privacy_expect_err("wrong fixed label must reject");
    assert!(wrong_profile.error() == surface_error::unsupported_profile);
    assert!(
        wrong_profile
            .candidate()
            .node(scene_slot::root.index())
            .privacy_expect("root")
            .label()
            .expose_declared_public()
            == Some("Wrong desktop")
    );

    let mut sequence = construct(state, labels[7]);
    sequence.surface.set_publication_sequence_for_test(
        publication_sequence::try_new(u64::MAX).privacy_expect("maximum sequence"),
    );
    let rejection = sequence
        .surface
        .submit_revision(
            &sequence.control,
            sequence.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect_err("publication exhaustion");
    assert!(rejection.error() == surface_error::publication_sequence_exhausted);
    assert!(rejection.into_candidate().state() == next);
    assert!(sequence.surface.phase() == publication_phase::steady);
    retire(sequence);

    let (mut generation, control, presentation, endpoint) = desktop_surface::new_presented(
        surface_id(),
        renderer_identity::try_new(renderer_slot_value, u64::MAX)
            .privacy_expect("maximum renderer"),
        candidate(scene_id(), state, labels[7]),
    )
    .privacy_expect("maximum renderer construction");
    generation
        .submit_revision(
            &control,
            generation.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("maximum renderer pending");
    let lease = generation
        .acquire_render(&endpoint)
        .privacy_expect("maximum renderer lease");
    assert!(matches!(
        generation.restart_renderer(&control),
        Err(surface_error::renderer_generation_exhausted)
    ));
    let ready = generation
        .complete_render(lease)
        .privacy_expect("failed restart leaves lease valid");
    let receipt = generation
        .attest_presented(&presentation, ready)
        .privacy_expect("maximum renderer receipt");
    generation
        .publish(&control, receipt)
        .privacy_expect("maximum renderer publication");
    generation
        .retire(control)
        .privacy_expect("maximum renderer retirement");
}

#[test]
fn snapshot_budget_fixed_selectors_and_error_formats_are_content_free() {
    assert!(snapshot_size() <= maximum_snapshot_bytes);
    assert!(snapshot_size() != 0);
    assert!(render_label_selector::protected_status.text() == "Protected status");
    assert!(render_label_selector::protected_status.disclosure() == scene_disclosure::protected);
    assert!(render_label_selector::system_ready.text() == labels[7]);
    assert!(rect(0).width() == 1_024);

    let sentinel = "must never appear in errors";
    for error in [
        surface_error::surface_instance_exhausted,
        surface_error::control_denied,
        surface_error::renderer_denied,
        surface_error::pending_full,
        surface_error::no_pending_revision,
        surface_error::wrong_phase,
        surface_error::stale_delivery,
        surface_error::attempt_limit_reached,
        surface_error::restart_limit_reached,
        surface_error::renderer_generation_exhausted,
        surface_error::publication_sequence_exhausted,
        surface_error::unsupported_profile,
        surface_error::scene_failure(scene_error::focus_disabled),
        surface_error::invariant_violation,
    ] {
        let debug = format!("{error:?}");
        let display = format!("{error}");
        assert!(debug == error.code());
        assert!(display == error.code());
        assert!(!debug.contains(sentinel));
        assert!(!display.contains(sentinel));
    }
}

#[test]
fn public_identities_are_nonzero_and_never_wrap() {
    assert!(matches!(
        surface_identity::try_new(0, 1),
        Err(identity_error::zero_slot)
    ));
    assert!(matches!(
        renderer_identity::try_new(1, 0),
        Err(identity_error::zero_generation)
    ));
    assert!(matches!(
        publication_sequence::try_new(0),
        Err(identity_error::zero_sequence)
    ));
    assert!(matches!(
        delivery_attempt::try_new(0),
        Err(identity_error::zero_sequence)
    ));
    assert!(matches!(
        surface_identity::try_new(1, u64::MAX)
            .privacy_expect("maximum surface")
            .next_generation(),
        Err(identity_error::generation_exhausted)
    ));
    assert!(matches!(
        renderer_identity::try_new(1, u64::MAX)
            .privacy_expect("maximum renderer")
            .next_generation(),
        Err(identity_error::generation_exhausted)
    ));
    assert!(matches!(
        publication_sequence::try_new(u64::MAX)
            .privacy_expect("maximum publication")
            .next(),
        Err(identity_error::sequence_exhausted)
    ));
    assert!(matches!(
        delivery_attempt::try_new(u64::MAX)
            .privacy_expect("maximum attempt")
            .next(),
        Err(identity_error::sequence_exhausted)
    ));
}

#[test]
fn retirement_requires_exact_surface_control_and_discards_one_pending_revision() {
    let state = state_from_ordinal(80);
    let next = state_from_ordinal(88);
    let mut first = construct(state, labels[7]);
    let second = construct(state, labels[7]);
    first
        .surface
        .submit_revision(
            &first.control,
            first.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("pending before retirement");
    let rejection = first
        .surface
        .retire(second.control)
        .privacy_expect_err("foreign controls cannot retire");
    assert!(rejection.error() == surface_error::control_denied);
    let (surface, second_control) = rejection.into_parts();
    first.surface = surface;
    let second_retirement = second
        .surface
        .retire(second_control)
        .privacy_expect("returned controls retire their owner");
    assert!(!second_retirement.discarded_pending_revision());
    let first_retirement = retire(first);
    assert!(first_retirement.discarded_pending_revision());
    assert!(first_retirement.final_revision().get() == 1);
    assert!(first_retirement.final_sequence().get() == 1);
}

#[test]
fn every_declared_lifecycle_phase_has_bounded_transitions_and_retirement() {
    let state = state_from_ordinal(80);
    let next = state_from_ordinal(88);

    let mut steady = construct(state, labels[7]);
    assert!(matches!(
        steady.surface.acquire_render(&steady.endpoint),
        Err(surface_error::no_pending_revision)
    ));
    assert!(matches!(
        steady.surface.restart_renderer(&steady.control),
        Err(surface_error::no_pending_revision)
    ));
    assert!(!retire(steady).discarded_pending_revision());

    let mut queued = construct(state, labels[7]);
    queued
        .surface
        .submit_revision(
            &queued.control,
            queued.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("queued phase");
    assert!(matches!(
        queued.surface.restart_renderer(&queued.control),
        Err(surface_error::wrong_phase)
    ));
    assert!(retire(queued).discarded_pending_revision());

    let mut leased = construct(state, labels[7]);
    leased
        .surface
        .submit_revision(
            &leased.control,
            leased.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("leased pending");
    let held_lease = leased
        .surface
        .acquire_render(&leased.endpoint)
        .privacy_expect("leased phase");
    assert!(matches!(
        leased.surface.acquire_render(&leased.endpoint),
        Err(surface_error::wrong_phase)
    ));
    let _ = held_lease;
    assert!(leased.surface.phase() == publication_phase::leased);
    assert!(retire(leased).discarded_pending_revision());

    let mut ready = construct(state, labels[7]);
    ready
        .surface
        .submit_revision(
            &ready.control,
            ready.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("ready pending");
    let lease = ready
        .surface
        .acquire_render(&ready.endpoint)
        .privacy_expect("ready lease");
    let held_ready = ready
        .surface
        .complete_render(lease)
        .privacy_expect("ready phase");
    assert!(matches!(
        ready.surface.acquire_render(&ready.endpoint),
        Err(surface_error::wrong_phase)
    ));
    let _ = held_ready;
    assert!(ready.surface.phase() == publication_phase::ready);
    assert!(retire(ready).discarded_pending_revision());

    let mut presented = construct(state, labels[7]);
    presented
        .surface
        .submit_revision(
            &presented.control,
            presented.surface.presented_revision(),
            candidate(scene_id(), next, labels[7]),
        )
        .privacy_expect("presented pending");
    let lease = presented
        .surface
        .acquire_render(&presented.endpoint)
        .privacy_expect("presented lease");
    let ready_owner = presented
        .surface
        .complete_render(lease)
        .privacy_expect("presented ready");
    let held_receipt = presented
        .surface
        .attest_presented(&presented.presentation, ready_owner)
        .privacy_expect("presented phase");
    assert!(presented.surface.phase() == publication_phase::presented);
    assert!(matches!(
        presented.surface.restart_renderer(&presented.control),
        Err(surface_error::wrong_phase)
    ));
    assert!(matches!(
        presented.surface.acquire_render(&presented.endpoint),
        Err(surface_error::wrong_phase)
    ));
    let _ = held_receipt;
    assert!(presented.surface.phase() == publication_phase::presented);
    assert!(retire(presented).discarded_pending_revision());
}
