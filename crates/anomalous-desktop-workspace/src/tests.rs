use super::*;
use std::vec::Vec;

fn arm_control(
    model: &mut desktop,
    control: &desktop_control,
    endpoint: &client_endpoint,
    target: desktop_target,
) {
    let revision = accepted(model.observe(endpoint)).revision();
    let ticket = accepted(model.prepare_batch(
        endpoint,
        revision,
        &[
            desktop_command::set_pointer { x: 100, y: 100 },
            desktop_command::set_pointer_buttons(1),
            desktop_command::arm_pointer(target),
        ],
    ));
    commit(model, control, ticket);
}

#[test]
fn armed_feedback_is_public_but_confers_no_manage_or_customize_authority() {
    let (mut model, control, endpoint) = fixture();
    let id = accepted(model.observe(&endpoint))
        .focused_window()
        .unwrap()
        .id();
    let arranger = accepted(model.bind_client(
        &control,
        client_origin::machine,
        client_grants::observe.union(client_grants::arrange),
    ));
    arm_control(&mut model, &control, &arranger, desktop_target::close(id));
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.pointer_armed(), Some(desktop_target::close(id)));
    assert_eq!(view.pointer_armed(), view.machine().pointer_armed());
    assert_eq!(view.pointer_armed(), view.accessibility().pointer_armed());
    let revision = view.revision();
    rejection(
        model.prepare(&arranger, revision, desktop_command::close(id)),
        desktop_error::grant_denied,
    );
    rejection(
        model.prepare(
            &arranger,
            revision,
            desktop_command::set_theme(theme_mode::dark),
        ),
        desktop_error::grant_denied,
    );
    let observer =
        accepted(model.bind_client(&control, client_origin::human, client_grants::observe));
    rejection(
        model.prepare(
            &observer,
            revision,
            desktop_command::arm_pointer(desktop_target::theme),
        ),
        desktop_error::grant_denied,
    );
    assert!(accepted(model.observe(&endpoint)).window(id).is_some());
    assert_eq!(
        accepted(model.observe(&endpoint)).pointer_armed(),
        Some(desktop_target::close(id))
    );
}

#[test]
fn armed_feedback_requires_explicit_continuation_for_each_published_revision() {
    let (mut model, control, endpoint) = fixture();
    arm_control(&mut model, &control, &endpoint, desktop_target::theme);
    let revision = accepted(model.observe(&endpoint)).revision();
    let ticket = accepted(model.prepare(
        &endpoint,
        revision,
        desktop_command::set_pointer { x: 120, y: 100 },
    ));
    assert_eq!(
        accepted(model.observe(&endpoint)).pointer_armed(),
        Some(desktop_target::theme)
    );
    assert_eq!(accepted(model.candidate(&ticket)).pointer_armed(), None);
    commit(&mut model, &control, ticket);
    assert_eq!(accepted(model.observe(&endpoint)).pointer_armed(), None);

    arm_control(&mut model, &control, &endpoint, desktop_target::theme);
    let revision = accepted(model.observe(&endpoint)).revision();
    let ticket = accepted(model.prepare_batch(
        &endpoint,
        revision,
        &[
            desktop_command::set_pointer { x: 121, y: 100 },
            desktop_command::set_pointer_buttons(1),
            desktop_command::arm_pointer(desktop_target::theme),
        ],
    ));
    commit(&mut model, &control, ticket);
    assert_eq!(
        accepted(model.observe(&endpoint)).pointer_armed(),
        Some(desktop_target::theme)
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::cancel_pointer,
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.pointer_armed(), None);
    assert_eq!(view.pointer_buttons(), 1);
}

#[test]
fn arm_rejects_noncontrols_missing_left_and_button_chords_atomically() {
    let (mut model, control, endpoint) = fixture();
    let id = accepted(model.observe(&endpoint))
        .focused_window()
        .unwrap()
        .id();
    for target in [
        desktop_target::desktop,
        desktop_target::window(id),
        desktop_target::titlebar(id),
        desktop_target::resize(id),
    ] {
        let revision = accepted(model.observe(&endpoint)).revision();
        rejection(
            model.prepare_batch(
                &endpoint,
                revision,
                &[
                    desktop_command::set_pointer { x: 100, y: 100 },
                    desktop_command::set_pointer_buttons(1),
                    desktop_command::arm_pointer(target),
                ],
            ),
            desktop_error::invalid_pointer_target,
        );
    }
    for buttons in [0, 2, 3, 4, 5, 6, 7] {
        let revision = accepted(model.observe(&endpoint)).revision();
        rejection(
            model.prepare_batch(
                &endpoint,
                revision,
                &[
                    desktop_command::set_pointer { x: 100, y: 100 },
                    desktop_command::set_pointer_buttons(buttons),
                    desktop_command::arm_pointer(desktop_target::close(id)),
                ],
            ),
            desktop_error::invalid_pointer_target,
        );
    }
    assert!(!accepted(model.observe(&endpoint)).pointer().visible());
    arm_control(&mut model, &control, &endpoint, desktop_target::close(id));
    for buttons in [0, 3] {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::set_pointer_buttons(buttons),
        );
        assert_eq!(accepted(model.observe(&endpoint)).pointer_armed(), None);
    }
}

#[test]
fn arm_validates_incarnation_visibility_and_control_semantics() {
    let (mut model, control, endpoint) = fixture();
    let id = accepted(model.observe(&endpoint))
        .focused_window()
        .unwrap()
        .id();
    arm_control(&mut model, &control, &endpoint, desktop_target::close(id));
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::minimize(id),
    );
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::arm_pointer(desktop_target::close(id)),
        ),
        desktop_error::window_not_visible,
    );
    // A minimized task still has a legitimate restoration control.
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::arm_pointer(desktop_target::task(id)),
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::restore(id),
    );
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::arm_pointer(desktop_target::appearance_choice(
                id,
                appearance_choice::dark,
            )),
        ),
        desktop_error::not_appearance_window,
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::horizontal),
    );
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::arm_pointer(desktop_target::maximize(id)),
        ),
        desktop_error::window_not_floating,
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::switch_workspace(workspace_id::second),
    );
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::arm_pointer(desktop_target::task(id)),
        ),
        desktop_error::window_not_visible,
    );
    apply(&mut model, &control, &endpoint, desktop_command::close(id));
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::arm_pointer(desktop_target::task(id)),
        ),
        desktop_error::stale_window,
    );
}

#[test]
fn failed_release_preserves_armed_revision_until_explicit_cancellation_is_presented() {
    let (mut model, control, endpoint) = fixture();
    let id = accepted(model.observe(&endpoint))
        .focused_window()
        .unwrap()
        .id();
    arm_control(&mut model, &control, &endpoint, desktop_target::close(id));
    let revision = accepted(model.observe(&endpoint)).revision();
    let transaction = model.next_transaction;
    rejection(
        model.prepare_batch(
            &endpoint,
            revision,
            &[
                desktop_command::set_pointer { x: 120, y: 110 },
                desktop_command::set_pointer_buttons(0),
                desktop_command::close(id),
                desktop_command::set_pointer_buttons(8),
            ],
        ),
        desktop_error::invalid_pointer_buttons,
    );
    assert_eq!(model.next_transaction, transaction);
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.revision(), revision);
    assert!(view.window(id).is_some());
    assert_eq!(view.pointer_buttons(), 1);
    assert_eq!(view.pointer_armed(), Some(desktop_target::close(id)));
    let ticket = accepted(model.prepare_batch(
        &endpoint,
        revision,
        &[
            desktop_command::set_pointer { x: 120, y: 110 },
            desktop_command::set_pointer_buttons(0),
        ],
    ));
    assert_eq!(accepted(model.candidate(&ticket)).pointer_armed(), None);
    commit(&mut model, &control, ticket);
    assert_eq!(accepted(model.observe(&endpoint)).pointer_armed(), None);
    assert!(accepted(model.observe(&endpoint)).window(id).is_some());
}

#[test]
fn scene_effect_after_an_arm_in_the_same_batch_clears_feedback() {
    let (mut model, control, endpoint) = fixture();
    arm_control(&mut model, &control, &endpoint, desktop_target::theme);
    let revision = accepted(model.observe(&endpoint)).revision();
    let ticket = accepted(model.prepare_batch(
        &endpoint,
        revision,
        &[
            desktop_command::arm_pointer(desktop_target::theme),
            desktop_command::set_theme(theme_mode::dark),
        ],
    ));
    assert_eq!(accepted(model.candidate(&ticket)).pointer_armed(), None);
    commit(&mut model, &control, ticket);
    assert_eq!(accepted(model.observe(&endpoint)).theme(), theme_mode::dark);
}

fn accepted<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("operation should succeed"),
    }
}
fn rejection<T>(result: Result<T, desktop_error>, expected: desktop_error) {
    match result {
        Err(error) => assert_eq!(error, expected),
        Ok(_) => panic!("operation should fail"),
    }
}
fn commit(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    accepted(model.begin(control, &ticket));
    accepted(model.attest_presented(control, &ticket));
    accepted(model.publish(control, ticket))
}
fn fixture() -> (desktop, desktop_control, client_endpoint) {
    let (mut model, control, initial) = accepted(desktop::new_demo(1024, 768));
    let endpoint = accepted(model.bind_client(&control, client_origin::human, client_grants::all));
    commit(&mut model, &control, initial);
    (model, control, endpoint)
}
fn apply(
    model: &mut desktop,
    control: &desktop_control,
    endpoint: &client_endpoint,
    command: desktop_command,
) -> publication {
    let revision = accepted(model.observe(endpoint)).revision();
    let ticket = accepted(model.prepare(endpoint, revision, command));
    commit(model, control, ticket)
}
fn windows(model: &desktop, endpoint: &client_endpoint) -> Vec<window_view> {
    accepted(model.observe(endpoint)).all_windows().collect()
}

#[test]
fn bootstrap_requires_actual_trusted_presentation_and_all_projections_share_revision() {
    let (mut model, control, initial) = accepted(desktop::new_demo(1024, 768));
    let endpoint = accepted(model.bind_client(&control, client_origin::human, client_grants::all));
    rejection(model.observe(&endpoint), desktop_error::unavailable);
    rejection(
        model.prepare(&endpoint, 0, desktop_command::set_theme(theme_mode::dark)),
        desktop_error::unavailable,
    );
    let view = accepted(model.candidate(&initial));
    assert_eq!(view.revision(), 1);
    assert_eq!(view.windows().count(), 2);
    assert_eq!(
        view.focused_window().unwrap().content(),
        demo_content::inspector
    );
    let failed = match model.publish(&control, initial) {
        Err(failed) => failed,
        Ok(_) => panic!("unpresented publication"),
    };
    assert_eq!(failed.error(), desktop_error::wrong_phase);
    let publication = commit(&mut model, &control, failed.into_ticket());
    assert_eq!(publication.revision(), 1);
    let view = accepted(model.observe(&endpoint));
    assert_eq!(
        view.windows().collect::<Vec<_>>(),
        view.accessibility().windows().collect::<Vec<_>>()
    );
    assert_eq!(
        view.windows().collect::<Vec<_>>(),
        view.machine().windows().collect::<Vec<_>>()
    );
    assert_eq!(view.revision(), view.machine().revision());
    assert_eq!(
        view.hit_test(420, 300),
        Some(view.focused_window().unwrap().id())
    );
}

#[test]
fn human_and_machine_use_identical_grants_and_commands() {
    let (mut human, human_control, human_endpoint) = fixture();
    let (mut machine, machine_control, _) = fixture();
    let machine_endpoint =
        accepted(machine.bind_client(&machine_control, client_origin::machine, client_grants::all));
    let human_id = windows(&human, &human_endpoint)[0].id();
    let machine_id = windows(&machine, &machine_endpoint)[0].id();
    for (human_command, machine_command) in [
        (
            desktop_command::focus(human_id),
            desktop_command::focus(machine_id),
        ),
        (
            desktop_command::move_window {
                window: human_id,
                x: 32,
                y: 88,
            },
            desktop_command::move_window {
                window: machine_id,
                x: 32,
                y: 88,
            },
        ),
        (
            desktop_command::set_layout(layout_mode::horizontal),
            desktop_command::set_layout(layout_mode::horizontal),
        ),
        (
            desktop_command::set_theme(theme_mode::high_contrast),
            desktop_command::set_theme(theme_mode::high_contrast),
        ),
        (
            desktop_command::move_to_workspace {
                window: human_id,
                workspace: workspace_id::second,
            },
            desktop_command::move_to_workspace {
                window: machine_id,
                workspace: workspace_id::second,
            },
        ),
        (
            desktop_command::switch_workspace(workspace_id::second),
            desktop_command::switch_workspace(workspace_id::second),
        ),
    ] {
        let a = apply(&mut human, &human_control, &human_endpoint, human_command);
        let b = apply(
            &mut machine,
            &machine_control,
            &machine_endpoint,
            machine_command,
        );
        assert_eq!(a.origin(), Some(client_origin::human));
        assert_eq!(b.origin(), Some(client_origin::machine));
        let a = accepted(human.observe(&human_endpoint));
        let b = accepted(machine.observe(&machine_endpoint));
        assert_eq!(a.revision(), b.revision());
        assert_eq!(a.theme(), b.theme());
        assert_eq!(a.layout(), b.layout());
        assert_eq!(a.active_workspace(), b.active_workspace());
        assert_eq!(
            a.all_windows().collect::<Vec<_>>(),
            b.all_windows().collect::<Vec<_>>()
        );
    }
    let observer = accepted(human.bind_client(
        &human_control,
        client_origin::machine,
        client_grants::observe,
    ));
    rejection(
        human.prepare(&observer, 7, desktop_command::close(human_id)),
        desktop_error::grant_denied,
    );
    let arranger = accepted(human.bind_client(
        &human_control,
        client_origin::machine,
        client_grants::arrange,
    ));
    rejection(human.observe(&arranger), desktop_error::grant_denied);
    rejection(
        human.prepare(&arranger, 7, desktop_command::set_theme(theme_mode::dark)),
        desktop_error::grant_denied,
    );
}

#[test]
fn pending_candidate_preserves_old_view_and_uncertain_copy_gates_actions_and_observation() {
    let (mut model, control, endpoint) = fixture();
    let ticket =
        accepted(model.prepare(&endpoint, 1, desktop_command::set_theme(theme_mode::dark)));
    assert_eq!(
        accepted(model.observe(&endpoint)).theme(),
        theme_mode::light
    );
    assert_eq!(accepted(model.candidate(&ticket)).theme(), theme_mode::dark);
    rejection(
        model.prepare(
            &endpoint,
            1,
            desktop_command::set_density(density_mode::compact),
        ),
        desktop_error::backpressure,
    );
    accepted(model.begin(&control, &ticket));
    rejection(model.observe(&endpoint), desktop_error::unavailable);
    rejection(
        model.prepare(
            &endpoint,
            1,
            desktop_command::set_density(density_mode::compact),
        ),
        desktop_error::unavailable,
    );
    let ticket = match model.cancel(&control, ticket) {
        Err(rejected) => {
            assert_eq!(rejected.error(), desktop_error::wrong_phase);
            rejected.into_ticket()
        }
        Ok(_) => panic!("uncertain frame cannot be canceled"),
    };
    accepted(model.attest_presented(&control, &ticket));
    rejection(model.observe(&endpoint), desktop_error::unavailable);
    let result = accepted(model.publish(&control, ticket));
    assert_eq!(result.revision(), 2);
    assert_eq!(accepted(model.observe(&endpoint)).theme(), theme_mode::dark);
    assert_eq!(model.phase(), presentation_phase::steady);
}

#[test]
fn foreign_controls_clients_and_tickets_cannot_progress_equal_public_state() {
    let (mut a, a_control, a_endpoint) = fixture();
    let (mut b, b_control, b_endpoint) = fixture();
    let a_ticket =
        accepted(a.prepare(&a_endpoint, 1, desktop_command::set_theme(theme_mode::dark)));
    let b_ticket =
        accepted(b.prepare(&b_endpoint, 1, desktop_command::set_theme(theme_mode::dark)));
    rejection(a.observe(&b_endpoint), desktop_error::client_denied);
    rejection(a.candidate(&b_ticket), desktop_error::stale_ticket);
    rejection(
        a.begin(&b_control, &a_ticket),
        desktop_error::control_denied,
    );
    let wrong = match a.publish(&a_control, b_ticket) {
        Err(rejected) => rejected,
        Ok(_) => panic!("foreign ticket"),
    };
    assert_eq!(wrong.error(), desktop_error::stale_ticket);
    commit(&mut b, &b_control, wrong.into_ticket());
    commit(&mut a, &a_control, a_ticket);
}

#[test]
fn revoked_sources_and_pending_work_are_rechecked_without_losing_candidate() {
    let (mut model, control, endpoint) = fixture();
    let ticket =
        accepted(model.prepare(&endpoint, 1, desktop_command::set_theme(theme_mode::dark)));
    accepted(model.revoke_client(&control, &endpoint));
    rejection(model.begin(&control, &ticket), desktop_error::client_denied);
    assert_eq!(accepted(model.candidate(&ticket)).theme(), theme_mode::dark);
    accepted(model.cancel(&control, ticket));
    let fresh = accepted(model.bind_client(&control, client_origin::human, client_grants::all));
    rejection(model.observe(&endpoint), desktop_error::client_denied);
    assert_eq!(accepted(model.observe(&fresh)).theme(), theme_mode::light);
    let ticket = accepted(model.prepare(&fresh, 1, desktop_command::set_theme(theme_mode::dark)));
    accepted(model.begin(&control, &ticket));
    accepted(model.revoke_client(&control, &fresh));
    accepted(model.attest_presented(&control, &ticket));
    let failed = match model.publish(&control, ticket) {
        Err(rejected) => rejected,
        Ok(_) => panic!("revoked prepared authority"),
    };
    assert_eq!(failed.error(), desktop_error::client_denied);
    assert_eq!(model.phase(), presentation_phase::copied);
    assert_eq!(
        accepted(model.candidate(&failed.into_ticket())).theme(),
        theme_mode::dark
    );
    // There is deliberately no old-view fallback or service-loss recovery claim.
}

#[test]
fn tiling_order_does_not_follow_focus_and_floating_geometry_survives_layout_changes() {
    let (mut model, control, endpoint) = fixture();
    let original = windows(&model, &endpoint);
    let first = original[0].id();
    let second = original[1].id();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::horizontal),
    );
    let tiles = windows(&model, &endpoint);
    assert!(tiles[0].bounds().x() + tiles[0].bounds().width() <= tiles[1].bounds().x());
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::focus(first),
    );
    let focused = accepted(model.observe(&endpoint));
    assert_eq!(focused.windows().last().unwrap().id(), first);
    assert_eq!(focused.window(first).unwrap().bounds(), tiles[0].bounds());
    assert_eq!(focused.window(second).unwrap().bounds(), tiles[1].bounds());
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::vertical),
    );
    let vertical = accepted(model.observe(&endpoint));
    assert!(
        vertical.window(first).unwrap().bounds().y()
            < vertical.window(second).unwrap().bounds().y()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::floating),
    );
    let restored = accepted(model.observe(&endpoint));
    for window in original {
        assert_eq!(
            restored.window(window.id()).unwrap().bounds(),
            window.bounds()
        );
    }
}

#[test]
fn focus_never_targets_hidden_minimized_or_closed_windows_across_workspaces() {
    let (mut model, control, endpoint) = fixture();
    let all = windows(&model, &endpoint);
    let first = all[0].id();
    let second = all[1].id();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::minimize(second),
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .unwrap()
            .id(),
        first
    );
    rejection(
        model.prepare(&endpoint, 2, desktop_command::focus(second)),
        desktop_error::window_not_visible,
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::move_to_workspace {
            window: first,
            workspace: workspace_id::second,
        },
    );
    assert!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .is_none()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::switch_workspace(workspace_id::second),
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .unwrap()
            .id(),
        first
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::close(first),
    );
    assert!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .is_none()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::restore(second),
    );
    assert!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .is_none()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::switch_workspace(workspace_id::first),
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .unwrap()
            .id(),
        second
    );
}

#[test]
fn stale_window_generations_and_stale_revisions_cannot_mutate_replacements() {
    let (mut model, control, endpoint) = fixture();
    let old = windows(&model, &endpoint)[0];
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::close(old.id()),
    );
    let created = apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds: old.bounds(),
        },
    )
    .created_window()
    .unwrap();
    assert_eq!(created.slot(), old.id().slot());
    assert_eq!(created.generation(), old.id().generation() + 1);
    rejection(
        model.prepare(&endpoint, 3, desktop_command::close(old.id())),
        desktop_error::stale_window,
    );
    rejection(
        model.prepare(&endpoint, 1, desktop_command::close(created)),
        desktop_error::stale_revision,
    );
    assert!(accepted(model.observe(&endpoint)).window(created).is_some());
}

#[test]
fn rejected_geometry_and_capacity_changes_are_failure_atomic() {
    let (mut model, control, endpoint) = fixture();
    let before = windows(&model, &endpoint);
    let id = before[0].id();
    for command in [
        desktop_command::move_window {
            window: id,
            x: u32::MAX,
            y: 72,
        },
        desktop_command::resize_window {
            window: id,
            width: 0,
            height: 100,
        },
        desktop_command::resize_window {
            window: id,
            width: 120,
            height: u32::MAX,
        },
        desktop_command::move_window {
            window: id,
            x: 0,
            y: 0,
        },
    ] {
        rejection(
            model.prepare(&endpoint, 1, command),
            desktop_error::invalid_geometry,
        );
        assert_eq!(windows(&model, &endpoint), before);
        assert_eq!(model.phase(), presentation_phase::steady);
    }
    let bounds = accepted(window_rect::try_new(32, 88, 200, 120));
    for _ in 0..2 {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds,
            },
        );
    }
    rejection(
        model.prepare(
            &endpoint,
            3,
            desktop_command::create {
                content: demo_content::welcome,
                workspace: workspace_id::first,
                bounds,
            },
        ),
        desktop_error::window_capacity_exhausted,
    );
    assert_eq!(windows(&model, &endpoint).len(), 4);
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_density(density_mode::compact),
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::vertical),
    );
    let view = accepted(model.observe(&endpoint));
    for window in view.windows() {
        assert!(window.bounds().within(view.work_area()));
    }
}

#[test]
fn all_monotonic_counters_fail_closed_and_exhausted_slots_are_retired() {
    let (mut model, control, endpoint) = fixture();
    model.next_transaction = u64::MAX;
    rejection(
        model.prepare(&endpoint, 1, desktop_command::set_theme(theme_mode::dark)),
        desktop_error::sequence_exhausted,
    );
    model.next_transaction = 2;
    model.committed.revision = u64::MAX;
    rejection(
        model.prepare(
            &endpoint,
            u64::MAX,
            desktop_command::set_theme(theme_mode::dark),
        ),
        desktop_error::sequence_exhausted,
    );
    model.committed.revision = 1;
    model.committed.next_tile_order = u64::MAX;
    let bounds = accepted(window_rect::try_new(32, 88, 200, 120));
    rejection(
        model.prepare(
            &endpoint,
            1,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds,
            },
        ),
        desktop_error::sequence_exhausted,
    );
    model.committed.next_tile_order = 3;
    let old = model.committed.windows[0].as_mut().unwrap();
    let max_id = window_id {
        slot: 0,
        generation: u64::MAX,
    };
    old.id = max_id;
    model.committed.generations[0] = u64::MAX;
    model.committed.order[0] = Some(max_id);
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::close(max_id),
    );
    assert!(model.committed.exhausted[0]);
    for _ in 0..2 {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds,
            },
        );
    }
    rejection(
        model.prepare(
            &endpoint,
            4,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds,
            },
        ),
        desktop_error::window_generation_exhausted,
    );
    assert!(model.committed.windows[0].is_none());
}

#[test]
fn dropped_ticket_never_publishes_or_reopens_an_uncertain_view() {
    let (mut model, control, endpoint) = fixture();
    let ticket =
        accepted(model.prepare(&endpoint, 1, desktop_command::set_theme(theme_mode::dark)));
    accepted(model.begin(&control, &ticket));
    fn abandon(_: update_ticket) {}
    abandon(ticket);
    rejection(model.observe(&endpoint), desktop_error::unavailable);
    rejection(
        model.prepare(&endpoint, 1, desktop_command::set_theme(theme_mode::light)),
        desktop_error::unavailable,
    );
    assert_eq!(model.committed.appearance.theme, theme_mode::light);
    assert_eq!(model.phase(), presentation_phase::presenting);
}

#[test]
fn client_capacity_revocation_and_generation_exhaustion_preserve_authority() {
    let (mut model, control, first) = fixture();
    let second =
        accepted(model.bind_client(&control, client_origin::machine, client_grants::observe));
    let third = accepted(model.bind_client(&control, client_origin::human, client_grants::all));
    let fourth = accepted(model.bind_client(&control, client_origin::machine, client_grants::all));
    rejection(
        model.bind_client(&control, client_origin::machine, client_grants::all),
        desktop_error::client_capacity_exhausted,
    );
    accepted(model.revoke_client(&control, &second));
    let fresh =
        accepted(model.bind_client(&control, client_origin::machine, client_grants::arrange));
    rejection(model.observe(&second), desktop_error::client_denied);
    let id = accepted(model.observe(&first))
        .focused_window()
        .unwrap()
        .id();
    rejection(
        model.prepare(&fresh, 1, desktop_command::focus(id)),
        desktop_error::grant_denied,
    );
    let max = client_endpoint {
        instance: model.instance,
        slot: fresh.slot,
        generation: u64::MAX,
    };
    model.clients[max.slot].generation = u64::MAX;
    accepted(model.revoke_client(&control, &max));
    assert!(model.clients[max.slot].exhausted);
    rejection(
        model.bind_client(&control, client_origin::machine, client_grants::all),
        desktop_error::client_generation_exhausted,
    );
    accepted(model.observe(&third));
    accepted(model.observe(&fourth));
}

#[test]
fn layout_failures_preserve_state_and_desktop_storage_fits_native_service_budget() {
    let (mut model, control, initial) = accepted(desktop::new_demo(640, 480));
    let endpoint = accepted(model.bind_client(&control, client_origin::human, client_grants::all));
    commit(&mut model, &control, initial);
    let bounds = accepted(window_rect::try_new(32, 88, 200, 120));
    for _ in 0..2 {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds,
            },
        );
    }
    let before = windows(&model, &endpoint);
    rejection(
        model.prepare(
            &endpoint,
            3,
            desktop_command::set_layout(layout_mode::vertical),
        ),
        desktop_error::layout_too_small,
    );
    assert_eq!(windows(&model, &endpoint), before);
    assert_eq!(
        accepted(model.observe(&endpoint)).layout(),
        layout_mode::floating
    );
    assert!(core::mem::size_of::<desktop>() <= 8192);
    std::println!(
        "canonical desktop owner bytes: {}",
        core::mem::size_of::<desktop>()
    );
}

#[test]
fn pointer_and_focus_are_published_as_one_transaction() {
    let (mut model, control, endpoint) = fixture();
    let id = windows(&model, &endpoint)[0].id();
    let before = accepted(model.observe(&endpoint)).pointer();
    assert!(!before.visible());
    assert_eq!(accepted(model.observe(&endpoint)).pointer_buttons(), 0);
    let ticket = accepted(model.prepare_batch(
        &endpoint,
        1,
        &[
            desktop_command::set_pointer {
                x: u32::MAX,
                y: u32::MAX,
            },
            desktop_command::set_pointer_buttons(1),
            desktop_command::focus(id),
        ],
    ));
    assert_eq!(accepted(model.observe(&endpoint)).pointer(), before);
    assert_eq!(accepted(model.observe(&endpoint)).pointer_buttons(), 0);
    let candidate = accepted(model.candidate(&ticket));
    assert_eq!(
        (candidate.pointer().x(), candidate.pointer().y()),
        (1023, 767)
    );
    assert!(candidate.pointer().visible());
    assert_eq!(candidate.pointer_buttons(), 1);
    assert_eq!(candidate.focused_window().unwrap().id(), id);
    accepted(model.begin(&control, &ticket));
    rejection(model.observe(&endpoint), desktop_error::unavailable);
    accepted(model.attest_presented(&control, &ticket));
    let result = accepted(model.publish(&control, ticket));
    assert_eq!(result.revision(), 2);
    assert_eq!(
        accepted(model.observe(&endpoint)).machine().pointer(),
        candidate_pointer(1023, 767)
    );
}
fn candidate_pointer(x: u32, y: u32) -> pointer_position {
    pointer_position {
        x,
        y,
        visible: true,
    }
}

#[test]
fn late_batch_rejection_retains_pointer_focus_revision_and_next_transaction() {
    let (mut model, _, endpoint) = fixture();
    let before_windows = windows(&model, &endpoint);
    let before_pointer = accepted(model.observe(&endpoint)).pointer();
    let transaction = model.next_transaction;
    rejection(
        model.prepare_batch(
            &endpoint,
            1,
            &[
                desktop_command::set_pointer { x: 10, y: 10 },
                desktop_command::focus(before_windows[0].id()),
                desktop_command::resize_window {
                    window: before_windows[0].id(),
                    width: 0,
                    height: 20,
                },
            ],
        ),
        desktop_error::invalid_geometry,
    );
    assert_eq!(windows(&model, &endpoint), before_windows);
    assert_eq!(accepted(model.observe(&endpoint)).pointer(), before_pointer);
    assert_eq!(accepted(model.observe(&endpoint)).revision(), 1);
    assert_eq!(model.next_transaction, transaction);
    assert!(model.pending.is_none());
}

#[test]
fn batch_requires_every_grant_and_has_bounded_unambiguous_results() {
    let (mut model, control, endpoint) = fixture();
    let arranger = accepted(model.bind_client(
        &control,
        client_origin::machine,
        client_grants::observe.union(client_grants::arrange),
    ));
    rejection(
        model.prepare_batch(
            &arranger,
            1,
            &[
                desktop_command::set_pointer { x: 10, y: 10 },
                desktop_command::set_theme(theme_mode::dark),
            ],
        ),
        desktop_error::grant_denied,
    );
    rejection(
        model.prepare_batch(&endpoint, 1, &[]),
        desktop_error::invalid_batch,
    );
    rejection(
        model.prepare_batch(
            &endpoint,
            1,
            &[desktop_command::set_pointer { x: 1, y: 1 }; 5],
        ),
        desktop_error::invalid_batch,
    );
    let create = desktop_command::create {
        content: demo_content::welcome,
        workspace: workspace_id::first,
        bounds: window_rect::try_new(32, 80, 200, 160).unwrap(),
    };
    rejection(
        model.prepare_batch(&endpoint, 1, &[create, create]),
        desktop_error::invalid_batch,
    );
    assert_eq!(accepted(model.observe(&endpoint)).revision(), 1);
    assert!(!accepted(model.observe(&endpoint)).pointer().visible());
}

#[test]
fn maximize_restore_and_tiling_preserve_floating_geometry() {
    let (mut model, control, endpoint) = fixture();
    let original = windows(&model, &endpoint)[0];
    let id = original.id();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::maximize(id),
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.window(id).unwrap().bounds(), view.work_area());
    assert!(view.window(id).unwrap().maximized());
    assert_eq!(
        view.window(id).unwrap().floating_bounds(),
        original.floating_bounds()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::horizontal),
    );
    assert!(
        !accepted(model.observe(&endpoint))
            .window(id)
            .unwrap()
            .maximized()
    );
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(&endpoint, revision, desktop_command::maximize(id)),
        desktop_error::window_not_floating,
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_layout(layout_mode::floating),
    );
    assert!(
        accepted(model.observe(&endpoint))
            .window(id)
            .unwrap()
            .maximized()
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::restore_geometry(id),
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .window(id)
            .unwrap()
            .bounds(),
        original.bounds()
    );
}

#[test]
fn theme_components_validate_atomically_and_share_observation() {
    let (mut model, control, endpoint) = fixture();
    let original = accepted(model.observe(&endpoint)).theme_settings();
    rejection(
        model.prepare(
            &endpoint,
            1,
            desktop_command::set_theme_settings(theme_settings {
                theme: theme_mode::dark,
                cursor_size: 0,
                ..original
            }),
        ),
        desktop_error::invalid_theme,
    );
    assert_eq!(
        accepted(model.observe(&endpoint)).theme_settings(),
        original
    );
    let settings = theme_settings {
        theme: theme_mode::dark,
        font_scale: 2,
        cursor: cursor_style::crosshair,
        cursor_size: 3,
        depth: false,
        ..original
    };
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_theme_settings(settings),
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.theme_settings(), settings);
    assert_eq!(
        view.machine().theme_definition(),
        view.accessibility().theme_definition()
    );
    assert_eq!(view.theme_definition().name, "Anomalous Dark");
    assert!(!view.depth());
    assert_eq!(view.theme_definition().animation_duration_ms, 0);
}

#[test]
fn appearance_geometry_that_would_strand_a_window_is_rejected() {
    let (mut model, control, endpoint) = fixture();
    let id = windows(&model, &endpoint)[0].id();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::move_window {
            window: id,
            x: 16,
            y: 64,
        },
    );
    let view = accepted(model.observe(&endpoint));
    let settings = view.theme_settings();
    let metrics = view.theme_definition().metrics;
    let revision = view.revision();
    let overridden = theme_settings {
        overrides: anomalous_desktop_theme::theme_overrides {
            metrics: Some(anomalous_desktop_theme::chrome_metrics {
                workarea_top: 80,
                ..metrics
            }),
            colors: None,
            ..Default::default()
        },
        ..settings
    };
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::set_theme_settings(overridden),
        ),
        desktop_error::invalid_geometry,
    );
    assert_eq!(
        accepted(model.observe(&endpoint)).theme_settings(),
        settings
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .window(id)
            .unwrap()
            .bounds()
            .y(),
        64
    );
}

#[test]
fn pointer_buttons_require_arrange_and_match_all_public_projections() {
    let (mut model, control, endpoint) = fixture();
    let observer =
        accepted(model.bind_client(&control, client_origin::machine, client_grants::observe));
    let customizer = accepted(model.bind_client(
        &control,
        client_origin::human,
        client_grants::observe.union(client_grants::customize),
    ));
    let arranger = accepted(model.bind_client(
        &control,
        client_origin::machine,
        client_grants::observe.union(client_grants::arrange),
    ));
    for denied in [&observer, &customizer] {
        rejection(
            model.prepare(denied, 1, desktop_command::set_pointer_buttons(1)),
            desktop_error::grant_denied,
        );
    }
    for buttons in 0..=7 {
        let publication = apply(
            &mut model,
            &control,
            &arranger,
            desktop_command::set_pointer_buttons(buttons),
        );
        assert_eq!(publication.origin(), Some(client_origin::machine));
        let human = accepted(model.observe(&endpoint));
        let machine = accepted(model.observe(&arranger));
        assert_eq!(human.pointer_buttons(), buttons);
        assert_eq!(human.pointer_buttons(), machine.pointer_buttons());
        assert_eq!(human.pointer_buttons(), human.machine().pointer_buttons());
        assert_eq!(
            human.pointer_buttons(),
            human.accessibility().pointer_buttons()
        );
        assert_eq!(human.revision(), machine.revision());
    }
}

#[test]
fn invalid_buttons_reject_the_entire_batch_without_consuming_a_transaction() {
    let (mut model, control, endpoint) = fixture();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_pointer_buttons(5),
    );
    let before_windows = windows(&model, &endpoint);
    let before = accepted(model.observe(&endpoint));
    let pointer = before.pointer();
    let revision = before.revision();
    let transaction = model.next_transaction;
    for invalid in 8..=u8::MAX {
        rejection(
            model.prepare_batch(
                &endpoint,
                revision,
                &[
                    desktop_command::set_pointer { x: 10, y: 10 },
                    desktop_command::set_pointer_buttons(1),
                    desktop_command::focus(before_windows[0].id()),
                    desktop_command::set_pointer_buttons(invalid),
                ],
            ),
            desktop_error::invalid_pointer_buttons,
        );
        let unchanged = accepted(model.observe(&endpoint));
        assert_eq!(unchanged.pointer(), pointer);
        assert_eq!(unchanged.pointer_buttons(), 5);
        assert_eq!(unchanged.revision(), revision);
        assert_eq!(windows(&model, &endpoint), before_windows);
        assert_eq!(model.next_transaction, transaction);
        assert_eq!(model.phase(), presentation_phase::steady);
        assert!(model.pending.is_none());
    }
}

fn open_appearance(
    model: &mut desktop,
    control: &desktop_control,
    endpoint: &client_endpoint,
) -> window_id {
    apply(model, control, endpoint, desktop_command::open_appearance)
        .created_window()
        .unwrap()
}

#[test]
fn appearance_navigation_visits_every_choice_once_and_wraps_both_ways() {
    let mut choice = appearance_choice::light;
    let mut visited = Vec::new();
    for _ in 0..appearance_choice::all.len() {
        assert!(!visited.contains(&choice));
        assert!(!choice.label().is_empty());
        assert_eq!(choice.next().previous(), choice);
        assert_eq!(choice.previous().next(), choice);
        visited.push(choice);
        choice = choice.next();
    }
    assert_eq!(choice, appearance_choice::light);
    assert_eq!(visited, appearance_choice::all);
    assert_eq!(choice.previous(), appearance_choice::reset);
}

#[test]
fn appearance_open_restores_the_singleton_across_workspaces_without_losing_selection() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    let view = accepted(model.observe(&endpoint));
    let window = view.window(id).unwrap();
    assert_eq!(window.title(), "Appearance");
    assert_eq!(
        window.bounds(),
        accepted(window_rect::try_new(92, 92, 840, 568))
    );
    assert_eq!(
        window.appearance_selection(),
        Some(appearance_choice::light)
    );
    assert!(window.focused());
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::select_appearance {
            window: id,
            choice: appearance_choice::square_frames,
        },
    );
    let original = accepted(model.observe(&endpoint)).theme_settings();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::minimize(id),
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::switch_workspace(workspace_id::second),
    );
    let publication = apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::open_appearance,
    );
    assert_eq!(publication.created_window(), None);
    let view = accepted(model.observe(&endpoint));
    let window = view.focused_window().unwrap();
    assert_eq!(window.id(), id);
    assert_eq!(window.workspace(), workspace_id::second);
    assert!(!window.minimized());
    assert_eq!(
        window.appearance_selection(),
        Some(appearance_choice::square_frames)
    );
    assert_eq!(view.theme_settings(), original);
    assert_eq!(
        view.all_windows()
            .filter(|window| window.content() == demo_content::appearance)
            .count(),
        1
    );
    rejection(
        model.prepare(
            &endpoint,
            view.revision(),
            desktop_command::create {
                content: demo_content::appearance,
                workspace: workspace_id::first,
                bounds: window.bounds(),
            },
        ),
        desktop_error::appearance_window_exists,
    );
}

#[test]
fn appearance_open_is_bounded_and_never_evicts_a_full_desktop() {
    for (width, height) in [(640, 480), (1024, 768), (4096, 2160)] {
        let (mut model, control, initial) = accepted(desktop::new(width, height));
        let endpoint =
            accepted(model.bind_client(&control, client_origin::human, client_grants::all));
        commit(&mut model, &control, initial);
        let id = open_appearance(&mut model, &control, &endpoint);
        let view = accepted(model.observe(&endpoint));
        let bounds = view.window(id).unwrap().bounds();
        assert!(bounds.within(view.work_area()));
        assert_eq!(bounds.width(), view.work_area().width().min(840));
        assert_eq!(bounds.height(), view.work_area().height().min(568));
    }
    let (mut model, control, endpoint) = fixture();
    for _ in 2..window_capacity {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::create {
                content: demo_content::activity,
                workspace: workspace_id::first,
                bounds: accepted(window_rect::try_new(120, 140, 480, 320)),
            },
        );
    }
    let before = windows(&model, &endpoint);
    let revision = accepted(model.observe(&endpoint)).revision();
    rejection(
        model.prepare(&endpoint, revision, desktop_command::open_appearance),
        desktop_error::window_capacity_exhausted,
    );
    assert_eq!(windows(&model, &endpoint), before);
    assert_eq!(accepted(model.observe(&endpoint)).revision(), revision);
    let to_close = before[3].id();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::close(to_close),
    );
    let id = open_appearance(&mut model, &control, &endpoint);
    assert_eq!(windows(&model, &endpoint).len(), window_capacity);
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::minimize(id),
    );
    assert_eq!(
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::open_appearance
        )
        .created_window(),
        None
    );
    assert_eq!(
        accepted(model.observe(&endpoint))
            .focused_window()
            .unwrap()
            .id(),
        id
    );
}

#[test]
fn appearance_commands_require_the_union_of_their_real_effects() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    let arranger = accepted(model.bind_client(
        &control,
        client_origin::human,
        client_grants::observe.union(client_grants::arrange),
    ));
    let customizer = accepted(model.bind_client(
        &control,
        client_origin::machine,
        client_grants::observe.union(client_grants::customize),
    ));
    let manager = accepted(model.bind_client(
        &control,
        client_origin::machine,
        client_grants::observe.union(client_grants::manage),
    ));
    for endpoint in [&arranger, &customizer, &manager] {
        let revision = accepted(model.observe(endpoint)).revision();
        rejection(
            model.prepare(endpoint, revision, desktop_command::open_appearance),
            desktop_error::grant_denied,
        );
        rejection(
            model.prepare(
                endpoint,
                revision,
                desktop_command::activate_appearance {
                    window: id,
                    choice: appearance_choice::dark,
                },
            ),
            desktop_error::grant_denied,
        );
    }
    let original = accepted(model.observe(&endpoint)).theme_settings();
    apply(
        &mut model,
        &control,
        &arranger,
        desktop_command::select_appearance {
            window: id,
            choice: appearance_choice::dark,
        },
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(
        view.window(id).unwrap().appearance_selection(),
        Some(appearance_choice::dark)
    );
    assert_eq!(view.theme_settings(), original);
    rejection(
        model.prepare(
            &customizer,
            view.revision(),
            desktop_command::select_appearance {
                window: id,
                choice: appearance_choice::light,
            },
        ),
        desktop_error::grant_denied,
    );
}

#[test]
fn appearance_targets_reject_unrelated_hidden_and_stale_windows() {
    let (mut model, control, endpoint) = fixture();
    let ordinary = windows(&model, &endpoint)[0].id();
    let id = open_appearance(&mut model, &control, &endpoint);
    for activate in [false, true] {
        let command = if activate {
            desktop_command::activate_appearance {
                window: ordinary,
                choice: appearance_choice::dark,
            }
        } else {
            desktop_command::select_appearance {
                window: ordinary,
                choice: appearance_choice::dark,
            }
        };
        rejection(
            model.prepare(
                &endpoint,
                accepted(model.observe(&endpoint)).revision(),
                command,
            ),
            desktop_error::not_appearance_window,
        );
    }
    for command in [
        desktop_command::minimize(id),
        desktop_command::move_to_workspace {
            window: id,
            workspace: workspace_id::second,
        },
    ] {
        apply(&mut model, &control, &endpoint, command);
        let revision = accepted(model.observe(&endpoint)).revision();
        for choice_command in [
            desktop_command::select_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
            desktop_command::activate_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
        ] {
            rejection(
                model.prepare(&endpoint, revision, choice_command),
                desktop_error::window_not_visible,
            );
        }
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::open_appearance,
        );
    }
    apply(&mut model, &control, &endpoint, desktop_command::close(id));
    let replacement = open_appearance(&mut model, &control, &endpoint);
    assert_eq!(id.slot(), replacement.slot());
    assert_ne!(id.generation(), replacement.generation());
    rejection(
        model.prepare(
            &endpoint,
            accepted(model.observe(&endpoint)).revision(),
            desktop_command::activate_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
        ),
        desktop_error::stale_window,
    );
}

#[test]
fn appearance_actions_are_identical_for_human_machine_and_accessibility_views() {
    let (mut human, human_control, human_endpoint) = fixture();
    let (mut machine, machine_control, _) = fixture();
    let machine_endpoint =
        accepted(machine.bind_client(&machine_control, client_origin::machine, client_grants::all));
    let human_id = open_appearance(&mut human, &human_control, &human_endpoint);
    let machine_id = open_appearance(&mut machine, &machine_control, &machine_endpoint);
    for choice in appearance_choice::all {
        let a = apply(
            &mut human,
            &human_control,
            &human_endpoint,
            desktop_command::activate_appearance {
                window: human_id,
                choice,
            },
        );
        let b = apply(
            &mut machine,
            &machine_control,
            &machine_endpoint,
            desktop_command::activate_appearance {
                window: machine_id,
                choice,
            },
        );
        assert_eq!(a.origin(), Some(client_origin::human));
        assert_eq!(b.origin(), Some(client_origin::machine));
        let a = accepted(human.observe(&human_endpoint));
        let b = accepted(machine.observe(&machine_endpoint));
        assert_eq!(a.theme_settings(), b.theme_settings());
        assert_eq!(a.theme_definition(), b.theme_definition());
        assert_eq!(
            a.all_windows().collect::<Vec<_>>(),
            b.all_windows().collect::<Vec<_>>()
        );
        assert_eq!(a.window(human_id), a.machine().window(human_id));
        assert_eq!(a.window(human_id), a.accessibility().window(human_id));
        assert_eq!(
            a.appearance_selected(choice),
            choice != appearance_choice::reset
        );
        if choice == appearance_choice::depth_on {
            assert!(!a.depth()); // High Contrast keeps the requested preference without shadows.
        }
    }
    assert_eq!(
        accepted(human.observe(&human_endpoint)).theme_settings(),
        theme_settings::initial
    );
}

#[test]
fn appearance_component_changes_preserve_overrides_and_reject_incompatible_theme_choices() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    let view = accepted(model.observe(&endpoint));
    let mut settings = view.theme_settings();
    let mut frame = view.theme_definition().frame;
    frame.shadow_opacity = 123;
    frame.control_radius = 7;
    settings.cursor_size = 3;
    settings.overrides.frame = Some(frame);
    settings.overrides.decorations = Some(view.theme_definition().decorations);
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_theme_settings(settings),
    );
    let view = accepted(model.observe(&endpoint));
    assert!(view.appearance_enabled(appearance_choice::light));
    assert!(!view.appearance_enabled(appearance_choice::dark));
    assert!(view.appearance_enabled(appearance_choice::reset));
    let revision = view.revision();
    let before = windows(&model, &endpoint);
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::activate_appearance {
                window: id,
                choice: appearance_choice::dark,
            },
        ),
        desktop_error::invalid_theme,
    );
    assert_eq!(windows(&model, &endpoint), before);
    assert_eq!(
        accepted(model.observe(&endpoint)).theme_settings(),
        settings
    );
    for choice in [
        appearance_choice::square_frames,
        appearance_choice::rounded_frames,
    ] {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::activate_appearance { window: id, choice },
        );
        settings.overrides.corner_radius = Some(if choice == appearance_choice::square_frames {
            0
        } else {
            6
        });
        assert_eq!(
            accepted(model.observe(&endpoint)).theme_settings(),
            settings
        );
    }
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::activate_appearance {
            window: id,
            choice: appearance_choice::large_text,
        },
    );
    settings.font_scale = 2;
    assert_eq!(
        accepted(model.observe(&endpoint)).theme_settings(),
        settings
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::activate_appearance {
            window: id,
            choice: appearance_choice::reset,
        },
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.theme_settings(), theme_settings::initial);
    assert!(!view.appearance_selected(appearance_choice::reset));
    assert!(view.appearance_enabled(appearance_choice::dark));
}

#[test]
fn appearance_batch_failure_restores_selection_theme_pointer_and_transaction() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    let view = accepted(model.observe(&endpoint));
    let before = windows(&model, &endpoint);
    let settings = view.theme_settings();
    let pointer = view.pointer();
    let revision = view.revision();
    let transaction = model.next_transaction;
    rejection(
        model.prepare_batch(
            &endpoint,
            revision,
            &[
                desktop_command::set_pointer { x: 600, y: 200 },
                desktop_command::set_pointer_buttons(1),
                desktop_command::activate_appearance {
                    window: id,
                    choice: appearance_choice::dark,
                },
                desktop_command::set_pointer_buttons(8),
            ],
        ),
        desktop_error::invalid_pointer_buttons,
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.pointer(), pointer);
    assert_eq!(view.pointer_buttons(), 0);
    assert_eq!(view.theme_settings(), settings);
    assert_eq!(view.revision(), revision);
    assert_eq!(windows(&model, &endpoint), before);
    assert_eq!(model.next_transaction, transaction);
    for second in [
        desktop_command::open_appearance,
        desktop_command::create {
            content: demo_content::activity,
            workspace: workspace_id::first,
            bounds: accepted(window_rect::try_new(120, 140, 480, 320)),
        },
    ] {
        rejection(
            model.prepare_batch(
                &endpoint,
                revision,
                &[desktop_command::open_appearance, second],
            ),
            desktop_error::invalid_batch,
        );
    }
    assert_eq!(model.next_transaction, transaction);
}

#[test]
fn appearance_enabled_includes_geometry_without_mutating_the_owner() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    let view = accepted(model.observe(&endpoint));
    let mut settings = view.theme_settings();
    let mut metrics = view.theme_definition().metrics;
    metrics.workarea_margin = 8;
    settings.overrides.metrics = Some(metrics);
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::set_theme_settings(settings),
    );
    let y = accepted(model.observe(&endpoint))
        .window(id)
        .unwrap()
        .floating_bounds()
        .y();
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::move_window {
            window: id,
            x: 8,
            y,
        },
    );
    let view = accepted(model.observe(&endpoint));
    let revision = view.revision();
    let transaction = model.next_transaction;
    assert!(!view.appearance_enabled(appearance_choice::reset));
    assert!(view.appearance_enabled(appearance_choice::regular_text));
    assert_eq!(view.theme_settings(), settings);
    assert_eq!(view.revision(), revision);
    assert_eq!(model.next_transaction, transaction);
    rejection(
        model.prepare(
            &endpoint,
            revision,
            desktop_command::activate_appearance {
                window: id,
                choice: appearance_choice::reset,
            },
        ),
        desktop_error::invalid_geometry,
    );
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::move_window {
            window: id,
            x: 16,
            y,
        },
    );
    assert!(accepted(model.observe(&endpoint)).appearance_enabled(appearance_choice::reset));
}

#[test]
fn appearance_corners_do_not_freeze_high_contrast_shadow_defaults() {
    let (mut model, control, endpoint) = fixture();
    let id = open_appearance(&mut model, &control, &endpoint);
    for choice in [
        appearance_choice::high_contrast,
        appearance_choice::rounded_frames,
        appearance_choice::dark,
        appearance_choice::depth_on,
    ] {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::activate_appearance { window: id, choice },
        );
    }
    let view = accepted(model.observe(&endpoint));
    let dark = anomalous_desktop_theme::builtin(theme_mode::dark, density_mode::comfortable);
    assert_eq!(view.theme_definition().frame, dark.frame);
    assert_eq!(view.theme_settings().overrides.frame, None);
    assert_eq!(view.theme_settings().overrides.corner_radius, Some(6));
    assert!(view.depth());
    assert!(view.theme_definition().frame.shadow_opacity > 0);
    for choice in [
        appearance_choice::square_frames,
        appearance_choice::high_contrast,
    ] {
        apply(
            &mut model,
            &control,
            &endpoint,
            desktop_command::activate_appearance { window: id, choice },
        );
    }
    let view = accepted(model.observe(&endpoint));
    let contrast =
        anomalous_desktop_theme::builtin(theme_mode::high_contrast, density_mode::comfortable);
    assert_eq!(view.theme_definition().frame, contrast.frame);
    assert_eq!(view.theme_settings().overrides.frame, None);
    assert_eq!(view.theme_settings().overrides.corner_radius, Some(0));
    assert!(view.theme_settings().depth);
    assert!(!view.depth());
    apply(
        &mut model,
        &control,
        &endpoint,
        desktop_command::activate_appearance {
            window: id,
            choice: appearance_choice::reset,
        },
    );
    let view = accepted(model.observe(&endpoint));
    assert_eq!(view.theme_settings(), theme_settings::initial);
    assert_eq!(view.theme_settings().overrides.corner_radius, None);
}
