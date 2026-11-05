use super::*;

type storage<'a> = borrowed_frame<'a, interactive_frame_byte_count>;

fn audit(sequence: u64, text: &[u8]) -> [u8; interactive_audit_size] {
    interactive_audit {
        sequence,
        revision: sequence,
        version: 1,
        pending_request: 0,
        pending_revision: 0,
        anchor: text.len() as u8,
        head: text.len() as u8,
        undo_depth: u8::from(!text.is_empty()),
        dirty: !text.is_empty(),
        outcome: 0,
        feedback: u8::from(!text.is_empty()),
        close_armed: false,
        closed: false,
        text,
    }
    .encode()
    .unwrap()
}
fn route(generation: u64) -> remote_route_binding {
    remote_route_binding::try_new(71, [1, 1], [2, generation], [11, 1], [12, generation]).unwrap()
}
fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap()
}
struct fixture<'a> {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    bridge: remote_editor_bridge<storage<'a>>,
    input: native_editor_input,
}
impl<'a> fixture<'a> {
    fn new(buffer: &'a mut [u8; interactive_frame_byte_count]) -> Self {
        Self::with_audit(buffer, &audit(1, b""))
    }
    fn with_audit(
        buffer: &'a mut [u8; interactive_frame_byte_count],
        initial_audit: &[u8],
    ) -> Self {
        let (mut model, control, initial) = desktop::new(1024, 768).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        let producer = model
            .bind_client(
                &control,
                client_origin::machine,
                client_grants::observe.union(client_grants::present),
            )
            .unwrap();
        publish(&mut model, &control, initial);
        let mut port = model.provision_remote_editor(&control, route(1)).unwrap();
        port.admit_initial_checked(
            route(1),
            1,
            &remote_message::try_observation(71, initial_audit)
                .unwrap()
                .encode(),
        )
        .unwrap();
        let frame = native_editor_frame::from_audit_in(port.audit_bytes().unwrap(), buffer)
            .ok()
            .unwrap();
        let ticket = model
            .prepare(
                &human,
                model.observe(&human).unwrap().revision(),
                desktop_command::open_native_editor(frame.descriptor()),
            )
            .unwrap();
        let window = publish(&mut model, &control, ticket)
            .created_window()
            .unwrap();
        let bridge = model
            .bind_remote_editor(
                &control,
                &producer,
                window,
                port,
                frame,
                editor_disclosure::public_fixture,
            )
            .ok()
            .unwrap();
        let input = bridge.bind_input(&model, &control, &human).unwrap();
        Self {
            model,
            control,
            human,
            bridge,
            input,
        }
    }
    fn submit(&mut self) -> remote_request {
        let revision = self.model.observe(&self.human).unwrap().revision();
        self.bridge
            .submit_action(
                &mut self.model,
                &self.input,
                revision,
                remote_action::edit(interactive_editor_action::insert(b'b')),
            )
            .unwrap()
    }
    fn complete(&mut self, request: remote_request, transport: u64) {
        let bytes = completion(request.action_id(), request.expected_sequence(), b"b");
        assert_eq!(
            self.bridge
                .admit_checked_message(&mut self.model, route(1), transport, &bytes),
            Ok(Some(remote_action_result {
                origin: client_origin::human,
                outcome: remote_outcome::edit_applied
            }))
        );
    }
}
fn completion(id: u64, expected: u64, text: &[u8]) -> [u8; 192] {
    remote_message::try_completion(
        71,
        id,
        expected,
        remote_outcome::edit_applied,
        &audit(expected + 1, text),
    )
    .unwrap()
    .encode()
}

#[test]
fn malformed_foreign_and_unmatched_completion_preserve_fence_sequence_and_latest() {
    let mut pixels = [0; interactive_frame_byte_count];
    let mut f = fixture::new(&mut pixels);
    let request = f.submit();
    let valid = completion(request.action_id(), request.expected_sequence(), b"b");
    let before = *f.bridge.latest_audit();
    let fence = f.model.native_action_fence;
    let mut malformed = valid;
    malformed[63] = 1;
    let wrong_id = completion(request.action_id() + 1, request.expected_sequence(), b"b");
    for (binding, sequence, bytes) in [
        (route(2), 2, valid.as_slice()),
        (route(1), 3, valid.as_slice()),
        (route(1), 2, malformed.as_slice()),
        (route(1), 2, wrong_id.as_slice()),
    ] {
        assert!(
            f.bridge
                .admit_checked_message(&mut f.model, binding, sequence, bytes)
                .is_err()
        );
        assert_eq!(f.model.native_action_fence, fence);
        assert_eq!(f.bridge.latest_audit(), &before);
        assert_eq!(f.bridge.port.next_transport, 2);
    }
    let revision = f.model.observe(&f.human).unwrap().revision();
    assert!(
        f.model
            .prepare(&f.human, revision, desktop_command::toggle_launcher)
            .is_err()
    );
    f.complete(request, 2);
    assert_eq!(f.model.native_action_fence, None);
    assert!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 3, &valid)
            .is_err()
    );
    assert_eq!(f.bridge.port.next_transport, 3);
}

#[test]
fn newer_unsolicited_observation_does_not_resolve_action_or_roll_back_on_completion() {
    let mut pixels = [0; interactive_frame_byte_count];
    let mut f = fixture::new(&mut pixels);
    let request = f.submit();
    let latest = audit(3, b"bc");
    let bytes = remote_message::try_observation(71, &latest)
        .unwrap()
        .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 2, &bytes),
        Ok(None)
    );
    assert!(f.model.native_action_fence.is_some());
    assert_eq!(f.bridge.observe(&f.model, &f.human).unwrap().text, b"");
    f.complete(request, 3);
    assert_eq!(f.bridge.latest_audit(), &latest);
    assert_eq!(f.model.native_action_fence, None);
    let revision = f.model.observe(&f.human).unwrap().revision();
    assert_eq!(
        f.bridge
            .submit_action(&mut f.model, &f.input, revision, request.action()),
        Err(bridge_error::backpressure)
    );
    let conflicting = remote_message::try_observation(71, &audit(3, b"xx"))
        .unwrap()
        .encode();
    let replay = remote_message::try_observation(71, &audit(2, b"b"))
        .unwrap()
        .encode();
    for bytes in [conflicting, replay] {
        assert!(
            f.bridge
                .admit_checked_message(&mut f.model, route(1), 4, &bytes)
                .is_err()
        );
        assert_eq!(f.bridge.latest_audit(), &latest);
        assert_eq!(f.bridge.port.next_transport, 4);
    }
}

#[test]
fn update_requires_authenticated_latest_bytes_and_rejection_returns_buffer() {
    let mut first = [0; interactive_frame_byte_count];
    let mut second = [0; interactive_frame_byte_count];
    let second_pointer = second.as_ptr();
    let mut f = fixture::new(&mut first);
    let request = f.submit();
    f.complete(request, 2);
    let forged = native_editor_frame::from_audit_in(&audit(2, b"x"), &mut second)
        .ok()
        .unwrap();
    let revision = f.model.observe(&f.human).unwrap().revision();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, revision, forged)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::observation_mismatch);
    let buffer = rejected.into_frame().into_buffer();
    assert_eq!(buffer.as_ptr(), second_pointer);
    let frame = native_editor_frame::from_audit_in(f.bridge.latest_audit(), buffer)
        .ok()
        .unwrap();
    let update = f
        .bridge
        .prepare_update(&mut f.model, revision, frame)
        .ok()
        .unwrap();
    let next = remote_message::try_observation(71, &audit(3, b"bc"))
        .unwrap()
        .encode();
    f.bridge
        .admit_checked_message(&mut f.model, route(1), 3, &next)
        .unwrap();
    assert_eq!(update.frame().observation().text, b"b");
    let returned = f
        .bridge
        .cancel(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert_eq!(returned.into_buffer().as_ptr(), second_pointer);
    assert_eq!(f.bridge.observe(&f.model, &f.human).unwrap().text, b"");
    assert_eq!(
        interactive_audit::decode(f.bridge.latest_audit())
            .unwrap()
            .text,
        b"bc"
    );
}

#[test]
fn terminal_withdrawal_preserves_pending_update_then_invalidates_route_and_fence() {
    let mut first = [0; interactive_frame_byte_count];
    let mut second = [0; interactive_frame_byte_count];
    let mut f = fixture::new(&mut first);
    let request = f.submit();
    assert_eq!(
        f.bridge
            .withdraw_terminal(&mut f.model, &f.control, route(2)),
        Err(bridge_error::wrong_binding)
    );
    assert!(f.model.native_action_fence.is_some());
    f.complete(request, 2);
    let frame = native_editor_frame::from_audit_in(f.bridge.latest_audit(), &mut second)
        .ok()
        .unwrap();
    let revision = f.model.observe(&f.human).unwrap().revision();
    let update = f
        .bridge
        .prepare_update(&mut f.model, revision, frame)
        .ok()
        .unwrap();
    assert_eq!(
        f.bridge
            .withdraw_terminal(&mut f.model, &f.control, route(1)),
        Err(bridge_error::backpressure)
    );
    assert!(f.bridge.port.live);
    let returned = f
        .bridge
        .cancel(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    let frame = native_editor_frame::from_audit_in(f.bridge.latest_audit(), returned.into_buffer())
        .ok()
        .unwrap();
    let update = f
        .bridge
        .prepare_update(&mut f.model, revision, frame)
        .ok()
        .unwrap();
    f.bridge.begin(&mut f.model, &f.control, &update).unwrap();
    assert_eq!(
        f.bridge
            .withdraw_terminal(&mut f.model, &f.control, route(1)),
        Err(bridge_error::backpressure)
    );
    f.bridge
        .attest_presented(&mut f.model, &f.control, &update)
        .unwrap();
    let _retired = f
        .bridge
        .publish_recycling(&mut f.model, &f.control, update)
        .ok()
        .unwrap()
        .1;
    let _request = f.submit();
    f.bridge
        .withdraw_terminal(&mut f.model, &f.control, route(1))
        .unwrap();
    assert_eq!(f.model.native_action_fence, None);
    assert!(!f.bridge.port.live);
    let bytes = remote_message::try_observation(71, &audit(3, b"bc"))
        .unwrap()
        .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 3, &bytes),
        Err(bridge_error::retired)
    );
    assert_eq!(
        f.bridge.bind_input(&f.model, &f.control, &f.human).err(),
        Some(bridge_error::retired)
    );
}

#[test]
fn initial_port_requires_controller_and_preserves_bytes_until_matching_binding() {
    let (model, control, _) = desktop::new(1024, 768).unwrap();
    let (_foreign_model, foreign_control, _) = desktop::new(1024, 768).unwrap();
    assert!(
        model
            .provision_remote_editor(&foreign_control, route(1))
            .is_err()
    );
    let mut port = model.provision_remote_editor(&control, route(1)).unwrap();
    let initial = remote_message::try_observation(71, &audit(1, b""))
        .unwrap()
        .encode();
    assert!(port.admit_initial_checked(route(2), 1, &initial).is_err());
    assert!(port.audit_bytes().is_none());
    assert!(
        port.admit_initial_checked(route(1), 1, &completion(1, 1, b"b"))
            .is_err()
    );
    assert_eq!(port.next_transport, 1);
    port.admit_initial_checked(route(1), 1, &initial).unwrap();
    assert_eq!(port.audit_bytes(), Some(&audit(1, b"")));
    assert!(port.admit_initial_checked(route(1), 2, &initial).is_err());
    assert_eq!(port.next_transport, 2);
}

#[test]
fn identical_observations_are_idempotent_and_accepted_closed_state_cannot_reopen() {
    let mut pixels = [0; interactive_frame_byte_count];
    let mut f = fixture::new(&mut pixels);
    let initial = remote_message::try_observation(71, &audit(1, b""))
        .unwrap()
        .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 2, &initial),
        Ok(None)
    );
    assert_eq!(f.bridge.port.next_transport, 3);
    let revision = f.model.observe(&f.human).unwrap().revision();
    let request = f
        .bridge
        .submit_action(
            &mut f.model,
            &f.input,
            revision,
            remote_action::request_close,
        )
        .unwrap();
    let source = audit(2, b"");
    let mut closed = interactive_audit::decode(&source).unwrap();
    closed.closed = true;
    closed.feedback = 13;
    let closed = closed.encode().unwrap();
    let observation = remote_message::try_observation(71, &closed)
        .unwrap()
        .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 3, &observation),
        Ok(None)
    );
    assert!(f.model.native_action_fence.is_some());
    let reopened = remote_message::try_observation(71, &audit(3, b"x"))
        .unwrap()
        .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 4, &reopened),
        Err(bridge_error::retired)
    );
    assert_eq!(f.bridge.latest_audit(), &closed);
    assert_eq!(f.bridge.port.next_transport, 4);
    let outcome = remote_message::try_completion(
        71,
        request.action_id(),
        request.expected_sequence(),
        remote_outcome::closed,
        &closed,
    )
    .unwrap()
    .encode();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 4, &outcome),
        Ok(Some(remote_action_result {
            origin: client_origin::human,
            outcome: remote_outcome::closed
        }))
    );
    assert_eq!(f.model.native_action_fence, None);
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, route(1), 5, &observation),
        Ok(None)
    );
    assert_eq!(f.bridge.latest_audit(), &closed);
    let revision = f.model.observe(&f.human).unwrap().revision();
    assert_eq!(
        f.bridge.submit_action(
            &mut f.model,
            &f.input,
            revision,
            remote_action::edit(interactive_editor_action::insert(b'x'))
        ),
        Err(bridge_error::backpressure)
    );
}

#[test]
fn save_only_rejection_cannot_complete_an_edit_or_close_action() {
    // Produce a genuine awaiting-save observation at the sole editor side.
    // A save_pending response is canonical for these bytes, but cannot be the
    // result of an edit or close action accepted by this workspace.
    let mut editor = interactive_editor_session::from_open(71, 3, 1, b"b").unwrap();
    let _snapshot = editor.begin_save().unwrap();
    let initial = interactive_audit::from_view(&editor.view())
        .encode()
        .unwrap();
    for action in [
        remote_action::edit(interactive_editor_action::insert(b'x')),
        remote_action::request_close,
    ] {
        let mut pixels = [0; interactive_frame_byte_count];
        let mut f = fixture::with_audit(&mut pixels, &initial);
        let revision = f.model.observe(&f.human).unwrap().revision();
        let request = f
            .bridge
            .submit_action(&mut f.model, &f.input, revision, action)
            .unwrap();
        let forged = remote_message::try_completion(
            71,
            request.action_id(),
            request.expected_sequence(),
            remote_outcome::rejected(interactive_editor_error::opened(
                anomalous_editor_contract::opened_editor_error::save_pending,
            )),
            &initial,
        )
        .unwrap()
        .encode();
        let fence = f.model.native_action_fence;
        assert_eq!(
            f.bridge
                .admit_checked_message(&mut f.model, route(1), 2, &forged),
            Err(bridge_error::wrong_binding)
        );
        assert_eq!(f.model.native_action_fence, fence);
        assert_eq!(f.bridge.latest_audit(), &initial);
        assert_eq!(f.bridge.port.next_transport, 2);
        assert_eq!(f.bridge.pending.as_ref().unwrap().id, request.action_id());
        f.bridge
            .withdraw_terminal(&mut f.model, &f.control, route(1))
            .unwrap();
        assert_eq!(f.model.native_action_fence, None);
    }
}
