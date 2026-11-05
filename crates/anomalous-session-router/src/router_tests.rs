use anomalous_session::{
    close_reason, consumer_identity, language_version, private_text, receive_preparation,
    route_epoch, semantic_action, semantic_action_kind, session_error, session_identity,
    submission_origin, wait_commit, wake_outcome,
};

use crate::{
    identity_error, principal_identity, route_target, router_error, router_identity,
    session_route_descriptor, session_router, source_kind,
};

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

struct fixture {
    router: session_router,
    control: crate::router_control,
    primary_consumer: consumer_identity,
    secondary_consumer: consumer_identity,
}

fn descriptor(slot: u64, consumer: u64) -> session_route_descriptor {
    session_route_descriptor::new(
        session_identity::try_new(slot, 1).privacy_expect("valid session"),
        consumer_identity::try_new(consumer, 1).privacy_expect("valid consumer"),
        route_epoch::try_new(1).privacy_expect("valid route"),
        language_version::new(0, 1),
    )
}

fn fixture_with_identities(route_slot: u64, router_slot: u64) -> fixture {
    let primary = descriptor(route_slot * 10 + 1, route_slot * 10 + 3);
    let secondary = descriptor(route_slot * 10 + 2, route_slot * 10 + 4);
    let primary_consumer = primary.consumer();
    let secondary_consumer = secondary.consumer();
    let (router, control) = session_router::new(
        router_identity::try_new(router_slot, 1).privacy_expect("valid router"),
        primary,
        secondary,
    )
    .privacy_expect("valid routes");
    fixture {
        router,
        control,
        primary_consumer,
        secondary_consumer,
    }
}

fn fixture_with_router(slot: u64) -> fixture {
    fixture_with_identities(slot, slot)
}

fn principal(slot: u64) -> principal_identity {
    principal_identity::try_new(slot, 1).privacy_expect("valid principal")
}

fn insertion(value: &str) -> semantic_action {
    semantic_action::insert_text(
        private_text::try_from_str(value).privacy_expect("valid private text"),
    )
    .privacy_expect("valid insertion")
}

fn semantic_case(case: u8) -> (semantic_action, semantic_action_kind) {
    match case {
        0 => (
            insertion("same mechanism"),
            semantic_action_kind::insert_text,
        ),
        1 => (
            semantic_action::erase_previous(3).privacy_expect("valid erase"),
            semantic_action_kind::erase_previous,
        ),
        2 => (semantic_action::submit(), semantic_action_kind::submit),
        3 => (
            semantic_action::cancel_submission(),
            semantic_action_kind::cancel_submission,
        ),
        4 => (
            semantic_action::source_discontinuity(),
            semantic_action_kind::source_discontinuity,
        ),
        _ => unreachable!(),
    }
}

#[test]
fn construction_rejects_aliased_routes() {
    let route = descriptor(1, 2);
    assert!(matches!(
        session_router::new(
            router_identity::try_new(1, 1).privacy_expect("valid router"),
            route,
            route,
        ),
        Err(router_error::duplicate_route)
    ));

    let primary = session_route_descriptor::new(
        session_identity::try_new(7, 1).privacy_expect("valid session"),
        consumer_identity::try_new(8, 1).privacy_expect("valid consumer"),
        route_epoch::try_new(1).privacy_expect("valid route"),
        language_version::new(0, 1),
    );
    let same_session_lineage = session_route_descriptor::new(
        session_identity::try_new(7, 2).privacy_expect("valid session"),
        consumer_identity::try_new(9, 1).privacy_expect("valid consumer"),
        route_epoch::try_new(2).privacy_expect("valid route"),
        language_version::new(0, 1),
    );
    assert!(matches!(
        session_router::new(
            router_identity::try_new(2, 1).privacy_expect("valid router"),
            primary,
            same_session_lineage,
        ),
        Err(router_error::duplicate_route)
    ));

    let same_consumer_lineage = session_route_descriptor::new(
        session_identity::try_new(10, 1).privacy_expect("valid session"),
        consumer_identity::try_new(8, 2).privacy_expect("valid consumer"),
        route_epoch::try_new(1).privacy_expect("valid route"),
        language_version::new(0, 1),
    );
    assert!(matches!(
        session_router::new(
            router_identity::try_new(3, 1).privacy_expect("valid router"),
            primary,
            same_consumer_lineage,
        ),
        Err(router_error::duplicate_route)
    ));
}

#[test]
fn host_binds_origin_and_destination_outside_source_control() {
    let mut state = fixture_with_router(1);
    let human = state
        .router
        .admit_human_source(&state.control, principal(11))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(12))
        .privacy_expect("machine admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");

    let human_prepared = state
        .router
        .prepare_human_submission(&human, &focus, insertion("same semantics"))
        .privacy_expect("human prepared");
    assert!(human_prepared.origin() == submission_origin::human_text);
    assert!(human_prepared.target() == route_target::primary);
    assert!(human_prepared.source() == human.identity());
    assert!(human_prepared.principal() == principal(11));
    let human_receipt = state
        .router
        .publish(human_prepared)
        .privacy_expect("human published");
    assert!(human_receipt.request_sequence().get() == 1);
    assert!(human_receipt.source() == human.identity());
    assert!(human_receipt.principal() == principal(11));
    assert!(human_receipt.origin() == submission_origin::human_text);
    assert!(human_receipt.target() == route_target::primary);
    assert!(human_receipt.focus() == Some(focus.identity()));

    let machine_prepared = state
        .router
        .prepare_machine_submission(&machine, insertion("same semantics"))
        .privacy_expect("machine prepared");
    assert!(machine_prepared.origin() == submission_origin::machine_text);
    assert!(machine_prepared.target() == route_target::secondary);
    assert!(machine_prepared.source() == machine.identity());
    assert!(machine_prepared.principal() == principal(12));
    assert!(machine_prepared.focus().is_none());
    let machine_receipt = state
        .router
        .publish(machine_prepared)
        .privacy_expect("machine published");
    assert!(machine_receipt.request_sequence().get() == 1);
    assert!(machine_receipt.source() == machine.identity());
    assert!(machine_receipt.principal() == principal(12));
    assert!(machine_receipt.origin() == submission_origin::machine_text);
    assert!(machine_receipt.target() == route_target::secondary);
    assert!(machine_receipt.focus().is_none());

    let human_delivery = state
        .router
        .receive(route_target::primary, state.primary_consumer)
        .privacy_expect("human delivery");
    let machine_delivery = state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("machine delivery");
    assert!(human_delivery.origin() == submission_origin::human_text);
    assert!(machine_delivery.origin() == submission_origin::machine_text);
    assert!(human_delivery.action().expose_text() == Some("same semantics"));
    assert!(machine_delivery.action().expose_text() == Some("same semantics"));

    let rejection = state
        .router
        .prepare_machine_submission(&human, semantic_action::submit())
        .privacy_expect_err("human token cannot select machine route");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.action().kind().is_some());
}

#[test]
fn human_and_machine_sources_share_action_wait_and_backpressure_mechanics() {
    let mut state = fixture_with_router(53);
    let human = state
        .router
        .admit_human_source(&state.control, principal(13))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(14))
        .privacy_expect("machine admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");

    for case in 0..5 {
        let (human_action, expected_kind) = semantic_case(case);
        let (machine_action, machine_kind) = semantic_case(case);
        assert!(machine_kind == expected_kind);
        let human_prepared = state
            .router
            .prepare_human_submission(&human, &focus, human_action)
            .privacy_expect("human semantic action prepared");
        let machine_prepared = state
            .router
            .prepare_machine_submission(&machine, machine_action)
            .privacy_expect("machine semantic action prepared");
        assert!(human_prepared.action_kind() == Some(expected_kind));
        assert!(machine_prepared.action_kind() == Some(expected_kind));
        assert!(human_prepared.request_sequence() == machine_prepared.request_sequence());

        let human_receipt = state
            .router
            .publish(human_prepared)
            .privacy_expect("human semantic action published");
        let machine_receipt = state
            .router
            .publish(machine_prepared)
            .privacy_expect("machine semantic action published");
        assert!(human_receipt.request_sequence() == machine_receipt.request_sequence());
        assert!(human_receipt.submission_sequence() == machine_receipt.submission_sequence());
        let human_delivery = state
            .router
            .receive(route_target::primary, state.primary_consumer)
            .privacy_expect("human semantic action received");
        let machine_delivery = state
            .router
            .receive(route_target::secondary, state.secondary_consumer)
            .privacy_expect("machine semantic action received");
        assert!(human_delivery.action().kind() == Some(expected_kind));
        assert!(machine_delivery.action().kind() == Some(expected_kind));
        assert!(human_delivery.origin() == submission_origin::human_text);
        assert!(machine_delivery.origin() == submission_origin::machine_text);
    }

    let human_prepared_wait = match state
        .router
        .prepare_receive(route_target::primary, state.primary_consumer)
        .privacy_expect("human receive prepared")
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("empty human route must wait"),
    };
    let machine_prepared_wait = match state
        .router
        .prepare_receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("machine receive prepared")
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("empty machine route must wait"),
    };
    let human_wait = match state
        .router
        .commit_wait(route_target::primary, human_prepared_wait)
        .privacy_expect("human wait committed")
    {
        wait_commit::blocked(wait) => wait,
        wait_commit::ready => panic!("empty human route must block"),
    };
    let machine_wait = match state
        .router
        .commit_wait(route_target::secondary, machine_prepared_wait)
        .privacy_expect("machine wait committed")
    {
        wait_commit::blocked(wait) => wait,
        wait_commit::ready => panic!("empty machine route must block"),
    };
    let human_prepared = state
        .router
        .prepare_human_submission(&human, &focus, insertion("wake human"))
        .privacy_expect("human wake action prepared");
    let machine_prepared = state
        .router
        .prepare_machine_submission(&machine, insertion("wake machine"))
        .privacy_expect("machine wake action prepared");
    let human_receipt = state
        .router
        .publish(human_prepared)
        .privacy_expect("human wake action published");
    let machine_receipt = state
        .router
        .publish(machine_prepared)
        .privacy_expect("machine wake action published");
    assert!(human_receipt.wake() == Some(human_wait));
    assert!(machine_receipt.wake() == Some(machine_wait));
    assert!(
        state
            .router
            .acknowledge_wake(route_target::primary, state.primary_consumer, human_wait)
            .privacy_expect("human wake acknowledged")
            .outcome()
            == wake_outcome::input_ready
    );
    assert!(
        state
            .router
            .acknowledge_wake(
                route_target::secondary,
                state.secondary_consumer,
                machine_wait,
            )
            .privacy_expect("machine wake acknowledged")
            .outcome()
            == wake_outcome::input_ready
    );
    state
        .router
        .receive(route_target::primary, state.primary_consumer)
        .privacy_expect("human wake action received");
    state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("machine wake action received");

    for value in ["one", "two", "three", "four"] {
        let human_prepared = state
            .router
            .prepare_human_submission(&human, &focus, insertion(value))
            .privacy_expect("human pressure action prepared");
        let machine_prepared = state
            .router
            .prepare_machine_submission(&machine, insertion(value))
            .privacy_expect("machine pressure action prepared");
        state
            .router
            .publish(human_prepared)
            .privacy_expect("human queue has capacity");
        state
            .router
            .publish(machine_prepared)
            .privacy_expect("machine queue has capacity");
    }
    let human_full = state
        .router
        .prepare_human_submission(&human, &focus, insertion("retry human"))
        .privacy_expect("human retry prepared");
    let machine_full = state
        .router
        .prepare_machine_submission(&machine, insertion("retry machine"))
        .privacy_expect("machine retry prepared");
    assert!(human_full.request_sequence() == machine_full.request_sequence());
    let human_rejection = state
        .router
        .publish(human_full)
        .privacy_expect_err("human queue full");
    let machine_rejection = state
        .router
        .publish(machine_full)
        .privacy_expect_err("machine queue full");
    assert!(human_rejection.error() == router_error::session_failure(session_error::queue_full));
    assert!(machine_rejection.error() == router_error::session_failure(session_error::queue_full));
    assert!(
        human_rejection.submission().request_sequence()
            == machine_rejection.submission().request_sequence()
    );
    state
        .router
        .receive(route_target::primary, state.primary_consumer)
        .privacy_expect("human capacity released");
    state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("machine capacity released");
    let human_receipt = state
        .router
        .publish(human_rejection.into_submission())
        .privacy_expect("human retry accepted");
    let machine_receipt = state
        .router
        .publish(machine_rejection.into_submission())
        .privacy_expect("machine retry accepted");
    assert!(human_receipt.request_sequence() == machine_receipt.request_sequence());
}

#[test]
fn authority_is_scoped_to_one_router() {
    let mut first = fixture_with_identities(2, 77);
    let mut second = fixture_with_identities(3, 77);
    assert!(first.router.identity() == second.router.identity());
    assert!(matches!(
        second
            .router
            .admit_human_source(&first.control, principal(19)),
        Err(router_error::control_denied)
    ));
    let source = first
        .router
        .admit_human_source(&first.control, principal(20))
        .privacy_expect("source admitted");
    let first_focus = first
        .router
        .activate_focus(&first.control)
        .privacy_expect("focus activated");
    let _second_source = second
        .router
        .admit_human_source(&second.control, principal(21))
        .privacy_expect("second source admitted");
    let second_focus = second
        .router
        .activate_focus(&second.control)
        .privacy_expect("focus activated");

    let rejection = second
        .router
        .prepare_human_submission(&source, &second_focus, insertion("private sentinel"))
        .privacy_expect_err("foreign source denied");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.action().expose_text() == Some("private sentinel"));

    let rejection = first
        .router
        .prepare_human_submission(&source, &second_focus, insertion("foreign focus sentinel"))
        .privacy_expect_err("foreign focus denied");
    assert!(rejection.error() == router_error::focus_denied);
    assert!(rejection.action().expose_text() == Some("foreign focus sentinel"));

    let foreign_publish = first
        .router
        .prepare_human_submission(&source, &first_focus, insertion("foreign publish sentinel"))
        .privacy_expect("local preparation");
    let rejection = second
        .router
        .publish(foreign_publish)
        .privacy_expect_err("foreign router cannot publish preparation");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.into_action().expose_text() == Some("foreign publish sentinel"));

    let prepared = first
        .router
        .prepare_human_submission(&source, &first_focus, insertion("local"))
        .privacy_expect("local source remains valid");
    first
        .router
        .publish(prepared)
        .privacy_expect("local publish");
}

#[test]
fn source_slots_and_focus_lifecycle_rejections_preserve_live_owners() {
    let mut state = fixture_with_router(54);
    assert!(matches!(
        state
            .router
            .revoke_source(&state.control, source_kind::human_focus),
        Err(router_error::source_slot_empty)
    ));
    assert!(matches!(
        state.router.activate_focus(&state.control),
        Err(router_error::source_slot_empty)
    ));

    let human = state
        .router
        .admit_human_source(&state.control, principal(15))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(16))
        .privacy_expect("machine admitted");
    assert!(matches!(
        state
            .router
            .admit_human_source(&state.control, principal(17)),
        Err(router_error::source_slot_occupied)
    ));
    assert!(matches!(
        state
            .router
            .admit_machine_source(&state.control, principal(18)),
        Err(router_error::source_slot_occupied)
    ));
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    assert!(matches!(
        state.router.activate_focus(&state.control),
        Err(router_error::focus_already_active)
    ));

    let rejection = state
        .router
        .prepare_machine_submission(&human, insertion("human cannot select machine mode"))
        .privacy_expect_err("human token rejected by machine path");
    assert!(rejection.error() == router_error::source_denied);
    let rejection = state
        .router
        .prepare_human_submission(
            &machine,
            &focus,
            insertion("machine cannot select human mode"),
        )
        .privacy_expect_err("machine token rejected by human path");
    assert!(rejection.error() == router_error::source_denied);

    let human_prepared = state
        .router
        .prepare_human_submission(&human, &focus, insertion("original human remains live"))
        .privacy_expect("original human owner remains live");
    let machine_prepared = state
        .router
        .prepare_machine_submission(&machine, insertion("original machine remains live"))
        .privacy_expect("original machine owner remains live");
    state
        .router
        .publish(human_prepared)
        .privacy_expect("original human publishes");
    state
        .router
        .publish(machine_prepared)
        .privacy_expect("original machine publishes");
    state
        .router
        .receive(route_target::primary, state.primary_consumer)
        .privacy_expect("original human received");
    state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("original machine received");

    let primary_revision = state.router.route_revision(route_target::primary);
    let secondary_revision = state.router.route_revision(route_target::secondary);
    state
        .router
        .revoke_source(&state.control, source_kind::human_focus)
        .privacy_expect("controller revokes human without source token");
    let rejection = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect_err("transfer requires a live human binding");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.authority().target() == route_target::primary);
    assert!(state.router.route_revision(route_target::primary) == primary_revision);
    assert!(state.router.route_revision(route_target::secondary) == secondary_revision);
    let focus = rejection.into_authority();
    state
        .router
        .revoke_focus(&state.control, focus)
        .privacy_expect("focus can still be retired");
}

#[test]
fn inactive_focus_rejects_a_held_human_preparation_without_consuming_it() {
    let mut state = fixture_with_router(55);
    let human = state
        .router
        .admit_human_source(&state.control, principal(21))
        .privacy_expect("human admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    let held = state
        .router
        .prepare_human_submission(&human, &focus, insertion("held across focus retirement"))
        .privacy_expect("human preparation captured");
    let request = held.request_sequence();
    state
        .router
        .revoke_focus(&state.control, focus)
        .privacy_expect("focus revoked");
    let rejection = state
        .router
        .publish(held)
        .privacy_expect_err("inactive focus rejects held preparation");
    assert!(rejection.error() == router_error::focus_inactive);
    assert!(rejection.submission().request_sequence() == request);
    assert!(rejection.into_action().expose_text() == Some("held across focus retirement"));
    assert!(!state.router.route_is_open(route_target::primary));
    assert!(state.router.route_queue_depth(route_target::primary) == 0);
}

#[test]
fn focus_activation_rejects_a_closed_primary_without_consuming_human_binding() {
    let mut state = fixture_with_router(57);
    let human = state
        .router
        .admit_human_source(&state.control, principal(25))
        .privacy_expect("human admitted");
    state
        .router
        .retire_unfocused_route(&state.control, route_target::primary)
        .privacy_expect("primary retired before focus activation");
    assert!(matches!(
        state.router.activate_focus(&state.control),
        Err(router_error::target_closed)
    ));
    state
        .router
        .replace_closed_route(&state.control, route_target::primary)
        .privacy_expect("primary replaced");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated after replacement");
    let prepared = state
        .router
        .prepare_human_submission(&human, &focus, insertion("binding preserved"))
        .privacy_expect("human binding remained live");
    assert!(prepared.target() == route_target::primary);
}

#[test]
fn wrong_route_waits_and_replaced_routes_are_failure_atomic() {
    let mut state = fixture_with_router(56);
    let human = state
        .router
        .admit_human_source(&state.control, principal(22))
        .privacy_expect("human admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    assert!(matches!(
        state
            .router
            .retire_unfocused_route(&state.control, route_target::primary),
        Err(router_error::focus_denied)
    ));
    assert!(matches!(
        state
            .router
            .replace_closed_route(&state.control, route_target::primary),
        Err(router_error::focus_denied)
    ));

    let wrong_route_wait = match state
        .router
        .prepare_receive(route_target::primary, state.primary_consumer)
        .privacy_expect("primary receive prepared")
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("empty primary route must wait"),
    };
    assert!(matches!(
        state
            .router
            .commit_wait(route_target::secondary, wrong_route_wait),
        Err(session_error::wrong_session)
    ));
    let fresh_wait = match state
        .router
        .prepare_receive(route_target::primary, state.primary_consumer)
        .privacy_expect("primary remains unchanged")
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("empty primary route must still wait"),
    };
    let wait = match state
        .router
        .commit_wait(route_target::primary, fresh_wait)
        .privacy_expect("fresh primary wait committed")
    {
        wait_commit::blocked(wait) => wait,
        wait_commit::ready => panic!("empty primary route must block"),
    };
    let cancellation = state
        .router
        .cancel_wait(route_target::primary, state.primary_consumer, wait)
        .privacy_expect("primary wait cancelled");
    assert!(cancellation.identity() == wait);

    let machine = state
        .router
        .admit_machine_source(&state.control, principal(23))
        .privacy_expect("machine admitted");
    let held = state
        .router
        .prepare_machine_submission(&machine, insertion("old route incarnation"))
        .privacy_expect("old route preparation captured");
    let old_session = state.router.session_identity(route_target::secondary);
    state
        .router
        .retire_unfocused_route(&state.control, route_target::secondary)
        .privacy_expect("secondary retired");
    assert!(matches!(
        state
            .router
            .admit_machine_source(&state.control, principal(24)),
        Err(router_error::target_closed)
    ));
    state
        .router
        .replace_closed_route(&state.control, route_target::secondary)
        .privacy_expect("secondary replaced");
    assert!(state.router.session_identity(route_target::secondary) != old_session);
    let rejection = state
        .router
        .publish(held)
        .privacy_expect_err("old route preparation rejected");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.into_action().expose_text() == Some("old route incarnation"));
    let rejection = state
        .router
        .prepare_machine_submission(&machine, insertion("old binding rejected"))
        .privacy_expect_err("old route binding rejected");
    assert!(rejection.error() == router_error::source_denied);
    state
        .router
        .revoke_source(&state.control, source_kind::machine_explicit)
        .privacy_expect("old machine binding revoked");
    let replacement = state
        .router
        .admit_machine_source(&state.control, principal(24))
        .privacy_expect("replacement machine admitted");
    let prepared = state
        .router
        .prepare_machine_submission(&replacement, insertion("new route incarnation"))
        .privacy_expect("replacement preparation accepted");
    state
        .router
        .publish(prepared)
        .privacy_expect("replacement publication accepted");

    let human_prepared = state
        .router
        .prepare_human_submission(&human, &focus, insertion("focus remains on primary"))
        .privacy_expect("human focus unaffected by secondary replacement");
    assert!(human_prepared.target() == route_target::primary);
}

#[test]
fn focus_transfer_clears_old_typeahead_and_revalidates_preparations() {
    let mut state = fixture_with_router(4);
    let human = state
        .router
        .admit_human_source(&state.control, principal(31))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(32))
        .privacy_expect("machine admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");

    let queued = state
        .router
        .prepare_human_submission(&human, &focus, insertion("discarded typeahead"))
        .privacy_expect("queued action prepared");
    state
        .router
        .publish(queued)
        .privacy_expect("queued action published");
    let stale_human = state
        .router
        .prepare_human_submission(&human, &focus, insertion("stale after transfer"))
        .privacy_expect("human preparation captured");
    let stale_human_request = stale_human.request_sequence();
    let independent_machine = state
        .router
        .prepare_machine_submission(&machine, insertion("machine route survives"))
        .privacy_expect("machine preparation captured");

    let transfer = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect("focus transferred");
    assert!(transfer.previous_target() == route_target::primary);
    assert!(transfer.close_receipt().discarded_count() == 1);
    assert!(!state.router.route_is_open(route_target::primary));
    assert!(state.router.route_queue_depth(route_target::primary) == 0);
    let (focus, _) = transfer.into_parts();

    let rejection = state
        .router
        .publish(stale_human)
        .privacy_expect_err("old focus preparation denied");
    assert!(rejection.error() == router_error::focus_denied);
    assert!(rejection.into_action().expose_text() == Some("stale after transfer"));

    state
        .router
        .publish(independent_machine)
        .privacy_expect("explicit machine route remained valid");
    let current_human = state
        .router
        .prepare_human_submission(&human, &focus, insertion("new focus route"))
        .privacy_expect("new focus preparation");
    assert!(current_human.request_sequence() == stale_human_request);
    state
        .router
        .publish(current_human)
        .privacy_expect("new focus publication");

    let first = state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("first secondary action");
    let second = state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("second secondary action");
    assert!(first.origin() == submission_origin::machine_text);
    assert!(second.origin() == submission_origin::human_text);

    let rejection = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect_err("the bounded proof permits only A-to-B once");
    assert!(rejection.error() == router_error::invalid_focus_transition);
    assert!(rejection.authority().target() == route_target::secondary);
}

#[test]
fn failed_focus_transfer_returns_authority_and_changes_nothing() {
    let mut state = fixture_with_router(5);
    let human = state
        .router
        .admit_human_source(&state.control, principal(41))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(42))
        .privacy_expect("machine admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    let machine_action = state
        .router
        .prepare_machine_submission(&machine, insertion("occupies target"))
        .privacy_expect("machine prepared");
    state
        .router
        .publish(machine_action)
        .privacy_expect("machine published");

    let rejection = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect_err("nonempty target denied");
    assert!(rejection.error() == router_error::target_not_empty);
    assert!(state.router.route_is_open(route_target::primary));
    assert!(state.router.route_queue_depth(route_target::primary) == 0);
    let focus = rejection.into_authority();
    let action = state
        .router
        .prepare_human_submission(&human, &focus, insertion("still focused"))
        .privacy_expect("old focus remains live");
    state
        .router
        .publish(action)
        .privacy_expect("old route unchanged");
}

#[test]
fn failed_focus_transfer_close_preserves_routes_and_authority() {
    let mut state = fixture_with_router(51);
    let human = state
        .router
        .admit_human_source(&state.control, principal(43))
        .privacy_expect("human admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    state
        .router
        .prepare_exhausted_revision_for_test(route_target::primary)
        .privacy_expect("pristine primary revision exhausted");

    let primary_session = state.router.session_identity(route_target::primary);
    let primary_consumer = state.router.consumer_identity(route_target::primary);
    let primary_route = state.router.route_epoch(route_target::primary);
    let primary_revision = state.router.route_revision(route_target::primary);
    let secondary_session = state.router.session_identity(route_target::secondary);
    let secondary_consumer = state.router.consumer_identity(route_target::secondary);
    let secondary_route = state.router.route_epoch(route_target::secondary);
    let secondary_revision = state.router.route_revision(route_target::secondary);
    let focus_identity = focus.identity();

    let rejection = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect_err("exhausted primary revision rejects close");
    assert!(rejection.error() == router_error::session_failure(session_error::revision_exhausted));
    assert!(rejection.authority().identity() == focus_identity);
    assert!(rejection.authority().target() == route_target::primary);
    assert!(state.router.session_identity(route_target::primary) == primary_session);
    assert!(state.router.consumer_identity(route_target::primary) == primary_consumer);
    assert!(state.router.route_epoch(route_target::primary) == primary_route);
    assert!(state.router.route_revision(route_target::primary) == primary_revision);
    assert!(state.router.route_is_open(route_target::primary));
    assert!(state.router.route_queue_depth(route_target::primary) == 0);
    assert!(state.router.session_identity(route_target::secondary) == secondary_session);
    assert!(state.router.consumer_identity(route_target::secondary) == secondary_consumer);
    assert!(state.router.route_epoch(route_target::secondary) == secondary_route);
    assert!(state.router.route_revision(route_target::secondary) == secondary_revision);
    assert!(state.router.route_is_open(route_target::secondary));
    assert!(state.router.route_queue_depth(route_target::secondary) == 0);

    let focus = rejection.into_authority();
    let prepared = state
        .router
        .prepare_human_submission(
            &human,
            &focus,
            insertion("authority survived close failure"),
        )
        .privacy_expect("returned authority remains routable");
    assert!(prepared.target() == route_target::primary);
    assert!(prepared.request_sequence().get() == 1);
}

#[test]
fn shared_secondary_revocation_requires_explicit_machine_coordination() {
    let mut state = fixture_with_router(52);
    let human = state
        .router
        .admit_human_source(&state.control, principal(44))
        .privacy_expect("human admitted");
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(45))
        .privacy_expect("machine admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    let held_machine = state
        .router
        .prepare_machine_submission(&machine, insertion("machine remains independent"))
        .privacy_expect("machine preparation captured");
    let transfer = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect("focus transferred");
    let (focus, _) = transfer.into_parts();
    let held_human = state
        .router
        .prepare_human_submission(&human, &focus, insertion("human now shares route"))
        .privacy_expect("human preparation captured");

    let rejection = state
        .router
        .revoke_focus(&state.control, focus)
        .privacy_expect_err("live machine binding prevents implicit route retirement");
    assert!(rejection.error() == router_error::shared_route_active);
    assert!(rejection.authority().target() == route_target::secondary);
    assert!(state.router.route_is_open(route_target::secondary));
    assert!(state.router.route_queue_depth(route_target::secondary) == 0);
    let focus = rejection.into_authority();

    state
        .router
        .publish(held_machine)
        .privacy_expect("failed focus revocation preserves machine preparation");
    state
        .router
        .publish(held_human)
        .privacy_expect("failed focus revocation preserves human preparation");
    let revoked_machine = state
        .router
        .prepare_machine_submission(&machine, insertion("revoked before publication"))
        .privacy_expect("machine preparation captured before revocation");
    state
        .router
        .revoke_source(&state.control, source_kind::machine_explicit)
        .privacy_expect("machine explicitly revoked");
    let publish_rejection = state
        .router
        .publish(revoked_machine)
        .privacy_expect_err("machine preparation invalidated by explicit revocation");
    assert!(publish_rejection.error() == router_error::source_denied);
    assert!(publish_rejection.into_action().expose_text() == Some("revoked before publication"));

    let revocation = state
        .router
        .revoke_focus(&state.control, focus)
        .privacy_expect("focus revoked after machine coordination");
    assert!(revocation.target() == route_target::secondary);
    assert!(revocation.close_receipt().discarded_count() == 2);
    assert!(!state.router.route_is_open(route_target::secondary));
    assert!(state.router.route_queue_depth(route_target::secondary) == 0);
}

#[test]
fn source_revocation_invalidates_prepared_work_and_rotates_generation() {
    let mut state = fixture_with_router(6);
    let old = state
        .router
        .admit_machine_source(&state.control, principal(51))
        .privacy_expect("source admitted");
    let old_identity = old.identity();
    let prepared = state
        .router
        .prepare_machine_submission(&old, insertion("return on revoke"))
        .privacy_expect("prepared");
    state
        .router
        .revoke_source(&state.control, source_kind::machine_explicit)
        .privacy_expect("revoked");

    let rejection = state
        .router
        .publish(prepared)
        .privacy_expect_err("revoked preparation denied");
    assert!(rejection.error() == router_error::source_denied);
    assert!(rejection.into_action().expose_text() == Some("return on revoke"));
    let prepare_rejection = state
        .router
        .prepare_machine_submission(&old, insertion("old token"))
        .privacy_expect_err("old token denied");
    assert!(prepare_rejection.error() == router_error::source_denied);

    assert!(matches!(
        state
            .router
            .admit_machine_source(&state.control, principal(52)),
        Err(router_error::source_route_not_retired)
    ));
    let close = state
        .router
        .retire_unfocused_route(&state.control, route_target::secondary)
        .privacy_expect("old machine route retired");
    assert!(close.was_first_close());
    assert!(close.discarded_count() == 0);
    state
        .router
        .replace_closed_route(&state.control, route_target::secondary)
        .privacy_expect("retired machine route replaced");

    let replacement = state
        .router
        .admit_machine_source(&state.control, principal(52))
        .privacy_expect("replacement admitted");
    assert!(replacement.identity().slot() == old_identity.slot());
    assert!(replacement.identity().generation() == old_identity.generation() + 1);
    assert!(replacement.principal() == principal(52));
}

#[test]
fn duplicate_preparation_is_replay_after_one_commit() {
    let mut state = fixture_with_router(7);
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(61))
        .privacy_expect("machine admitted");
    let first = state
        .router
        .prepare_machine_submission(&machine, insertion("first"))
        .privacy_expect("first prepared");
    let duplicate = state
        .router
        .prepare_machine_submission(&machine, insertion("different replay body"))
        .privacy_expect("duplicate sequence prepared");
    assert!(first.request_sequence() == duplicate.request_sequence());
    state
        .router
        .publish(first)
        .privacy_expect("first committed");

    let rejection = state
        .router
        .publish(duplicate)
        .privacy_expect_err("duplicate rejected");
    assert!(rejection.error() == router_error::stale_preparation);
    assert!(rejection.into_action().expose_text() == Some("different replay body"));
    let next = state
        .router
        .prepare_machine_submission(&machine, insertion("second"))
        .privacy_expect("next prepared");
    assert!(next.request_sequence().get() == 2);
}

#[test]
fn backpressure_returns_retryable_owner_without_consuming_request_sequence() {
    let mut state = fixture_with_router(8);
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(71))
        .privacy_expect("machine admitted");
    for value in ["one", "two", "three", "four"] {
        let prepared = state
            .router
            .prepare_machine_submission(&machine, insertion(value))
            .privacy_expect("prepared");
        state
            .router
            .publish(prepared)
            .privacy_expect("queue has capacity");
    }
    let prepared = state
        .router
        .prepare_machine_submission(&machine, insertion("retry me"))
        .privacy_expect("fifth prepared");
    assert!(prepared.request_sequence().get() == 5);
    let rejection = state
        .router
        .publish(prepared)
        .privacy_expect_err("queue full");
    assert!(rejection.error() == router_error::session_failure(session_error::queue_full));
    assert!(rejection.submission().request_sequence().get() == 5);

    state
        .router
        .receive(route_target::secondary, state.secondary_consumer)
        .privacy_expect("capacity released");
    let receipt = state
        .router
        .publish(rejection.into_submission())
        .privacy_expect("same owner retries");
    assert!(receipt.request_sequence().get() == 5);
}

#[test]
fn focus_transfer_terminalizes_exact_wait_before_route_replacement() {
    let mut state = fixture_with_router(9);
    let _human = state
        .router
        .admit_human_source(&state.control, principal(75))
        .privacy_expect("human admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    let prepared_wait = match state
        .router
        .prepare_receive(route_target::primary, state.primary_consumer)
        .privacy_expect("receive prepared")
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("empty route must wait"),
    };
    let wait = match state
        .router
        .commit_wait(route_target::primary, prepared_wait)
        .privacy_expect("wait committed")
    {
        wait_commit::blocked(wait) => wait,
        wait_commit::ready => panic!("empty route must block"),
    };

    let transfer = state
        .router
        .transfer_focus(&state.control, focus)
        .privacy_expect("focus transferred");
    assert!(transfer.close_receipt().terminal_wake() == Some(wait));
    let closed_identity = state.router.session_identity(route_target::primary);
    assert!(matches!(
        state
            .router
            .replace_closed_route(&state.control, route_target::primary),
        Err(router_error::session_failure(
            session_error::terminal_wake_pending
        ))
    ));
    assert!(!state.router.route_is_open(route_target::primary));
    assert!(state.router.session_identity(route_target::primary) == closed_identity);
    let acknowledgement = state
        .router
        .acknowledge_wake(route_target::primary, state.primary_consumer, wait)
        .privacy_expect("terminal wake acknowledged");
    assert!(acknowledgement.outcome() == wake_outcome::terminal(close_reason::route_replaced));
    assert!(matches!(
        state
            .router
            .acknowledge_wake(route_target::primary, state.primary_consumer, wait),
        Err(session_error::stale_wait)
    ));

    state
        .router
        .replace_closed_route(&state.control, route_target::primary)
        .privacy_expect("retired route replaced");
    assert!(state.router.route_is_open(route_target::primary));
    assert!(
        state
            .router
            .session_identity(route_target::primary)
            .generation()
            == 2
    );
    assert!(
        state
            .router
            .consumer_identity(route_target::primary)
            .generation()
            == 2
    );
    assert!(state.router.route_epoch(route_target::primary).get() == 2);
}

#[test]
fn exhaustion_accepts_last_request_and_preserves_revocation() {
    let mut state = fixture_with_router(10);
    let machine = state
        .router
        .admit_machine_source(&state.control, principal(81))
        .privacy_expect("machine admitted");
    state
        .router
        .set_request_sequence_for_test(source_kind::machine_explicit, u64::MAX);
    let last = state
        .router
        .prepare_machine_submission(&machine, insertion("last request"))
        .privacy_expect("last request prepared");
    assert!(last.request_sequence().get() == u64::MAX);
    state
        .router
        .publish(last)
        .privacy_expect("last request accepted");
    let rejection = state
        .router
        .prepare_machine_submission(&machine, insertion("must not wrap"))
        .privacy_expect_err("request sequence exhausted");
    assert!(rejection.error() == router_error::request_sequence_exhausted);
    state
        .router
        .revoke_source(&state.control, source_kind::machine_explicit)
        .privacy_expect("exhausted source remains revocable");
}

#[test]
fn generation_exhaustion_never_strands_live_authority() {
    let mut state = fixture_with_router(11);
    state
        .router
        .set_source_generation_for_test(source_kind::human_focus, u64::MAX);
    let human = state
        .router
        .admit_human_source(&state.control, principal(91))
        .privacy_expect("final source generation admitted");
    assert!(human.identity().generation() == u64::MAX);
    state
        .router
        .revoke_source(&state.control, source_kind::human_focus)
        .privacy_expect("final source generation revoked");
    assert!(matches!(
        state
            .router
            .admit_human_source(&state.control, principal(92)),
        Err(router_error::source_generation_exhausted)
    ));

    let mut focus_state = fixture_with_router(111);
    let _focus_source = focus_state
        .router
        .admit_human_source(&focus_state.control, principal(93))
        .privacy_expect("focus source admitted");
    focus_state.router.set_focus_generation_for_test(u64::MAX);
    let focus = focus_state
        .router
        .activate_focus(&focus_state.control)
        .privacy_expect("final focus generation activated");
    let rejection = focus_state
        .router
        .transfer_focus(&focus_state.control, focus)
        .privacy_expect_err("focus generation cannot wrap");
    assert!(rejection.error() == router_error::focus_generation_exhausted);
    let focus = rejection.into_authority();
    let revocation = focus_state
        .router
        .revoke_focus(&focus_state.control, focus)
        .privacy_expect("final focus remains revocable");
    assert!(revocation.target() == route_target::primary);
    assert!(matches!(
        focus_state.router.activate_focus(&focus_state.control),
        Err(router_error::focus_generation_exhausted)
    ));
}

#[test]
fn human_source_replacement_requires_focus_retirement() {
    let mut state = fixture_with_router(12);
    let old = state
        .router
        .admit_human_source(&state.control, principal(101))
        .privacy_expect("human admitted");
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("focus activated");
    state
        .router
        .revoke_source(&state.control, source_kind::human_focus)
        .privacy_expect("source revoked");
    assert!(matches!(
        state
            .router
            .admit_human_source(&state.control, principal(102)),
        Err(router_error::source_route_not_retired)
    ));

    let revocation = state
        .router
        .revoke_focus(&state.control, focus)
        .privacy_expect("focus retired");
    assert!(revocation.close_receipt().was_first_close());
    state
        .router
        .replace_closed_route(&state.control, route_target::primary)
        .privacy_expect("route replaced");
    let replacement = state
        .router
        .admit_human_source(&state.control, principal(102))
        .privacy_expect("replacement admitted after retirement");
    assert!(replacement.identity().generation() == old.identity().generation() + 1);
    assert!(replacement.principal() == principal(102));
    let focus = state
        .router
        .activate_focus(&state.control)
        .privacy_expect("replacement focus activated");
    let rejection = state
        .router
        .prepare_human_submission(&old, &focus, insertion("old principal denied"))
        .privacy_expect_err("old source denied");
    assert!(rejection.error() == router_error::source_denied);
}

#[test]
fn session_sequence_and_revision_exhaustion_are_failure_atomic() {
    let mut sequence_state = fixture_with_router(13);
    sequence_state
        .router
        .prepare_final_submission_sequence_for_test(route_target::secondary)
        .privacy_expect("pristine sequence fixture");
    let machine = sequence_state
        .router
        .admit_machine_source(&sequence_state.control, principal(111))
        .privacy_expect("machine admitted");
    let last = sequence_state
        .router
        .prepare_machine_submission(&machine, insertion("last session sequence"))
        .privacy_expect("last session sequence prepared");
    let receipt = sequence_state
        .router
        .publish(last)
        .privacy_expect("last session sequence published");
    assert!(receipt.submission_sequence().get() == u64::MAX);
    let before_revision = sequence_state
        .router
        .route_revision(route_target::secondary);
    let exhausted = sequence_state
        .router
        .prepare_machine_submission(&machine, insertion("session sequence exhausted"))
        .privacy_expect("exhausted publication prepared");
    assert!(exhausted.request_sequence().get() == 2);
    let rejection = sequence_state
        .router
        .publish(exhausted)
        .privacy_expect_err("session sequence exhaustion rejects");
    assert!(
        rejection.error()
            == router_error::session_failure(session_error::submission_sequence_exhausted)
    );
    assert!(rejection.submission().request_sequence().get() == 2);
    assert!(
        sequence_state
            .router
            .route_revision(route_target::secondary)
            == before_revision
    );
    assert!(
        sequence_state
            .router
            .route_queue_depth(route_target::secondary)
            == 1
    );

    let mut revision_state = fixture_with_router(14);
    revision_state
        .router
        .prepare_close_only_revision_for_test(route_target::secondary)
        .privacy_expect("pristine revision fixture");
    let machine = revision_state
        .router
        .admit_machine_source(&revision_state.control, principal(112))
        .privacy_expect("machine admitted");
    let before_revision = revision_state
        .router
        .route_revision(route_target::secondary);
    let prepared = revision_state
        .router
        .prepare_machine_submission(&machine, insertion("revision exhausted"))
        .privacy_expect("revision rejection prepared");
    let rejection = revision_state
        .router
        .publish(prepared)
        .privacy_expect_err("revision exhaustion rejects");
    assert!(rejection.error() == router_error::session_failure(session_error::revision_exhausted));
    assert!(rejection.submission().request_sequence().get() == 1);
    assert!(
        revision_state
            .router
            .route_revision(route_target::secondary)
            == before_revision
    );
    assert!(
        revision_state
            .router
            .route_queue_depth(route_target::secondary)
            == 0
    );
    let close = revision_state
        .router
        .retire_unfocused_route(&revision_state.control, route_target::secondary)
        .privacy_expect("close reserve remains available");
    assert!(close.was_first_close());
}

#[test]
fn bounded_route_interleavings_preserve_focus_and_queue_invariants() {
    const operation_count: u64 = 7;
    const trace_length: u32 = 4;
    const trace_count: u64 = operation_count.pow(trace_length);

    for encoded_trace in 0..trace_count {
        let mut state = fixture_with_router(1_000 + encoded_trace);
        let human = state
            .router
            .admit_human_source(&state.control, principal(201))
            .privacy_expect("human admitted");
        let mut machine = Some(
            state
                .router
                .admit_machine_source(&state.control, principal(202))
                .privacy_expect("machine admitted"),
        );
        let mut focus = Some(
            state
                .router
                .activate_focus(&state.control)
                .privacy_expect("focus activated"),
        );
        let mut primary_open = true;
        let mut secondary_open = true;
        let mut primary_depth = 0_u8;
        let mut secondary_depth = 0_u8;
        let mut human_request = 1_u64;
        let mut machine_request = 1_u64;
        let mut trace = encoded_trace;

        for _ in 0..trace_length {
            let operation = trace % operation_count;
            trace /= operation_count;
            match operation {
                0 => {
                    let authority = focus.as_ref().expect("focus owner retained");
                    let prepared = state
                        .router
                        .prepare_human_submission(&human, authority, insertion("human trace"))
                        .privacy_expect("live human prepares");
                    assert!(prepared.request_sequence().get() == human_request);
                    match state.router.publish(prepared) {
                        Ok(receipt) => {
                            assert!(receipt.request_sequence().get() == human_request);
                            human_request += 1;
                            match authority.target() {
                                route_target::primary => primary_depth += 1,
                                route_target::secondary => secondary_depth += 1,
                            }
                        }
                        Err(rejection) => {
                            assert!(
                                rejection.error()
                                    == router_error::session_failure(session_error::queue_full)
                            );
                            assert!(
                                rejection.submission().request_sequence().get() == human_request
                            );
                        }
                    }
                }
                1 => {
                    if let Some(authority) = machine.as_ref() {
                        let prepared = state
                            .router
                            .prepare_machine_submission(authority, insertion("machine trace"))
                            .privacy_expect("live machine prepares");
                        assert!(prepared.request_sequence().get() == machine_request);
                        match state.router.publish(prepared) {
                            Ok(receipt) => {
                                assert!(receipt.request_sequence().get() == machine_request);
                                machine_request += 1;
                                secondary_depth += 1;
                            }
                            Err(rejection) => {
                                assert!(
                                    rejection.error()
                                        == router_error::session_failure(session_error::queue_full)
                                );
                                assert!(
                                    rejection.submission().request_sequence().get()
                                        == machine_request
                                );
                            }
                        }
                    }
                }
                2 => {
                    let authority = focus.take().expect("focus owner retained");
                    match state.router.transfer_focus(&state.control, authority) {
                        Ok(transfer) => {
                            match transfer.previous_target() {
                                route_target::primary => {
                                    primary_open = false;
                                    primary_depth = 0;
                                }
                                route_target::secondary => {
                                    secondary_open = false;
                                    secondary_depth = 0;
                                }
                            }
                            focus = Some(transfer.into_parts().0);
                        }
                        Err(rejection) => focus = Some(rejection.into_authority()),
                    }
                }
                3 => {
                    if state
                        .router
                        .receive(route_target::primary, state.primary_consumer)
                        .is_ok()
                    {
                        primary_depth -= 1;
                    }
                }
                4 => {
                    if state
                        .router
                        .receive(route_target::secondary, state.secondary_consumer)
                        .is_ok()
                    {
                        secondary_depth -= 1;
                    }
                }
                5 => {
                    if machine.take().is_some() {
                        state
                            .router
                            .revoke_source(&state.control, source_kind::machine_explicit)
                            .privacy_expect("machine revoked");
                    }
                }
                6 => {
                    if machine.is_none()
                        && state
                            .router
                            .retire_unfocused_route(&state.control, route_target::secondary)
                            .is_ok()
                    {
                        secondary_depth = 0;
                        state
                            .router
                            .replace_closed_route(&state.control, route_target::secondary)
                            .privacy_expect("retired route replaced");
                        secondary_open = true;
                        machine = Some(
                            state
                                .router
                                .admit_machine_source(&state.control, principal(203))
                                .privacy_expect("machine generation replaced"),
                        );
                        machine_request = 1;
                    }
                }
                _ => unreachable!(),
            }

            assert!(primary_depth <= 4);
            assert!(secondary_depth <= 4);
            assert!(state.router.route_is_open(route_target::primary) == primary_open);
            assert!(state.router.route_is_open(route_target::secondary) == secondary_open);
            assert!(state.router.route_queue_depth(route_target::primary) == primary_depth);
            assert!(state.router.route_queue_depth(route_target::secondary) == secondary_depth);
            let authority = focus.as_ref().expect("focus remains explicit");
            assert!(state.router.route_is_open(authority.target()));
        }
    }
}

#[test]
fn errors_are_content_free_static_codes() {
    use std::format;

    let sentinel = "do-not-log-private-router-sentinel";
    for (error, expected) in [
        (router_error::duplicate_route, "duplicate_route"),
        (
            router_error::router_instance_exhausted,
            "router_instance_exhausted",
        ),
        (router_error::control_denied, "control_denied"),
        (router_error::source_slot_occupied, "source_slot_occupied"),
        (router_error::source_slot_empty, "source_slot_empty"),
        (
            router_error::source_generation_exhausted,
            "source_generation_exhausted",
        ),
        (
            router_error::source_route_not_retired,
            "source_route_not_retired",
        ),
        (router_error::source_denied, "source_denied"),
        (router_error::focus_already_active, "focus_already_active"),
        (router_error::focus_inactive, "focus_inactive"),
        (router_error::focus_denied, "focus_denied"),
        (router_error::shared_route_active, "shared_route_active"),
        (
            router_error::focus_generation_exhausted,
            "focus_generation_exhausted",
        ),
        (
            router_error::invalid_focus_transition,
            "invalid_focus_transition",
        ),
        (router_error::target_closed, "target_closed"),
        (router_error::target_not_empty, "target_not_empty"),
        (
            router_error::request_sequence_exhausted,
            "request_sequence_exhausted",
        ),
        (router_error::stale_preparation, "stale_preparation"),
        (
            router_error::session_failure(session_error::queue_full),
            "session_failure",
        ),
        (router_error::invariant_violation, "invariant_violation"),
    ] {
        let debug = format!("{error:?}");
        let display = format!("{error}");
        assert!(error.code() == expected);
        assert!(debug == expected);
        assert!(display == expected);
        let rendered = format!("{debug} {display}");
        assert!(!rendered.contains(sentinel));
        assert!(!rendered.contains("human_text"));
        assert!(!rendered.contains("machine_text"));
    }

    for nested in [
        session_error::invalid_action,
        session_error::paste_restricted,
        session_error::session_closed,
        session_error::session_still_open,
        session_error::terminal_wake_pending,
        session_error::session_generation_exhausted,
        session_error::consumer_generation_exhausted,
        session_error::route_epoch_exhausted,
        session_error::wrong_session,
        session_error::wrong_consumer,
        session_error::stale_route,
        session_error::queue_full,
        session_error::submission_sequence_exhausted,
        session_error::wait_sequence_exhausted,
        session_error::revision_exhausted,
        session_error::queue_empty,
        session_error::wait_already_present,
        session_error::stale_wait,
        session_error::stale_revision,
        session_error::wake_not_pending,
        session_error::wake_already_pending,
        session_error::wake_acknowledgement_required,
        session_error::invariant_violation,
    ] {
        let error = router_error::session_failure(nested);
        assert!(format!("{error:?}") == "session_failure");
        assert!(format!("{error}") == "session_failure");
        assert!(error.session_error() == Some(nested));
    }

    for (error, expected) in [
        (identity_error::zero_slot, "zero_slot"),
        (identity_error::zero_generation, "zero_generation"),
        (identity_error::generation_exhausted, "generation_exhausted"),
    ] {
        assert!(error.code() == expected);
        assert!(format!("{error:?}") == expected);
        assert!(format!("{error}") == expected);
    }
}
