//! Repeated native edits over the unchanged copied document and sector routes.
//! A host-only challenge reaches the editor; the service learns it from copied
//! surfaces. Three acknowledged saves cross both checkpoint slots repeatedly.
use super::*;
use anomalous_editor_contract::{edit_effect, repeated_opened_editor_state, save_outcome};

fn initialize(
    slot: &mut MaybeUninit<repeated_opened_editor_state>,
    opened: record,
) -> &mut repeated_opened_editor_state {
    assert_eq!(opened.kind(), operation::opened);
    assert_eq!(opened.status(), None);
    assert_eq!(opened.generation(), 1);
    slot.write(
        repeated_opened_editor_state::from_open(
            opened.document(),
            opened.generation(),
            opened.version(),
            opened.text(),
        )
        .unwrap(),
    )
}

fn edit(state: &mut repeated_opened_editor_state, text: &[u8], discarded_undo: bool) {
    assert_eq!(
        state.insert(text),
        Ok(edit_effect::changed { discarded_undo })
    );
}

fn changed(byte: u8) -> u8 {
    if byte == b'~' { b'!' } else { byte + 1 }
}

fn newer(state: &mut repeated_opened_editor_state) {
    let byte = changed(state.state().text()[0]);
    state.set_selection(0, 1).unwrap();
    edit(state, &[byte], true);
    assert_eq!(state.state().revision(), 5);
    assert_eq!(state.state().selection(), (1, 1));
    assert!(state.state().is_dirty());
}

fn final_edit(state: &mut repeated_opened_editor_state) {
    let length = state.state().text().len();
    let byte = changed(state.state().text()[length - 1]);
    state.set_selection(length - 1, length).unwrap();
    edit(state, &[byte], false);
    assert_eq!(state.state().revision(), 7);
    assert_eq!(state.state().selection(), (length, length));
}

fn accept(state: &mut repeated_opened_editor_state, reply: record) {
    assert_eq!(reply.kind(), operation::saved);
    assert_eq!(reply.status(), None);
    state
        .acknowledge_save(save_acknowledgement {
            document: reply.document(),
            generation: reply.generation(),
            request: reply.request(),
            base_version: reply.base_version(),
            revision: reply.revision(),
            version: reply.version(),
            text: reply.text(),
        })
        .unwrap();
    assert_eq!(state.state().save_outcome(), save_outcome::idle);
    assert!(state.state().pending_save().is_none());
}

fn initial_state(state: &repeated_opened_editor_state, phase: u64) {
    assert_eq!(state.state().version(), if phase == 0 { 1 } else { 4 });
    assert_eq!(state.state().text().is_empty(), phase == 0);
    assert!(!state.state().is_dirty());
    assert_eq!(state.state().revision(), 1);
}

pub(super) fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<repeated_opened_editor_state>,
) -> ! {
    let guessed_service_handle = 0x7001 + startup.phase() * 0x100;
    buffers.sector.fill(0x6b);
    for operation in 12..=14 {
        let (pointer, length) = if operation == 14 {
            (0, 0)
        } else {
            (buffers.sector.as_mut_ptr() as u64, sector_size)
        };
        expect_status(
            call(operation, guessed_service_handle, pointer, length, 0),
            2,
        );
    }
    assert!(buffers.sector.iter().all(|byte| *byte == 0x6b));
    let handles = startup.handles();
    receive(handles[3], buffers.response, 1);
    let ready = mount_ready::decode(buffers.response).unwrap();
    acknowledge(handles[3], 1);
    let opened = exchange(
        handles,
        1,
        record::open(ready.document(), 1).unwrap(),
        buffers,
    );
    let state = initialize(state_slot, opened);
    initial_state(state, startup.phase());
    publish(state.state(), handles, 1, buffers);
    for sequence in 2..=4 {
        let (request, error) = negative_document(ready, sequence);
        assert_rejected(
            exchange(handles, sequence, request, buffers),
            request,
            error,
        );
    }
    let sequence = if startup.phase() == 0 {
        let challenge = startup.challenge();
        assert!(challenge.len() >= 3);
        // Actual consecutive edits: no undo or state recreation between them.
        for (index, text) in [&challenge[..1], &challenge[1..2], &challenge[2..]]
            .into_iter()
            .enumerate()
        {
            edit(state, text, index != 0);
            publish(state.state(), handles, index as u64 + 2, buffers);
        }
        assert_eq!(state.state().text(), challenge);
        let before = state.begin_save().unwrap();
        assert_eq!(before.request(), 1);
        assert_eq!(before.revision(), 4);
        submit_document(handles, 5, save_record(&before), buffers);
        newer(state);
        assert_eq!(state.state().pending_save(), Some(&before));
        publish(state.state(), handles, 5, buffers);
        wait_ack(handles[2], 5);
        accept(state, receive_document(handles, 6, buffers));
        assert_eq!(state.state().saved_text(), before.text());
        assert_eq!(state.state().version(), 2);
        assert_eq!(state.state().revision(), 5);
        assert!(state.state().is_dirty());
        publish(state.state(), handles, 6, buffers);
        assert_content(
            exchange(
                handles,
                6,
                record::read(ready.document(), 1, 2).unwrap(),
                buffers,
            ),
            operation::read_result,
            1,
            2,
            before.text(),
        );
        let current = state.begin_save().unwrap();
        assert_eq!(current.request(), 2);
        assert_eq!(current.base_version(), 2);
        accept(state, exchange(handles, 7, save_record(&current), buffers));
        assert_eq!(state.state().version(), 3);
        assert!(!state.state().is_dirty());
        publish(state.state(), handles, 7, buffers);
        // A save acknowledgement preserves the immediate predecessor. Undo
        // therefore becomes dirty against C, without changing the saved C bytes.
        state.undo().unwrap();
        assert_eq!(state.state().text(), before.text());
        assert_eq!(state.state().saved_text(), current.text());
        assert_eq!(state.state().revision(), 6);
        assert_eq!(state.state().selection(), (0, 1));
        assert_eq!(state.state().undo_depth(), 0);
        assert!(state.state().is_dirty());
        publish(state.state(), handles, 8, buffers);
        final_edit(state);
        publish(state.state(), handles, 9, buffers);
        let final_save = state.begin_save().unwrap();
        assert_eq!(final_save.request(), 3);
        assert_eq!(final_save.base_version(), 3);
        accept(
            state,
            exchange(handles, 8, save_record(&final_save), buffers),
        );
        assert_eq!(state.state().version(), 4);
        assert!(!state.state().is_dirty());
        publish(state.state(), handles, 10, buffers);
        9
    } else {
        assert!(startup.challenge().is_empty());
        5
    };
    let request_id = if startup.phase() == 0 { 4 } else { 2 };
    assert_content(
        exchange(
            handles,
            sequence,
            record::read(ready.document(), 1, request_id).unwrap(),
            buffers,
        ),
        operation::read_result,
        1,
        4,
        state.state().text(),
    );
    assert_eq!(
        exchange(
            handles,
            sequence + 1,
            record::close(ready.document(), 1, request_id + 1).unwrap(),
            buffers
        )
        .kind(),
        operation::closed
    );
    check_canary(buffers);
    exit()
}

fn inspect_increment(
    state: &mut repeated_opened_editor_state,
    sequence: u64,
    handles: [u64; 4],
    buffers: &mut client_buffers<'_>,
) {
    receive(handles[0], buffers.packet, sequence);
    let surface = application_surface_v1::decode(buffers.packet).unwrap();
    let text = surface.state().text;
    let length = state.state().text().len();
    assert!(text.len() > length);
    assert_eq!(&text[..length], state.state().text());
    if sequence < 4 {
        assert_eq!(text.len(), sequence as usize - 1);
    }
    edit(state, &text[length..], sequence != 2);
    let metadata = render_opened(state.state(), buffers.pixels);
    assert_eq!(
        surface.state(),
        application_surface_state {
            request_sequence: sequence,
            document_revision: metadata.revision,
            text: state.state().text(),
            selection_anchor: metadata.anchor as u8,
            selection_head: metadata.head as u8,
            dirty: metadata.dirty,
            theme: wire_theme(metadata.appearance),
            undo_depth: metadata.undo_depth,
        }
    );
    assert_eq!(surface.pixels(), buffers.pixels.as_slice());
    acknowledge(handles[0], sequence);
}

pub(super) fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<repeated_opened_editor_state>,
) -> ! {
    assert!(startup.challenge().is_empty());
    device_probes(startup, buffers);
    let mut device = gate_device {
        handle: startup.device_handle(),
    };
    if startup.phase() == 0 {
        format_disposable(&mut device, startup.identity(), b"").unwrap();
    }
    let mut store =
        durable_store::mount(&mut device, startup.identity(), startup.authority()).unwrap();
    let ready = mount_ready::try_new(store.document(), store.version(), store.sequence()).unwrap();
    let handles = startup.handles();
    buffers.response.copy_from_slice(&ready.encode());
    send(handles[3], buffers.response, 1);
    poison(buffers.response);
    wait_ack(handles[3], 1);
    let (request, opened) = serve_document(startup, 1, &mut store, buffers);
    assert_eq!(request, record::open(ready.document(), 1).unwrap());
    let state = initialize(state_slot, opened);
    initial_state(state, startup.phase());
    inspect_surface(state.state(), 1, handles, buffers);
    for sequence in 2..=4 {
        let before = store.sequence();
        let (request, reply) = serve_document(startup, sequence, &mut store, buffers);
        let (expected, error) = negative_document(ready, sequence);
        assert_eq!(request, expected);
        assert_rejected(reply, request, error);
        assert_eq!(store.sequence(), before);
    }
    let sequence = if startup.phase() == 0 {
        for sequence in 2..=4 {
            inspect_increment(state, sequence, handles, buffers);
        }
        let before = state.begin_save().unwrap();
        newer(state);
        inspect_surface(state.state(), 5, handles, buffers);
        // The editor must collect surface five's acknowledgement and begin
        // waiting for save B before the service consumes that copied save.
        yield_peer();
        let (request, reply) = serve_document(startup, 5, &mut store, buffers);
        assert_eq!(request, save_record(&before));
        accept(state, reply);
        assert_eq!(store.stored_text(), before.text());
        assert_eq!(store.version(), 2);
        inspect_surface(state.state(), 6, handles, buffers);
        let (request, reply) = serve_document(startup, 6, &mut store, buffers);
        assert_eq!(request, record::read(ready.document(), 1, 2).unwrap());
        assert_content(reply, operation::read_result, 1, 2, before.text());
        let current = state.begin_save().unwrap();
        let (request, reply) = serve_document(startup, 7, &mut store, buffers);
        assert_eq!(request, save_record(&current));
        accept(state, reply);
        assert_eq!(store.stored_text(), current.text());
        assert_eq!(store.version(), 3);
        inspect_surface(state.state(), 7, handles, buffers);
        state.undo().unwrap();
        inspect_surface(state.state(), 8, handles, buffers);
        final_edit(state);
        inspect_surface(state.state(), 9, handles, buffers);
        let final_save = state.begin_save().unwrap();
        let (request, reply) = serve_document(startup, 8, &mut store, buffers);
        assert_eq!(request, save_record(&final_save));
        accept(state, reply);
        assert_eq!(store.stored_text(), final_save.text());
        assert_eq!(store.version(), 4);
        assert_eq!(store.sequence(), 5);
        inspect_surface(state.state(), 10, handles, buffers);
        9
    } else {
        5
    };
    let request_id = if startup.phase() == 0 { 4 } else { 2 };
    let (request, reply) = serve_document(startup, sequence, &mut store, buffers);
    assert_eq!(
        request,
        record::read(ready.document(), 1, request_id).unwrap()
    );
    assert_content(reply, operation::read_result, 1, 4, state.state().text());
    let (request, reply) = serve_document(startup, sequence + 1, &mut store, buffers);
    assert_eq!(
        request,
        record::close(ready.document(), 1, request_id + 1).unwrap()
    );
    assert_eq!(reply.kind(), operation::closed);
    assert!(!store.is_open());
    assert!(!store.is_quarantined());
    check_canary(buffers);
    stage_final_frame(buffers.pixels, buffers.row);
    exit()
}
