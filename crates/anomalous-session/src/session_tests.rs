use super::*;
use crate::{action_error, private_text};

fn identity(generation: u64) -> session_identity {
    session_identity::try_new(1, generation).expect("session identity")
}

fn consumer(generation: u64) -> consumer_identity {
    consumer_identity::try_new(2, generation).expect("consumer identity")
}

fn route(value: u64) -> route_epoch {
    route_epoch::try_new(value).expect("route epoch")
}

fn fixture() -> shell_session {
    shell_session::new(
        identity(1),
        consumer(1),
        route(1),
        language_version::new(0, 1),
    )
}

fn insertion(value: &str) -> semantic_action {
    semantic_action::insert_text(private_text::try_from_str(value).expect("valid text"))
        .expect("nonempty insertion")
}

fn publish(
    session: &mut shell_session,
    origin: submission_origin,
    action: semantic_action,
) -> publish_receipt {
    let prepared = match session.prepare_submission(origin, action) {
        Ok(prepared) => prepared,
        Err(_) => panic!("prepare submission"),
    };
    match session.publish(prepared) {
        Ok(receipt) => receipt,
        Err(_) => panic!("publish submission"),
    }
}

fn prepare_wait(session: &shell_session) -> prepared_wait {
    match session
        .prepare_receive(session.consumer())
        .unwrap_or_else(|error| panic!("prepare wait: {}", error.code()))
    {
        receive_preparation::wait(wait) => wait,
        receive_preparation::ready => panic!("unexpected ready queue"),
    }
}

fn block(session: &mut shell_session) -> wait_identity {
    let prepared = prepare_wait(session);
    match session
        .commit_wait(prepared)
        .unwrap_or_else(|error| panic!("commit wait: {}", error.code()))
    {
        wait_commit::blocked(identity) => identity,
        wait_commit::ready => panic!("unexpected ready queue"),
    }
}

#[test]
fn every_origin_uses_the_same_bounded_fifo_semantics() {
    let origins = [
        submission_origin::human_text,
        submission_origin::machine_text,
        submission_origin::paste,
        submission_origin::accessibility,
        submission_origin::remote_text,
    ];
    for origin in origins {
        let mut session = fixture();
        let receipt = publish(&mut session, origin, insertion("same private text"));
        assert!(receipt.sequence().get() == 1);
        assert!(receipt.wake().is_none());
        let delivered = session.receive(session.consumer()).expect("delivery");
        assert!(delivered.origin() == origin);
        assert!(delivered.sequence().get() == 1);
        assert!(delivered.action().expose_text() == Some("same private text"));
        assert!(session.invariants_hold());
    }
}

#[test]
fn paste_is_only_text_insertion_and_newline_never_submits() {
    let mut session = fixture();
    let receipt = publish(
        &mut session,
        submission_origin::paste,
        insertion("first\nsecond"),
    );
    assert!(receipt.sequence().get() == 1);
    let delivered = session.receive(session.consumer()).expect("delivery");
    assert!(delivered.action().kind() == Some(semantic_action_kind::insert_text));
    assert!(delivered.action().expose_text() == Some("first\nsecond"));

    let restricted = [
        semantic_action::erase_previous(1).expect("valid erase"),
        semantic_action::submit(),
        semantic_action::cancel_submission(),
        semantic_action::source_discontinuity(),
    ];
    for action in restricted {
        let revision = session.revision();
        let error = match session.prepare_submission(submission_origin::paste, action) {
            Ok(_) => panic!("paste action unexpectedly accepted"),
            Err(error) => error,
        };
        assert!(error.error() == session_error::paste_restricted);
        assert!(error.action().kind() != Some(semantic_action_kind::insert_text));
        assert!(session.revision() == revision);
        assert!(session.queue_depth() == 0);
        drop(error.into_action());
    }
    assert!(session.invariants_hold());
}

#[test]
fn all_nonpaste_actions_retain_exact_fifo_shape() {
    let mut session = fixture();
    let actions = [
        semantic_action::erase_previous(7).expect("valid erase"),
        semantic_action::submit(),
        semantic_action::cancel_submission(),
        semantic_action::source_discontinuity(),
    ];
    for action in actions {
        publish(&mut session, submission_origin::human_text, action);
    }

    let erase = session.receive(session.consumer()).expect("erase");
    assert!(erase.action().expose_erase_count() == Some(7));
    let submit = session.receive(session.consumer()).expect("submit");
    assert!(submit.action().kind() == Some(semantic_action_kind::submit));
    let cancel = session.receive(session.consumer()).expect("cancel");
    assert!(cancel.action().kind() == Some(semantic_action_kind::cancel_submission));
    let loss = session.receive(session.consumer()).expect("discontinuity");
    assert!(loss.action().kind() == Some(semantic_action_kind::source_discontinuity));
    assert!(session.invariants_hold());
}

#[test]
fn full_queue_returns_private_ownership_without_mutation_or_sequence_use() {
    let mut session = fixture();
    for value in ["one", "two", "three", "four"] {
        publish(
            &mut session,
            submission_origin::human_text,
            insertion(value),
        );
    }
    let revision = session.revision();
    let prepared = match session
        .prepare_submission(submission_origin::human_text, insertion("private fifth"))
    {
        Ok(prepared) => prepared,
        Err(_) => panic!("prepare fifth submission"),
    };
    let rejection = match session.publish(prepared) {
        Ok(_) => panic!("full queue accepted input"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::queue_full);
    assert!(rejection.submission().action_kind() == Some(semantic_action_kind::insert_text));
    assert!(session.revision() == revision);
    assert!(session.queue_depth() == session_queue_capacity as u8);
    let returned = rejection.into_submission();
    assert!(returned.action.expose_text() == Some("private fifth"));
    drop(returned);

    for (index, value) in ["one", "two", "three", "four"].into_iter().enumerate() {
        let delivered = session.receive(session.consumer()).expect("fifo delivery");
        assert!(delivered.sequence().get() == index as u64 + 1);
        assert!(delivered.action().expose_text() == Some(value));
    }
    let fifth = publish(
        &mut session,
        submission_origin::human_text,
        insertion("replacement fifth"),
    );
    assert!(fifth.sequence().get() == 5);
    assert!(session.invariants_hold());
}

#[test]
fn source_discontinuity_cannot_be_overtaken() {
    let mut session = fixture();
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("before"),
    );
    publish(
        &mut session,
        submission_origin::human_text,
        semantic_action::source_discontinuity(),
    );
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("after"),
    );

    assert!(
        session
            .receive(session.consumer())
            .expect("before")
            .action()
            .expose_text()
            == Some("before")
    );
    assert!(
        session
            .receive(session.consumer())
            .expect("barrier")
            .action()
            .kind()
            == Some(semantic_action_kind::source_discontinuity)
    );
    assert!(
        session
            .receive(session.consumer())
            .expect("after")
            .action()
            .expose_text()
            == Some("after")
    );
    assert!(session.invariants_hold());
}

#[test]
fn empty_poll_and_blocked_wait_reject_receive_without_mutation() {
    let mut session = fixture();
    let empty_revision = session.revision();
    assert!(matches!(
        session.receive(session.consumer()),
        Err(session_error::queue_empty)
    ));
    assert!(session.revision() == empty_revision);

    let wait = block(&mut session);
    let blocked_revision = session.revision();
    assert!(matches!(
        session.receive(session.consumer()),
        Err(session_error::wake_acknowledgement_required)
    ));
    assert!(session.revision() == blocked_revision);
    assert!(session.queue_depth() == 0);
    session
        .cancel_wait(session.consumer(), wait)
        .expect("cancel blocked wait");
    assert!(session.invariants_hold());
}

#[test]
fn input_before_preparation_is_immediately_ready() {
    let mut session = fixture();
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("ready"),
    );
    assert!(matches!(
        session.prepare_receive(session.consumer()),
        Ok(receive_preparation::ready)
    ));
    assert!(
        session
            .receive(session.consumer())
            .expect("ready input")
            .action()
            .expose_text()
            == Some("ready")
    );
}

#[test]
fn input_between_prepare_and_commit_prevents_blocking() {
    let mut session = fixture();
    let prepared_wait = prepare_wait(&session);
    let receipt = publish(
        &mut session,
        submission_origin::machine_text,
        insertion("arrived"),
    );
    assert!(receipt.wake().is_none());
    let revision = session.revision();
    assert!(matches!(
        session.commit_wait(prepared_wait),
        Ok(wait_commit::ready)
    ));
    assert!(session.revision() == revision);
    assert!(
        session
            .receive(session.consumer())
            .expect("arrived input")
            .action()
            .expose_text()
            == Some("arrived")
    );
}

#[test]
fn stale_empty_preparation_cannot_install_a_wait() {
    let mut session = fixture();
    let prepared_wait = prepare_wait(&session);
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("transient"),
    );
    drop(
        session
            .receive(session.consumer())
            .expect("consume transient"),
    );
    let revision = session.revision();
    assert!(matches!(
        session.commit_wait(prepared_wait),
        Err(session_error::stale_revision)
    ));
    assert!(session.revision() == revision);
    assert!(session.invariants_hold());
}

#[test]
fn input_after_block_queues_before_wake_and_receive_waits_for_ack() {
    let mut session = fixture();
    let wait = block(&mut session);
    let receipt = publish(
        &mut session,
        submission_origin::accessibility,
        insertion("wake"),
    );
    assert!(receipt.wake() == Some(wait));
    assert!(session.queue_depth() == 1);
    let revision = session.revision();
    assert!(matches!(
        session.receive(session.consumer()),
        Err(session_error::wake_acknowledgement_required)
    ));
    assert!(session.revision() == revision);

    let acknowledgement = session
        .acknowledge_wake(session.consumer(), wait)
        .expect("acknowledge exact wake");
    assert!(acknowledgement.identity() == wait);
    assert!(acknowledgement.outcome() == wake_outcome::input_ready);
    assert!(
        session
            .receive(session.consumer())
            .expect("woken input")
            .action()
            .expose_text()
            == Some("wake")
    );
    assert!(matches!(
        session.acknowledge_wake(session.consumer(), wait),
        Err(session_error::stale_wait)
    ));
    assert!(session.invariants_hold());
}

#[test]
fn one_pending_wake_covers_later_fifo_publications() {
    let mut session = fixture();
    let wait = block(&mut session);
    let first = publish(
        &mut session,
        submission_origin::human_text,
        insertion("first"),
    );
    let second = publish(
        &mut session,
        submission_origin::machine_text,
        insertion("second"),
    );
    assert!(first.wake() == Some(wait));
    assert!(second.wake().is_none());
    session
        .acknowledge_wake(session.consumer(), wait)
        .expect("acknowledge wake");
    assert!(
        session
            .receive(session.consumer())
            .expect("first")
            .action()
            .expose_text()
            == Some("first")
    );
    assert!(
        session
            .receive(session.consumer())
            .expect("second")
            .action()
            .expose_text()
            == Some("second")
    );
}

#[test]
fn exact_blocked_wait_can_be_cancelled_but_pending_wake_cannot() {
    let mut session = fixture();
    let wait = block(&mut session);
    let cancellation = session
        .cancel_wait(session.consumer(), wait)
        .expect("cancel exact wait");
    assert!(cancellation.identity() == wait);
    assert!(matches!(
        session.cancel_wait(session.consumer(), wait),
        Err(session_error::stale_wait)
    ));

    let next = block(&mut session);
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("pending"),
    );
    let revision = session.revision();
    assert!(matches!(
        session.cancel_wait(session.consumer(), next),
        Err(session_error::wake_already_pending)
    ));
    assert!(session.revision() == revision);
    assert!(session.invariants_hold());
}

#[test]
fn wrong_and_stale_wait_identities_are_failure_atomic() {
    let mut session = fixture();
    let wait = block(&mut session);
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("pending"),
    );
    let revision = session.revision();

    let wrong_session = wait_identity {
        session: identity(2),
        ..wait
    };
    let wrong_consumer = wait_identity {
        consumer: consumer(2),
        ..wait
    };
    let wrong_route = wait_identity {
        route: route(2),
        ..wait
    };
    let wrong_sequence = wait_identity {
        sequence: wait.sequence() + 1,
        ..wait
    };
    for (candidate, expected) in [
        (wrong_session, session_error::wrong_session),
        (wrong_consumer, session_error::wrong_consumer),
        (wrong_route, session_error::stale_route),
        (wrong_sequence, session_error::stale_wait),
    ] {
        assert!(matches!(
            session.acknowledge_wake(session.consumer(), candidate),
            Err(error) if error == expected
        ));
        assert!(session.revision() == revision);
        assert!(session.queue_depth() == 1);
    }
    assert!(session.invariants_hold());
}

#[test]
fn prepared_submissions_reject_wrong_targets_with_ownership() {
    let source = fixture();
    let prepared_wrong_session = source
        .prepare_submission(submission_origin::human_text, insertion("session"))
        .unwrap_or_else(|_| panic!("prepare wrong-session probe"));
    let prepared_wrong_consumer = source
        .prepare_submission(submission_origin::human_text, insertion("consumer"))
        .unwrap_or_else(|_| panic!("prepare wrong-consumer probe"));
    let prepared_wrong_route = source
        .prepare_submission(submission_origin::human_text, insertion("route"))
        .unwrap_or_else(|_| panic!("prepare wrong-route probe"));

    let mut wrong_session = shell_session::new(
        identity(2),
        source.consumer(),
        source.route(),
        language_version::new(0, 1),
    );
    let mut wrong_consumer = shell_session::new(
        source.identity(),
        consumer(2),
        source.route(),
        language_version::new(0, 1),
    );
    let mut wrong_route = shell_session::new(
        source.identity(),
        source.consumer(),
        route(2),
        language_version::new(0, 1),
    );

    let rejection = match wrong_session.publish(prepared_wrong_session) {
        Ok(_) => panic!("wrong session accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::wrong_session);
    assert!(rejection.submission().action.expose_text() == Some("session"));
    assert!(wrong_session.queue_depth() == 0);

    let rejection = match wrong_consumer.publish(prepared_wrong_consumer) {
        Ok(_) => panic!("wrong consumer accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::wrong_consumer);
    assert!(rejection.submission().action.expose_text() == Some("consumer"));
    assert!(wrong_consumer.queue_depth() == 0);

    let rejection = match wrong_route.publish(prepared_wrong_route) {
        Ok(_) => panic!("wrong route accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::stale_route);
    assert!(rejection.submission().action.expose_text() == Some("route"));
    assert!(wrong_route.queue_depth() == 0);
}

#[test]
fn close_clears_fifo_and_reports_one_discard_count() {
    let mut session = fixture();
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("private one"),
    );
    publish(
        &mut session,
        submission_origin::remote_text,
        insertion("private two"),
    );
    let revision = session.revision().get();
    let receipt = session.close(close_reason::requested).expect("first close");
    assert!(receipt.was_first_close());
    assert!(receipt.discarded_count() == 2);
    assert!(receipt.terminal_wake().is_none());
    assert!(!session.is_open());
    assert!(session.queue_depth() == 0);
    assert!(session.revision().get() == revision + 1);
    assert!(session.queue.iter().all(queue_slot::storage_is_clear));

    let revision = session.revision();
    let repeated = session
        .close(close_reason::source_failed)
        .expect("repeat close");
    assert!(!repeated.was_first_close());
    assert!(repeated.discarded_count() == 0);
    assert!(repeated.terminal_wake().is_none());
    assert!(session.revision() == revision);
    assert!(session.invariants_hold());
}

#[test]
fn close_converts_blocked_and_ready_waits_to_terminal_tombstones() {
    for input_ready in [false, true] {
        let mut session = fixture();
        let wait = block(&mut session);
        if input_ready {
            publish(
                &mut session,
                submission_origin::human_text,
                insertion("discarded"),
            );
        }
        let receipt = session
            .close(close_reason::route_replaced)
            .expect("close with waiter");
        assert!(receipt.terminal_wake() == Some(wait));
        assert!(receipt.discarded_count() == u8::from(input_ready));
        assert!(session.queue.iter().all(queue_slot::storage_is_clear));
        assert!(matches!(
            session.receive(session.consumer()),
            Err(session_error::session_closed)
        ));

        let acknowledgement = session
            .acknowledge_wake(session.consumer(), wait)
            .expect("terminal acknowledgement");
        assert!(acknowledgement.outcome() == wake_outcome::terminal(close_reason::route_replaced));
        assert!(matches!(
            session.receive(session.consumer()),
            Err(session_error::session_closed)
        ));
        assert!(matches!(
            session.acknowledge_wake(session.consumer(), wait),
            Err(session_error::stale_wait)
        ));
        assert!(session.invariants_hold());
    }
}

#[test]
fn independent_lineage_rejects_old_submission_wait_and_typeahead() {
    let mut old = fixture();
    let old_prepared = old
        .prepare_submission(submission_origin::human_text, insertion("old prepared"))
        .unwrap_or_else(|_| panic!("prepare old submission"));
    let old_wait = block(&mut old);
    publish(
        &mut old,
        submission_origin::human_text,
        insertion("old queued"),
    );
    old.close(close_reason::consumer_replaced)
        .expect("close old");
    let terminal = old
        .acknowledge_wake(old.consumer(), old_wait)
        .expect("retire old terminal tombstone");
    assert!(terminal.outcome() == wake_outcome::terminal(close_reason::consumer_replaced));

    let mut replacement = shell_session::new(
        session_identity::try_new(9, 1).expect("independent session lineage"),
        consumer_identity::try_new(10, 1).expect("independent consumer"),
        route_epoch::try_new(11).expect("independent route"),
        old.language(),
    );
    assert!(replacement.queue_depth() == 0);
    let rejection = match replacement.publish(old_prepared) {
        Ok(_) => panic!("old submission accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::wrong_session);
    assert!(matches!(
        replacement.acknowledge_wake(replacement.consumer(), old_wait),
        Err(session_error::wrong_session)
    ));
    assert!(replacement.queue_depth() == 0);
    assert!(replacement.invariants_hold());
}

#[test]
fn sequence_and_revision_exhaustion_are_failure_atomic() {
    let mut session = fixture();
    session.submissions.next = u64::MAX;
    let final_receipt = publish(
        &mut session,
        submission_origin::human_text,
        insertion("last"),
    );
    assert!(final_receipt.sequence().get() == u64::MAX);
    drop(
        session
            .receive(session.consumer())
            .expect("last submission"),
    );
    let revision = session.revision();
    let prepared = session
        .prepare_submission(submission_origin::human_text, insertion("rejected"))
        .unwrap_or_else(|_| panic!("prepare exhausted probe"));
    let rejection = match session.publish(prepared) {
        Ok(_) => panic!("exhausted sequence accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::submission_sequence_exhausted);
    assert!(rejection.submission().action.expose_text() == Some("rejected"));
    assert!(session.revision() == revision);

    let mut wait_session = fixture();
    wait_session.waits.next = u64::MAX;
    let final_wait = block(&mut wait_session);
    assert!(final_wait.sequence() == u64::MAX);
    wait_session
        .cancel_wait(wait_session.consumer(), final_wait)
        .expect("cancel final wait");
    assert!(matches!(
        wait_session.prepare_receive(wait_session.consumer()),
        Err(session_error::wait_sequence_exhausted)
    ));

    let mut revision_session = fixture();
    revision_session.revision = u64::MAX;
    let prepared = revision_session
        .prepare_submission(submission_origin::human_text, insertion("held"))
        .unwrap_or_else(|_| panic!("prepare revision probe"));
    let rejection = match revision_session.publish(prepared) {
        Ok(_) => panic!("exhausted revision accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::revision_exhausted);
    assert!(rejection.submission().action.expose_text() == Some("held"));
    assert!(revision_session.revision().get() == u64::MAX);
    assert!(revision_session.queue_depth() == 0);
    assert!(matches!(
        revision_session.close(close_reason::requested),
        Err(session_error::revision_exhausted)
    ));
    assert!(revision_session.is_open());
}

#[test]
fn wait_install_publish_ack_receive_each_advance_revision_once() {
    let mut session = fixture();
    let initial = session.revision().get();
    let wait = block(&mut session);
    assert!(session.revision().get() == initial + 1);
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("wake"),
    );
    assert!(session.revision().get() == initial + 2);
    session
        .acknowledge_wake(session.consumer(), wait)
        .expect("acknowledge");
    assert!(session.revision().get() == initial + 3);
    drop(session.receive(session.consumer()).expect("receive"));
    assert!(session.revision().get() == initial + 4);
}

#[test]
fn source_cannot_select_consumer_or_wake_identity() {
    let mut session = fixture();
    let wait = block(&mut session);
    let receipt = publish(
        &mut session,
        submission_origin::remote_text,
        insertion("routed"),
    );
    let wake = receipt.wake().expect("host-owned wake");
    assert!(wake == wait);
    assert!(wake.consumer() == session.consumer());
    assert!(wake.session() == session.identity());
    assert!(wake.route() == session.route());
}

#[test]
fn wait_sequence_replay_cannot_regain_ready_validity() {
    let mut session = fixture();
    let installed = prepare_wait(&session);
    let replay = prepare_wait(&session);
    let wait = match session.commit_wait(installed).expect("install wait") {
        wait_commit::blocked(identity) => identity,
        wait_commit::ready => panic!("empty session became ready"),
    };
    session
        .cancel_wait(session.consumer(), wait)
        .expect("cancel installed wait");
    publish(
        &mut session,
        submission_origin::human_text,
        insertion("later input"),
    );

    let revision = session.revision();
    let depth = session.queue_depth();
    assert!(matches!(
        session.commit_wait(replay),
        Err(session_error::stale_wait)
    ));
    assert!(session.revision() == revision);
    assert!(session.queue_depth() == depth);
    assert!(session.invariants_hold());
}

#[test]
fn ring_wrap_and_straddle_preserve_exact_fifo_order() {
    let mut session = fixture();
    for value in ["one", "two", "three", "four"] {
        publish(
            &mut session,
            submission_origin::human_text,
            insertion(value),
        );
    }
    for (sequence, value) in [(1, "one"), (2, "two")] {
        let delivered = session.receive(session.consumer()).expect("front delivery");
        assert!(delivered.sequence().get() == sequence);
        assert!(delivered.action().expose_text() == Some(value));
    }
    for value in ["five", "six"] {
        publish(
            &mut session,
            submission_origin::machine_text,
            insertion(value),
        );
    }
    for (sequence, value) in [(3, "three"), (4, "four"), (5, "five"), (6, "six")] {
        let delivered = session
            .receive(session.consumer())
            .expect("straddled delivery");
        assert!(delivered.sequence().get() == sequence);
        assert!(delivered.action().expose_text() == Some(value));
    }
    assert!(session.invariants_hold());
}

#[test]
fn replace_closed_requires_retirement_and_invalidates_same_lineage_work() {
    let mut session = fixture();
    let original_identity = session.identity();
    let original_consumer = session.consumer();
    let original_route = session.route();
    let original_language = session.language();
    assert!(matches!(
        session.replace_closed(),
        Err(session_error::session_still_open)
    ));

    let stale_submission = session
        .prepare_submission(
            submission_origin::machine_text,
            insertion("stale submission"),
        )
        .unwrap_or_else(|_| panic!("prepare stale submission"));
    let installed = prepare_wait(&session);
    let stale_preparation = prepare_wait(&session);
    let old_wait = match session.commit_wait(installed).expect("install old wait") {
        wait_commit::blocked(identity) => identity,
        wait_commit::ready => panic!("empty session became ready"),
    };
    session
        .close(close_reason::consumer_replaced)
        .expect("close old incarnation");

    let pending_revision = session.revision();
    assert!(matches!(
        session.replace_closed(),
        Err(session_error::terminal_wake_pending)
    ));
    assert!(session.revision() == pending_revision);
    let terminal = session
        .acknowledge_wake(session.consumer(), old_wait)
        .expect("retire tombstone");
    assert!(terminal.outcome() == wake_outcome::terminal(close_reason::consumer_replaced));

    session.replace_closed().expect("replace retired session");
    assert!(session.is_open());
    assert!(session.identity().slot() == original_identity.slot());
    assert!(session.identity().generation() == original_identity.generation() + 1);
    assert!(session.consumer().value() == original_consumer.value());
    assert!(session.consumer().generation() == original_consumer.generation() + 1);
    assert!(session.route().get() == original_route.get() + 1);
    assert!(session.language() == original_language);
    assert!(session.revision().get() == first_revision);
    assert!(session.queue_depth() == 0);
    assert!(session.submissions.next == first_sequence);
    assert!(!session.submissions.exhausted);
    assert!(session.waits.next == first_sequence);
    assert!(!session.waits.exhausted);
    assert!(session.queue.iter().all(queue_slot::storage_is_clear));

    let rejection = match session.publish(stale_submission) {
        Ok(_) => panic!("same-lineage stale submission accepted"),
        Err(rejection) => rejection,
    };
    assert!(rejection.error() == session_error::wrong_session);
    assert!(rejection.submission().action.expose_text() == Some("stale submission"));
    assert!(matches!(
        session.commit_wait(stale_preparation),
        Err(session_error::wrong_session)
    ));
    assert!(matches!(
        session.acknowledge_wake(session.consumer(), old_wait),
        Err(session_error::wrong_session)
    ));
    assert!(session.revision().get() == first_revision);

    let receipt = publish(
        &mut session,
        submission_origin::human_text,
        insertion("new incarnation"),
    );
    assert!(receipt.sequence().get() == first_sequence);
    drop(session.receive(session.consumer()).expect("new delivery"));
    let new_wait = block(&mut session);
    assert!(new_wait.sequence() == first_sequence);
    assert!(session.invariants_hold());
}

#[test]
fn replace_closed_preflights_every_identity_without_partial_mutation() {
    for (session_generation, consumer_generation, route_value, expected) in [
        (u64::MAX, 1, 1, session_error::session_generation_exhausted),
        (1, u64::MAX, 1, session_error::consumer_generation_exhausted),
        (1, 1, u64::MAX, session_error::route_epoch_exhausted),
    ] {
        let mut session = shell_session::new(
            identity(session_generation),
            consumer(consumer_generation),
            route(route_value),
            language_version::new(7, 9),
        );
        session
            .close(close_reason::requested)
            .expect("close exhausted lineage");
        let before_identity = session.identity();
        let before_consumer = session.consumer();
        let before_route = session.route();
        let before_revision = session.revision();
        let before_submission_next = session.submissions.next;
        let before_wait_next = session.waits.next;

        assert!(matches!(session.replace_closed(), Err(error) if error == expected));
        assert!(!session.is_open());
        assert!(session.identity() == before_identity);
        assert!(session.consumer() == before_consumer);
        assert!(session.route() == before_route);
        assert!(session.revision() == before_revision);
        assert!(session.submissions.next == before_submission_next);
        assert!(session.waits.next == before_wait_next);
        assert!(session.invariants_hold());
    }
}

#[test]
fn wait_install_reserves_two_future_revisions_failure_atomically() {
    for revision in [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        session.revision = revision;
        let prepared = prepare_wait(&session);
        let wait_next = session.waits.next;
        let result = session.commit_wait(prepared);
        if revision == u64::MAX - 3 {
            let wait = match result.expect("boundary wait install") {
                wait_commit::blocked(identity) => identity,
                wait_commit::ready => panic!("empty session became ready"),
            };
            assert!(session.revision().get() == u64::MAX - 2);
            let close = session
                .close(close_reason::requested)
                .expect("reserved terminal close");
            assert!(close.terminal_wake() == Some(wait));
            assert!(session.revision().get() == u64::MAX - 1);
            session
                .acknowledge_wake(session.consumer(), wait)
                .expect("reserved terminal acknowledgement");
            assert!(session.revision().get() == u64::MAX);
            session
                .replace_closed()
                .expect("replace fully retired max-revision session");
            assert!(session.revision().get() == first_revision);
            assert!(session.is_open());
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == revision);
            assert!(session.wait == wait_state::none);
            assert!(session.waits.next == wait_next);
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn publish_reserves_retirement_and_live_wait_completion_failure_atomically() {
    for revision in [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        session.revision = revision;
        let prepared = session
            .prepare_submission(submission_origin::human_text, insertion("boundary publish"))
            .unwrap_or_else(|_| panic!("prepare boundary publish"));
        let submission_next = session.submissions.next;
        let result = session.publish(prepared);
        if revision == u64::MAX - 3 {
            let receipt = match result {
                Ok(receipt) => receipt,
                Err(_) => panic!("boundary publish rejected"),
            };
            assert!(receipt.sequence().get() == first_sequence);
            assert!(session.revision().get() == u64::MAX - 2);
            assert!(session.queue_depth() == 1);
        } else {
            let rejection = match result {
                Ok(_) => panic!("unretireable publish accepted"),
                Err(rejection) => rejection,
            };
            assert!(rejection.error() == session_error::revision_exhausted);
            assert!(rejection.submission().action.expose_text() == Some("boundary publish"));
            assert!(session.revision().get() == revision);
            assert!(session.queue_depth() == 0);
            assert!(session.submissions.next == submission_next);
        }
        assert!(session.invariants_hold());
    }

    for revision in [
        u64::MAX - 5,
        u64::MAX - 4,
        u64::MAX - 3,
        u64::MAX - 2,
        u64::MAX - 1,
        u64::MAX,
    ] {
        let mut session = fixture();
        let wait = block(&mut session);
        session.revision = revision;
        let prepared = session
            .prepare_submission(submission_origin::human_text, insertion("boundary wake"))
            .unwrap_or_else(|_| panic!("prepare boundary wake"));
        let submission_next = session.submissions.next;
        let result = session.publish(prepared);
        if revision == u64::MAX - 5 {
            let receipt = match result {
                Ok(receipt) => receipt,
                Err(_) => panic!("boundary wake publish rejected"),
            };
            assert!(receipt.wake() == Some(wait));
            assert!(session.revision().get() == u64::MAX - 4);
            assert!(session.wait == wait_state::wake_pending(wait));
            session
                .acknowledge_wake(session.consumer(), wait)
                .expect("acknowledge boundary input wake");
            assert!(session.revision().get() == u64::MAX - 3);
            drop(
                session
                    .receive(session.consumer())
                    .expect("receive boundary input"),
            );
            assert!(session.revision().get() == u64::MAX - 2);
            assert!(session.queue_depth() == 0);
            let close = session
                .close(close_reason::requested)
                .expect("close boundary completed wake");
            assert!(close.discarded_count() == 0);
            assert!(close.terminal_wake().is_none());
            assert!(session.revision().get() == u64::MAX - 1);
        } else {
            let rejection = match result {
                Ok(_) => panic!("unretireable wake publish accepted"),
                Err(rejection) => rejection,
            };
            assert!(rejection.error() == session_error::revision_exhausted);
            assert!(rejection.submission().action.expose_text() == Some("boundary wake"));
            assert!(session.revision().get() == revision);
            assert!(session.queue_depth() == 0);
            assert!(session.wait == wait_state::blocked(wait));
            assert!(session.submissions.next == submission_next);
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn input_wake_ack_reserves_two_future_revisions_failure_atomically() {
    for revision in [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        let wait = block(&mut session);
        publish(
            &mut session,
            submission_origin::human_text,
            insertion("pending boundary input"),
        );
        session.revision = revision;
        let result = session.acknowledge_wake(session.consumer(), wait);
        if revision == u64::MAX - 3 {
            let acknowledgement = result.expect("boundary input acknowledgement");
            assert!(acknowledgement.outcome() == wake_outcome::input_ready);
            assert!(session.revision().get() == u64::MAX - 2);
            assert!(session.wait == wait_state::none);
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == revision);
            assert!(session.wait == wait_state::wake_pending(wait));
            assert!(session.queue_depth() == 1);
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn cancellation_reserves_two_future_revisions_failure_atomically() {
    for revision in [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        let wait = block(&mut session);
        session.revision = revision;
        let result = session.cancel_wait(session.consumer(), wait);
        if revision == u64::MAX - 3 {
            result.expect("boundary cancellation");
            assert!(session.revision().get() == u64::MAX - 2);
            assert!(session.wait == wait_state::none);
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == revision);
            assert!(session.wait == wait_state::blocked(wait));
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn receive_reserves_two_future_revisions_failure_atomically() {
    for revision in [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        publish(
            &mut session,
            submission_origin::human_text,
            insertion("boundary delivery"),
        );
        session.revision = revision;
        let result = session.receive(session.consumer());
        if revision == u64::MAX - 3 {
            let delivered = result.expect("boundary receive");
            assert!(delivered.action().expose_text() == Some("boundary delivery"));
            assert!(session.revision().get() == u64::MAX - 2);
            assert!(session.queue_depth() == 0);
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == revision);
            assert!(session.queue_depth() == 1);
            assert!(session.queue[session.head].action.expose_text() == Some("boundary delivery"));
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn close_revision_boundary_depends_on_terminal_tombstone() {
    for revision in [u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        session.revision = revision;
        let result = session.close(close_reason::requested);
        if revision == u64::MAX - 1 {
            let receipt = result.expect("last-revision close without waiter");
            assert!(receipt.terminal_wake().is_none());
            assert!(session.revision().get() == u64::MAX);
            assert!(!session.is_open());
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == u64::MAX);
            assert!(session.is_open());
        }
        assert!(session.invariants_hold());
    }

    for revision in [u64::MAX - 2, u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        let wait = block(&mut session);
        session.revision = revision;
        let result = session.close(close_reason::requested);
        if revision == u64::MAX - 2 {
            let receipt = result.expect("reserved close with waiter");
            assert!(receipt.terminal_wake() == Some(wait));
            assert!(session.revision().get() == u64::MAX - 1);
            assert!(!session.is_open());
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == revision);
            assert!(session.is_open());
            assert!(session.wait == wait_state::blocked(wait));
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn terminal_acknowledgement_may_consume_the_last_revision() {
    for revision in [u64::MAX - 1, u64::MAX] {
        let mut session = fixture();
        let wait = block(&mut session);
        session
            .close(close_reason::source_failed)
            .expect("create terminal tombstone");
        session.revision = revision;
        let result = session.acknowledge_wake(session.consumer(), wait);
        if revision == u64::MAX - 1 {
            let acknowledgement = result.expect("last terminal acknowledgement");
            assert!(
                acknowledgement.outcome() == wake_outcome::terminal(close_reason::source_failed)
            );
            assert!(session.revision().get() == u64::MAX);
            assert!(session.wait == wait_state::none);
        } else {
            assert!(matches!(result, Err(session_error::revision_exhausted)));
            assert!(session.revision().get() == u64::MAX);
            assert!(session.wait == wait_state::terminal(wait, close_reason::source_failed));
        }
        assert!(session.invariants_hold());
    }
}

#[test]
fn cancellation_close_and_terminal_acknowledgement_each_advance_once() {
    let mut cancellation_session = fixture();
    let cancelled_wait = block(&mut cancellation_session);
    let before_cancel = cancellation_session.revision().get();
    cancellation_session
        .cancel_wait(cancellation_session.consumer(), cancelled_wait)
        .expect("cancel wait");
    assert!(cancellation_session.revision().get() == before_cancel + 1);

    let mut close_session = fixture();
    let before_close = close_session.revision().get();
    close_session
        .close(close_reason::requested)
        .expect("close without wait");
    assert!(close_session.revision().get() == before_close + 1);

    let mut terminal_session = fixture();
    let terminal_wait = block(&mut terminal_session);
    let before_terminal_close = terminal_session.revision().get();
    terminal_session
        .close(close_reason::requested)
        .expect("close with wait");
    assert!(terminal_session.revision().get() == before_terminal_close + 1);
    let before_terminal_ack = terminal_session.revision().get();
    terminal_session
        .acknowledge_wake(terminal_session.consumer(), terminal_wait)
        .expect("acknowledge terminal wake");
    assert!(terminal_session.revision().get() == before_terminal_ack + 1);
}

#[test]
fn error_formatting_is_static_and_contains_no_private_sentinel() {
    let sentinel = "NEVER_FORMAT_PRIVATE_7d1e";
    let errors = [
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
    ];
    for error in errors {
        let display = std::format!("{error}");
        let debug = std::format!("{error:?}");
        assert!(display == error.code());
        assert!(debug == error.code());
        assert!(!display.contains(sentinel));
        assert!(!debug.contains(sentinel));
    }
    assert!(std::format!("{:?}", action_error::empty_text) == "empty_text");
}

#[derive(Clone, Copy)]
enum trace_step {
    publish,
    prepare,
    commit,
    wake,
    cancel,
    receive,
    close,
}

#[test]
fn exhaustive_four_step_transition_traces_preserve_invariants() {
    const steps: [trace_step; 7] = [
        trace_step::publish,
        trace_step::prepare,
        trace_step::commit,
        trace_step::wake,
        trace_step::cancel,
        trace_step::receive,
        trace_step::close,
    ];
    let trace_count = steps.len().pow(4);
    for encoded in 0..trace_count {
        let mut session = fixture();
        let mut prepared_wait = None;
        let mut wait = None;
        let mut cursor = encoded;
        for _ in 0..4 {
            let step = steps[cursor % steps.len()];
            cursor /= steps.len();
            match step {
                trace_step::publish => {
                    if let Ok(prepared) =
                        session.prepare_submission(submission_origin::human_text, insertion("x"))
                        && let Ok(receipt) = session.publish(prepared)
                        && let Some(identity) = receipt.wake()
                    {
                        wait = Some(identity);
                    }
                }
                trace_step::prepare => {
                    if let Ok(receive_preparation::wait(prepared)) =
                        session.prepare_receive(session.consumer())
                    {
                        prepared_wait = Some(prepared);
                    }
                }
                trace_step::commit => {
                    if let Some(prepared) = prepared_wait.take()
                        && let Ok(wait_commit::blocked(identity)) = session.commit_wait(prepared)
                    {
                        wait = Some(identity);
                    }
                }
                trace_step::wake => {
                    if let Some(identity) = wait {
                        let _ = session.acknowledge_wake(session.consumer(), identity);
                    }
                }
                trace_step::cancel => {
                    if let Some(identity) = wait {
                        let _ = session.cancel_wait(session.consumer(), identity);
                    }
                }
                trace_step::receive => {
                    let _ = session.receive(session.consumer());
                }
                trace_step::close => {
                    let _ = session.close(close_reason::requested);
                }
            }
            assert!(session.invariants_hold());
        }
    }
}
