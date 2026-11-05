#![cfg(feature = "native_editor_remote")]
#![allow(non_camel_case_types)]

//! Host integration of copied packets between separate workspace/editor owners.
//! The controller below authenticates live local capabilities before calling the
//! checked transport seam. It is not evidence of a native gate or device driver.

use anomalous_desktop_compositor::{
    client_viewport, composition_frame, frame_height, frame_width, row_byte_count, surface_binding,
};
use anomalous_desktop_workspace::native_bridge::native_coordinator::{
    native_coordinator, native_role_binding, native_role_owners, native_route, native_route_access,
};
use anomalous_desktop_workspace::native_bridge::native_remote::{
    remote_action_result, remote_editor_bridge, remote_route_binding,
};
use anomalous_desktop_workspace::native_bridge::{
    bridge_error, editor_disclosure, native_editor_frame, native_editor_input, native_editor_update,
};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, desktop, desktop_command, desktop_control,
    desktop_error, render_view, update_ticket, window_id,
};
use anomalous_document_protocol::durable::{
    disk_identity, durable_store, format_disposable, io_error, recover, sector_count, sector_io,
    sector_size,
};
use anomalous_document_protocol::{client_identity, operation, record};
use anomalous_editor_contract::remote_protocol::{
    remote_action, remote_editor_session, remote_effect, remote_execution, remote_message,
    remote_outcome, remote_request,
};
use anomalous_editor_contract::{
    interactive_editor_action as edit, interactive_editor_session, interactive_frame_byte_count,
    save_acknowledgement, save_outcome, save_snapshot,
};
use anomalous_window_surface::{borrowed_frame, frame};

type content<'a> = native_editor_frame<borrowed_frame<'a, interactive_frame_byte_count>>;
type update<'a> = native_editor_update<borrowed_frame<'a, interactive_frame_byte_count>>;

fn slot() -> Box<[u8; interactive_frame_byte_count]> {
    vec![0; interactive_frame_byte_count]
        .into_boxed_slice()
        .try_into()
        .ok()
        .unwrap()
}

struct memory_disk {
    volatile: [[u8; sector_size]; sector_count],
    persisted: [[u8; sector_size]; sector_count],
}
impl memory_disk {
    fn blank() -> Self {
        Self {
            volatile: [[0; sector_size]; sector_count],
            persisted: [[0; sector_size]; sector_count],
        }
    }
    fn crash(&mut self) {
        self.volatile = self.persisted;
    }
}
impl sector_io for memory_disk {
    fn read_sector(&mut self, sector: u64, output: &mut [u8; sector_size]) -> Result<(), io_error> {
        *output = *self.volatile.get(sector as usize).ok_or(io_error::failed)?;
        Ok(())
    }
    fn write_sector(&mut self, sector: u64, bytes: &[u8; sector_size]) -> Result<(), io_error> {
        *self
            .volatile
            .get_mut(sector as usize)
            .ok_or(io_error::failed)? = *bytes;
        Ok(())
    }
    fn flush(&mut self) -> Result<(), io_error> {
        self.persisted = self.volatile;
        Ok(())
    }
}

fn compose(view: &render_view<'_>, content: Option<(window_id, &frame<'_>)>) -> Vec<u8> {
    let bindings: Vec<_> = content
        .map(|(window, pixels)| surface_binding::new(window, pixels))
        .into_iter()
        .collect();
    let composition = composition_frame::try_new(view, &bindings).unwrap();
    let mut pixels = vec![0x5a; row_byte_count * frame_height as usize];
    for (row, output) in pixels.chunks_exact_mut(row_byte_count).enumerate() {
        composition.compose_row(row as u32, output).unwrap();
    }
    pixels
}
fn present_desktop(
    model: &mut desktop,
    control: &desktop_control,
    ticket: update_ticket,
    content: Option<(window_id, &frame<'_>)>,
) -> Vec<u8> {
    compose(&model.candidate(&ticket).unwrap(), content);
    model.begin(control, &ticket).unwrap();
    let pixels = compose(&model.candidate(&ticket).unwrap(), content);
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap();
    pixels
}

// The host controller owns both endpoints, just as the native coordinator will.
// Neither endpoint gets references into the other endpoint's editor or desktop.
struct fixture<'a> {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    machine: client_endpoint,
    human_input: native_editor_input,
    machine_input: native_editor_input,
    coordinator: native_coordinator,
    roles: native_role_owners,
    binding: remote_route_binding,
    bridge: remote_editor_bridge<borrowed_frame<'a, interactive_frame_byte_count>>,
    editor: remote_editor_session,
    next_delivery: u64,
    available: Option<&'a mut [u8; interactive_frame_byte_count]>,
    addresses: [usize; 2],
    displayed: Vec<u8>,
}
impl<'a> fixture<'a> {
    fn new(
        store: &mut durable_store<'_, memory_disk>,
        authority: client_identity,
        first: &'a mut [u8; interactive_frame_byte_count],
        second: &'a mut [u8; interactive_frame_byte_count],
    ) -> Self {
        let addresses = [first.as_ptr() as usize, second.as_ptr() as usize];
        assert_ne!(addresses[0], addresses[1]);
        let (mut model, control, initial) = desktop::new(frame_width, frame_height).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        let machine = model
            .bind_client(&control, client_origin::machine, client_grants::all)
            .unwrap();
        let (coordinator, roles) = native_coordinator::provision(
            &model,
            &control,
            [
                native_role_binding::try_new(100, 1).unwrap(),
                native_role_binding::try_new(authority.task(), authority.generation()).unwrap(),
                native_role_binding::try_new(200, 1).unwrap(),
            ],
        )
        .unwrap();
        let binding = remote_route_binding::try_new(
            0x7101,
            [100, 1],
            [authority.task(), authority.generation()],
            [10, 3],
            [11, 3],
        )
        .unwrap();
        authorize(&coordinator, &roles, native_route::document_request);
        let open = record::decode(&record::open(store.document(), 1).unwrap().encode()).unwrap();
        let opened = store.execute(authority, &open).unwrap();
        authorize(&coordinator, &roles, native_route::document_reply);
        let opened = record::decode(&opened.encode()).unwrap();
        assert_eq!(opened.kind(), operation::opened);
        let editor = remote_editor_session::new(
            0x7101,
            interactive_editor_session::from_open(
                opened.document(),
                opened.generation(),
                opened.version(),
                opened.text(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut port = model.provision_remote_editor(&control, binding).unwrap();
        authorize(&coordinator, &roles, native_route::editor_observation);
        let initial_packet = editor.observation().encode().to_vec();
        port.admit_initial_checked(binding, 1, &initial_packet)
            .unwrap();
        let first = content::from_audit_in(port.audit_bytes().unwrap(), first)
            .ok()
            .unwrap();
        assert_eq!(
            first.pixels().row(0).unwrap().as_ptr() as usize,
            addresses[0]
        );
        present_desktop(&mut model, &control, initial, None);
        let revision = model.observe(&human).unwrap().revision();
        let ticket = model
            .prepare(
                &human,
                revision,
                desktop_command::open_native_editor(first.descriptor()),
            )
            .unwrap();
        let window = model
            .candidate(&ticket)
            .unwrap()
            .windows()
            .next()
            .unwrap()
            .id();
        let displayed = present_desktop(
            &mut model,
            &control,
            ticket,
            Some((window, &first.pixels())),
        );
        let bridge = model
            .bind_remote_editor(
                &control,
                &human,
                window,
                port,
                first,
                editor_disclosure::public_fixture,
            )
            .ok()
            .unwrap();
        let human_input = bridge.bind_input(&model, &control, &human).unwrap();
        let machine_input = bridge.bind_input(&model, &control, &machine).unwrap();
        Self {
            model,
            control,
            human,
            machine,
            human_input,
            machine_input,
            coordinator,
            roles,
            binding,
            bridge,
            editor,
            next_delivery: 2,
            available: Some(second),
            addresses,
            displayed,
        }
    }
    fn revision(&self) -> u64 {
        self.model.observe(&self.human).unwrap().revision()
    }
    fn submit(&mut self, action: remote_action, machine: bool) -> Vec<u8> {
        let revision = self.revision();
        self.bridge
            .submit_action(
                &mut self.model,
                if machine {
                    &self.machine_input
                } else {
                    &self.human_input
                },
                revision,
                action,
            )
            .unwrap()
            .encode()
            .to_vec()
    }
    fn execute(&mut self, bytes: &[u8]) -> remote_execution {
        authorize(&self.coordinator, &self.roles, native_route::editor_action);
        let copied = bytes.to_vec();
        self.editor
            .execute(remote_request::decode(&copied).unwrap())
            .unwrap()
    }
    fn deliver(&mut self, bytes: &[u8]) -> Option<remote_action_result> {
        authorize(
            &self.coordinator,
            &self.roles,
            native_route::editor_observation,
        );
        let copied = bytes.to_vec();
        let result = self
            .bridge
            .admit_checked_message(&mut self.model, self.binding, self.next_delivery, &copied)
            .unwrap();
        self.next_delivery += 1;
        result
    }
    fn drain(&mut self) {
        let bytes = self.editor.observation().encode().to_vec();
        assert!(self.deliver(&bytes).is_none());
    }
    fn action(&mut self, action: remote_action, machine: bool) -> remote_effect {
        let packet = self.submit(action, machine);
        let execution = self.execute(&packet);
        self.assert_fenced();
        let result = self.deliver(&execution.message.encode()).unwrap();
        assert_eq!(
            result.origin,
            if machine {
                client_origin::machine
            } else {
                client_origin::human
            }
        );
        execution.effect
    }
    fn edit(&mut self, action: edit) {
        assert!(matches!(
            self.action(remote_action::edit(action), false),
            remote_effect::none
        ));
        self.publish_latest();
    }
    fn save(&mut self) -> save_snapshot {
        let remote_effect::save(snapshot) = self.action(remote_action::save, false) else {
            panic!("save snapshot belongs to the editor endpoint");
        };
        self.publish_latest();
        snapshot
    }
    fn request(
        &self,
        store: &mut durable_store<'_, memory_disk>,
        snapshot: &save_snapshot,
    ) -> record {
        authorize(
            &self.coordinator,
            &self.roles,
            native_route::document_request,
        );
        let request = record::save(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
            snapshot.revision(),
            snapshot.text(),
        )
        .unwrap();
        let copied = record::decode(&request.encode()).unwrap();
        let identity = self.roles.editor.binding();
        store
            .execute(
                client_identity::try_new(identity.slot(), identity.generation()).unwrap(),
                &copied,
            )
            .unwrap()
    }
    fn acknowledge(&mut self, reply: &record) {
        authorize(&self.coordinator, &self.roles, native_route::document_reply);
        let copied = record::decode(&reply.encode()).unwrap();
        assert_eq!(copied.kind(), operation::saved);
        self.editor
            .acknowledge_save(save_acknowledgement {
                document: copied.document(),
                generation: copied.generation(),
                request: copied.request(),
                base_version: copied.base_version(),
                revision: copied.revision(),
                version: copied.version(),
                text: copied.text(),
            })
            .unwrap();
    }
    fn prepare_latest(&mut self) -> update<'a> {
        let buffer = self.available.take().unwrap();
        let address = buffer.as_ptr() as usize;
        assert!(self.addresses.contains(&address));
        let frame = content::from_audit_in(self.bridge.latest_audit(), buffer)
            .ok()
            .unwrap();
        assert_eq!(frame.pixels().row(0).unwrap().as_ptr() as usize, address);
        let revision = self.revision();
        self.bridge
            .prepare_update(&mut self.model, revision, frame)
            .ok()
            .unwrap()
    }
    fn candidate_pixels(&self, ticket: &update<'a>) -> Vec<u8> {
        let view = self.bridge.candidate(&self.model, ticket).unwrap();
        let pixels = ticket.frame().pixels();
        let content = view
            .window(self.bridge.window())
            .map(|_| (self.bridge.window(), &pixels));
        compose(&view, content)
    }
    fn publish(&mut self, ticket: update<'a>) {
        let before = self.candidate_pixels(&ticket);
        self.bridge
            .begin(&mut self.model, &self.control, &ticket)
            .unwrap();
        let pixels = self.candidate_pixels(&ticket);
        assert_eq!(pixels, before);
        self.bridge
            .attest_presented(&mut self.model, &self.control, &ticket)
            .unwrap();
        let (_, previous) = self
            .bridge
            .publish_recycling(&mut self.model, &self.control, ticket)
            .ok()
            .unwrap();
        let returned = previous.into_buffer();
        assert!(self.addresses.contains(&(returned.as_ptr() as usize)));
        assert!(self.available.is_none());
        self.available = Some(returned);
        self.displayed = pixels;
    }
    fn publish_latest(&mut self) {
        let update = self.prepare_latest();
        self.publish(update);
    }
    fn committed_content(&self) -> (usize, Vec<u8>) {
        let pixels = self
            .bridge
            .presented_frame(&self.model, &self.human)
            .unwrap();
        let mut bytes = Vec::with_capacity(interactive_frame_byte_count);
        for row in 0..pixels.descriptor().height() {
            bytes.extend_from_slice(pixels.row(row).unwrap());
        }
        (pixels.row(0).unwrap().as_ptr() as usize, bytes)
    }
    #[allow(clippy::too_many_arguments)]
    fn expect(
        &self,
        text: &[u8],
        sequence: u64,
        revision: u64,
        version: u64,
        dirty: bool,
        outcome: u8,
        pending: (u64, u64),
    ) {
        let human = self.bridge.observe(&self.model, &self.human).unwrap();
        let machine = self.bridge.observe(&self.model, &self.machine).unwrap();
        assert_eq!(human.encode().unwrap(), machine.encode().unwrap());
        assert_eq!(human.text, text);
        assert_eq!(
            (human.sequence, human.revision, human.version),
            (sequence, revision, version)
        );
        assert_eq!((human.dirty, human.outcome), (dirty, outcome));
        assert_eq!((human.pending_request, human.pending_revision), pending);
    }
    fn assert_fenced(&mut self) {
        let revision = self.revision();
        let window = self.bridge.window();
        for command in [
            desktop_command::focus(window),
            desktop_command::minimize(window),
            desktop_command::toggle_launcher,
            desktop_command::close(window),
        ] {
            for client in [&self.human, &self.machine] {
                assert_eq!(
                    self.model.prepare(client, revision, command).err(),
                    Some(desktop_error::backpressure)
                );
            }
        }
        assert_eq!(
            self.model.revoke_client(&self.control, &self.human),
            Err(desktop_error::backpressure)
        );
        assert_eq!(
            self.model
                .bind_client(&self.control, client_origin::machine, client_grants::all)
                .err(),
            Some(desktop_error::backpressure)
        );
        assert_eq!(self.revision(), revision);
    }
}

fn authorize(coordinator: &native_coordinator, roles: &native_role_owners, route: native_route) {
    let (sender, receiver) = match route {
        native_route::editor_action => (&roles.workspace, &roles.editor),
        native_route::editor_observation => (&roles.editor, &roles.workspace),
        native_route::document_request => (&roles.editor, &roles.document),
        native_route::document_reply => (&roles.document, &roles.editor),
    };
    coordinator
        .authorize_route(sender, route, native_route_access::send)
        .unwrap();
    coordinator
        .authorize_route(receiver, route, native_route_access::receive)
        .unwrap();
}

fn assert_c_glyph_and_composition(f: &fixture<'_>) {
    let source = f.bridge.presented_frame(&f.model, &f.human).unwrap();
    // Independent literal 5x7 C: no renderer call computes this expected glyph.
    // The collapsed caret is at x6, outside these five text columns.
    for (dy, bits) in [
        0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
    ]
    .into_iter()
    .enumerate()
    {
        for x in 0..5 {
            let expected = if bits & (1 << (4 - x)) == 0 {
                [18, 24, 34]
            } else {
                [236, 241, 247]
            };
            assert_eq!(
                &source.row((8 + dy) as u32).unwrap()[x * 3..x * 3 + 3],
                &expected
            );
        }
    }
    let view = f.model.observe(&f.human).unwrap();
    let viewport = client_viewport(&view, view.window(f.bridge.window()).unwrap());
    let descriptor = source.descriptor();
    for y in viewport.y..viewport.y + viewport.height.saturating_sub(12) {
        let sy =
            ((y - viewport.y) as u64 * descriptor.height() as u64 / viewport.height as u64) as u32;
        for x in viewport.x..viewport.x + viewport.width.saturating_sub(12) {
            let sx = ((x - viewport.x) as u64 * descriptor.width() as u64 / viewport.width as u64)
                as usize;
            let index = y as usize * row_byte_count + x as usize * 3;
            assert_eq!(
                &f.displayed[index..index + 3],
                &source.row(sy).unwrap()[sx * 3..sx * 3 + 3]
            );
        }
    }
}

#[test]
fn copied_save_results_preserve_dirty_edits_and_immutable_two_slot_publication() {
    let identity = disk_identity::try_new(0xc100, 1).unwrap();
    let authority = client_identity::try_new(1, 5).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut first = slot();
    let mut second = slot();
    {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let mut f = fixture::new(&mut store, authority, &mut first, &mut second);
        f.expect(b"A", 1, 1, 1, false, 0, (0, 0));
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let save_b = f.save();
        let saved_b = f.request(&mut store, &save_b);
        assert_eq!((store.stored_text(), store.version()), (b"B".as_slice(), 2));
        f.edit(edit::select_all);
        assert!(matches!(
            f.action(remote_action::edit(edit::insert(b'C')), false),
            remote_effect::none
        ));
        let ticket = f.prepare_latest();
        let candidate = f.candidate_pixels(&ticket);
        let address = ticket.frame().pixels().row(0).unwrap().as_ptr() as usize;
        assert_eq!(ticket.frame().observation().text, b"C");
        f.acknowledge(&saved_b);
        f.drain();
        assert_eq!(f.editor.view().version(), 2);
        assert!(f.editor.view().is_dirty());
        assert_eq!(f.candidate_pixels(&ticket), candidate);
        assert_eq!(
            ticket.frame().pixels().row(0).unwrap().as_ptr() as usize,
            address
        );
        f.publish(ticket);
        f.expect(b"C", 6, 3, 1, true, 1, (1, 2));
        // Cancellation returns the same spare allocation with its copied pixels.
        let ticket = f.prepare_latest();
        let address = ticket.frame().pixels().row(0).unwrap().as_ptr() as usize;
        let canceled = f
            .bridge
            .cancel(&mut f.model, &f.control, ticket)
            .ok()
            .unwrap();
        assert_eq!(canceled.pixels().row(0).unwrap().as_ptr() as usize, address);
        f.available = Some(canceled.into_buffer());
        f.publish_latest();
        f.expect(b"C", 7, 3, 2, true, 0, (0, 0));
        let save_c = f.save();
        assert_eq!(
            (save_c.request(), save_c.base_version(), save_c.text()),
            (2, 2, b"C".as_slice())
        );
        let saved_c = f.request(&mut store, &save_c);
        f.acknowledge(&saved_c);
        f.drain();
        f.publish_latest();
        f.expect(b"C", 9, 3, 3, false, 0, (0, 0));
        assert_c_glyph_and_composition(&f);
    }
    disk.crash();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"C".as_slice(), 3)
    );
}

#[test]
fn checked_delivery_keeps_future_observation_and_requires_exact_completion_to_release_focus() {
    let identity = disk_identity::try_new(0xc100, 2).unwrap();
    let authority = client_identity::try_new(2, 5).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
    let mut first = slot();
    let mut second = slot();
    let mut f = fixture::new(&mut store, authority, &mut first, &mut second);
    f.edit(edit::select_all);
    f.edit(edit::insert(b'B'));
    let snapshot = f.save();
    let saved = f.request(&mut store, &snapshot);

    let request = f.submit(remote_action::edit(edit::insert(b'x')), true);
    f.assert_fenced();
    let execution = f.execute(&request);
    assert_eq!(
        execution.message.outcome(),
        Some(remote_outcome::edit_applied)
    );
    let completion = execution.message.encode();
    // Delivery/ACK of request bytes and even executing the action do not resolve
    // workspace focus. Losing an owned copy of the result cannot resolve it.
    drop(request);
    drop(completion.to_vec());
    f.assert_fenced();
    f.acknowledge(&saved);
    f.drain();
    f.assert_fenced();
    assert_eq!(f.editor.view().text(), b"Bx");
    let newest = *f.bridge.latest_audit();
    let committed = f.displayed.clone();
    let committed_content = f.committed_content();
    let spare_address = f.available.as_ref().unwrap().as_ptr() as usize;
    let spare = f.available.as_ref().unwrap().to_vec();

    let foreign = remote_route_binding::try_new(
        f.binding.connection(),
        f.binding.workspace(),
        [authority.task(), authority.generation() + 1],
        f.binding.action_route(),
        f.binding.observation_route(),
    )
    .unwrap();
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, foreign, f.next_delivery, &completion,)
            .err(),
        Some(bridge_error::wrong_binding)
    );
    let mut malformed = completion;
    malformed[40] = 1; // Reserved remote-message header byte.
    assert!(
        f.bridge
            .admit_checked_message(&mut f.model, f.binding, f.next_delivery, &malformed,)
            .is_err()
    );
    let wrong = remote_message::try_completion(
        execution.message.connection(),
        execution.message.action_id() + 1,
        execution.message.expected_sequence(),
        remote_outcome::edit_applied,
        execution.message.audit_bytes(),
    )
    .unwrap()
    .encode();
    assert!(remote_message::decode(&wrong).is_ok());
    assert_eq!(
        f.bridge
            .admit_checked_message(&mut f.model, f.binding, f.next_delivery, &wrong,)
            .err(),
        Some(bridge_error::wrong_binding)
    );
    // Every rejection preserves both pixel slots, latest semantics and the
    // pending fence. The same transport sequence remains usable by the result.
    assert_eq!(f.bridge.latest_audit(), &newest);
    assert_eq!(f.displayed, committed);
    assert_eq!(f.committed_content(), committed_content);
    assert_eq!(
        f.available.as_ref().unwrap().as_ptr() as usize,
        spare_address
    );
    assert_eq!(f.available.as_ref().unwrap().as_slice(), spare);
    f.assert_fenced();
    let result = f.deliver(&completion).unwrap();
    assert_eq!(
        (result.origin, result.outcome),
        (client_origin::machine, remote_outcome::edit_applied)
    );
    assert_eq!(f.bridge.latest_audit(), &newest);
    f.publish_latest();
    f.expect(b"Bx", 6, 3, 2, true, 0, (0, 0));

    // A real service acknowledgement interposes before a later action executes.
    // The editor must consume that action ID with stale status and no text edit.
    let snapshot = f.save();
    let saved = f.request(&mut store, &snapshot);
    let request = f.submit(remote_action::edit(edit::insert(b'?')), false);
    f.acknowledge(&saved);
    let before = f.editor.view();
    let stale = f.execute(&request);
    assert_eq!(stale.message.outcome(), Some(remote_outcome::stale));
    assert_eq!(f.editor.view(), before);
    f.drain();
    f.assert_fenced();
    let result = f.deliver(&stale.message.encode()).unwrap();
    assert_eq!(
        (result.origin, result.outcome),
        (client_origin::human, remote_outcome::stale)
    );
    f.publish_latest();
    f.expect(b"Bx", 8, 3, 3, false, 0, (0, 0));
    // Both origins have the same live policy path, and both were fenced above.
    assert!(matches!(
        f.action(remote_action::edit(edit::insert(b'!')), false),
        remote_effect::none
    ));
    f.publish_latest();
    f.expect(b"Bx!", 9, 4, 3, true, 0, (0, 0));
    let revision = f.revision();
    let ticket = f
        .model
        .prepare(&f.machine, revision, desktop_command::toggle_launcher)
        .unwrap();
    f.model.cancel(&f.control, ticket).ok().unwrap();
}

#[test]
fn service_loss_preserves_unknown_save_and_local_edits_then_editor_retirement_withdraws_fence() {
    let identity = disk_identity::try_new(0xc100, 3).unwrap();
    let authority = client_identity::try_new(3, 5).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut first = slot();
    let mut second = slot();
    {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let mut f = fixture::new(&mut store, authority, &mut first, &mut second);
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let pending = f.save();
        let lost_reply = f.request(&mut store, &pending);
        f.edit(edit::select_all);
        f.edit(edit::insert(b'C'));
        let retirement = f.coordinator.retire(&f.roles.document).unwrap();
        assert_eq!(retirement.closed_routes, [false, false, true, true]);
        assert!(
            f.coordinator
                .authorize_route(
                    &f.roles.document,
                    native_route::document_reply,
                    native_route_access::send,
                )
                .is_err()
        );
        assert!(
            f.coordinator
                .authorize_route(
                    &f.roles.editor,
                    native_route::document_request,
                    native_route_access::send,
                )
                .is_err()
        );
        assert_eq!(lost_reply.text(), b"B");
        // The authenticated terminal service event authorizes unknown, never a
        // forged durable ACK. Workspace/editor routes remain usable.
        f.editor
            .mark_save_unknown(
                pending.document(),
                pending.generation(),
                pending.request(),
                pending.base_version(),
            )
            .unwrap();
        f.drain();
        f.publish_latest();
        f.expect(b"C", 7, 3, 1, true, 2, (1, 2));
        assert_eq!(f.editor.view().save_outcome(), save_outcome::unknown);
        f.edit(edit::insert(b'!'));
        f.expect(b"C!", 8, 4, 1, true, 2, (1, 2));
        let request = f.submit(remote_action::request_close, false);
        f.assert_fenced();
        let before = f.editor.view();
        let retirement = f.coordinator.retire(&f.roles.editor).unwrap();
        assert_eq!(retirement.closed_routes, [true, true, false, false]);
        assert!(
            f.coordinator
                .authorize_route(
                    &f.roles.workspace,
                    native_route::editor_action,
                    native_route_access::send,
                )
                .is_err()
        );
        drop(request);
        f.assert_fenced();
        f.bridge
            .withdraw_terminal(&mut f.model, &f.control, f.binding)
            .unwrap();
        assert_eq!(f.editor.view(), before);
        assert_eq!(f.editor.view().save_outcome(), save_outcome::unknown);
        assert!(!f.editor.view().is_closed());
        let revision = f.revision();
        let ticket = f
            .model
            .prepare_native_editor_withdrawal(&f.control, &f.human, revision, f.bridge.window())
            .unwrap();
        assert!(
            f.model
                .candidate(&ticket)
                .unwrap()
                .window(f.bridge.window())
                .is_none()
        );
        present_desktop(&mut f.model, &f.control, ticket, None);
        assert!(f.bridge.observe(&f.model, &f.human).is_err());
        assert_eq!((store.stored_text(), store.version()), (b"B".as_slice(), 2));
    }
    disk.crash();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"B".as_slice(), 2)
    );
}
