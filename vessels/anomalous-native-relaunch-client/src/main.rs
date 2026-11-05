#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Bounded native document client and independent RAM document/render service.
//! Documents arrive through copied replies, not editor launch text. Save replies
//! acknowledge exact retained snapshots; they do not claim disk durability.

use anomalous_abi::application::{
    application_input_action, application_input_v1, application_input_v1_size,
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_document_protocol::relaunch::{replacement_error, validate_successor};
use anomalous_document_protocol::{
    arena_byte_count, document_store, operation, record, record_size, service_error,
    startup_descriptor, startup_size,
};
use anomalous_editor_contract::{
    frame_byte_count, frame_width, opened_editor_state, render_opened, save_acknowledgement,
    save_snapshot, theme,
};
use anomalous_native_runtime::arena;
use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));
const scaled_row_bytes: usize = frame_width * 3 * 3;
const wait_limit: usize = 256;

#[repr(C)]
struct call_record {
    operation: u64,
    channel: u64,
    pointer: u64,
    length: u64,
    sequence: u64,
    status: u64,
    returned_sequence: u64,
    returned_length: u64,
}

unsafe extern "C" {
    fn anomalous_native_relaunch_client_call(record: *mut call_record);
    fn anomalous_native_relaunch_client_arm_before(scenario: u64, reserved: u64) -> !;
    fn anomalous_native_relaunch_client_arm_after(scenario: u64, reserved: u64) -> !;
}

// References point to disjoint arena allocations; no bulk buffer lives on the
// 8 KiB task stack or in a hidden static allocator.
struct client_buffers<'a> {
    pixels: &'a mut [u8; frame_byte_count],
    packet: &'a mut [u8; application_surface_v1_size],
    input: &'a mut [u8; application_input_v1_size],
    row: &'a mut [u8; scaled_row_bytes],
    canary: &'a mut [u8; 32],
    request: &'a mut [u8; record_size],
    response: &'a mut [u8; record_size],
}

fn check_canary(buffers: &client_buffers<'_>) {
    assert!(buffers.canary.iter().all(|byte| *byte == 0x39));
}

fn call(operation: u64, channel: u64, pointer: u64, length: usize, sequence: u64) -> call_record {
    let mut record = call_record {
        operation,
        channel,
        pointer,
        length: length as u64,
        sequence,
        status: u64::MAX,
        returned_sequence: 0,
        returned_length: 0,
    };
    // SAFETY: Assembly preserves SysV callee-saved registers and writes only the
    // three result fields in this live private record. Kernel copy admission
    // validates packet pointers; the stub never dereferences those pointers.
    unsafe { anomalous_native_relaunch_client_call(&mut record) };
    record
}

fn expect_status(reply: call_record, expected: u64) -> call_record {
    assert_eq!(reply.status, expected);
    reply
}

fn yield_peer() {
    expect_status(call(4, 0, 0, 0, 0), 0);
}

fn wait_ack(channel: u64, sequence: u64) {
    for _ in 0..wait_limit {
        let reply = call(6, channel, 0, 0, sequence);
        match reply.status {
            0 => {
                assert_eq!(reply.returned_sequence, sequence);
                return;
            }
            1 => yield_peer(),
            _ => panic!("acknowledgement rejected"),
        }
    }
    panic!("acknowledgement wait exhausted");
}

fn receive(channel: u64, bytes: &mut [u8], sequence: u64) {
    for _ in 0..wait_limit {
        let reply = call(2, channel, bytes.as_mut_ptr() as u64, bytes.len(), 0);
        match reply.status {
            0 => {
                assert_eq!(reply.returned_sequence, sequence);
                assert_eq!(reply.returned_length, bytes.len() as u64);
                return;
            }
            1 => yield_peer(),
            _ => panic!("receive rejected"),
        }
    }
    panic!("receive wait exhausted");
}

fn acknowledge(channel: u64, sequence: u64) {
    let reply = expect_status(call(3, channel, 0, 0, sequence), 0);
    assert_eq!(reply.returned_sequence, sequence);
}

fn send(channel: u64, bytes: &[u8], sequence: u64) {
    let reply = expect_status(
        call(1, channel, bytes.as_ptr() as u64, bytes.len(), sequence),
        0,
    );
    assert_eq!(reply.returned_sequence, sequence);
    assert_eq!(reply.returned_length, bytes.len() as u64);
}

fn wire_theme(value: theme) -> application_theme {
    match value {
        theme::light => application_theme::light,
        theme::dark => application_theme::dark,
    }
}

fn poison(bytes: &mut [u8]) {
    for byte in bytes {
        // SAFETY: Private uniquely borrowed allocated bytes. This deliberate
        // write must occur before yielding to the receiver of the copied packet.
        unsafe { ptr::write_volatile(byte, 0xa5) };
    }
}

fn apply(state: &mut opened_editor_state, action: application_input_action<'_>) {
    match action {
        application_input_action::replace_selection(text) => state.insert(text).unwrap(),
        application_input_action::set_selection { anchor, head } => {
            state
                .set_selection(usize::from(anchor), usize::from(head))
                .unwrap();
        }
        application_input_action::set_theme(value) => state.set_theme(match value {
            application_theme::light => theme::light,
            application_theme::dark => theme::dark,
        }),
        application_input_action::undo => state.undo().unwrap(),
    }
}

fn publish(
    state: &opened_editor_state,
    handles: [u64; 4],
    sequence: u64,
    buffers: &mut client_buffers<'_>,
) {
    check_canary(buffers);
    let metadata = render_opened(state, buffers.pixels);
    application_surface_v1::try_new(
        application_surface_state {
            request_sequence: sequence,
            document_revision: metadata.revision,
            text: state.text(),
            selection_anchor: metadata.anchor as u8,
            selection_head: metadata.head as u8,
            dirty: metadata.dirty,
            theme: wire_theme(metadata.appearance),
            undo_depth: metadata.undo_depth,
        },
        buffers.pixels,
    )
    .unwrap()
    .encode_into(buffers.packet)
    .unwrap();
    send(handles[0], buffers.packet, sequence);
    poison(buffers.packet);
    wait_ack(handles[0], sequence);
}

fn receive_input(handles: [u64; 4], sequence: u64, buffers: &mut client_buffers<'_>) {
    receive(handles[1], buffers.input, sequence);
    assert_eq!(
        application_input_v1::decode(buffers.input)
            .unwrap()
            .sequence(),
        sequence
    );
}

fn submit_document(
    handles: [u64; 4],
    sequence: u64,
    request: record,
    buffers: &mut client_buffers<'_>,
) {
    buffers.request.copy_from_slice(&request.encode());
    send(handles[2], buffers.request, sequence);
    poison(buffers.request);
}

fn receive_document(handles: [u64; 4], sequence: u64, buffers: &mut client_buffers<'_>) -> record {
    receive(handles[3], buffers.response, sequence);
    let reply = record::decode(buffers.response).unwrap();
    acknowledge(handles[3], sequence);
    reply
}

fn exchange(
    handles: [u64; 4],
    sequence: u64,
    request: record,
    buffers: &mut client_buffers<'_>,
) -> record {
    submit_document(handles, sequence, request, buffers);
    wait_ack(handles[2], sequence);
    let reply = receive_document(handles, sequence, buffers);
    assert_eq!(reply.document(), request.document());
    assert_eq!(reply.request(), request.request());
    reply
}

fn save_record(snapshot: &save_snapshot) -> record {
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

fn accept_save(state: &mut opened_editor_state, reply: record) {
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
}

fn assert_content(reply: record, kind: operation, generation: u64, version: u64, text: &[u8]) {
    assert_eq!(reply.kind(), kind);
    assert_eq!(reply.status(), None);
    assert_eq!(reply.generation(), generation);
    assert_eq!(reply.version(), version);
    assert_eq!(reply.text(), text);
}

fn assert_rejected(reply: record, request: record, error: service_error) {
    assert_eq!(reply, record::rejection_for(&request, error).unwrap());
}

const initial: &[u8] = b"const before = 1;";
const committed: &[u8] = b"const after = 1;";
const pending: &[u8] = b"const later = 1;";
const final_text: &[u8] = b"const newer = 1;";

fn selection(anchor: u8, head: u8) -> application_input_action<'static> {
    application_input_action::set_selection { anchor, head }
}

fn receive_command(
    state: &mut opened_editor_state,
    handles: [u64; 4],
    sequence: u64,
    expected: application_input_action<'_>,
    buffers: &mut client_buffers<'_>,
) {
    receive_input(handles, sequence, buffers);
    let decoded = application_input_v1::decode(buffers.input).unwrap();
    assert_eq!(decoded.action(), expected);
    apply(state, decoded.action());
    acknowledge(handles[1], sequence);
}

fn old_editor(
    startup: &startup_descriptor,
    state: &mut opened_editor_state,
    buffers: &mut client_buffers<'_>,
) -> ! {
    let handles = startup.handles();
    assert_eq!(state.text(), initial);
    assert_eq!(state.version(), 1);
    publish(state, handles, 1, buffers);
    receive_command(state, handles, 1, selection(6, 12), buffers);
    receive_command(
        state,
        handles,
        2,
        application_input_action::replace_selection(b"after"),
        buffers,
    );
    assert_eq!(state.text(), committed);
    publish(state, handles, 2, buffers);
    let snapshot = state.begin_save().unwrap();
    accept_save(state, exchange(handles, 2, save_record(&snapshot), buffers));
    assert_eq!(state.version(), 2);
    assert!(!state.is_dirty());
    publish(state, handles, 3, buffers);
    receive_command(state, handles, 3, application_input_action::undo, buffers);
    receive_command(state, handles, 4, selection(6, 12), buffers);
    receive_command(
        state,
        handles,
        5,
        application_input_action::replace_selection(b"later"),
        buffers,
    );
    assert_eq!(state.text(), pending);
    assert_eq!(state.saved_text(), committed);
    assert!(state.is_dirty());
    publish(state, handles, 4, buffers);
    if startup.scenario() == 0 {
        exit();
    }
    let snapshot = state.begin_save().unwrap();
    assert_eq!(snapshot.request(), 2);
    submit_document(handles, 3, save_record(&snapshot), buffers);
    if startup.scenario() == 1 {
        // SAFETY: The kernel arms this exact editor-only before-commit UD2
        // after observing the third request queued, before any service receive.
        unsafe { anomalous_native_relaunch_client_arm_before(1, 0) }
    }
    assert_eq!(startup.scenario(), 2);
    // A separate input handshake proves the service has reached its cut point.
    // The semantic saved reply stays unread in its route when this task faults.
    receive_command(state, handles, 6, selection(11, 11), buffers);
    state
        .mark_save_unknown(
            snapshot.document(),
            snapshot.generation(),
            snapshot.request(),
            snapshot.base_version(),
        )
        .unwrap();
    assert!(state.is_dirty());
    assert_eq!(state.saved_text(), committed);
    // SAFETY: Exact editor-only after-commit UD2, with the copied saved reply
    // still queued. No result or private model is transferred to the successor.
    unsafe { anomalous_native_relaunch_client_arm_after(2, 0) }
}

fn replacement_editor(
    startup: &startup_descriptor,
    state: &mut opened_editor_state,
    buffers: &mut client_buffers<'_>,
) -> ! {
    let handles = startup.handles();
    let recovered = if startup.scenario() == 2 {
        pending
    } else {
        committed
    };
    let version = if startup.scenario() == 2 { 3 } else { 2 };
    assert_eq!(state.text(), recovered);
    assert_eq!(state.version(), version);
    assert_eq!(state.revision(), 1);
    assert_eq!(state.selection(), (0, 0));
    assert_eq!(state.undo_depth(), 0);
    assert!(!state.is_dirty());
    assert!(state.pending_save().is_none());
    let old_save = record::save(startup.document(), 1, 2, 2, 4, pending).unwrap();
    // The same numeric route family and document handle from the retired
    // editor grant no access to this fresh task or the new service route.
    buffers.request.copy_from_slice(&old_save.encode());
    expect_status(
        call(
            1,
            handles[2] - 0x10,
            buffers.request.as_ptr() as u64,
            record_size,
            1,
        ),
        2,
    );
    assert_rejected(
        exchange(handles, 2, old_save, buffers),
        old_save,
        service_error::stale_handle,
    );
    publish(state, handles, 1, buffers);
    receive_command(state, handles, 1, selection(6, 11), buffers);
    receive_command(
        state,
        handles,
        2,
        application_input_action::replace_selection(b"newer"),
        buffers,
    );
    assert_eq!(state.text(), final_text);
    publish(state, handles, 2, buffers);
    let snapshot = state.begin_save().unwrap();
    assert_eq!(snapshot.request(), 1);
    accept_save(state, exchange(handles, 3, save_record(&snapshot), buffers));
    assert_eq!(state.version(), version + 1);
    assert!(!state.is_dirty());
    publish(state, handles, 3, buffers);
    assert_content(
        exchange(
            handles,
            4,
            record::read(startup.document(), 2, 2).unwrap(),
            buffers,
        ),
        operation::read_result,
        2,
        version + 1,
        final_text,
    );
    assert_eq!(
        exchange(
            handles,
            5,
            record::close(startup.document(), 2, 3).unwrap(),
            buffers
        )
        .kind(),
        operation::closed
    );
    let opened = exchange(
        handles,
        6,
        record::open(startup.document(), 4).unwrap(),
        buffers,
    );
    assert_content(opened, operation::opened, 3, version + 1, final_text);
    *state = opened_editor_state::from_open(
        opened.document(),
        opened.generation(),
        opened.version(),
        opened.text(),
    )
    .unwrap();
    let stale = record::read(startup.document(), 2, 5).unwrap();
    assert_rejected(
        exchange(handles, 7, stale, buffers),
        stale,
        service_error::stale_handle,
    );
    assert_content(
        exchange(
            handles,
            8,
            record::read(startup.document(), 3, 6).unwrap(),
            buffers,
        ),
        operation::read_result,
        3,
        version + 1,
        final_text,
    );
    assert_eq!(
        exchange(
            handles,
            9,
            record::close(startup.document(), 3, 7).unwrap(),
            buffers
        )
        .kind(),
        operation::closed
    );
    publish(state, handles, 4, buffers);
    exit()
}

fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: *mut opened_editor_state,
) -> ! {
    for operation in 9..=11 {
        expect_status(call(operation, 0, 0, 0, 0), 2);
    }
    let opened = exchange(
        startup.handles(),
        1,
        record::open(startup.document(), 1).unwrap(),
        buffers,
    );
    assert_eq!(opened.kind(), operation::opened);
    assert!(matches!(opened.generation(), 1 | 2));
    // SAFETY: Unique aligned arena allocation, initialized exactly once before
    // creating its typed borrow. No state from the retired editor is reused.
    let state = unsafe {
        state_slot.write(
            opened_editor_state::from_open(
                opened.document(),
                opened.generation(),
                opened.version(),
                opened.text(),
            )
            .unwrap(),
        );
        &mut *state_slot
    };
    if opened.generation() == 1 {
        old_editor(startup, state, buffers)
    } else {
        replacement_editor(startup, state, buffers)
    }
}

fn serve_document(
    route: &startup_descriptor,
    sequence: u64,
    store: &mut document_store,
    buffers: &mut client_buffers<'_>,
) -> record {
    let handles = route.handles();
    receive(handles[2], buffers.request, sequence);
    let request = record::decode(buffers.request).unwrap();
    let reply = match store.execute(route.authority(), &request) {
        Ok(reply) => reply,
        Err(error) => record::rejection_for(&request, error).unwrap(),
    };
    acknowledge(handles[2], sequence);
    buffers.response.copy_from_slice(&reply.encode());
    send(handles[3], buffers.response, sequence);
    poison(buffers.response);
    wait_ack(handles[3], sequence);
    reply
}

fn service_command(
    state: &mut opened_editor_state,
    handles: [u64; 4],
    sequence: u64,
    action: application_input_action<'_>,
    buffers: &mut client_buffers<'_>,
) {
    application_input_v1::try_new(sequence, action)
        .unwrap()
        .encode_into(buffers.input)
        .unwrap();
    send(handles[1], buffers.input, sequence);
    poison(buffers.input);
    apply(state, action);
    wait_ack(handles[1], sequence);
}

fn inspect_surface(
    state: &opened_editor_state,
    sequence: u64,
    handles: [u64; 4],
    buffers: &mut client_buffers<'_>,
) {
    check_canary(buffers);
    receive(handles[0], buffers.packet, sequence);
    let surface = application_surface_v1::decode(buffers.packet).unwrap();
    let observed = surface.state();
    let metadata = render_opened(state, buffers.pixels);
    assert_eq!(observed.request_sequence, sequence);
    assert_eq!(observed.document_revision, metadata.revision);
    assert_eq!(observed.text, state.text());
    assert_eq!(
        (observed.selection_anchor, observed.selection_head),
        (metadata.anchor as u8, metadata.head as u8)
    );
    assert_eq!(observed.dirty, metadata.dirty);
    assert_eq!(observed.theme, wire_theme(metadata.appearance));
    assert_eq!(observed.undo_depth, metadata.undo_depth);
    assert_eq!(surface.pixels(), buffers.pixels.as_slice());
    acknowledge(handles[0], sequence);
}

fn observe_store(store: &document_store, expected: &[u8], version: u64) {
    // SAFETY: One live initialized arena object, read only for observation.
    // This copy never acts as another store or authority. Volatile reads make
    // committed bytes/version real RAM state before retirement maintenance.
    let observed = unsafe { ptr::read_volatile(store) };
    assert_eq!(observed.version(), version);
    assert_eq!(observed.stored_text().len(), expected.len());
    for (actual, expected) in store.stored_text().iter().zip(expected) {
        // SAFETY: Exact initialized byte in this service's retained private RAM.
        assert_eq!(unsafe { ptr::read_volatile(actual) }, *expected);
    }
}

fn replace_editor(
    startup: &startup_descriptor,
    store: &mut document_store,
    buffers: &mut client_buffers<'_>,
) -> startup_descriptor {
    let expected_text = if startup.scenario() == 2 {
        pending
    } else {
        committed
    };
    let expected_version = if startup.scenario() == 2 { 3 } else { 2 };
    observe_store(store, expected_text, expected_version);
    check_canary(buffers);
    expect_status(
        call(
            2,
            startup.handles()[2],
            buffers.request.as_mut_ptr() as u64,
            record_size,
            0,
        ),
        5,
    );
    expect_status(
        call(
            1,
            startup.handles()[1],
            buffers.input.as_ptr() as u64,
            application_input_v1_size,
            9,
        ),
        5,
    );
    expect_status(call(9, 0, 0, 0, 0), 0);
    // The service resumes on its original stack/arena/CPU continuation before
    // any successor runs. Failed metadata copies leave the receipt available.
    expect_status(call(4, 0, 0, 0, 0), 4);
    expect_status(call(5, 0, 0, 0, 0), 4);
    check_canary(buffers);
    observe_store(store, expected_text, expected_version);
    buffers.response.fill(0x6b);
    expect_status(call(10, 0, 0, startup_size, 0), 3);
    assert!(buffers.response.iter().all(|byte| *byte == 0x6b));
    expect_status(
        call(
            10,
            0,
            buffers.response.as_mut_ptr() as u64,
            startup_size - 1,
            0,
        ),
        4,
    );
    assert!(buffers.response.iter().all(|byte| *byte == 0x6b));
    let receipt = expect_status(
        call(10, 0, buffers.response.as_mut_ptr() as u64, startup_size, 0),
        0,
    );
    assert_eq!(receipt.returned_length, startup_size as u64);
    let successor = startup_descriptor::decode(buffers.response).unwrap();
    validate_successor(startup, &successor).unwrap();
    assert_eq!(
        receipt.returned_sequence,
        successor.authority().generation()
    );
    for (old, new) in startup.handles().iter().zip(successor.handles()) {
        assert_eq!(old.checked_add(0x10), Some(new));
    }
    assert_eq!(
        store.reassign_editor(successor.authority(), startup.authority()),
        Err(replacement_error::stale_authority)
    );
    store
        .reassign_editor(startup.authority(), successor.authority())
        .unwrap();
    assert!(!store.is_open());
    assert_eq!(
        store.reassign_editor(startup.authority(), successor.authority()),
        Err(replacement_error::stale_authority)
    );
    let replay = record::save(startup.document(), 1, 2, 2, 4, pending).unwrap();
    assert_eq!(
        store.execute(startup.authority(), &replay),
        Err(service_error::wrong_authority)
    );
    observe_store(store, expected_text, expected_version);
    expect_status(call(11, 0, 0, 0, startup.authority().generation()), 4);
    expect_status(call(11, 0, 0, 0, successor.authority().generation()), 0);
    expect_status(call(11, 0, 0, 0, successor.authority().generation()), 4);
    expect_status(call(9, 0, 0, 0, 0), 4);
    expect_status(
        call(
            1,
            startup.handles()[1],
            buffers.input.as_ptr() as u64,
            application_input_v1_size,
            1,
        ),
        2,
    );
    successor
}

fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state: &mut opened_editor_state,
    store: &mut document_store,
) -> ! {
    assert_eq!(startup.seed(), initial);
    assert!(startup.scenario() <= 2);
    let handles = startup.handles();
    for operation in 9..=11 {
        expect_status(call(operation, 0, 0, 0, 0), 4);
    }
    assert_content(
        serve_document(startup, 1, store, buffers),
        operation::opened,
        1,
        1,
        initial,
    );
    inspect_surface(state, 1, handles, buffers);
    service_command(state, handles, 1, selection(6, 12), buffers);
    service_command(
        state,
        handles,
        2,
        application_input_action::replace_selection(b"after"),
        buffers,
    );
    inspect_surface(state, 2, handles, buffers);
    state.begin_save().unwrap();
    accept_save(state, serve_document(startup, 2, store, buffers));
    observe_store(store, committed, 2);
    inspect_surface(state, 3, handles, buffers);
    service_command(state, handles, 3, application_input_action::undo, buffers);
    service_command(state, handles, 4, selection(6, 12), buffers);
    service_command(
        state,
        handles,
        5,
        application_input_action::replace_selection(b"later"),
        buffers,
    );
    inspect_surface(state, 4, handles, buffers);
    if startup.scenario() == 2 {
        receive(handles[2], buffers.request, 3);
        let request = record::decode(buffers.request).unwrap();
        assert_eq!(
            request,
            record::save(startup.document(), 1, 2, 2, 4, pending).unwrap()
        );
        let reply = store.execute(startup.authority(), &request).unwrap();
        assert_eq!(reply.kind(), operation::saved);
        observe_store(store, pending, 3);
        acknowledge(handles[2], 3);
        buffers.response.copy_from_slice(&reply.encode());
        send(handles[3], buffers.response, 3);
        poison(buffers.response);
        application_input_v1::try_new(6, selection(11, 11))
            .unwrap()
            .encode_into(buffers.input)
            .unwrap();
        send(handles[1], buffers.input, 6);
        poison(buffers.input);
    }
    // The old editor consumes the last surface ACK, then exits or faults. In
    // case2 it consumes only input6; response3 remains queued and is revoked.
    yield_peer();
    let successor = replace_editor(startup, store, buffers);
    let handles = successor.handles();
    yield_peer();
    let opened = serve_document(&successor, 1, store, buffers);
    let version = if startup.scenario() == 2 { 3 } else { 2 };
    let recovered = if startup.scenario() == 2 {
        pending
    } else {
        committed
    };
    assert_content(opened, operation::opened, 2, version, recovered);
    *state = opened_editor_state::from_open(
        opened.document(),
        opened.generation(),
        opened.version(),
        opened.text(),
    )
    .unwrap();
    assert_eq!(
        serve_document(&successor, 2, store, buffers).status(),
        Some(service_error::stale_handle)
    );
    inspect_surface(state, 1, handles, buffers);
    service_command(state, handles, 1, selection(6, 11), buffers);
    service_command(
        state,
        handles,
        2,
        application_input_action::replace_selection(b"newer"),
        buffers,
    );
    inspect_surface(state, 2, handles, buffers);
    state.begin_save().unwrap();
    accept_save(state, serve_document(&successor, 3, store, buffers));
    observe_store(store, final_text, version + 1);
    inspect_surface(state, 3, handles, buffers);
    assert_content(
        serve_document(&successor, 4, store, buffers),
        operation::read_result,
        2,
        version + 1,
        final_text,
    );
    assert_eq!(
        serve_document(&successor, 5, store, buffers).kind(),
        operation::closed
    );
    let opened = serve_document(&successor, 6, store, buffers);
    assert_content(opened, operation::opened, 3, version + 1, final_text);
    *state = opened_editor_state::from_open(
        opened.document(),
        opened.generation(),
        opened.version(),
        opened.text(),
    )
    .unwrap();
    assert_eq!(
        serve_document(&successor, 7, store, buffers).status(),
        Some(service_error::stale_handle)
    );
    assert_content(
        serve_document(&successor, 8, store, buffers),
        operation::read_result,
        3,
        version + 1,
        final_text,
    );
    assert_eq!(
        serve_document(&successor, 9, store, buffers).kind(),
        operation::closed
    );
    assert!(!store.is_open());
    inspect_surface(state, 4, handles, buffers);
    yield_peer();
    check_canary(buffers);
    observe_store(store, final_text, version + 1);
    let surface = application_surface_v1::decode(buffers.packet).unwrap();
    stage_final_frame(surface.pixels(), buffers.row);
    exit()
}

fn stage_final_frame(pixels: &[u8], row: &mut [u8; scaled_row_bytes]) {
    for y in 0..24 {
        for x in 0..384 {
            let source = ((y / 3) * frame_width + x / 3) * 3;
            row[x * 3..x * 3 + 3].copy_from_slice(&pixels[source..source + 3]);
        }
        expect_status(call(7, y as u64, row.as_ptr() as u64, row.len(), 0), 0);
    }
}

fn exit() -> ! {
    call(5, 0, 0, 0, 0);
    panic!("terminal exit returned");
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_native_relaunch_client_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: Launch-admitted descriptor span is copied before interpretation.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    assert!(startup.scenario() <= 2);
    // SAFETY: The independently admitted private RW/NX arena is disjoint from
    // code/data/descriptor/stack. Entry forms its sole mutable borrow once.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_byte_count as usize)
    };
    for byte in storage.iter() {
        // SAFETY: Retained readable RAM, observed before the first allocation.
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let mut allocator = arena::new(storage);
    let state_slot = allocator
        .allocate(core::mem::size_of::<opened_editor_state>(), 64)
        .unwrap()
        .as_mut_ptr()
        .cast::<opened_editor_state>();
    let store_slot = allocator
        .allocate(core::mem::size_of::<document_store>(), 64)
        .unwrap()
        .as_mut_ptr()
        .cast::<document_store>();
    let mut buffers = client_buffers {
        pixels: allocator
            .allocate(frame_byte_count, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        packet: allocator
            .allocate(application_surface_v1_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        input: allocator
            .allocate(application_input_v1_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        row: allocator
            .allocate(scaled_row_bytes, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        canary: allocator.allocate(32, 256).unwrap().try_into().unwrap(),
        request: allocator
            .allocate(record_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
        response: allocator
            .allocate(record_size, 64)
            .unwrap()
            .try_into()
            .unwrap(),
    };
    buffers.canary.fill(0x39);
    assert_eq!(buffers.packet.as_ptr() as usize % 64, 0);
    if startup.role() == 0 {
        source(&startup, &mut buffers, state_slot)
    } else {
        // SAFETY: Each allocation has sufficient size/alignment and is disjoint.
        // Fully initialize before forming the unique typed service references.
        unsafe {
            state_slot.write(
                opened_editor_state::from_open(startup.document(), 1, 1, startup.seed()).unwrap(),
            );
            store_slot.write(
                document_store::try_new(startup.authority(), startup.document(), startup.seed())
                    .unwrap(),
            );
            service(&startup, &mut buffers, &mut *state_slot, &mut *store_slot)
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // An unarmed panic is always a fatal qualification failure.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
