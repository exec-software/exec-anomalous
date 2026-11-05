#![cfg(feature = "native_editor_bridge")]
#![allow(non_camel_case_types)]

use anomalous_desktop_compositor::{
    client_viewport, composition_error, composition_frame, frame_height, frame_width,
    row_byte_count, surface_binding,
};
use anomalous_desktop_workspace::native_bridge::{
    bridge_error, editor_disclosure, native_editor_action, native_editor_bridge,
    native_editor_effect, native_editor_frame, native_editor_input, native_editor_owner,
    native_editor_update,
};
use anomalous_desktop_workspace::{
    client_endpoint, client_grants, client_origin, demo_content, desktop, desktop_command,
    desktop_control, desktop_error, publication, render_view, update_ticket, window_id,
    window_rect, workspace_id,
};
use anomalous_document_protocol::durable::{
    disk_identity, durable_store, format_disposable, io_error, recover, sector_count, sector_io,
    sector_size,
};
use anomalous_document_protocol::{client_identity, operation, record};
use anomalous_editor_contract::{
    interactive_close_result, interactive_editor_action as edit, interactive_editor_session,
    save_acknowledgement, save_outcome, save_snapshot,
};
use anomalous_window_surface::frame;

// Exclusive test device: writes are volatile until a successful flush. Recovery
// below reads the persisted image after the live service is dropped. This is a
// host contract test, not evidence of a storage driver or a VM reboot.
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
    fn power_cycle(&mut self) {
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
        .map(|(window, pixels)| surface_binding::new(window, pixels))
        .into_iter()
        .collect();
    let composition = composition_frame::try_new(view, &bindings).unwrap();
    let mut result = vec![0xa5; row_byte_count * frame_height as usize];
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
    // Validate the retained candidate/binding set before presentation begins.
    {
        let view = model.candidate(&ticket).unwrap();
        let bindings: Vec<_> = content
            .map(|(window, pixels)| surface_binding::new(window, pixels))
            .into_iter()
            .collect();
        composition_frame::try_new(&view, &bindings).unwrap();
    }
    model.begin(control, &ticket).unwrap();
    let pixels = compose(&model.candidate(&ticket).unwrap(), content);
    model.attest_presented(control, &ticket).unwrap();
    (model.publish(control, ticket).ok().unwrap(), pixels)
}

struct fixture {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    machine: client_endpoint,
    owner: native_editor_owner,
    bridge: native_editor_bridge,
    input: native_editor_input,
    displayed: Vec<u8>,
}
impl fixture {
    fn from_open(opened: &record) -> Self {
        assert_eq!(opened.kind(), operation::opened);
        let session = interactive_editor_session::from_open(
            opened.document(),
            opened.generation(),
            opened.version(),
            opened.text(),
        )
        .unwrap();
        let mut owner = native_editor_owner::new(session);
        let frame = owner.frame().unwrap();
        let (mut model, control, initial) = desktop::new(frame_width, frame_height).unwrap();
        let human = model
            .bind_client(&control, client_origin::human, client_grants::all)
            .unwrap();
        let machine = model
            .bind_client(&control, client_origin::machine, client_grants::observe)
            .unwrap();
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
        Self {
            model,
            control,
            human,
            machine,
            owner,
            bridge,
            input,
            displayed,
        }
    }
    fn revision(&self) -> u64 {
        self.model.observe(&self.human).unwrap().revision()
    }
    fn action(&mut self, action: native_editor_action) -> native_editor_effect {
        let ticket = self
            .bridge
            .prepare_action(&self.model, &self.input, self.revision(), action)
            .unwrap();
        let result = self
            .bridge
            .consume_action(&self.model, &mut self.owner, ticket)
            .ok()
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
            panic!("save effect required")
        };
        self.publish_owner();
        snapshot
    }
    fn prepare(&mut self, frame: native_editor_frame) -> native_editor_update {
        let revision = self.revision();
        self.bridge
            .prepare_update(&mut self.model, &self.owner, revision, frame)
            .ok()
            .unwrap()
    }
    fn present_update(&mut self, update: native_editor_update) {
        {
            let view = self.bridge.candidate(&self.model, &update).unwrap();
            let pixels = update.frame().pixels();
            let bindings: Vec<_> = view
                .window(self.bridge.window())
                .map(|_| surface_binding::new(self.bridge.window(), &pixels))
                .into_iter()
                .collect();
            composition_frame::try_new(&view, &bindings).unwrap();
        }
        self.bridge
            .begin(&mut self.model, &self.control, &update)
            .unwrap();
        let view = self.bridge.candidate(&self.model, &update).unwrap();
        let frame = update.frame().pixels();
        let content = view
            .window(self.bridge.window())
            .map(|_| (self.bridge.window(), &frame));
        let pixels = compose(&view, content);
        self.bridge
            .attest_presented(&mut self.model, &self.control, &update)
            .unwrap();
        let expected_sequence = update.frame().observation().sequence;
        let publication = self
            .bridge
            .publish(&mut self.model, &self.control, update)
            .ok()
            .unwrap();
        assert_eq!(publication.observation_sequence, expected_sequence);
        self.displayed = pixels;
    }
    fn publish_owner(&mut self) {
        let frame = self.owner.frame().unwrap();
        let update = self.prepare(frame);
        self.present_update(update);
    }
    fn desktop_action(&mut self, command: desktop_command) -> publication {
        let ticket = self
            .model
            .prepare(&self.human, self.revision(), command)
            .unwrap();
        let pixels = self
            .bridge
            .presented_frame(&self.model, &self.human)
            .unwrap();
        let (publication, displayed) = present_desktop(
            &mut self.model,
            &self.control,
            ticket,
            Some((self.bridge.window(), &pixels)),
        );
        self.displayed = displayed;
        publication
    }
    // Keep the observation oracle literal rather than deriving expected fields
    // from the editor implementation under test.
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
        assert!(
            self.model
                .observe(&self.human)
                .unwrap()
                .window(self.bridge.window())
                .unwrap()
                .editor()
                .is_none()
        );
    }
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
fn durable_reply_updates_the_sole_owner_before_cancellable_workspace_publication() {
    let identity = disk_identity::try_new(0xa110, 0xd001).unwrap();
    let authority = client_identity::try_new(1, 7).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let opened = store
            .execute(authority, &record::open(store.document(), 1).unwrap())
            .unwrap();
        let mut f = fixture::from_open(&opened);
        f.expect(b"A", 1, 1, 1, false, 0, (0, 0));
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let b = f.save();
        assert_eq!(
            (b.request(), b.base_version(), b.revision(), b.text()),
            (1, 1, 2, b"B".as_slice())
        );
        f.expect(b"B", 4, 2, 1, true, 1, (1, 2));
        let saved_b = store.execute(authority, &save_request(&b)).unwrap();
        assert_eq!((store.stored_text(), store.version()), (b"B".as_slice(), 2));
        f.edit(edit::select_all);
        f.edit(edit::insert(b'C'));
        f.expect(b"C", 6, 3, 1, true, 1, (1, 2));
        f.owner.acknowledge_save(acknowledgement(&saved_b)).unwrap();
        assert_eq!(f.owner.session().view().version(), 2);
        assert!(f.owner.session().view().is_dirty());
        let old_pixels = f.displayed.clone();
        let old_desktop = f.revision();
        let update = f.prepare(f.owner.frame().unwrap());
        assert_eq!(update.frame().observation().version, 2);
        // Even a prepared frame cannot change the displayed baseline.
        f.expect(b"C", 6, 3, 1, true, 1, (1, 2));
        let retry = f
            .bridge
            .cancel(&mut f.model, &f.control, update)
            .ok()
            .unwrap();
        assert_eq!(f.revision(), old_desktop);
        assert_eq!(f.displayed, old_pixels);
        assert_eq!(f.owner.session().view().version(), 2);
        assert_eq!(store.version(), 2);
        let update = f.prepare(retry);
        f.present_update(update);
        f.expect(b"C", 7, 3, 2, true, 0, (0, 0));
        assert_ne!(f.displayed, old_pixels);
        let c = f.save();
        assert_eq!(
            (c.request(), c.base_version(), c.revision(), c.text()),
            (2, 2, 3, b"C".as_slice())
        );
        let saved_c = store.execute(authority, &save_request(&c)).unwrap();
        f.owner.acknowledge_save(acknowledgement(&saved_c)).unwrap();
        f.publish_owner();
        f.expect(b"C", 9, 3, 3, false, 0, (0, 0));
        let read = store
            .execute(
                authority,
                &record::read(opened.document(), opened.generation(), 10).unwrap(),
            )
            .unwrap();
        assert_eq!((read.text(), read.version()), (b"C".as_slice(), 3));
    }
    assert_eq!(disk.flushes, 8); // format2 + mount2 + each of two saves2
    disk.power_cycle();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"C".as_slice(), 3)
    );
}

#[test]
fn lost_committed_reply_stays_unknown_through_undo_and_requires_owner_confirmed_close() {
    let identity = disk_identity::try_new(0xa110, 0xd002).unwrap();
    let authority = client_identity::try_new(2, 4).unwrap();
    let mut disk = memory_disk::blank();
    format_disposable(&mut disk, identity, b"A").unwrap();
    {
        let mut store = durable_store::mount(&mut disk, identity, authority).unwrap();
        let opened = store
            .execute(authority, &record::open(store.document(), 1).unwrap())
            .unwrap();
        let mut f = fixture::from_open(&opened);
        f.edit(edit::select_all);
        f.edit(edit::insert(b'B'));
        let snapshot = f.save();
        let lost_reply = store.execute(authority, &save_request(&snapshot)).unwrap();
        assert_eq!(
            (lost_reply.text(), lost_reply.version()),
            (b"B".as_slice(), 2)
        );
        f.edit(edit::undo);
        f.expect(b"A", 5, 3, 1, false, 1, (1, 2));
        f.owner
            .mark_save_unknown(
                snapshot.document(),
                snapshot.generation(),
                snapshot.request(),
                snapshot.base_version(),
            )
            .unwrap();
        f.publish_owner();
        f.expect(b"A", 6, 3, 1, true, 2, (1, 2));
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
            f.owner.session().view().save_outcome(),
            save_outcome::unknown
        );
        assert_eq!(
            f.model
                .prepare(
                    &f.human,
                    f.revision(),
                    desktop_command::close(f.bridge.window())
                )
                .err(),
            Some(desktop_error::native_editor_close_required)
        );
        assert_eq!(
            f.action(native_editor_action::request_close),
            native_editor_effect::close(interactive_close_result::confirmation_required)
        );
        f.publish_owner();
        assert!(f.bridge.observe(&f.model, &f.human).unwrap().close_armed);
        assert!(
            f.model
                .observe(&f.human)
                .unwrap()
                .window(f.bridge.window())
                .is_some()
        );
        // A processed input reset disarms close even though text is unchanged.
        f.edit(edit::input_reset);
        assert!(!f.bridge.observe(&f.model, &f.human).unwrap().close_armed);
        assert_eq!(
            f.action(native_editor_action::request_close),
            native_editor_effect::close(interactive_close_result::confirmation_required)
        );
        f.publish_owner();
        assert_eq!(
            f.action(native_editor_action::request_close),
            native_editor_effect::close(interactive_close_result::closed)
        );
        let closing = f.prepare(f.owner.frame().unwrap());
        assert!(
            f.model
                .observe(&f.human)
                .unwrap()
                .window(f.bridge.window())
                .is_some()
        );
        assert!(
            f.bridge
                .candidate(&f.model, &closing)
                .unwrap()
                .window(f.bridge.window())
                .is_none()
        );
        f.present_update(closing);
        assert!(
            f.model
                .observe(&f.human)
                .unwrap()
                .window(f.bridge.window())
                .is_none()
        );
        assert_eq!(
            f.bridge.observe(&f.model, &f.human).err(),
            Some(bridge_error::retired)
        );
        assert_eq!(
            f.bridge
                .prepare_action(&f.model, &f.input, f.revision(), native_editor_action::save)
                .err(),
            Some(bridge_error::retired)
        );
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
        assert!(f.owner.session().view().is_closed());
    }
    disk.power_cycle();
    let recovered = recover(&mut disk, identity).unwrap();
    assert_eq!(
        (recovered.text(), recovered.version()),
        (b"B".as_slice(), 2)
    );
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> &[u8] {
    let index = y as usize * row_byte_count + x as usize * 3;
    &pixels[index..index + 3]
}

fn assert_client_sampling(f: &fixture) {
    let view = f.model.observe(&f.human).unwrap();
    let viewport = client_viewport(&view, view.window(f.bridge.window()).unwrap());
    let source = f.bridge.presented_frame(&f.model, &f.human).unwrap();
    let descriptor = source.descriptor();
    // Independent integer sampling oracle; exclude the decoration grip near the
    // bottom/right edge. No compositor output is reused as its own reference.
    for y in viewport.y..viewport.y + viewport.height.saturating_sub(12) {
        let sy = (u64::from(y - viewport.y) * u64::from(descriptor.height())
            / u64::from(viewport.height)) as u32;
        for x in viewport.x..viewport.x + viewport.width.saturating_sub(12) {
            let sx = (u64::from(x - viewport.x) * u64::from(descriptor.width())
                / u64::from(viewport.width)) as usize;
            assert_eq!(
                pixel(&f.displayed, x, y),
                &source.row(sy).unwrap()[sx * 3..sx * 3 + 3]
            );
        }
    }
}

#[test]
fn geometry_focus_and_occlusion_use_exact_retained_content_without_input_or_binding_leaks() {
    let authority = client_identity::try_new(3, 1).unwrap();
    let mut store =
        anomalous_document_protocol::document_store::try_new(authority, 81, b"AB").unwrap();
    let opened = store
        .execute(authority, &record::open(81, 1).unwrap())
        .unwrap();
    let mut f = fixture::from_open(&opened);
    assert_client_sampling(&f);
    let before = f.owner.session().view();
    let ticket = f
        .bridge
        .prepare_action(
            &f.model,
            &f.input,
            f.revision(),
            native_editor_action::edit(edit::insert(b'x')),
        )
        .unwrap();
    f.desktop_action(desktop_command::minimize(f.bridge.window()));
    assert_eq!(
        f.bridge
            .consume_action(&f.model, &mut f.owner, ticket)
            .err()
            .unwrap()
            .error(),
        bridge_error::desktop(desktop_error::stale_revision)
    );
    assert_eq!(f.owner.session().view(), before);
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), native_editor_action::save)
            .err(),
        Some(bridge_error::not_focused)
    );
    let view = f.model.observe(&f.human).unwrap();
    assert_eq!(
        composition_frame::try_new(&view, &[]).err(),
        Some(composition_error::missing_surface_binding)
    );
    f.desktop_action(desktop_command::restore(f.bridge.window()));
    f.desktop_action(desktop_command::resize_window {
        window: f.bridge.window(),
        width: 400,
        height: 280,
    });
    assert_client_sampling(&f);
    f.desktop_action(desktop_command::move_window {
        window: f.bridge.window(),
        x: 32,
        y: 88,
    });
    assert_client_sampling(&f);
    let unobscured = f.displayed.clone();
    let cover = f
        .desktop_action(desktop_command::create {
            content: demo_content::welcome,
            workspace: workspace_id::first,
            bounds: window_rect::try_new(100, 120, 180, 160).unwrap(),
        })
        .created_window()
        .unwrap();
    assert_ne!(pixel(&f.displayed, 160, 200), pixel(&unobscured, 160, 200));
    assert_eq!(pixel(&f.displayed, 390, 190), pixel(&unobscured, 390, 190));
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), native_editor_action::save)
            .err(),
        Some(bridge_error::not_focused)
    );
    f.desktop_action(desktop_command::close(cover));
    f.desktop_action(desktop_command::focus(f.bridge.window()));
    assert_client_sampling(&f);
    f.desktop_action(desktop_command::toggle_launcher);
    assert_eq!(
        f.bridge
            .prepare_action(&f.model, &f.input, f.revision(), native_editor_action::save)
            .err(),
        Some(bridge_error::not_focused)
    );
    f.desktop_action(desktop_command::dismiss_launcher);
    f.expect(b"AB", 1, 1, 1, false, 0, (0, 0));
    // An independently valid frame with identical observation bytes still has a
    // distinct descriptor; it cannot stand in for the retained committed frame.
    let forged = f.owner.frame().unwrap();
    let pixels = forged.pixels();
    let view = f.model.observe(&f.human).unwrap();
    assert_eq!(
        composition_frame::try_new(&view, &[surface_binding::new(f.bridge.window(), &pixels)])
            .err(),
        Some(composition_error::surface_descriptor_mismatch)
    );
}
