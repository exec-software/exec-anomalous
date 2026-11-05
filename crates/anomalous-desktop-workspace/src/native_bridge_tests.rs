use super::*;

struct fixture {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    producer: client_endpoint,
    owner: native_editor_owner,
    bridge: native_editor_bridge,
    input: native_editor_input,
}

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap()
}

fn owner(text: &[u8]) -> native_editor_owner {
    native_editor_owner::new(interactive_editor_session::from_open(71, 3, 1, text).unwrap())
}

fn duplicate(ticket: &native_editor_action_ticket) -> native_editor_action_ticket {
    // Deliberately forge a duplicate only inside this private adversarial test.
    native_editor_action_ticket {
        key: ticket.key,
        sequence: ticket.sequence,
        desktop_revision: ticket.desktop_revision,
        observation_sequence: ticket.observation_sequence,
        slot: ticket.slot,
        generation: ticket.generation,
        action: ticket.action,
    }
}

fn insert(byte: u8) -> native_editor_action {
    native_editor_action::edit(interactive_editor_action::insert(byte))
}

impl fixture {
    fn new() -> Self {
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
        let mut owner = owner(b"");
        let frame = owner.frame().unwrap();
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
            .bind_native_editor(
                &control,
                &producer,
                window,
                &mut owner,
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
            producer,
            owner,
            bridge,
            input,
        }
    }

    fn revision(&self) -> u64 {
        self.model.observe(&self.human).unwrap().revision()
    }

    fn action(&mut self, action: native_editor_action) -> native_editor_action_ticket {
        self.bridge
            .prepare_action(&self.model, &self.input, self.revision(), action)
            .unwrap()
    }

    fn consume(&mut self, action: native_editor_action) -> native_editor_action_result {
        let ticket = self.action(action);
        self.bridge
            .consume_action(&self.model, &mut self.owner, ticket)
            .ok()
            .unwrap()
    }

    fn update(&mut self) -> native_editor_update {
        let frame = self.owner.frame().unwrap();
        let revision = self.revision();
        self.bridge
            .prepare_update(&mut self.model, &self.owner, revision, frame)
            .ok()
            .unwrap()
    }

    fn sync(&mut self) {
        let update = self.update();
        self.bridge
            .begin(&mut self.model, &self.control, &update)
            .unwrap();
        self.bridge
            .attest_presented(&mut self.model, &self.control, &update)
            .unwrap();
        self.bridge
            .publish(&mut self.model, &self.control, update)
            .ok()
            .unwrap();
    }

    fn desktop_action(&mut self, action: desktop_command) -> publication {
        let ticket = self
            .model
            .prepare(&self.human, self.revision(), action)
            .unwrap();
        publish(&mut self.model, &self.control, ticket)
    }
}

#[test]
fn authority_rejects_foreign_desktops_clients_controllers_and_missing_grants() {
    let mut f = fixture::new();
    let foreign = fixture::new();
    assert_eq!(
        f.bridge.observe(&foreign.model, &foreign.human).err(),
        Some(bridge_error::wrong_binding)
    );
    assert_eq!(
        f.bridge.observe(&f.model, &foreign.human).err(),
        Some(bridge_error::desktop(desktop_error::client_denied))
    );
    assert_eq!(
        f.bridge
            .bind_input(&f.model, &foreign.control, &f.human)
            .err(),
        Some(bridge_error::desktop(desktop_error::control_denied))
    );
    let observer = f
        .model
        .bind_client(&f.control, client_origin::machine, client_grants::observe)
        .unwrap();
    assert_eq!(f.bridge.observe(&f.model, &observer).unwrap().text, b"");
    assert_eq!(
        f.bridge.bind_input(&f.model, &f.control, &observer).err(),
        Some(bridge_error::desktop(desktop_error::grant_denied))
    );
    let presenter = f
        .model
        .bind_client(&f.control, client_origin::machine, client_grants::present)
        .unwrap();
    assert_eq!(
        f.bridge.observe(&f.model, &presenter).err(),
        Some(bridge_error::desktop(desktop_error::grant_denied))
    );
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &foreign.input, f.revision(), insert(b'x'))
            .err(),
        Some(bridge_error::wrong_binding)
    );
    assert_eq!(f.owner.session().view().sequence(), 1);
    assert!(f.bridge.in_flight.is_none());
}

#[test]
fn producer_and_input_revocation_are_rechecked_at_use() {
    let mut f = fixture::new();
    let ticket = f.action(insert(b'x'));
    f.model.revoke_client(&f.control, &f.human).unwrap();
    let rejected = f
        .bridge
        .consume_action(&f.model, &mut f.owner, ticket)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error(),
        bridge_error::desktop(desktop_error::client_denied)
    );
    assert!(rejected.into_ticket().is_none());
    assert!(f.bridge.in_flight.is_none());
    assert_eq!(f.owner.session().view().text(), b"");
    let replacement = f
        .model
        .bind_client(&f.control, client_origin::human, client_grants::all)
        .unwrap();
    assert_eq!(replacement.slot, f.human.slot);
    assert_ne!(replacement.generation, f.human.generation);
    let revision = f.model.observe(&replacement).unwrap().revision();
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, revision, insert(b'x'))
            .err(),
        Some(bridge_error::desktop(desktop_error::client_denied))
    );
    f.model.revoke_client(&f.control, &f.producer).unwrap();
    assert_eq!(
        f.bridge.observe(&f.model, &replacement).err(),
        Some(bridge_error::desktop(desktop_error::client_denied))
    );
}

#[test]
fn focus_modal_and_minimization_fence_actions_without_editor_mutation() {
    let mut f = fixture::new();
    for command in [
        desktop_command::toggle_launcher,
        desktop_command::open_appearance,
        desktop_command::minimize(f.bridge.window()),
    ] {
        let before = f.owner.session().view();
        let ticket = f.action(insert(b'x'));
        f.desktop_action(command);
        let rejected = f
            .bridge
            .consume_action(&f.model, &mut f.owner, ticket)
            .err()
            .unwrap();
        assert_eq!(
            rejected.error(),
            bridge_error::desktop(desktop_error::stale_revision)
        );
        assert!(rejected.into_ticket().is_none());
        assert_eq!(f.owner.session().view(), before);
        assert_eq!(
            f.bridge
                .prepare_action(&f.model, &f.input, f.revision(), insert(b'x'))
                .err(),
            Some(bridge_error::not_focused)
        );
        if f.model.observe(&f.human).unwrap().launcher_open() {
            f.desktop_action(desktop_command::dismiss_launcher);
        }
        f.desktop_action(desktop_command::open_native_editor(
            f.bridge.displayed.descriptor(),
        ));
    }
    assert_eq!(f.consume(insert(b'a')).origin, client_origin::human);
    f.sync();
    let machine = f
        .bridge
        .bind_input(&f.model, &f.control, &f.producer)
        .unwrap();
    let ticket = f
        .bridge
        .prepare_action(&f.model, &machine, f.revision(), insert(b'b'))
        .unwrap();
    let result = f
        .bridge
        .consume_action(&f.model, &mut f.owner, ticket)
        .ok()
        .unwrap();
    assert_eq!(result.origin, client_origin::machine);
    assert_eq!(result.effect, native_editor_effect::edited);
    assert_eq!(f.owner.session().view().text(), b"ab");
}

#[test]
fn actions_are_one_shot_and_wrong_bridge_preserves_the_original_ticket() {
    let mut f = fixture::new();
    let mut foreign = fixture::new();
    let ticket = f.action(insert(b'a'));
    let forged = duplicate(&ticket);
    let rejected = foreign
        .bridge
        .consume_action(&foreign.model, &mut foreign.owner, ticket)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::stale_action);
    let ticket = rejected.into_ticket().unwrap();
    assert!(f.bridge.in_flight.is_some());
    assert!(foreign.bridge.in_flight.is_none());
    f.bridge
        .consume_action(&f.model, &mut f.owner, ticket)
        .ok()
        .unwrap();
    let rejected = f
        .bridge
        .consume_action(&f.model, &mut f.owner, forged)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::stale_action);
    assert!(rejected.into_ticket().is_some());
    assert_eq!(f.owner.session().view().text(), b"a");
    f.sync();
    let ticket = f.action(insert(b'b'));
    let cancelled = duplicate(&ticket);
    f.bridge.cancel_action(ticket).ok().unwrap();
    let current = f.action(insert(b'c'));
    let sequence = f.bridge.in_flight;
    let rejected = f.bridge.cancel_action(cancelled).err().unwrap();
    assert_eq!(rejected.error(), bridge_error::stale_action);
    assert!(rejected.into_ticket().is_some());
    assert_eq!(f.bridge.in_flight, sequence);
    f.bridge
        .consume_action(&f.model, &mut f.owner, current)
        .ok()
        .unwrap();
    assert_eq!(f.owner.session().view().text(), b"ac");
}

#[test]
fn wrong_owner_consumes_an_exact_permit_without_touching_either_session() {
    let mut f = fixture::new();
    let mut wrong = owner(b"Other");
    let ticket = f.action(insert(b'x'));
    let before = wrong.session().view();
    let rejected = f
        .bridge
        .consume_action(&f.model, &mut wrong, ticket)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::wrong_binding);
    assert!(rejected.into_ticket().is_none());
    assert_eq!(wrong.session().view(), before);
    assert_eq!(f.owner.session().view().text(), b"");
    assert!(f.bridge.in_flight.is_none());
}

#[test]
fn a_service_ack_invalidates_previously_admitted_input_until_observation_resync() {
    let mut f = fixture::new();
    f.consume(insert(b'a'));
    f.sync();
    let native_editor_effect::save(snapshot) = f.consume(native_editor_action::save).effect else {
        panic!("save snapshot");
    };
    f.sync();
    let ticket = f.action(insert(b'b'));
    f.owner
        .acknowledge_save(save_acknowledgement {
            document: snapshot.document(),
            generation: snapshot.generation(),
            request: snapshot.request(),
            base_version: snapshot.base_version(),
            revision: snapshot.revision(),
            version: snapshot.base_version() + 1,
            text: snapshot.text(),
        })
        .unwrap();
    let acknowledged = f.owner.session().view();
    let rejected = f
        .bridge
        .consume_action(&f.model, &mut f.owner, ticket)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::stale_observation);
    assert!(rejected.into_ticket().is_none());
    assert_eq!(f.owner.session().view(), acknowledged);
    assert!(!acknowledged.is_dirty());
    f.sync();
    f.consume(insert(b'b'));
    assert_eq!(f.owner.session().view().text(), b"ab");
    assert!(f.owner.session().view().is_dirty());
}

#[test]
fn malformed_forged_stale_and_wrong_owner_frames_preserve_published_state() {
    let mut f = fixture::new();
    let audit = f.bridge.displayed.audit;
    for index in [0, 4, 57, 127] {
        let mut malformed = audit;
        malformed[index] ^= 1;
        assert_eq!(
            native_editor_frame::from_audit(&malformed).err(),
            Some(bridge_error::invalid_observation)
        );
    }
    assert_eq!(
        native_editor_frame::from_audit(&audit[..127]).err(),
        Some(bridge_error::invalid_observation)
    );
    let mut malformed = audit;
    malformed[8..16].fill(0);
    assert_eq!(
        native_editor_frame::from_audit(&malformed).err(),
        Some(bridge_error::invalid_observation)
    );
    let old_descriptor = f.bridge.displayed.descriptor();
    let revision = f.revision();
    let frame = f.owner.frame().unwrap();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::stale_observation);
    let frame = rejected.into_frame();
    let wrong = owner(b"");
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &wrong, revision, frame)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::wrong_binding);
    let mut forged = audit;
    forged[8..16].copy_from_slice(&2_u64.to_le_bytes());
    let frame = native_editor_frame::from_audit(&forged).unwrap();
    let forged_descriptor = frame.descriptor();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::observation_mismatch);
    assert_eq!(rejected.into_frame().descriptor(), forged_descriptor);
    assert_eq!(f.bridge.displayed.descriptor(), old_descriptor);
    assert_eq!(f.revision(), revision);
    assert!(f.model.pending.is_none());
}

#[test]
fn cancellation_and_failed_publication_retain_the_complete_update_for_retry() {
    let mut f = fixture::new();
    let foreign = fixture::new();
    let original_descriptor = f.bridge.displayed.descriptor();
    f.consume(insert(b'a'));
    let update = f.update();
    let descriptor = update.frame().descriptor();
    let audit = update.frame.audit;
    let row = update.frame().pixels().row(0).unwrap().to_vec();
    let rejected = f
        .bridge
        .cancel(&mut f.model, &foreign.control, update)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error(),
        bridge_error::desktop(desktop_error::control_denied)
    );
    let frame = f
        .bridge
        .cancel(&mut f.model, &f.control, rejected.into_update())
        .ok()
        .unwrap();
    assert_eq!(frame.descriptor(), descriptor);
    assert_eq!(frame.audit, audit);
    assert_eq!(frame.pixels().row(0).unwrap(), row);
    assert_eq!(f.bridge.displayed.descriptor(), original_descriptor);
    assert_eq!(f.owner.session().view().text(), b"a");
    let revision = f.revision();
    let update = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .ok()
        .unwrap();
    f.bridge.begin(&mut f.model, &f.control, &update).unwrap();
    let rejected = f
        .bridge
        .publish(&mut f.model, &f.control, update)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error(),
        bridge_error::desktop(desktop_error::wrong_phase)
    );
    let update = rejected.into_update();
    assert_eq!(update.frame().descriptor(), descriptor);
    assert_eq!(update.frame.audit, audit);
    assert_eq!(update.frame().pixels().row(0).unwrap(), row);
    assert_eq!(
        f.bridge.observe(&f.model, &f.human).err(),
        Some(bridge_error::desktop(desktop_error::unavailable))
    );
    f.bridge
        .attest_presented(&mut f.model, &f.control, &update)
        .unwrap();
    let rejected = f
        .bridge
        .publish(&mut f.model, &foreign.control, update)
        .err()
        .unwrap();
    assert_eq!(
        rejected.error(),
        bridge_error::desktop(desktop_error::control_denied)
    );
    let receipt = f
        .bridge
        .publish(&mut f.model, &f.control, rejected.into_update())
        .ok()
        .unwrap();
    assert_eq!(
        receipt.observation_sequence,
        f.owner.session().view().sequence()
    );
    assert_eq!(f.bridge.displayed.descriptor(), descriptor);
    assert_eq!(f.bridge.observe(&f.model, &f.human).unwrap().text, b"a");
}

#[test]
fn revoked_producer_can_cancel_before_presentation_but_not_publish_after_it() {
    for begin in [false, true] {
        let mut f = fixture::new();
        f.consume(insert(b'a'));
        let update = f.update();
        let descriptor = update.frame().descriptor();
        if begin {
            f.bridge.begin(&mut f.model, &f.control, &update).unwrap();
        }
        f.model.revoke_client(&f.control, &f.producer).unwrap();
        let rejected = f
            .bridge
            .publish(&mut f.model, &f.control, update)
            .err()
            .unwrap();
        assert_eq!(
            rejected.error(),
            bridge_error::desktop(desktop_error::client_denied)
        );
        let update = rejected.into_update();
        assert_eq!(update.frame().descriptor(), descriptor);
        let result = f.bridge.cancel(&mut f.model, &f.control, update);
        if begin {
            let rejected = result.err().unwrap();
            assert_eq!(
                rejected.error(),
                bridge_error::desktop(desktop_error::wrong_phase)
            );
            assert_eq!(rejected.into_update().frame().descriptor(), descriptor);
            assert_eq!(f.model.phase(), presentation_phase::presenting);
        } else {
            assert_eq!(result.ok().unwrap().descriptor(), descriptor);
            assert!(f.model.pending.is_none());
        }
        assert_eq!(f.owner.session().view().text(), b"a");
    }
}

#[test]
fn action_and_update_backpressure_never_discards_an_owned_frame_or_permit() {
    let mut f = fixture::new();
    let ticket = f.action(insert(b'a'));
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), insert(b'b'))
            .err(),
        Some(bridge_error::backpressure)
    );
    let frame = f.owner.frame().unwrap();
    let descriptor = frame.descriptor();
    let revision = f.revision();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::backpressure);
    assert_eq!(rejected.into_frame().descriptor(), descriptor);
    f.bridge
        .consume_action(&f.model, &mut f.owner, ticket)
        .ok()
        .unwrap();
    let update = f.update();
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), insert(b'b'))
            .err(),
        Some(bridge_error::backpressure)
    );
    f.bridge
        .cancel(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert!(f.bridge.in_flight.is_none());
    assert_eq!(f.owner.session().view().text(), b"a");
}

#[test]
fn exhausted_action_and_surface_sequences_fail_without_partial_mutation() {
    let mut f = fixture::new();
    f.bridge.next_action = u64::MAX;
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), insert(b'a'))
            .err(),
        Some(bridge_error::sequence_exhausted)
    );
    assert_eq!(f.bridge.next_action, u64::MAX);
    assert!(f.bridge.in_flight.is_none());
    assert_eq!(f.owner.session().view().sequence(), 1);
    f.bridge.next_action = 1;
    f.consume(insert(b'a'));
    f.bridge.surface_sequence = u64::MAX;
    f.model
        .committed
        .window_mut(f.bridge.window())
        .unwrap()
        .surface_sequence = u64::MAX;
    let revision = f.revision();
    let frame = f.owner.frame().unwrap();
    let descriptor = frame.descriptor();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::sequence_exhausted);
    assert_eq!(rejected.into_frame().descriptor(), descriptor);
    assert!(f.model.pending.is_none());
    assert_eq!(f.revision(), revision);
}

#[test]
fn terminal_withdrawal_and_fresh_rebind_invalidate_old_routes_and_permits() {
    let mut f = fixture::new();
    let foreign = fixture::new();
    let old_window = f.bridge.window();
    let old_input = native_editor_input {
        key: f.input.key,
        slot: f.input.slot,
        generation: f.input.generation,
    };
    let action = f.action(insert(b'x'));
    let revision = f.revision();
    assert_eq!(
        f.model
            .prepare_native_editor_withdrawal(&foreign.control, &f.human, revision, old_window)
            .err(),
        Some(desktop_error::control_denied)
    );
    let withdrawal = f
        .model
        .prepare_native_editor_withdrawal(&f.control, &f.human, revision, old_window)
        .unwrap();
    publish(&mut f.model, &f.control, withdrawal);
    assert!(f.bridge.observe(&f.model, &f.human).is_err());
    assert!(
        f.bridge
            .consume_action(&f.model, &mut f.owner, action)
            .is_err()
    );
    assert_eq!(f.owner.session().view().text(), b"");
    let mut fresh = owner(b"Recovered");
    let frame = fresh.frame().unwrap();
    let new_window = f
        .desktop_action(desktop_command::open_native_editor(frame.descriptor()))
        .created_window()
        .unwrap();
    assert_ne!(new_window, old_window);
    let mut replacement = f
        .model
        .bind_native_editor(
            &f.control,
            &f.producer,
            new_window,
            &mut fresh,
            frame,
            editor_disclosure::public_fixture,
        )
        .ok()
        .unwrap();
    assert_ne!(replacement.key.generation, f.bridge.key.generation);
    assert_eq!(
        replacement
            .prepare_action(&f.model, &old_input, f.revision(), insert(b'x'))
            .err(),
        Some(bridge_error::wrong_binding)
    );
    assert_eq!(
        f.bridge.observe(&f.model, &f.human).err(),
        Some(bridge_error::wrong_binding)
    );
    assert_eq!(
        replacement.observe(&f.model, &f.human).unwrap().text,
        b"Recovered"
    );
}

#[test]
fn binding_requires_unbound_owner_exact_presented_frame_and_available_generation() {
    let mut f = fixture::new();
    let frame = f.owner.frame().unwrap();
    let descriptor = frame.descriptor();
    let rejected = f
        .model
        .bind_native_editor(
            &f.control,
            &f.producer,
            f.bridge.window(),
            &mut f.owner,
            frame,
            editor_disclosure::public_fixture,
        )
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::owner_already_bound);
    assert_eq!(rejected.into_frame().descriptor(), descriptor);

    let mut fresh = owner(b"Different");
    let frame = fresh.frame().unwrap();
    let rejected = f
        .model
        .bind_native_editor(
            &f.control,
            &f.producer,
            f.bridge.window(),
            &mut fresh,
            frame,
            editor_disclosure::public_fixture,
        )
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::observation_mismatch);
    assert!(fresh.key.is_none());
    let frame = rejected.into_frame();
    let revision = f.revision();
    let withdrawal = f
        .model
        .prepare_native_editor_withdrawal(&f.control, &f.human, revision, f.bridge.window())
        .unwrap();
    publish(&mut f.model, &f.control, withdrawal);
    let window = f
        .desktop_action(desktop_command::open_native_editor(frame.descriptor()))
        .created_window()
        .unwrap();
    f.model.native_editor_binding = u64::MAX;
    let descriptor = frame.descriptor();
    let rejected = f
        .model
        .bind_native_editor(
            &f.control,
            &f.producer,
            window,
            &mut fresh,
            frame,
            editor_disclosure::public_fixture,
        )
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::sequence_exhausted);
    assert_eq!(rejected.into_frame().descriptor(), descriptor);
    assert_eq!(f.model.native_editor_binding, u64::MAX);
    assert!(fresh.key.is_none());
    assert!(f.model.pending.is_none());
    assert_eq!(
        f.model
            .observe(&f.human)
            .unwrap()
            .window(window)
            .unwrap()
            .surface(),
        Some(descriptor)
    );
}

#[test]
fn foreign_publication_and_pending_withdrawal_preserve_the_original_transaction() {
    let mut f = fixture::new();
    let mut foreign = fixture::new();
    f.consume(insert(b'a'));
    let update = f.update();
    let descriptor = update.frame().descriptor();
    let revision = f.revision();
    assert_eq!(
        f.model
            .prepare_native_editor_withdrawal(&f.control, &f.human, revision, f.bridge.window(),)
            .err(),
        Some(desktop_error::backpressure)
    );
    let rejected = foreign
        .bridge
        .publish(&mut foreign.model, &foreign.control, update)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::wrong_binding);
    let update = rejected.into_update();
    assert_eq!(update.frame().descriptor(), descriptor);
    let rejected = foreign
        .bridge
        .cancel(&mut foreign.model, &foreign.control, update)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::wrong_binding);
    assert!(foreign.model.pending.is_none());
    let frame = f
        .bridge
        .cancel(&mut f.model, &f.control, rejected.into_update())
        .ok()
        .unwrap();
    assert_eq!(frame.descriptor(), descriptor);
    assert_eq!(f.owner.session().view().text(), b"a");
    assert_eq!(f.bridge.observe(&f.model, &f.human).unwrap().text, b"");
}

#[test]
fn application_close_removes_window_only_with_its_exact_published_observation() {
    let mut f = fixture::new();
    let window = f.bridge.window();
    f.consume(insert(b'a'));
    f.sync();
    assert_eq!(
        f.model
            .prepare(&f.human, f.revision(), desktop_command::close(window))
            .err(),
        Some(desktop_error::native_editor_close_required)
    );
    assert_eq!(
        f.consume(native_editor_action::request_close).effect,
        native_editor_effect::close(interactive_close_result::confirmation_required)
    );
    f.sync();
    assert_eq!(
        f.consume(native_editor_action::request_close).effect,
        native_editor_effect::close(interactive_close_result::closed)
    );
    let update = f.update();
    assert!(update.frame().observation().closed);
    assert!(
        f.bridge
            .candidate(&f.model, &update)
            .unwrap()
            .window(window)
            .is_none()
    );
    assert!(f.model.observe(&f.human).unwrap().window(window).is_some());
    let frame = f
        .bridge
        .cancel(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert!(f.owner.session().view().is_closed());
    assert!(!f.bridge.retired);
    let revision = f.revision();
    let update = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, frame)
        .ok()
        .unwrap();
    f.bridge.begin(&mut f.model, &f.control, &update).unwrap();
    f.bridge
        .attest_presented(&mut f.model, &f.control, &update)
        .unwrap();
    let receipt = f
        .bridge
        .publish(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert!(receipt.closed);
    assert!(f.model.observe(&f.human).unwrap().window(window).is_none());
    assert_eq!(
        f.bridge.observe(&f.model, &f.human).err(),
        Some(bridge_error::retired)
    );
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), insert(b'x'))
            .err(),
        Some(bridge_error::retired)
    );
}
