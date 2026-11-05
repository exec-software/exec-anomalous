//! Failure-only userspace policy. The kernel observes operation boundaries and
//! task ownership; it never interprets the document request or its outcome.

use super::*;
use anomalous_editor_contract::{opened_editor_error, save_outcome};
use core::cell::Cell;

unsafe extern "C" {
    fn anomalous_native_durable_client_cut_fault_instruction() -> !;
}

fn cut() -> ! {
    // SAFETY: The service previously armed this exact instruction through gate
    // eight. It deliberately terminates without returning or issuing another
    // operation. An unarmed invocation is a fatal proof failure in the kernel.
    unsafe { anomalous_native_durable_client_cut_fault_instruction() }
}

struct cut_control {
    case: u64,
    armed: Cell<bool>,
    completed: Cell<u64>,
}

impl cut_control {
    fn completed_operation(&self) {
        if !self.armed.get() {
            return;
        }
        let completed = self.completed.get().checked_add(1).unwrap();
        assert!(completed <= 4);
        self.completed.set(completed);
        if (2..=5).contains(&self.case) && completed == self.case - 1 {
            cut();
        }
        if completed == 4 {
            assert_eq!(self.case, 6);
            self.armed.set(false);
        }
    }
}

struct failure_device<'a> {
    gate: gate_device,
    control: &'a cut_control,
}

impl sector_io for failure_device<'_> {
    fn read_sector(&mut self, sector: u64, out: &mut [u8; sector_size]) -> Result<(), io_error> {
        assert!(!self.control.armed.get());
        self.gate.read_sector(sector, out)
    }

    fn write_sector(&mut self, sector: u64, bytes: &[u8; sector_size]) -> Result<(), io_error> {
        self.gate.write_sector(sector, bytes)?;
        self.control.completed_operation();
        Ok(())
    }

    fn flush(&mut self) -> Result<(), io_error> {
        self.gate.flush()?;
        self.control.completed_operation();
        Ok(())
    }
}

fn editor_device_probes(startup: &startup_descriptor, buffers: &mut client_buffers<'_>) {
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
}

fn check_open(phase: u64, ready: mount_ready, state: &opened_editor_state) {
    assert_eq!(state.version(), ready.version());
    if phase == 0 {
        assert_eq!(state.version(), 1);
        assert!(state.text().is_empty());
    } else {
        assert_eq!(phase, 2);
        assert!(matches!(state.version(), 1 | 2));
        assert_eq!(state.text().is_empty(), state.version() == 1);
    }
    assert!(!state.is_dirty());
}

fn advance_to_unsaved(state: &mut opened_editor_state, snapshot: &save_snapshot) {
    assert_eq!(state.text(), snapshot.text());
    let mut newer = [0; anomalous_document_protocol::text_capacity];
    let length = snapshot.text().len();
    assert!(length > 0);
    newer[..length].copy_from_slice(snapshot.text());
    newer[0] = if newer[0] == b'~' { b'!' } else { newer[0] + 1 };
    state.undo().unwrap();
    assert!(state.text().is_empty());
    state.insert(&newer[..length]).unwrap();
    assert_ne!(state.text(), snapshot.text());
    assert_eq!(state.revision(), 4);
    assert_eq!(state.selection(), (length, length));
    assert_eq!(state.undo_depth(), 1);
    assert_eq!(state.pending_save(), Some(snapshot));
    assert!(state.is_dirty());
}

fn wait_save_ack(channel: u64, sequence: u64) -> bool {
    for _ in 0..wait_limit {
        let reply = call(6, channel, 0, 0, sequence);
        match reply.status {
            0 => {
                assert_eq!(reply.returned_sequence, sequence);
                return true;
            }
            1 => yield_peer(),
            5 => return false,
            _ => panic!("save acknowledgement rejected"),
        }
    }
    panic!("save acknowledgement wait exhausted");
}

fn unknown_snapshot(
    state: &mut opened_editor_state,
    snapshot: &save_snapshot,
    handles: [u64; 4],
    buffers: &mut client_buffers<'_>,
) -> ! {
    // Both directions are revoked. A receive must not expose even part of an
    // old reply or mutate the destination when the route is closed.
    buffers.response.fill(0x77);
    expect_status(
        call(
            2,
            handles[3],
            buffers.response.as_mut_ptr() as u64,
            record_size,
            0,
        ),
        5,
    );
    assert!(buffers.response.iter().all(|byte| *byte == 0x77));
    state
        .mark_save_unknown(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
        )
        .unwrap();
    assert_eq!(state.pending_save(), Some(snapshot));
    assert_eq!(state.save_outcome(), save_outcome::unknown);
    assert!(state.saved_text().is_empty());
    assert_eq!(state.version(), 1);
    assert_eq!(state.revision(), 4);
    assert_ne!(state.text(), snapshot.text());
    assert!(state.is_dirty());
    assert_eq!(state.begin_save(), Err(opened_editor_error::save_pending));
    assert_eq!(
        state.reject_save(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
        ),
        Err(opened_editor_error::outcome_unknown),
    );
    assert_eq!(state.pending_save(), Some(snapshot));
    assert_eq!(state.save_outcome(), save_outcome::unknown);
    assert!(render_opened(state, buffers.pixels).dirty);
    check_canary(buffers);
    // This survivor-only route is admitted after the actual service fault and
    // route revocation. It reports the unsaved C buffer, never a save success.
    stage_final_frame(buffers.pixels, buffers.row);
    exit()
}

pub(super) fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<opened_editor_state>,
) -> ! {
    editor_device_probes(startup, buffers);
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
    let state = initialize_state(state_slot, opened);
    check_open(startup.phase(), ready, state);
    publish(state, handles, 1, buffers);
    for sequence in 2..=4 {
        let (request, error) = negative_document(ready, sequence);
        assert_rejected(
            exchange(handles, sequence, request, buffers),
            request,
            error,
        );
    }
    let sequence = if startup.phase() == 0 {
        state.insert(startup.challenge()).unwrap();
        assert_eq!(state.text(), startup.challenge());
        publish(state, handles, 2, buffers);
        let snapshot = state.begin_save().unwrap();
        assert_eq!(snapshot.request(), 1);
        assert_eq!(snapshot.base_version(), 1);
        assert_eq!(snapshot.revision(), 2);
        submit_document(handles, 5, save_record(&snapshot), buffers);
        advance_to_unsaved(state, &snapshot);
        publish(state, handles, 3, buffers);
        if !wait_save_ack(handles[2], 5) {
            assert_ne!(startup.case(), 6);
            unknown_snapshot(state, &snapshot, handles, buffers);
        }
        assert_eq!(startup.case(), 6);
        let reply = receive_document(handles, 6, buffers);
        accept_save(state, reply);
        assert_eq!(state.saved_text(), snapshot.text());
        assert_eq!(state.version(), 2);
        assert_eq!(state.revision(), 4);
        assert_eq!(state.save_outcome(), save_outcome::idle);
        assert!(state.pending_save().is_none());
        assert!(state.is_dirty());
        publish(state, handles, 4, buffers);
        6
    } else {
        assert!(startup.challenge().is_empty());
        5
    };
    assert_content(
        exchange(
            handles,
            sequence,
            record::read(ready.document(), 1, 2).unwrap(),
            buffers,
        ),
        operation::read_result,
        1,
        state.version(),
        state.saved_text(),
    );
    assert_eq!(
        exchange(
            handles,
            sequence + 1,
            record::close(ready.document(), 1, 3).unwrap(),
            buffers
        )
        .kind(),
        operation::closed,
    );
    assert_eq!(state.is_dirty(), startup.phase() == 0);
    check_canary(buffers);
    exit()
}

fn reply_document(
    handles: [u64; 4],
    sequence: u64,
    reply: record,
    buffers: &mut client_buffers<'_>,
) {
    acknowledge(handles[2], sequence);
    buffers.response.copy_from_slice(&reply.encode());
    send(handles[3], buffers.response, sequence + 1);
    poison(buffers.response);
    wait_ack(handles[3], sequence + 1);
}

fn serve_document(
    startup: &startup_descriptor,
    sequence: u64,
    store: &mut durable_store<'_, failure_device<'_>>,
    buffers: &mut client_buffers<'_>,
) -> (record, record) {
    let handles = startup.handles();
    receive(handles[2], buffers.request, sequence);
    let request = record::decode(buffers.request).unwrap();
    let reply = match store.execute(startup.authority(), &request) {
        Ok(reply) => reply,
        Err(durable_error::rejected(error)) => record::rejection_for(&request, error).unwrap(),
        Err(_) => panic!("unexpected durable failure"),
    };
    reply_document(handles, sequence, reply, buffers);
    (request, reply)
}

fn inspect_edited_surface(
    state: &mut opened_editor_state,
    handles: [u64; 4],
    buffers: &mut client_buffers<'_>,
) {
    receive(handles[0], buffers.packet, 2);
    let surface = application_surface_v1::decode(buffers.packet).unwrap();
    assert!(!surface.state().text.is_empty());
    state.insert(surface.state().text).unwrap();
    let metadata = render_opened(state, buffers.pixels);
    assert_eq!(surface.state().request_sequence, 2);
    assert_eq!(surface.state().document_revision, metadata.revision);
    assert_eq!(surface.state().text, state.text());
    assert_eq!(surface.state().selection_anchor, metadata.anchor as u8);
    assert_eq!(surface.state().selection_head, metadata.head as u8);
    assert_eq!(surface.state().dirty, metadata.dirty);
    assert_eq!(surface.state().theme, wire_theme(metadata.appearance));
    assert_eq!(surface.state().undo_depth, metadata.undo_depth);
    assert_eq!(surface.pixels(), buffers.pixels.as_slice());
    acknowledge(handles[0], 2);
}

fn poisoned_device_probes(handle: u64, buffers: &mut client_buffers<'_>) {
    buffers.sector.fill(0x6b);
    for operation in 12..=14 {
        let (pointer, length) = if operation == 14 {
            (0, 0)
        } else {
            (buffers.sector.as_mut_ptr() as u64, sector_size)
        };
        expect_status(call(operation, handle, pointer, length, 0), 6);
    }
    assert!(buffers.sector.iter().all(|byte| *byte == 0x6b));
}

fn execute_save(
    startup: &startup_descriptor,
    store: &mut durable_store<'_, failure_device<'_>>,
    control: &cut_control,
    snapshot: &save_snapshot,
    buffers: &mut client_buffers<'_>,
) -> record {
    let handles = startup.handles();
    receive(handles[2], buffers.request, 5);
    let request = record::decode(buffers.request).unwrap();
    assert_eq!(request, save_record(snapshot));
    assert!(!control.armed.get());
    assert_eq!(control.completed.get(), 0);
    expect_status(call(8, startup.case(), 0, 0, 0), 0);
    control.armed.set(true);
    if startup.case() == 1 {
        cut();
    }
    let result = store.execute(startup.authority(), &request);
    if startup.case() == 7 {
        assert_eq!(result, Err(durable_error::outcome_unknown));
        assert!(store.is_quarantined());
        assert_eq!(control.completed.get(), 3);
        assert_eq!(store.version(), 1);
        assert_eq!(store.sequence(), 2);
        assert!(store.stored_text().is_empty());
        // No further store.execute call is made: the instance remains poisoned
        // until destruction. These direct probes prove device quarantine too.
        poisoned_device_probes(startup.device_handle(), buffers);
        cut();
    }
    assert_eq!(startup.case(), 6);
    assert!(!control.armed.get());
    assert_eq!(control.completed.get(), 4);
    let reply = result.unwrap();
    assert_eq!(reply.kind(), operation::saved);
    assert_eq!(store.version(), 2);
    assert_eq!(store.sequence(), 3);
    assert_eq!(store.stored_text(), snapshot.text());
    reply_document(handles, 5, reply, buffers);
    reply
}

pub(super) fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<opened_editor_state>,
) -> ! {
    assert!(startup.challenge().is_empty());
    device_probes(startup, buffers);
    let control = cut_control {
        case: startup.case(),
        armed: Cell::new(false),
        completed: Cell::new(0),
    };
    let mut device = failure_device {
        gate: gate_device {
            handle: startup.device_handle(),
        },
        control: &control,
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
    let state = initialize_state(state_slot, opened);
    check_open(startup.phase(), ready, state);
    inspect_surface(state, 1, handles, buffers);
    for sequence in 2..=4 {
        let before = store.sequence();
        let (request, reply) = serve_document(startup, sequence, &mut store, buffers);
        let (expected, error) = negative_document(ready, sequence);
        assert_eq!(request, expected);
        assert_rejected(reply, request, error);
        assert_eq!(store.sequence(), before);
    }
    let sequence = if startup.phase() == 0 {
        inspect_edited_surface(state, handles, buffers);
        let snapshot = state.begin_save().unwrap();
        advance_to_unsaved(state, &snapshot);
        // C must have crossed the copied surface route and been checked before
        // request five is delivered. Its retained save bytes remain exactly B.
        inspect_surface(state, 3, handles, buffers);
        // Let the editor collect C's surface acknowledgement before a cut can
        // revoke that route. Its next wait is then specifically for save five.
        yield_peer();
        let reply = execute_save(startup, &mut store, &control, &snapshot, buffers);
        accept_save(state, reply);
        assert_eq!(state.saved_text(), snapshot.text());
        assert!(state.is_dirty());
        assert_eq!(state.revision(), 4);
        inspect_surface(state, 4, handles, buffers);
        6
    } else {
        5
    };
    let (request, reply) = serve_document(startup, sequence, &mut store, buffers);
    assert_eq!(request, record::read(ready.document(), 1, 2).unwrap());
    assert_content(
        reply,
        operation::read_result,
        1,
        state.version(),
        state.saved_text(),
    );
    let (request, reply) = serve_document(startup, sequence + 1, &mut store, buffers);
    assert_eq!(request, record::close(ready.document(), 1, 3).unwrap());
    assert_eq!(reply.kind(), operation::closed);
    assert!(!store.is_open());
    assert!(!store.is_quarantined());
    assert_eq!(state.is_dirty(), startup.phase() == 0);
    check_canary(buffers);
    stage_final_frame(buffers.pixels, buffers.row);
    exit()
}
