use super::*;
use crate::{interactive_editor_action as edit, save_outcome};

fn editor() -> remote_editor_session {
    remote_editor_session::new(
        7,
        interactive_editor_session::from_open(11, 13, 1, b"A").unwrap(),
    )
    .unwrap()
}
fn perform(editor: &mut remote_editor_session, action: remote_action) -> remote_execution {
    let request =
        remote_request::try_new(7, editor.next_action, editor.view().sequence(), action).unwrap();
    let result = editor.execute_bytes(&request.encode()).unwrap();
    assert_eq!(
        remote_message::decode(&result.message.encode()),
        Ok(result.message)
    );
    result
}
fn ack(snapshot: &save_snapshot) -> save_acknowledgement<'_> {
    save_acknowledgement {
        document: snapshot.document(),
        generation: snapshot.generation(),
        request: snapshot.request(),
        base_version: snapshot.base_version(),
        revision: snapshot.revision(),
        version: snapshot.base_version() + 1,
        text: snapshot.text(),
    }
}

#[test]
fn literal_request_and_observation_bytes_define_layout_without_rust_serialization() {
    let value = remote_request::try_new(
        0x0102030405060708,
        9,
        7,
        remote_action::edit(edit::insert(b'B')),
    )
    .unwrap();
    let literal = [
        65, 78, 69, 65, 1, 0, 64, 0, 8, 7, 6, 5, 4, 3, 2, 1, 9, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0,
        0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ];
    assert_eq!(value.encode(), literal);
    assert_eq!(remote_request::decode(&literal), Ok(value));

    let mut expected = [0; remote_message_size];
    expected[..8].copy_from_slice(&[65, 78, 69, 82, 1, 0, 192, 0]);
    expected[8] = 1;
    expected[16] = 7;
    expected[64..72].copy_from_slice(&[65, 78, 85, 73, 1, 0, 128, 0]);
    expected[72] = 1; // Session sequence.
    expected[80] = 1; // Text revision.
    expected[88] = 1; // Saved version.
    expected[112] = 1; // Text length.
    expected[128] = b'A';
    assert_eq!(editor().observation().encode(), expected);
    assert_eq!(
        remote_message::decode(&expected)
            .unwrap()
            .observation()
            .text,
        b"A"
    );
}

#[test]
fn every_action_round_trips_with_canonical_operands() {
    let actions = [
        edit::insert(0),
        edit::insert(255),
        edit::backspace,
        edit::delete_forward,
        edit::set_selection {
            anchor: usize::MAX,
            head: 2,
        },
        edit::select_all,
        edit::collapse_selection,
        edit::undo,
        edit::input_reset,
        edit::unsupported_key,
    ];
    for action in actions {
        let request = remote_request::try_new(7, 1, 1, remote_action::edit(action)).unwrap();
        assert_eq!(remote_request::decode(&request.encode()), Ok(request));
    }
    for motion in [
        caret_motion::left,
        caret_motion::right,
        caret_motion::start,
        caret_motion::end,
    ] {
        for extend in [false, true] {
            let request = remote_request::try_new(
                7,
                1,
                1,
                remote_action::edit(edit::move_caret { motion, extend }),
            )
            .unwrap();
            assert_eq!(remote_request::decode(&request.encode()), Ok(request));
        }
    }
    for action in [remote_action::save, remote_action::request_close] {
        let request = remote_request::try_new(7, 1, 1, action).unwrap();
        assert_eq!(remote_request::decode(&request.encode()), Ok(request));
    }
    let basic = remote_request::try_new(7, 1, 1, remote_action::edit(edit::backspace))
        .unwrap()
        .encode();
    for tag in 0..=255 {
        let mut bytes = basic;
        bytes[32] = tag;
        assert_eq!(
            remote_request::decode(&bytes).is_ok(),
            (1..=12).contains(&tag)
        );
    }
    for tag in [1, 2, 3, 6, 7, 8, 9, 10, 11, 12] {
        let mut bytes = basic;
        bytes[32] = tag;
        bytes[48] = 1;
        assert_eq!(remote_request::decode(&bytes), Err(remote_error::action));
    }
    for (tag, offset, value) in [(1, 41, 1), (4, 40, 4), (4, 48, 2), (2, 40, 1)] {
        let mut bytes = basic;
        bytes[32] = tag;
        bytes[offset] = value;
        assert_eq!(remote_request::decode(&bytes), Err(remote_error::action));
    }
}

#[test]
fn malformed_requests_never_advance_or_mutate_the_editor() {
    let request = remote_request::try_new(7, 1, 1, remote_action::edit(edit::insert(b'B')))
        .unwrap()
        .encode();
    let mut owner = editor();
    let before = owner.view();
    for length in 0..remote_request_size {
        assert_eq!(
            owner.execute_bytes(&request[..length]),
            Err(remote_error::length)
        );
    }
    let mut oversized = [0; remote_request_size + 1];
    oversized[..remote_request_size].copy_from_slice(&request);
    assert_eq!(owner.execute_bytes(&oversized), Err(remote_error::length));
    for offset in (0..8).chain(33..40).chain(56..64) {
        let mut bytes = request;
        bytes[offset] ^= 1;
        assert_eq!(owner.execute_bytes(&bytes), Err(remote_error::encoding));
    }
    for offset in [8, 16, 24] {
        let mut bytes = request;
        bytes[offset..offset + 8].fill(0);
        assert_eq!(owner.execute_bytes(&bytes), Err(remote_error::fields));
    }
    assert_eq!(owner.view(), before);
    assert_eq!(owner.next_action, 1);
    assert!(owner.execute_bytes(&request).is_ok());
}

#[test]
fn exact_ids_stale_resolution_replay_and_exhaustion_preserve_owner_boundaries() {
    let mut owner = editor();
    let before = owner.view();
    for (connection, id, error) in [
        (8, 1, remote_error::connection),
        (7, 2, remote_error::action_id),
        (7, u64::MAX, remote_error::action_id),
    ] {
        let request = remote_request::try_new(connection, id, 1, remote_action::save).unwrap();
        assert_eq!(owner.execute(request), Err(error));
        assert_eq!(owner.view(), before);
        assert_eq!(owner.next_action, 1);
    }
    let stale = remote_request::try_new(7, 1, 99, remote_action::edit(edit::insert(b'X'))).unwrap();
    let result = owner.execute(stale).unwrap();
    assert_eq!(result.message.outcome(), Some(remote_outcome::stale));
    assert_eq!(result.effect, remote_effect::none);
    assert_eq!(owner.view(), before);
    assert_eq!(owner.next_action, 2);
    assert_eq!(owner.execute(stale), Err(remote_error::action_id));
    perform(&mut owner, remote_action::edit(edit::insert(b'B')));
    assert_eq!(owner.view().text(), b"BA");
    let before = owner.view();
    owner.next_action = u64::MAX;
    let final_request =
        remote_request::try_new(7, u64::MAX, before.sequence(), remote_action::save).unwrap();
    assert_eq!(owner.execute(final_request), Err(remote_error::exhausted));
    assert_eq!(owner.view(), before);
    assert_eq!(owner.next_action, u64::MAX);
}

#[test]
fn save_result_reserves_only_and_late_exact_ack_preserves_newer_dirty_text() {
    let mut owner = editor();
    perform(&mut owner, remote_action::edit(edit::select_all));
    perform(&mut owner, remote_action::edit(edit::insert(b'B')));
    let result = perform(&mut owner, remote_action::save);
    let remote_effect::save(snapshot) = result.effect else {
        panic!("snapshot missing")
    };
    assert_eq!(
        (snapshot.request(), snapshot.revision(), snapshot.text()),
        (1, 2, b"B".as_slice())
    );
    assert_eq!(
        result.message.outcome(),
        Some(remote_outcome::save_reserved)
    );
    let audit = result.message.observation();
    assert_eq!(
        (audit.sequence, audit.revision, audit.version, audit.outcome),
        (4, 2, 1, 1)
    );
    assert_eq!((audit.pending_request, audit.pending_revision), (1, 2));
    let before = owner.view();
    let rejected = perform(&mut owner, remote_action::save);
    assert_eq!(
        rejected.message.outcome(),
        Some(remote_outcome::rejected(interactive_editor_error::opened(
            opened_editor_error::save_pending
        )))
    );
    assert_eq!(rejected.effect, remote_effect::none);
    assert_eq!(owner.view(), before);
    perform(&mut owner, remote_action::edit(edit::select_all));
    perform(&mut owner, remote_action::edit(edit::insert(b'C')));
    let before = owner.view();
    let mut wrong = ack(&snapshot);
    wrong.request += 1;
    assert!(owner.acknowledge_save(wrong).is_err());
    assert_eq!(owner.view(), before);
    owner.acknowledge_save(ack(&snapshot)).unwrap();
    let audit = owner.observation();
    assert_eq!(audit.outcome(), None);
    assert_eq!((audit.action_id(), audit.expected_sequence()), (0, 0));
    let audit = audit.observation();
    assert_eq!(
        (audit.sequence, audit.revision, audit.version, audit.text),
        (7, 3, 2, b"C".as_slice())
    );
    assert!(audit.dirty);
    assert_eq!(
        (audit.outcome, audit.pending_request, audit.pending_revision),
        (0, 0, 0)
    );
    let before = owner.view();
    assert!(owner.acknowledge_save(ack(&snapshot)).is_err());
    assert_eq!(owner.view(), before);
}

#[test]
fn unknown_save_survives_local_edits_and_close_results_preserve_uncertainty() {
    let mut owner = editor();
    let remote_effect::save(snapshot) = perform(&mut owner, remote_action::save).effect else {
        panic!("snapshot missing")
    };
    owner
        .mark_save_unknown(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
        )
        .unwrap();
    perform(&mut owner, remote_action::edit(edit::insert(b'B')));
    assert_eq!(owner.view().save_outcome(), save_outcome::unknown);
    let before = owner.view();
    assert!(
        owner
            .reject_save(
                snapshot.document(),
                snapshot.generation(),
                snapshot.request(),
                snapshot.base_version()
            )
            .is_err()
    );
    assert_eq!(owner.view(), before);
    let first = perform(&mut owner, remote_action::request_close);
    assert_eq!(
        first.message.outcome(),
        Some(remote_outcome::close_confirmation)
    );
    assert!(first.message.observation().close_armed);
    let second = perform(&mut owner, remote_action::request_close);
    assert_eq!(second.message.outcome(), Some(remote_outcome::closed));
    assert!(second.message.observation().closed);
    assert_eq!(second.message.observation().outcome, 2);
    let before = owner.view();
    let third = perform(&mut owner, remote_action::edit(edit::input_reset));
    assert_eq!(
        third.message.outcome(),
        Some(remote_outcome::rejected(interactive_editor_error::closed))
    );
    assert_eq!(owner.view(), before);
}

#[test]
fn message_parser_rejects_all_header_reserved_and_inconsistent_outcomes() {
    let observation = editor().observation().encode();
    for length in 0..remote_message_size {
        assert_eq!(
            remote_message::decode(&observation[..length]),
            Err(remote_error::length)
        );
    }
    let oversized = [0; remote_message_size + 1];
    assert_eq!(
        remote_message::decode(&oversized),
        Err(remote_error::length)
    );
    for offset in (0..8)
        .chain(11..16)
        .chain(40..64)
        .chain(121..128)
        .chain(129..192)
    {
        let mut bytes = observation;
        bytes[offset] ^= 1;
        assert!(remote_message::decode(&bytes).is_err(), "offset {offset}");
    }
    for kind in 0..=255 {
        let mut bytes = observation;
        bytes[8] = kind;
        assert_eq!(remote_message::decode(&bytes).is_ok(), kind == 1);
    }
    for offset in [9, 10] {
        for value in 1..=255 {
            let mut bytes = observation;
            bytes[offset] = value;
            assert!(remote_message::decode(&bytes).is_err());
        }
    }
    for offset in [24, 32] {
        let mut bytes = observation;
        bytes[offset] = 1;
        assert_eq!(remote_message::decode(&bytes), Err(remote_error::fields));
    }
    let mut owner = editor();
    let edit = perform(&mut owner, remote_action::edit(edit::insert(b'B'))).message;
    let good = edit.encode();
    for outcome in 0..=255 {
        let mut bytes = good;
        bytes[9] = outcome;
        // Only edit_applied and stale satisfy this audit's field relations.
        // Stale is meaningful for another request; the workspace additionally
        // binds every result to the exact submitted action and expected value.
        assert_eq!(
            remote_message::decode(&bytes).is_ok(),
            outcome == 1 || outcome == 5
        );
    }
    for outcome in [
        remote_outcome::save_reserved,
        remote_outcome::close_confirmation,
        remote_outcome::closed,
        remote_outcome::rejected(interactive_editor_error::closed),
    ] {
        assert_eq!(
            remote_message::try_completion(7, 1, 1, outcome, edit.audit_bytes()),
            Err(remote_error::outcome)
        );
    }
    assert_eq!(
        remote_message::try_completion(7, 1, 2, remote_outcome::stale, edit.audit_bytes()),
        Err(remote_error::outcome)
    );
    let mut unknown_error = good;
    unknown_error[9] = 6;
    for error in 0..=255 {
        unknown_error[10] = error;
        assert!(remote_message::decode(&unknown_error).is_err());
    }
}

#[test]
fn valid_edit_rejection_is_feedback_while_service_reply_can_stale_an_action() {
    let mut owner = editor();
    let rejected_edit = perform(&mut owner, remote_action::edit(edit::insert(0)));
    assert_eq!(
        rejected_edit.message.outcome(),
        Some(remote_outcome::edit_applied)
    );
    assert_eq!(rejected_edit.message.observation().feedback, 17);
    assert_eq!(
        (
            owner.view().sequence(),
            owner.view().revision(),
            owner.view().text()
        ),
        (2, 1, b"A".as_slice())
    );
    let remote_effect::save(snapshot) = perform(&mut owner, remote_action::save).effect else {
        panic!("snapshot missing")
    };
    let request = remote_request::try_new(
        7,
        owner.next_action,
        owner.view().sequence(),
        remote_action::edit(edit::insert(b'X')),
    )
    .unwrap();
    owner.acknowledge_save(ack(&snapshot)).unwrap();
    let before = owner.view();
    let completion = owner.execute(request).unwrap();
    assert_eq!(completion.message.outcome(), Some(remote_outcome::stale));
    assert_eq!(owner.view(), before);
}

#[test]
fn rejection_codes_preserve_session_validation_order() {
    let observation = editor().observation();
    let mut audit = observation.observation();
    for (error, sequence, version) in [
        (interactive_editor_error::sequence_exhausted, u64::MAX, 1),
        (
            interactive_editor_error::opened(opened_editor_error::request_exhausted),
            1,
            1,
        ),
        (
            interactive_editor_error::opened(opened_editor_error::version_exhausted),
            1,
            u64::MAX,
        ),
    ] {
        audit.sequence = sequence;
        audit.version = version;
        let bytes = audit.encode().unwrap();
        let response =
            remote_message::try_completion(7, 1, sequence, remote_outcome::rejected(error), &bytes)
                .unwrap();
        assert_eq!(remote_message::decode(&response.encode()), Ok(response));
        for unknown in 6..=255 {
            let mut corrupt = response.encode();
            corrupt[10] = unknown;
            assert_eq!(remote_message::decode(&corrupt), Err(remote_error::outcome));
        }
    }
    audit.sequence = u64::MAX;
    audit.version = u64::MAX;
    assert_eq!(
        remote_message::try_completion(
            7,
            1,
            u64::MAX,
            remote_outcome::rejected(interactive_editor_error::opened(
                opened_editor_error::version_exhausted
            )),
            &audit.encode().unwrap()
        ),
        Err(remote_error::outcome)
    );
    audit.sequence = 1;
    assert_eq!(
        remote_message::try_completion(
            7,
            1,
            1,
            remote_outcome::rejected(interactive_editor_error::opened(
                opened_editor_error::request_exhausted
            )),
            &audit.encode().unwrap()
        ),
        Err(remote_error::outcome)
    );

    let mut owner = remote_editor_session::new(
        7,
        interactive_editor_session::from_open(11, 13, u64::MAX, b"A").unwrap(),
    )
    .unwrap();
    let before = owner.view();
    let result = perform(&mut owner, remote_action::save);
    assert_eq!(
        result.message.outcome(),
        Some(remote_outcome::rejected(interactive_editor_error::opened(
            opened_editor_error::version_exhausted
        )))
    );
    assert_eq!(owner.view(), before);
    assert_eq!(result.effect, remote_effect::none);
}
