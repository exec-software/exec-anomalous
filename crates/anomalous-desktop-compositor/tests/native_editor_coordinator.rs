#![cfg(feature = "native_editor_bridge")]
#![allow(non_camel_case_types)]

use anomalous_desktop_compositor::{
    client_viewport, composition_frame, frame_height, frame_width, row_byte_count, surface_binding,
};
use anomalous_desktop_workspace::native_bridge::native_coordinator::{
    coordinator_error, native_coordinator, native_role, native_role_binding, native_role_owners,
    native_route, native_route_access,
};
use anomalous_desktop_workspace::native_bridge::{
    bridge_error, editor_disclosure, native_editor_action, native_editor_bridge,
    native_editor_effect, native_editor_frame, native_editor_input, native_editor_owner,
    native_editor_update,
};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, desktop, desktop_command, desktop_control,
    desktop_error, publication, render_view, update_ticket, window_id,
};
use anomalous_document_protocol::durable::{
    disk_identity, durable_store, format_disposable, io_error, recover, sector_count, sector_io,
    sector_size,
};
use anomalous_document_protocol::{client_identity, operation, record};
use anomalous_editor_contract::{
    interactive_editor_action as edit, interactive_editor_session, interactive_frame_byte_count,
    save_acknowledgement, save_outcome, save_snapshot,
};
use anomalous_window_surface::{borrowed_frame, frame};

// The host fixture owns allocation. The bridge borrows two exact disjoint slots;
// neither the desktop snapshot nor an action completion owns these arrays.
type pixels<'a> = borrowed_frame<'a, interactive_frame_byte_count>;
type content<'a> = native_editor_frame<pixels<'a>>;
type update<'a> = native_editor_update<pixels<'a>>;

fn pixel_buffer() -> Box<[u8; interactive_frame_byte_count]> {
    vec![0; interactive_frame_byte_count]
        .into_boxed_slice()
        .try_into()
        .ok()
        .unwrap()
}

// An independently controlled volatile/persisted sector backend. This exercises
// the real durable implementation without claiming device I/O or a VM reboot.
struct memory_disk {
    volatile: [[u8; sector_size]; sector_count],
    persisted: [[u8; sector_size]; sector_count],
    flushes: usize,
}
impl memory_disk {
    fn blank() -> Self {
        Self {
            volatile: [[0; sector_size]; sector_count],
            persisted: [[0; sector_size]; sector_count],
            flushes: 0,
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
        self.flushes += 1;
        Ok(())
    }
}

fn compose(view: &render_view<'_>, content: Option<(window_id, &frame<'_>)>) -> Vec<u8> {
    let bindings: Vec<_> = content
        .map(|(window, frame)| surface_binding::new(window, frame))
        .into_iter()
        .collect();
    let composition = composition_frame::try_new(view, &bindings).unwrap();
    let mut result = vec![0x5a; row_byte_count * frame_height as usize];
    for (row, output) in result.chunks_exact_mut(row_byte_count).enumerate() {
        composition.compose_row(row as u32, output).unwrap();
    }
    result
}

fn present_desktop(
    model: &mut desktop,
    control: &desktop_control,
    ticket: update_ticket,
    content: Option<(window_id, &frame<'_>)>,
) -> (publication, Vec<u8>) {
    compose(&model.candidate(&ticket).unwrap(), content);
    model.begin(control, &ticket).unwrap();
    let pixels = compose(&model.candidate(&ticket).unwrap(), content);
    model.attest_presented(control, &ticket).unwrap();
    (model.publish(control, ticket).ok().unwrap(), pixels)
}

struct fixture<'a> {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    machine: client_endpoint,
    owner: native_editor_owner,
    coordinator: native_coordinator,
    roles: native_role_owners,
    bridge: native_editor_bridge<pixels<'a>>,
    input: native_editor_input,
    machine_input: native_editor_input,
    available: Option<&'a mut [u8; interactive_frame_byte_count]>,
    addresses: [usize; 2],
    displayed: Vec<u8>,
}
impl<'a> fixture<'a> {
    fn from_store(
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
        let open = record::open(store.document(), 1).unwrap();
        let opened = routed_request(&coordinator, &roles, store, &open).unwrap();
        let opened = routed_reply(&coordinator, &roles, &opened).unwrap();
        assert_eq!(opened.kind(), operation::opened);
        let session = interactive_editor_session::from_open(
            opened.document(),
            opened.generation(),
            opened.version(),
            opened.text(),
        )
        .unwrap();
        let mut owner = native_editor_owner::new(session);
        let frame = owner.frame_in(first).ok().unwrap();
        assert_eq!(
            frame.pixels().row(0).unwrap().as_ptr() as usize,
            addresses[0]
        );
        present_desktop(&mut model, &control, initial, None);
        let revision = model.observe(&human).unwrap().revision();
        let ticket = model
            .prepare(
                &human,
                revision,
                desktop_command::open_native_editor(frame.descriptor()),
            )
            .unwrap();
        let window = model
            .candidate(&ticket)
            .unwrap()
            .windows()
            .next()
            .unwrap()
            .id();
        let (_, displayed) = present_desktop(
            &mut model,
            &control,
            ticket,
            Some((window, &frame.pixels())),
        );
        let bridge = model
            .bind_native_editor(
                &control,
                &human,
                window,
                &mut owner,
                frame,
                editor_disclosure::public_fixture,
            )
            .ok()
            .unwrap();
        let input = bridge.bind_input(&model, &control, &human).unwrap();
        let machine_input = bridge.bind_input(&model, &control, &machine).unwrap();
        Self {
            model,
            control,
            human,
            machine,
            owner,
            coordinator,
            roles,
            bridge,
            input,
            machine_input,
            available: Some(second),
            addresses,
            displayed,
        }
    }
    fn request(
        &self,
        store: &mut durable_store<'_, memory_disk>,
        request: &record,
    ) -> Result<record, coordinator_error> {
        routed_request(&self.coordinator, &self.roles, store, request)
    }
    fn acknowledge(&mut self, reply: &record) -> Result<(), coordinator_error> {
        let accepted = routed_reply(&self.coordinator, &self.roles, reply)?;
        self.owner
            .acknowledge_save(acknowledgement(&accepted))
            .unwrap();
        Ok(())
    }
    fn revision(&self) -> u64 {
        self.model.observe(&self.human).unwrap().revision()
    }
    fn action(&mut self, action: native_editor_action) -> native_editor_effect {
        let revision = self.revision();
        let submitted = self
            .bridge
            .submit_action(
                &mut self.model,
                &self.coordinator,
                &self.roles.workspace,
                &self.input,
                revision,
                action,
            )
            .unwrap();
        // Resolution needs only the editor owner. Desktop publication remains
        // fenced until the workspace consumes this exact completion.
        let completed = submitted
            .resolve(&self.coordinator, &self.roles.editor, &mut self.owner)
            .ok()
            .unwrap();
        let result = self
            .bridge
            .complete_action(
                &mut self.model,
                &self.coordinator,
                &self.roles.workspace,
                completed,
            )
            .ok()
            .unwrap()
            .unwrap();
        assert_eq!(result.origin, client_origin::human);
        result.effect
    }
    fn edit(&mut self, action: edit) {
        assert_eq!(
            self.action(native_editor_action::edit(action)),
            native_editor_effect::edited
        );
        self.publish_owner();
    }
    fn save(&mut self) -> save_snapshot {
        let native_editor_effect::save(snapshot) = self.action(native_editor_action::save) else {
            panic!("save must retain the exact snapshot")
        };
        self.publish_owner();
        snapshot
    }
    fn prepare_owner(&mut self) -> update<'a> {
        self.coordinator
            .authorize_route(
                &self.roles.editor,
                native_route::editor_observation,
                native_route_access::send,
            )
            .unwrap();
        self.coordinator
            .authorize_route(
                &self.roles.workspace,
                native_route::editor_observation,
                native_route_access::receive,
            )
            .unwrap();
        let buffer = self
            .available
            .take()
            .expect("exactly one spare slot before preparation");
        let address = buffer.as_ptr() as usize;
        assert!(self.addresses.contains(&address));
        let frame = self.owner.frame_in(buffer).ok().unwrap();
        assert_eq!(frame.pixels().row(0).unwrap().as_ptr() as usize, address);
        let revision = self.revision();
        self.bridge
            .prepare_update(&mut self.model, &self.owner, revision, frame)
            .ok()
            .unwrap()
    }
    fn candidate_pixels(&self, update: &update<'a>) -> Vec<u8> {
        let view = self.bridge.candidate(&self.model, update).unwrap();
        let frame = update.frame().pixels();
        let content = view
            .window(self.bridge.window())
            .map(|_| (self.bridge.window(), &frame));
        compose(&view, content)
    }
    fn present_update(&mut self, update: update<'a>) {
        assert!(self.available.is_none());
        let expected = self.candidate_pixels(&update);
        self.bridge
            .begin(&mut self.model, &self.control, &update)
            .unwrap();
        let pixels = self.candidate_pixels(&update);
        assert_eq!(pixels, expected);
        self.bridge
            .attest_presented(&mut self.model, &self.control, &update)
            .unwrap();
        let (published, previous) = self
            .bridge
            .publish_recycling(&mut self.model, &self.control, update)
            .ok()
            .unwrap();
        assert_eq!(published.desktop.revision(), self.revision());
        let reusable = previous.into_buffer();
        assert!(self.addresses.contains(&(reusable.as_ptr() as usize)));
        self.available = Some(reusable);
        self.displayed = pixels;
    }
    fn publish_owner(&mut self) {
        let update = self.prepare_owner();
        self.present_update(update);
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
        // Scheduling remains available while desktop mutations are fenced.
        assert_eq!(
            self.coordinator.next_runnable(),
            Some(native_role::workspace)
        );
        assert_eq!(self.coordinator.next_runnable(), Some(native_role::editor));
        assert_eq!(
            self.coordinator.next_runnable(),
            Some(native_role::document)
        );
        let revision = self.revision();
        let window = self.bridge.window();
        for command in [
            desktop_command::focus(window),
            desktop_command::minimize(window),
            desktop_command::toggle_launcher,
            desktop_command::close(window),
            desktop_command::move_window {
                window,
                x: 100,
                y: 100,
            },
        ] {
            assert_eq!(
                self.model.prepare(&self.human, revision, command).err(),
                Some(desktop_error::backpressure)
            );
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
        assert!(
            self.model
                .observe(&self.machine)
                .unwrap()
                .window(window)
                .is_some()
        );
    }
}

// Route validation is in the effect path, before the real service call or
// editor ACK mutation. Copying numeric endpoint fields cannot bypass it.
fn routed_request(
    coordinator: &native_coordinator,
    roles: &native_role_owners,
    store: &mut durable_store<'_, memory_disk>,
    request: &record,
) -> Result<record, coordinator_error> {
    coordinator.authorize_route(
        &roles.editor,
        native_route::document_request,
        native_route_access::send,
    )?;
    coordinator.authorize_route(
        &roles.document,
        native_route::document_request,
        native_route_access::receive,
    )?;
    let binding = roles.editor.binding();
    let authority = client_identity::try_new(binding.slot(), binding.generation()).unwrap();
    let copied = record::decode(&request.encode()).unwrap();
    Ok(store.execute(authority, &copied).unwrap())
}
fn routed_reply(
    coordinator: &native_coordinator,
    roles: &native_role_owners,
    reply: &record,
) -> Result<record, coordinator_error> {
    coordinator.authorize_route(
        &roles.document,
        native_route::document_reply,
        native_route_access::send,
    )?;
    coordinator.authorize_route(
        &roles.editor,
        native_route::document_reply,
        native_route_access::receive,
    )?;
    Ok(record::decode(&reply.encode()).unwrap())
}

fn save_request(snapshot: &save_snapshot) -> record {
    record::save(
        snapshot.document(),
        snapshot.generation(),
        snapshot.request(),
        snapshot.base_version(),
        snapshot.revision(),
        snapshot.text(),
    )
    .unwrap()
}
fn acknowledgement(reply: &record) -> save_acknowledgement<'_> {
    assert_eq!(reply.kind(), operation::saved);
    save_acknowledgement {
        document: reply.document(),
        generation: reply.generation(),
        request: reply.request(),
        base_version: reply.base_version(),
        revision: reply.revision(),
        version: reply.version(),
        text: reply.text(),
    }
}

#[test]
fn split_action_resolution_and_late_durable_reply_preserve_two_slot_publication() {
    let identity = disk_identity::try_new(0xb100, 0xe001).unwrap();
    let authority = client_identity::try_new(1, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
        f.expect(b"A", 1, 1, 1, false, 0, (0, 0));
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let snapshot_b = f.save();
        assert_eq!(
            (
                snapshot_b.request(),
                snapshot_b.base_version(),
                snapshot_b.revision(),
                snapshot_b.text()
            ),
            (1, 1, 2, b"B".as_slice())
        );
        f.expect(b"B", 4, 2, 1, true, 1, (1, 2));
        let saved_b = f.request(&mut store, &save_request(&snapshot_b)).unwrap();
        assert_eq!((store.stored_text(), store.version()), (b"B".as_slice(), 2));
        f.edit(edit::select_all);
        assert_eq!(
            f.action(native_editor_action::edit(edit::insert(b'C'))),
            native_editor_effect::edited
        );
        let pending = f.prepare_owner();
        let before = f.candidate_pixels(&pending);
        let staged_address = pending.frame().pixels().row(0).unwrap().as_ptr() as usize;
        assert_eq!(pending.frame().observation().text, b"C");
        assert_eq!(
            (
                pending.frame().observation().sequence,
                pending.frame().observation().version
            ),
            (6, 1)
        );
        f.acknowledge(&saved_b).unwrap();
        assert_eq!(f.owner.session().view().version(), 2);
        assert!(f.owner.session().view().is_dirty());
        assert_eq!(f.candidate_pixels(&pending), before);
        assert_eq!(
            pending.frame().pixels().row(0).unwrap().as_ptr() as usize,
            staged_address
        );
        f.present_update(pending);
        f.expect(b"C", 6, 3, 1, true, 1, (1, 2));
        assert_eq!(f.owner.session().view().version(), 2);
        f.publish_owner();
        f.expect(b"C", 7, 3, 2, true, 0, (0, 0));
        let snapshot_c = f.save();
        assert_eq!(
            (
                snapshot_c.request(),
                snapshot_c.base_version(),
                snapshot_c.revision(),
                snapshot_c.text()
            ),
            (2, 2, 3, b"C".as_slice())
        );
        let saved_c = f.request(&mut store, &save_request(&snapshot_c)).unwrap();
        f.acknowledge(&saved_c).unwrap();
        f.publish_owner();
        f.expect(b"C", 9, 3, 3, false, 0, (0, 0));
        assert_client_pixels(&f);
    }
    assert_eq!(disk.flushes, 8);
    disk.crash();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"C".as_slice(), 3)
    );
}

#[test]
fn pending_remote_action_fences_desktop_even_after_editor_resolution_and_save_ack_makes_it_stale() {
    let identity = disk_identity::try_new(0xb100, 0xe002).unwrap();
    let authority = client_identity::try_new(2, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
    f.edit(edit::select_all);
    f.edit(edit::insert(b'B'));
    let snapshot = f.save();
    let saved = f.request(&mut store, &save_request(&snapshot)).unwrap();
    let revision = f.revision();
    let submitted = f
        .bridge
        .submit_action(
            &mut f.model,
            &f.coordinator,
            &f.roles.workspace,
            &f.input,
            revision,
            native_editor_action::edit(edit::insert(b'x')),
        )
        .unwrap();
    f.assert_fenced();
    f.acknowledge(&saved).unwrap();
    let after_ack = f.owner.session().view();
    let completed = submitted
        .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
        .ok()
        .unwrap();
    assert_eq!(f.owner.session().view(), after_ack);
    f.assert_fenced();
    assert_eq!(
        f.bridge
            .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
            .ok()
            .unwrap()
            .err(),
        Some(bridge_error::stale_observation)
    );
    assert_eq!(f.owner.session().view(), after_ack);
    f.publish_owner();
    f.expect(b"B", 5, 2, 2, false, 0, (0, 0));
    // Once the exact outcome is consumed the ordinary publication path opens.
    let ticket = f
        .model
        .prepare(&f.human, f.revision(), desktop_command::toggle_launcher)
        .unwrap();
    f.model.cancel(&f.control, ticket).ok().unwrap();
    let revision = f.revision();
    let machine = f
        .bridge
        .submit_action(
            &mut f.model,
            &f.coordinator,
            &f.roles.workspace,
            &f.machine_input,
            revision,
            native_editor_action::edit(edit::insert(b'!')),
        )
        .unwrap();
    let completed = machine
        .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
        .ok()
        .unwrap();
    f.assert_fenced();
    let result = f
        .bridge
        .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
        .ok()
        .unwrap()
        .unwrap();
    assert_eq!(result.origin, client_origin::machine);
    assert_eq!(result.effect, native_editor_effect::edited);
    f.publish_owner();
    f.expect(b"B!", 6, 3, 2, true, 0, (0, 0));
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> &[u8] {
    let offset = y as usize * row_byte_count + x as usize * 3;
    &pixels[offset..offset + 3]
}
fn assert_client_pixels(f: &fixture<'_>) {
    let view = f.model.observe(&f.human).unwrap();
    let viewport = client_viewport(&view, view.window(f.bridge.window()).unwrap());
    let source = f.bridge.presented_frame(&f.model, &f.human).unwrap();
    let descriptor = source.descriptor();
    for y in viewport.y..viewport.y + viewport.height.saturating_sub(12) {
        let sy =
            ((y - viewport.y) as u64 * descriptor.height() as u64 / viewport.height as u64) as u32;
        for x in viewport.x..viewport.x + viewport.width.saturating_sub(12) {
            let sx = ((x - viewport.x) as u64 * descriptor.width() as u64 / viewport.width as u64)
                as usize;
            assert_eq!(
                pixel(&f.displayed, x, y),
                &source.row(sy).unwrap()[sx * 3..sx * 3 + 3]
            );
        }
    }
}

#[test]
fn borrowed_bridge_and_publication_handles_do_not_embed_editor_pixel_arrays() {
    // These are host API storage assertions, not native stack certificates.
    assert!(core::mem::size_of::<content<'_>>() < 512);
    assert!(core::mem::size_of::<update<'_>>() < 1024);
    assert!(core::mem::size_of::<native_editor_bridge<pixels<'_>>>() < 1024);
    assert_eq!(interactive_frame_byte_count, 18_432);
}

#[test]
fn retired_document_route_blocks_lost_reply_but_editor_and_workspace_keep_newer_dirty_pixels() {
    let identity = disk_identity::try_new(0xb100, 0xe003).unwrap();
    let authority = client_identity::try_new(3, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    let (mut f, snapshot, committed_reply) = {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let snapshot = f.save();
        let committed_reply = f.request(&mut store, &save_request(&snapshot)).unwrap();
        assert_eq!(
            (committed_reply.text(), committed_reply.version()),
            (b"B".as_slice(), 2)
        );
        f.edit(edit::select_all);
        f.edit(edit::insert(b'C'));
        f.expect(b"C", 6, 3, 1, true, 1, (1, 2));
        let retired = f.coordinator.retire(&f.roles.document).unwrap();
        assert_eq!(retired.closed_routes, [false, false, true, true]);
        let before = (
            store.sequence(),
            store.version(),
            store.stored_text().to_vec(),
        );
        assert_eq!(
            f.request(&mut store, &save_request(&snapshot)).err(),
            Some(coordinator_error::route_closed)
        );
        assert_eq!(
            (
                store.sequence(),
                store.version(),
                store.stored_text().to_vec()
            ),
            before
        );
        (f, snapshot, committed_reply)
    };
    // The real service borrow is gone. A copied valid saved record does not
    // reopen its retired route or prove delivery to the editor.
    let before = f.owner.session().view();
    assert_eq!(
        f.acknowledge(&committed_reply),
        Err(coordinator_error::retired)
    );
    assert_eq!(f.owner.session().view(), before);
    f.owner
        .mark_save_unknown(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
        )
        .unwrap();
    f.publish_owner();
    f.expect(b"C", 7, 3, 1, true, 2, (1, 2));
    assert_eq!(
        f.owner.session().view().save_outcome(),
        save_outcome::unknown
    );
    let unknown_frame = f.displayed.clone();
    f.edit(edit::insert(b'!'));
    f.expect(b"C!", 8, 4, 1, true, 2, (1, 2));
    assert_ne!(f.displayed, unknown_frame);
    assert_client_pixels(&f);
    assert_eq!(
        f.owner
            .session()
            .model()
            .state()
            .pending_save()
            .unwrap()
            .text(),
        b"B"
    );
    for _ in 0..3 {
        assert_eq!(f.coordinator.next_runnable(), Some(native_role::workspace));
        assert_eq!(f.coordinator.next_runnable(), Some(native_role::editor));
    }
    disk.crash();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"B".as_slice(), 2)
    );
}

#[test]
fn retired_editor_rejects_delivered_action_and_trusted_withdrawal_does_not_acknowledge_save() {
    let identity = disk_identity::try_new(0xb100, 0xe004).unwrap();
    let authority = client_identity::try_new(4, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
    f.edit(edit::select_all);
    f.edit(edit::insert(b'B'));
    let snapshot = f.save();
    let revision = f.revision();
    let submitted = f
        .bridge
        .submit_action(
            &mut f.model,
            &f.coordinator,
            &f.roles.workspace,
            &f.input,
            revision,
            native_editor_action::edit(edit::insert(b'x')),
        )
        .unwrap();
    f.assert_fenced();
    let before = f.owner.session().view();
    let retired = f.coordinator.retire(&f.roles.editor).unwrap();
    assert_eq!(retired.closed_routes, [true, true, true, true]);
    let completed = submitted
        .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
        .ok()
        .unwrap();
    assert_eq!(
        completed.outcome().as_ref().err(),
        Some(&bridge_error::retired)
    );
    assert_eq!(f.owner.session().view(), before);
    assert_eq!(
        f.bridge
            .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
            .ok()
            .unwrap()
            .err(),
        Some(bridge_error::retired)
    );
    let window = f.bridge.window();
    let revision = f.revision();
    let ticket = f
        .model
        .prepare_native_editor_withdrawal(&f.control, &f.human, revision, window)
        .unwrap();
    assert!(f.model.candidate(&ticket).unwrap().window(window).is_none());
    let (_, pixels) = present_desktop(&mut f.model, &f.control, ticket, None);
    assert_ne!(pixels, f.displayed);
    assert!(f.model.observe(&f.human).unwrap().window(window).is_none());
    assert!(f.bridge.observe(&f.model, &f.human).is_err());
    assert_eq!(f.owner.session().view(), before);
    assert!(!f.owner.session().view().is_closed());
    assert_eq!(
        f.owner
            .session()
            .model()
            .state()
            .pending_save()
            .unwrap()
            .text(),
        b"B"
    );
    assert_eq!(
        f.request(&mut store, &save_request(&snapshot)).err(),
        Some(coordinator_error::retired)
    );
    assert_eq!((store.stored_text(), store.version()), (b"A".as_slice(), 1));
}

#[test]
fn terminal_withdrawal_consumes_unresolved_capability_without_synthesizing_editor_close() {
    let identity = disk_identity::try_new(0xb100, 0xe005).unwrap();
    let authority = client_identity::try_new(5, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    {
        let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
        let revision = f.revision();
        let submitted = f
            .bridge
            .submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                native_editor_action::edit(edit::insert(b'x')),
            )
            .unwrap();
        // A timeout/unproven terminal condition cannot cancel accepted work.
        let rejected = f
            .bridge
            .withdraw_terminal_submission(&mut f.model, &f.control, &f.coordinator, submitted)
            .err()
            .unwrap();
        let submitted = rejected.into_submission();
        f.assert_fenced();
        f.coordinator.retire(&f.roles.editor).unwrap();
        let before = f.owner.session().view();
        f.bridge
            .withdraw_terminal_submission(&mut f.model, &f.control, &f.coordinator, submitted)
            .ok()
            .unwrap();
        assert_eq!(f.owner.session().view(), before);
        let window = f.bridge.window();
        let revision = f.revision();
        let ticket = f
            .model
            .prepare_native_editor_withdrawal(&f.control, &f.human, revision, window)
            .unwrap();
        present_desktop(&mut f.model, &f.control, ticket, None);
        assert_eq!(
            f.bridge.observe(&f.model, &f.human).err(),
            Some(bridge_error::retired)
        );
    }
    assert_eq!((store.stored_text(), store.version()), (b"A".as_slice(), 1));
}

#[test]
fn workspace_retirement_still_drains_accepted_save_through_live_document_routes() {
    let identity = disk_identity::try_new(0xb100, 0xe006).unwrap();
    let authority = client_identity::try_new(6, 3).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
    let mut first = pixel_buffer();
    let mut second = pixel_buffer();
    let mut f = fixture::from_store(&mut store, authority, &mut first, &mut second);
    f.edit(edit::select_all);
    f.edit(edit::insert(b'B'));
    let revision = f.revision();
    let submitted = f
        .bridge
        .submit_action(
            &mut f.model,
            &f.coordinator,
            &f.roles.workspace,
            &f.input,
            revision,
            native_editor_action::save,
        )
        .unwrap();
    let retired = f.coordinator.retire(&f.roles.workspace).unwrap();
    assert_eq!(retired.closed_routes, [true, true, false, false]);
    let completed = submitted
        .resolve(&f.coordinator, &f.roles.editor, &mut f.owner)
        .ok()
        .unwrap();
    let result = f
        .bridge
        .complete_action(&mut f.model, &f.coordinator, &f.roles.workspace, completed)
        .ok()
        .unwrap()
        .unwrap();
    let native_editor_effect::save(snapshot) = result.effect else {
        panic!("accepted Save must complete")
    };
    assert_eq!(
        (
            snapshot.text(),
            snapshot.revision(),
            snapshot.base_version()
        ),
        (b"B".as_slice(), 2, 1)
    );
    let reply = f.request(&mut store, &save_request(&snapshot)).unwrap();
    f.acknowledge(&reply).unwrap();
    assert_eq!(f.owner.session().view().text(), b"B");
    assert!(!f.owner.session().view().is_dirty());
    assert_eq!(store.version(), 2);
    let revision = f.revision();
    assert_eq!(
        f.bridge
            .submit_action(
                &mut f.model,
                &f.coordinator,
                &f.roles.workspace,
                &f.input,
                revision,
                native_editor_action::edit(edit::insert(b'x'))
            )
            .err(),
        Some(bridge_error::retired)
    );
    assert_eq!(
        f.coordinator.authorize_route(
            &f.roles.editor,
            native_route::editor_observation,
            native_route_access::send
        ),
        Err(coordinator_error::route_closed)
    );
}
