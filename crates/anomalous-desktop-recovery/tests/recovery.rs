use anomalous_desktop_recovery::{
    presenter_endpoint, recovery_control, recovery_phase, recovery_surface,
};
use anomalous_desktop_renderer::{frame_height, row_byte_count, transport_render_snapshot};
use anomalous_desktop_scene::{
    action_identity, declared_public_text, declared_scene_state, node_identity, protected_text,
    scene_action_definition, scene_action_kind, scene_candidate, scene_depth, scene_disclosure,
    scene_focus, scene_identity, scene_label, scene_motion, scene_node_definition, scene_rect,
    scene_revision, scene_role, scene_slot,
};
use anomalous_desktop_service::{renderer_identity, surface_identity};

fn accepted<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("expected successful operation"),
    }
}

fn rejected<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("expected rejected operation"),
        Err(error) => error,
    }
}

fn state(ordinal: u8) -> declared_scene_state {
    let packed = ordinal / 8;
    declared_scene_state::new(
        (packed / 3) & 2 != 0,
        (packed / 3) & 1 != 0,
        match packed % 3 {
            0 => scene_focus::none,
            1 => scene_focus::primary,
            _ => scene_focus::secondary,
        },
        if (ordinal / 4).is_multiple_of(2) {
            scene_disclosure::declared_public
        } else {
            scene_disclosure::protected
        },
        if (ordinal / 2).is_multiple_of(2) {
            scene_depth::standard
        } else {
            scene_depth::flat
        },
        if ordinal.is_multiple_of(2) {
            scene_motion::standard
        } else {
            scene_motion::reduced
        },
    )
}

fn candidate(ordinal: u8, secret: &str) -> scene_candidate {
    let identity = accepted(scene_identity::try_new(41, 7));
    let slots = [
        scene_slot::root,
        scene_slot::masthead,
        scene_slot::navigation,
        scene_slot::primary_action,
        scene_slot::secondary_action,
        scene_slot::content,
        scene_slot::heading,
        scene_slot::status,
    ];
    let roles = [
        scene_role::desktop,
        scene_role::region,
        scene_role::region,
        scene_role::control,
        scene_role::control,
        scene_role::region,
        scene_role::heading,
        scene_role::status,
    ];
    let parents = [
        None,
        Some(0),
        Some(0),
        Some(2),
        Some(2),
        Some(0),
        Some(5),
        Some(5),
    ];
    let bounds = [
        (0, 0, 1024, 768),
        (32, 24, 960, 88),
        (32, 136, 256, 600),
        (56, 184, 208, 64),
        (56, 272, 208, 64),
        (312, 136, 680, 600),
        (352, 176, 600, 48),
        (352, 248, 600, 96),
    ];
    let labels = [
        "Anomalous desktop",
        "Anomalous",
        "Navigation",
        "Open shell",
        "Inspect system",
        "Workspace",
        "System workspace",
        "System ready",
    ];
    let declared = state(ordinal);
    let node = |index: usize| accepted(node_identity::try_new(slots[index].identity_slot(), 7));
    let nodes = core::array::from_fn(|index| {
        let (x, y, width, height) = bounds[index];
        scene_node_definition::new(
            slots[index],
            node(index),
            roles[index],
            parents[index].map(node),
            accepted(scene_rect::try_new(x, y, width, height)),
            if index == 7 && declared.status_disclosure() == scene_disclosure::protected {
                scene_label::protected(accepted(protected_text::try_from_str(secret)))
            } else {
                scene_label::declared_public(accepted(declared_public_text::try_from_str(
                    labels[index],
                )))
            },
            match index {
                3 | 4 => Some(scene_action_definition::new(
                    accepted(action_identity::try_new((index - 2) as u64, 7)),
                    scene_action_kind::activate,
                )),
                _ => None,
            },
        )
    });
    scene_candidate::new(identity, nodes, declared)
}

fn construct() -> (recovery_surface, recovery_control, presenter_endpoint) {
    accepted(recovery_surface::new_presented(
        accepted(surface_identity::try_new(51, 3)),
        accepted(renderer_identity::try_new(61, 1)),
        candidate(0, ""),
    ))
}

fn revision(value: u64) -> scene_revision {
    accepted(scene_revision::try_new(value))
}

fn prepare(surface: &mut recovery_surface, secret: &str) {
    accepted(surface.submit_revision(revision(1), candidate(61, secret)));
    accepted(surface.acquire_render());
    accepted(surface.complete_render());
}

fn counters(surface: &recovery_surface) -> (u64, u64, bool) {
    let status = surface.status();
    (
        status.committed_revision().get(),
        status.publication_sequence().get(),
        status.copy_confirmed(),
    )
}

fn assert_queries_do_not_progress(surface: &recovery_surface) {
    let before = surface.status();
    let counts = counters(surface);
    for _ in 0..4 {
        assert!(surface.status().phase() == before.phase());
        assert_eq!(counters(surface), counts);
    }
}

#[test]
fn read_gate_precedes_writes_and_stays_closed_through_semantic_publication() {
    let (mut surface, control, endpoint) = construct();
    assert_eq!(accepted(surface.presented()).revision().get(), 1);
    accepted(surface.submit_revision(revision(1), candidate(61, "private candidate")));
    assert_eq!(accepted(surface.presented()).revision().get(), 1);
    accepted(surface.acquire_render());
    assert_eq!(accepted(surface.presented()).revision().get(), 1);
    accepted(surface.complete_render());
    assert_eq!(accepted(surface.presented()).revision().get(), 1);
    let ticket = accepted(surface.begin(&endpoint));
    assert!(surface.presented().is_err());
    assert_eq!(counters(&surface), (1, 1, false));
    assert!(surface.publish_semantics(&endpoint, &ticket).is_err());
    accepted(surface.attest_copied(&endpoint, &ticket));
    assert_eq!(counters(&surface), (1, 1, true));
    assert!(surface.presented().is_err());
    accepted(surface.publish_semantics(&endpoint, &ticket));
    assert_eq!(counters(&surface), (2, 2, true));
    assert!(surface.presented().is_err());
    assert_queries_do_not_progress(&surface);
    accepted(surface.finish(&endpoint, ticket));
    let presented = accepted(surface.presented());
    assert_eq!(presented.revision().get(), 2);
    assert!(presented.state() == state(61));
    assert_eq!(
        presented.accessibility_node_count(),
        presented.machine_node_count()
    );
    accepted(surface.retire(control));
}

#[test]
fn each_uncertain_copy_cut_recovers_forward_without_exposing_mismatched_pixels() {
    // This deliberately tiny fake framebuffer models partial writes and the
    // semantic availability contract. It is not a raster correctness oracle.
    for cut in 0..6 {
        let (mut surface, control, old_endpoint) = construct();
        let mut pixels = [0_u8; 64];
        prepare(&mut surface, "retained private candidate");
        let old_ticket = accepted(surface.begin(&old_endpoint));
        let original_body =
            *accepted(surface.redraw_candidate(&old_endpoint, &old_ticket)).as_bytes();
        assert!(surface.presented().is_err());
        if cut >= 1 {
            pixels[..32].fill(61);
        }
        if cut >= 2 {
            pixels.fill(61);
        }
        if cut >= 3 {
            accepted(surface.attest_copied(&old_endpoint, &old_ticket));
        }
        if cut >= 4 {
            accepted(surface.publish_semantics(&old_endpoint, &old_ticket));
        }
        if cut == 5 {
            // Even a copy once acknowledged and semantically published becomes
            // uncertain when the presenter fails before finalization.
            pixels[7] = 0;
        }
        assert_queries_do_not_progress(&surface);
        assert!(surface.presented().is_err());
        let committed = if cut >= 4 { 2 } else { 1 };
        assert_eq!(counters(&surface).0, committed);
        let endpoint = accepted(surface.replace_presenter(&control));
        assert_eq!(counters(&surface), (committed, committed, false));
        assert!(
            surface
                .redraw_candidate(&old_endpoint, &old_ticket)
                .is_err()
        );
        assert!(surface.attest_copied(&old_endpoint, &old_ticket).is_err());
        assert!(
            surface
                .publish_semantics(&old_endpoint, &old_ticket)
                .is_err()
        );
        let stale = rejected(surface.finish(&old_endpoint, old_ticket)).into_ticket();
        let ticket = accepted(surface.begin(&endpoint));
        assert!(
            *accepted(surface.redraw_candidate(&endpoint, &ticket)).as_bytes() == original_body
        );
        assert!(surface.attest_copied(&endpoint, &stale).is_err());
        assert!(surface.publish_semantics(&endpoint, &ticket).is_err());
        let ticket = rejected(surface.finish(&endpoint, ticket)).into_ticket();
        assert!(surface.presented().is_err());
        assert!(!surface.status().copy_confirmed());
        pixels.fill(61);
        accepted(surface.attest_copied(&endpoint, &ticket));
        if committed == 1 {
            accepted(surface.publish_semantics(&endpoint, &ticket));
        }
        assert!(surface.presented().is_err());
        accepted(surface.finish(&endpoint, ticket));
        let public = accepted(surface.presented());
        assert_eq!(public.revision().get(), 2);
        assert!(public.state() == state(pixels[0]));
        assert!(pixels.iter().all(|pixel| *pixel == 61));
        assert_eq!(counters(&surface).1, 2);
        accepted(surface.retire(control));
    }
}

#[test]
#[allow(clippy::drop_non_drop)] // Explicitly model losing both move-only owners.
fn dropped_ticket_or_endpoint_cannot_publish_or_reopen_observation() {
    for copied in [false, true] {
        let (mut surface, control, endpoint) = construct();
        prepare(&mut surface, "owned while request is lost");
        let ticket = accepted(surface.begin(&endpoint));
        if copied {
            accepted(surface.attest_copied(&endpoint, &ticket));
        }
        let before = counters(&surface);
        drop(ticket);
        drop(endpoint);
        assert_eq!(counters(&surface), before);
        assert_queries_do_not_progress(&surface);
        assert!(surface.presented().is_err());
        let replacement = accepted(surface.replace_presenter(&control));
        let fresh = accepted(surface.begin(&replacement));
        assert!(surface.publish_semantics(&replacement, &fresh).is_err());
        accepted(surface.attest_copied(&replacement, &fresh));
        accepted(surface.publish_semantics(&replacement, &fresh));
        accepted(surface.finish(&replacement, fresh));
        assert_eq!(accepted(surface.presented()).revision().get(), 2);
        accepted(surface.retire(control));
    }
}

#[test]
fn equal_public_identities_cannot_exchange_private_authority() {
    let (mut first, first_control, first_endpoint) = construct();
    let (mut second, second_control, second_endpoint) = construct();
    prepare(&mut first, "first private owner");
    prepare(&mut second, "second private owner");
    let first_ticket = accepted(first.begin(&first_endpoint));
    let second_ticket = accepted(second.begin(&second_endpoint));
    let before = counters(&first);
    assert!(first.begin(&second_endpoint).is_err());
    assert!(
        first
            .redraw_candidate(&second_endpoint, &first_ticket)
            .is_err()
    );
    assert!(
        first
            .redraw_candidate(&first_endpoint, &second_ticket)
            .is_err()
    );
    assert!(
        first
            .attest_copied(&first_endpoint, &second_ticket)
            .is_err()
    );
    assert!(
        first
            .publish_semantics(&second_endpoint, &first_ticket)
            .is_err()
    );
    assert!(first.replace_presenter(&second_control).is_err());
    let second_ticket = rejected(first.finish(&first_endpoint, second_ticket)).into_ticket();
    assert_eq!(counters(&first), before);
    assert!(first.presented().is_err());
    // A foreign rejection returned the exact owner, still valid at its home.
    accepted(second.attest_copied(&second_endpoint, &second_ticket));
    accepted(second.publish_semantics(&second_endpoint, &second_ticket));
    accepted(second.finish(&second_endpoint, second_ticket));
    accepted(first.attest_copied(&first_endpoint, &first_ticket));
    accepted(first.publish_semantics(&first_endpoint, &first_ticket));
    accepted(first.finish(&first_endpoint, first_ticket));
    let rejection = rejected(first.retire(second_control));
    let (first, second_control) = rejection.into_parts();
    assert_eq!(accepted(first.presented()).revision().get(), 2);
    accepted(first.retire(first_control));
    accepted(second.retire(second_control));
}

#[test]
fn replayed_progression_and_second_replacement_are_failure_atomic() {
    let (mut surface, control, endpoint) = construct();
    prepare(&mut surface, "single bounded recovery");
    let stale = accepted(surface.begin(&endpoint));
    let replacement = accepted(surface.replace_presenter(&control));
    let ticket = accepted(surface.begin(&replacement));
    let before = counters(&surface);
    assert!(surface.replace_presenter(&control).is_err());
    assert!(surface.begin(&replacement).is_err());
    assert!(surface.attest_copied(&replacement, &stale).is_err());
    assert_eq!(counters(&surface), before);
    accepted(surface.attest_copied(&replacement, &ticket));
    let before = counters(&surface);
    assert!(surface.attest_copied(&replacement, &ticket).is_err());
    assert_eq!(counters(&surface), before);
    accepted(surface.publish_semantics(&replacement, &ticket));
    let before = counters(&surface);
    assert!(surface.publish_semantics(&replacement, &ticket).is_err());
    assert_eq!(counters(&surface), before);
    accepted(surface.finish(&replacement, ticket));
    assert!(surface.finish(&endpoint, stale).is_err());
    assert_eq!(accepted(surface.presented()).revision().get(), 2);
    accepted(surface.retire(control));
}

#[test]
fn rejected_candidates_keep_their_complete_private_owner_in_every_pending_phase() {
    for phase in 0..6 {
        let (mut surface, control, endpoint) = construct();
        accepted(surface.submit_revision(revision(1), candidate(61, "first retained candidate")));
        if phase >= 1 {
            accepted(surface.acquire_render());
        }
        if phase >= 2 {
            accepted(surface.complete_render());
        }
        let ticket = if phase >= 3 {
            Some(accepted(surface.begin(&endpoint)))
        } else {
            None
        };
        if phase >= 4 {
            accepted(
                surface.attest_copied(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        if phase >= 5 {
            accepted(
                surface.publish_semantics(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        let private = "rejected owner has a distinct longer protected value";
        let before = counters(&surface);
        let error = rejected(surface.submit_revision(revision(before.0), candidate(61, private)));
        let returned = error.into_candidate();
        assert_eq!(
            returned
                .node(7)
                .expect("status node")
                .label()
                .expose_protected(),
            Some(private)
        );
        assert!(returned.state() == state(61));
        assert_eq!(counters(&surface), before);
        accepted(surface.retire(control));
    }
}

#[test]
fn invalid_candidate_rejection_returns_source_and_keeps_initial_view() {
    let (mut surface, control, _) = construct();
    let private = "owned protected invalid candidate";
    // Ordinal 12 focuses the disabled primary action and is invalid.
    let error = rejected(surface.submit_revision(revision(1), candidate(12, private)));
    let returned = error.into_candidate();
    assert_eq!(
        returned
            .node(7)
            .expect("status node")
            .label()
            .expose_protected(),
        Some(private)
    );
    assert!(returned.state() == state(12));
    assert_eq!(accepted(surface.presented()).revision().get(), 1);
    assert_eq!(counters(&surface), (1, 1, false));
    accepted(surface.retire(control));
}

#[test]
fn protected_source_value_and_length_change_no_redraw_bytes_or_results() {
    let secrets = [
        "amber",
        "violet sentinel with a deliberately different protected length",
    ];
    let mut observations = Vec::new();
    for secret in secrets {
        let (mut surface, control, endpoint) = construct();
        prepare(&mut surface, secret);
        let ticket = accepted(surface.begin(&endpoint));
        let body = accepted(surface.redraw_candidate(&endpoint, &ticket));
        let bytes = *body.as_bytes();
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
        );
        let renderer = accepted(transport_render_snapshot::try_new(body));
        let mut frame = vec![0_u8; row_byte_count * frame_height as usize];
        for (row, pixels) in frame.chunks_exact_mut(row_byte_count).enumerate() {
            accepted(renderer.render_row(row as u32, pixels));
        }
        let premature = rejected(surface.publish_semantics(&endpoint, &ticket)).code();
        let waiting = counters(&surface);
        let replacement = accepted(surface.replace_presenter(&control));
        let fresh = accepted(surface.begin(&replacement));
        let redrawn = *accepted(surface.redraw_candidate(&replacement, &fresh)).as_bytes();
        assert!(bytes == redrawn);
        let stale = rejected(surface.attest_copied(&endpoint, &ticket)).code();
        accepted(surface.attest_copied(&replacement, &fresh));
        accepted(surface.publish_semantics(&replacement, &fresh));
        accepted(surface.finish(&replacement, fresh));
        let view = accepted(surface.presented());
        let public_labels = (0..view.machine_node_count())
            .map(|index| {
                let human = view
                    .accessibility_label(index)
                    .expect("accessibility label");
                let machine = view.machine_label(index).expect("machine label");
                assert!(human.disclosure() == machine.disclosure());
                assert_eq!(human.expose_str(), machine.expose_str());
                human.expose_str().to_owned()
            })
            .collect::<Vec<_>>();
        observations.push((bytes, frame, premature, waiting, stale, public_labels));
        accepted(surface.retire(control));
    }
    assert!(observations[0] == observations[1]);
}

#[test]
fn explicit_retirement_remains_available_at_every_recovery_cut() {
    for cut in 0..7 {
        let (mut surface, control, endpoint) = construct();
        if cut >= 1 {
            accepted(
                surface.submit_revision(revision(1), candidate(61, "retired private candidate")),
            );
        }
        if cut >= 2 {
            accepted(surface.acquire_render());
        }
        if cut >= 3 {
            accepted(surface.complete_render());
        }
        let ticket = if cut >= 4 {
            Some(accepted(surface.begin(&endpoint)))
        } else {
            None
        };
        if cut >= 5 {
            accepted(
                surface.attest_copied(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        if cut >= 6 {
            accepted(
                surface.publish_semantics(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        accepted(surface.retire(control));
    }
}

#[test]
fn denied_phase_transitions_leave_status_and_observation_gate_unchanged() {
    let phases = [
        recovery_phase::steady,
        recovery_phase::queued,
        recovery_phase::leased,
        recovery_phase::ready,
        recovery_phase::presenting,
        recovery_phase::copied,
        recovery_phase::published,
    ];
    for (cut, phase) in phases.into_iter().enumerate() {
        let (mut surface, control, endpoint) = construct();
        if cut >= 1 {
            accepted(surface.submit_revision(revision(1), candidate(61, "matrix candidate")));
        }
        if cut >= 2 {
            accepted(surface.acquire_render());
        }
        if cut >= 3 {
            accepted(surface.complete_render());
        }
        let mut ticket = if cut >= 4 {
            Some(accepted(surface.begin(&endpoint)))
        } else {
            None
        };
        if cut >= 5 {
            accepted(
                surface.attest_copied(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        if cut >= 6 {
            accepted(
                surface.publish_semantics(&endpoint, ticket.as_ref().expect("presentation ticket")),
            );
        }
        assert_eq!(surface.status().phase(), phase);
        let before = surface.status();
        let readable = surface.presented().is_ok();
        if phase != recovery_phase::queued {
            assert!(surface.acquire_render().is_err());
        }
        if phase != recovery_phase::leased {
            assert!(surface.render_candidate().is_err());
            assert!(surface.complete_render().is_err());
        }
        if phase != recovery_phase::ready {
            assert!(surface.begin(&endpoint).is_err());
        }
        if cut < 4 {
            assert!(surface.replace_presenter(&control).is_err());
        }
        if matches!(
            phase,
            recovery_phase::presenting | recovery_phase::published
        ) {
            assert!(
                surface
                    .publish_semantics(&endpoint, ticket.as_ref().expect("presentation ticket"))
                    .is_err()
            );
        }
        if matches!(phase, recovery_phase::copied | recovery_phase::published) {
            assert!(
                surface
                    .attest_copied(&endpoint, ticket.as_ref().expect("presentation ticket"))
                    .is_err()
            );
        }
        if matches!(phase, recovery_phase::presenting | recovery_phase::copied) {
            ticket = Some(
                rejected(surface.finish(&endpoint, ticket.take().expect("presentation ticket")))
                    .into_ticket(),
            );
        }
        assert!(surface.status() == before);
        assert_eq!(surface.presented().is_ok(), readable);
        assert_queries_do_not_progress(&surface);
        assert_eq!(ticket.is_some(), cut >= 4);
        accepted(surface.retire(control));
    }
}

#[test]
fn complete_profile_matrix_retains_service_validation_and_snapshot_binding() {
    let mut accepted_states = 0;
    let mut invalid_states = 0;
    for ordinal in 0..96 {
        let (mut surface, control, endpoint) = construct();
        let declared = state(ordinal);
        let disabled_focus = (declared.focus() == scene_focus::primary
            && !declared.primary_enabled())
            || (declared.focus() == scene_focus::secondary && !declared.secondary_enabled());
        let submission =
            surface.submit_revision(revision(1), candidate(ordinal, "profile matrix source"));
        if disabled_focus {
            let returned = rejected(submission).into_candidate();
            assert!(returned.state() == declared);
            assert_eq!(accepted(surface.presented()).revision().get(), 1);
            invalid_states += 1;
        } else {
            accepted(submission);
            accepted(surface.acquire_render());
            let leased = accepted(surface.render_candidate());
            assert_eq!(leased.revision().get(), 2);
            accepted(surface.complete_render());
            let ticket = accepted(surface.begin(&endpoint));
            let snapshot = accepted(surface.redraw_candidate(&endpoint, &ticket));
            assert_eq!(snapshot.binding().base_revision(), 1);
            assert_eq!(snapshot.binding().revision(), 2);
            assert_eq!(snapshot.binding().attempt(), 1);
            assert_eq!(
                snapshot.command_count(),
                if declared.depth() == scene_depth::flat {
                    21
                } else {
                    25
                }
            );
            accepted(surface.attest_copied(&endpoint, &ticket));
            accepted(surface.publish_semantics(&endpoint, &ticket));
            accepted(surface.finish(&endpoint, ticket));
            assert!(accepted(surface.presented()).state() == declared);
            accepted_states += 1;
        }
        accepted(surface.retire(control));
    }
    assert_eq!((accepted_states, invalid_states), (64, 32));
}

#[test]
fn later_transaction_preserves_lifetime_replacement_budget_and_rejects_old_ticket() {
    let (mut surface, control, endpoint) = construct();
    prepare(&mut surface, "first transaction");
    let old = accepted(surface.begin(&endpoint));
    let endpoint = accepted(surface.replace_presenter(&control));
    let first = accepted(surface.begin(&endpoint));
    accepted(surface.attest_copied(&endpoint, &first));
    accepted(surface.publish_semantics(&endpoint, &first));
    accepted(surface.finish(&endpoint, first));
    accepted(surface.submit_revision(revision(2), candidate(61, "second transaction")));
    accepted(surface.acquire_render());
    accepted(surface.complete_render());
    let second = accepted(surface.begin(&endpoint));
    let before = surface.status();
    assert!(surface.attest_copied(&endpoint, &old).is_err());
    assert!(surface.publish_semantics(&endpoint, &old).is_err());
    let old = rejected(surface.finish(&endpoint, old)).into_ticket();
    assert!(surface.replace_presenter(&control).is_err());
    assert!(surface.status() == before);
    let body = accepted(surface.redraw_candidate(&endpoint, &second));
    assert_eq!(body.binding().base_revision(), 2);
    assert_eq!(body.binding().revision(), 3);
    accepted(surface.attest_copied(&endpoint, &second));
    accepted(surface.publish_semantics(&endpoint, &second));
    accepted(surface.finish(&endpoint, second));
    assert_eq!(accepted(surface.presented()).revision().get(), 3);
    assert_eq!(counters(&surface).1, 3);
    drop(old);
    accepted(surface.retire(control));
}
