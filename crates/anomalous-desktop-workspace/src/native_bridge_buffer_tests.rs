use super::*;

type storage<'a> = borrowed_frame<'a, interactive_frame_byte_count>;
type retained_frame<'a> = native_editor_frame<storage<'a>>;

struct fixture<'a> {
    model: desktop,
    control: desktop_control,
    human: client_endpoint,
    owner: native_editor_owner,
    bridge: native_editor_bridge<storage<'a>>,
    input: native_editor_input,
}

fn publish(model: &mut desktop, control: &desktop_control, ticket: update_ticket) -> publication {
    model.begin(control, &ticket).unwrap();
    model.attest_presented(control, &ticket).unwrap();
    model.publish(control, ticket).ok().unwrap()
}

impl<'a> fixture<'a> {
    fn new(buffer: &'a mut [u8; interactive_frame_byte_count]) -> Self {
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
        let mut owner =
            native_editor_owner::new(interactive_editor_session::from_open(71, 3, 1, b"").unwrap());
        let frame = owner.frame_in(buffer).ok().unwrap();
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
            owner,
            bridge,
            input,
        }
    }
    fn revision(&self) -> u64 {
        self.model.observe(&self.human).unwrap().revision()
    }
    fn insert(&mut self, byte: u8) {
        let ticket = self
            .bridge
            .prepare_action(
                &self.model,
                &self.input,
                self.revision(),
                native_editor_action::edit(interactive_editor_action::insert(byte)),
            )
            .unwrap();
        self.bridge
            .consume_action(&self.model, &mut self.owner, ticket)
            .ok()
            .unwrap();
    }
    fn prepare(&mut self, frame: retained_frame<'a>) -> native_editor_update<storage<'a>> {
        let revision = self.revision();
        self.bridge
            .prepare_update(&mut self.model, &self.owner, revision, frame)
            .ok()
            .unwrap()
    }
    fn finish(&mut self, update: native_editor_update<storage<'a>>) -> retained_frame<'a> {
        self.bridge
            .begin(&mut self.model, &self.control, &update)
            .unwrap();
        self.bridge
            .attest_presented(&mut self.model, &self.control, &update)
            .unwrap();
        self.bridge
            .publish_recycling(&mut self.model, &self.control, update)
            .ok()
            .unwrap()
            .1
    }
}

#[test]
fn borrowed_render_matches_owned_pixels_without_moving_the_supplied_allocation() {
    let session = interactive_editor_session::from_open(71, 3, 1, b"const hello = 1;").unwrap();
    let mut buffer = [0xa5; interactive_frame_byte_count];
    let pointer = buffer.as_ptr();
    let borrowed = native_editor_frame::from_view_in(&session.view(), &mut buffer)
        .ok()
        .unwrap();
    let owned = native_editor_frame::from_view(&session.view()).unwrap();
    assert_eq!(borrowed.audit, owned.audit);
    assert_eq!(borrowed.pixels().row(0).unwrap().as_ptr(), pointer);
    let descriptor = borrowed.descriptor();
    for row in 0..interactive_frame_height as u32 {
        assert_eq!(borrowed.pixels().row(row), owned.pixels().row(row));
        assert_eq!(borrowed.pixels().descriptor(), descriptor);
    }
    assert_eq!(borrowed.into_buffer().as_ptr(), pointer);
}

#[test]
fn invalid_audit_returns_the_exact_unmodified_buffer() {
    let session = interactive_editor_session::from_open(71, 3, 1, b"").unwrap();
    let valid = interactive_audit::from_view(&session.view())
        .encode()
        .unwrap();
    for offset in [0, 4, 6, 57, 127] {
        let mut bad = valid;
        bad[offset] ^= 0x80;
        let mut buffer = [0xa5; interactive_frame_byte_count];
        let pointer = buffer.as_ptr();
        let error = native_editor_frame::from_audit_in(&bad, &mut buffer)
            .err()
            .unwrap();
        assert_eq!(error.error(), bridge_error::invalid_observation);
        let buffer = error.into_buffer();
        assert_eq!(buffer.as_ptr(), pointer);
        assert!(buffer.iter().all(|byte| *byte == 0xa5));
    }
}

#[test]
fn repeated_publication_rotates_exactly_two_buffers_with_fresh_content_identities() {
    let mut first = [0; interactive_frame_byte_count];
    let mut second = [0; interactive_frame_byte_count];
    let pointers = [first.as_ptr(), second.as_ptr()];
    let mut f = fixture::new(&mut first);
    let mut available = &mut second;
    let mut expected_displayed = pointers[0];
    let mut expected_available = pointers[1];
    for byte in b"abcdefghijklmnopqrst" {
        f.insert(*byte);
        let next = f.owner.frame_in(available).ok().unwrap();
        let next_id = next.descriptor().id();
        assert_eq!(next.pixels().row(0).unwrap().as_ptr(), expected_available);
        let update = f.prepare(next);
        let retired = f.finish(update);
        assert_eq!(
            retired.pixels().row(0).unwrap().as_ptr(),
            expected_displayed
        );
        assert_ne!(retired.descriptor().id(), next_id);
        assert_eq!(
            f.bridge
                .presented_frame(&f.model, &f.human)
                .unwrap()
                .row(0)
                .unwrap()
                .as_ptr(),
            expected_available
        );
        assert_eq!(
            f.bridge.observe(&f.model, &f.human).unwrap().text,
            f.owner.session().view().text()
        );
        available = retired.into_buffer();
        core::mem::swap(&mut expected_displayed, &mut expected_available);
    }
    assert_eq!(available.as_ptr(), expected_available);
}

#[test]
fn rejection_cancellation_and_retry_retain_the_pending_buffer_and_displayed_identity() {
    let mut first = [0; interactive_frame_byte_count];
    let mut second = [0; interactive_frame_byte_count];
    let first_pointer = first.as_ptr();
    let second_pointer = second.as_ptr();
    let mut f = fixture::new(&mut first);
    let initial_descriptor = f
        .bridge
        .presented_frame(&f.model, &f.human)
        .unwrap()
        .descriptor();
    let stale = f.owner.frame_in(&mut second).ok().unwrap();
    f.insert(b'x');
    let revision = f.revision();
    let rejected = f
        .bridge
        .prepare_update(&mut f.model, &f.owner, revision, stale)
        .err()
        .unwrap();
    assert_eq!(rejected.error(), bridge_error::observation_mismatch);
    let buffer = rejected.into_frame().into_buffer();
    assert_eq!(buffer.as_ptr(), second_pointer);
    assert!(f.model.pending.is_none());
    let frame = f.owner.frame_in(buffer).ok().unwrap();
    let descriptor = frame.descriptor();
    let update = f.prepare(frame);
    let rejected = f
        .bridge
        .publish_recycling(&mut f.model, &f.control, update)
        .err()
        .unwrap();
    let update = rejected.into_update();
    assert_eq!(update.frame().descriptor(), descriptor);
    assert_eq!(
        update.frame().pixels().row(0).unwrap().as_ptr(),
        second_pointer
    );
    assert_eq!(
        f.bridge
            .presented_frame(&f.model, &f.human)
            .unwrap()
            .descriptor(),
        initial_descriptor
    );
    let frame = f
        .bridge
        .cancel(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert_eq!(frame.descriptor(), descriptor);
    let update = f.prepare(frame);
    let retired = f.finish(update);
    assert_eq!(retired.descriptor(), initial_descriptor);
    assert_eq!(retired.into_buffer().as_ptr(), first_pointer);
    assert_eq!(
        f.bridge
            .presented_frame(&f.model, &f.human)
            .unwrap()
            .descriptor(),
        descriptor
    );
}

#[test]
fn begun_presentation_cannot_release_its_pending_buffer_through_cancellation() {
    let mut first = [0; interactive_frame_byte_count];
    let mut second = [0; interactive_frame_byte_count];
    let pointer = second.as_ptr();
    let mut f = fixture::new(&mut first);
    f.insert(b'x');
    let frame = f.owner.frame_in(&mut second).ok().unwrap();
    let update = f.prepare(frame);
    f.bridge.begin(&mut f.model, &f.control, &update).unwrap();
    let rejected = f
        .bridge
        .cancel(&mut f.model, &f.control, update)
        .err()
        .unwrap();
    let update = rejected.into_update();
    assert_eq!(update.frame().pixels().row(0).unwrap().as_ptr(), pointer);
    f.bridge
        .attest_presented(&mut f.model, &f.control, &update)
        .unwrap();
    let (_, retired) = f
        .bridge
        .publish_recycling(&mut f.model, &f.control, update)
        .ok()
        .unwrap();
    assert_ne!(retired.into_buffer().as_ptr(), pointer);
    assert_eq!(
        f.bridge
            .presented_frame(&f.model, &f.human)
            .unwrap()
            .row(0)
            .unwrap()
            .as_ptr(),
        pointer
    );
}

#[test]
fn borrowed_result_and_publication_handles_do_not_contain_pixel_arrays() {
    use core::mem::size_of;
    assert!(size_of::<retained_frame<'_>>() <= 384);
    assert!(size_of::<native_editor_bridge<storage<'_>>>() <= 512);
    assert!(size_of::<native_editor_update<storage<'_>>>() <= 512);
    assert!(size_of::<rejected_editor_frame<storage<'_>>>() <= 512);
    assert!(size_of::<rejected_editor_update<storage<'_>>>() <= 512);
    assert!(size_of::<rejected_editor_buffer<'_>>() <= 32);
}
