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
use anomalous_document_protocol::{
    arena_byte_count, document_store, operation, record, record_size, service_error,
    startup_descriptor, startup_size,
};
use anomalous_editor_contract::{
    frame_byte_count, frame_width, opened_editor_state, render_opened, save_acknowledgement,
    save_outcome, save_snapshot, theme,
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
    fn anomalous_native_document_client_call(record: *mut call_record);
    fn anomalous_native_document_client_arm_before(scenario: u64, reserved: u64) -> !;
    fn anomalous_native_document_client_arm_after(scenario: u64, reserved: u64) -> !;
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
    unsafe { anomalous_native_document_client_call(&mut record) };
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

fn command(sequence: u64) -> application_input_action<'static> {
    match sequence {
        1 => application_input_action::set_selection {
            anchor: 6,
            head: 12,
        },
        2 => application_input_action::replace_selection(b"after"),
        3 => application_input_action::undo,
        4 => application_input_action::replace_selection(b"later"),
        _ => panic!("unknown input"),
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
    let reply = receive_document(handles, sequence - 1, buffers);
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

fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: *mut opened_editor_state,
) -> ! {
    assert!(startup.seed().is_empty());
    let handles = startup.handles();
    let document = startup.document();
    let retired_document = if startup.scenario() == 0 {
        document + 1
    } else {
        document - 1
    };
    expect_status(call(2, handles[2], 0, record_size, 0), 2);
    expect_status(call(1, handles[3], 0, record_size, 1), 2);
    expect_status(call(1, handles[2], 0, record_size, 1), 3);
    // A malformed but copied packet is consumed by userspace without creating
    // an open handle or altering the store. There is no forged semantic reply.
    buffers
        .request
        .copy_from_slice(&record::open(document, 1).unwrap().encode());
    buffers.request[0] ^= 1;
    send(handles[2], buffers.request, 1);
    poison(buffers.request);
    wait_ack(handles[2], 1);
    let open = record::open(document, 1).unwrap();
    submit_document(handles, 2, open, buffers);
    wait_ack(handles[2], 2);
    expect_status(call(2, handles[3], 0, record_size, 0), 3);
    expect_status(
        call(
            2,
            handles[3],
            buffers.response.as_mut_ptr() as u64,
            record_size - 1,
            0,
        ),
        4,
    );
    let opened = receive_document(handles, 1, buffers);
    assert_eq!(opened.kind(), operation::opened);
    assert_eq!(opened.document(), document);
    assert_eq!(opened.request(), 1);
    assert_eq!((opened.generation(), opened.version()), (1, 1));
    // SAFETY: The caller reserved one correctly sized/aligned disjoint arena
    // allocation. It is initialized exactly once from an authenticated reply.
    let state = unsafe {
        state_slot.write(
            opened_editor_state::from_open(
                document,
                opened.generation(),
                opened.version(),
                opened.text(),
            )
            .unwrap(),
        );
        &mut *state_slot
    };
    let mut original = [0; 31];
    let original_length = state.text().len();
    original[..original_length].copy_from_slice(state.text());
    for (sequence, request, error) in [
        (
            3,
            record::read(retired_document, 1, 2).unwrap(),
            service_error::wrong_document,
        ),
        (
            4,
            record::read(document, 2, 3).unwrap(),
            service_error::stale_handle,
        ),
        (
            5,
            record::save(document, 1, 90, 9, 1, b"inert").unwrap(),
            service_error::version_conflict,
        ),
    ] {
        let reply = exchange(handles, sequence, request, buffers);
        assert_rejected(reply, request, error);
        assert!(!state.is_dirty());
        assert_eq!(state.version(), 1);
    }
    for sequence in 1..=3 {
        publish(state, handles, sequence, buffers);
        if sequence < 3 {
            receive_input(handles, sequence, buffers);
            let input = application_input_v1::decode(buffers.input).unwrap();
            apply(state, input.action());
            acknowledge(handles[1], sequence);
        }
    }
    let snapshot = state.begin_save().unwrap();
    submit_document(handles, 6, save_record(&snapshot), buffers);
    if matches!(startup.scenario(), 1 | 2) {
        buffers.response.fill(0x5a);
        for _ in 0..wait_limit {
            let reply = call(
                2,
                handles[3],
                buffers.response.as_mut_ptr() as u64,
                record_size,
                0,
            );
            if reply.status == 1 {
                yield_peer();
                continue;
            }
            assert_eq!(reply.status, 5);
            assert!(buffers.response.iter().all(|byte| *byte == 0x5a));
            state
                .mark_save_unknown(
                    snapshot.document(),
                    snapshot.generation(),
                    snapshot.request(),
                    snapshot.base_version(),
                )
                .unwrap();
            assert_eq!(state.save_outcome(), save_outcome::unknown);
            assert!(state.is_dirty());
            assert_eq!(state.saved_text(), &original[..original_length]);
            assert_eq!(state.text(), snapshot.text());
            buffers
                .request
                .copy_from_slice(&save_record(&snapshot).encode());
            expect_status(
                call(
                    1,
                    handles[2],
                    buffers.request.as_ptr() as u64,
                    record_size,
                    7,
                ),
                5,
            );
            exit();
        }
        panic!("fault closure missing");
    }
    receive_input(handles, 3, buffers);
    apply(
        state,
        application_input_v1::decode(buffers.input)
            .unwrap()
            .action(),
    );
    acknowledge(handles[1], 3);
    assert_eq!(state.text(), &original[..original_length]);
    assert!(!state.is_dirty());
    publish(state, handles, 4, buffers);
    receive_input(handles, 4, buffers);
    apply(
        state,
        application_input_v1::decode(buffers.input)
            .unwrap()
            .action(),
    );
    acknowledge(handles[1], 4);
    publish(state, handles, 5, buffers);
    wait_ack(handles[2], 6);
    let saved_b = receive_document(handles, 5, buffers);
    accept_save(state, saved_b);
    assert!(state.is_dirty());
    assert_eq!(state.revision(), 4);
    assert_eq!(state.saved_text(), snapshot.text());
    publish(state, handles, 6, buffers);
    let read_b = exchange(handles, 7, record::read(document, 1, 4).unwrap(), buffers);
    assert_content(read_b, operation::read_result, 1, 2, snapshot.text());
    let snapshot_c = state.begin_save().unwrap();
    let saved_c = exchange(handles, 8, save_record(&snapshot_c), buffers);
    accept_save(state, saved_c);
    assert!(!state.is_dirty());
    publish(state, handles, 7, buffers);
    let close = record::close(document, 1, 5).unwrap();
    let closed = exchange(handles, 9, close, buffers);
    assert_eq!(closed.kind(), operation::closed);
    assert_eq!(closed.generation(), 1);
    let old_read = record::read(document, 1, 6).unwrap();
    let rejected = exchange(handles, 10, old_read, buffers);
    assert_rejected(rejected, old_read, service_error::closed_handle);
    let reopened = exchange(handles, 11, record::open(document, 7).unwrap(), buffers);
    assert_content(reopened, operation::opened, 2, 3, snapshot_c.text());
    *state = opened_editor_state::from_open(document, 2, 3, reopened.text()).unwrap();
    let stale = record::read(document, 1, 8).unwrap();
    let rejected = exchange(handles, 12, stale, buffers);
    assert_rejected(rejected, stale, service_error::stale_handle);
    let read_c = exchange(handles, 13, record::read(document, 2, 9).unwrap(), buffers);
    assert_content(read_c, operation::read_result, 2, 3, snapshot_c.text());
    let closed = exchange(
        handles,
        14,
        record::close(document, 2, 10).unwrap(),
        buffers,
    );
    assert_eq!(closed.kind(), operation::closed);
    assert_eq!(closed.generation(), 2);
    publish(state, handles, 8, buffers);
    exit();
}

fn serve_document(
    startup: &startup_descriptor,
    sequence: u64,
    store: &mut document_store,
    buffers: &mut client_buffers<'_>,
) -> record {
    let handles = startup.handles();
    receive(handles[2], buffers.request, sequence);
    let request = record::decode(buffers.request).unwrap();
    let version = store.version();
    let opened = store.is_open();
    let mut before = [0; 31];
    let length = store.stored_text().len();
    before[..length].copy_from_slice(store.stored_text());
    let reply = match store.execute(startup.authority(), &request) {
        Ok(reply) => reply,
        Err(error) => {
            assert_eq!(store.version(), version);
            assert_eq!(store.is_open(), opened);
            assert_eq!(store.stored_text(), &before[..length]);
            record::rejection_for(&request, error).unwrap()
        }
    };
    acknowledge(handles[2], sequence);
    buffers.response.copy_from_slice(&reply.encode());
    send(handles[3], buffers.response, sequence - 1);
    poison(buffers.response);
    wait_ack(handles[3], sequence - 1);
    reply
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
    // SAFETY: The live arena-resident store is fully initialized and exclusively
    // owned by this task. A volatile value observation forces actual stored
    // version/length fields to exist before the nonreturning fault. This local
    // observation is never executed as a second service or used as authority.
    let observed = unsafe { ptr::read_volatile(ptr::from_ref(store)) };
    assert_eq!(observed.version(), version);
    assert_eq!(observed.stored_text().len(), expected.len());
    for (actual, expected) in store.stored_text().iter().zip(expected) {
        // SAFETY: Exact live private initialized store byte, observed in RAM.
        assert_eq!(unsafe { ptr::read_volatile(actual) }, *expected);
    }
}

fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state: &mut opened_editor_state,
    store: &mut document_store,
) -> ! {
    let handles = startup.handles();
    expect_status(call(2, handles[2], 0, record_size, 0), 3);
    expect_status(
        call(
            2,
            handles[2],
            buffers.request.as_mut_ptr() as u64,
            record_size - 1,
            0,
        ),
        4,
    );
    receive(handles[2], buffers.request, 1);
    assert!(record::decode(buffers.request).is_err());
    assert!(!store.is_open());
    assert_eq!(store.version(), 1);
    assert_eq!(store.stored_text(), startup.seed());
    acknowledge(handles[2], 1);
    let opened = serve_document(startup, 2, store, buffers);
    assert_content(opened, operation::opened, 1, 1, startup.seed());
    for sequence in 3..=5 {
        assert_eq!(
            serve_document(startup, sequence, store, buffers).kind(),
            operation::rejected
        );
    }
    for sequence in 1..=8 {
        inspect_surface(state, sequence, handles, buffers);
        if sequence == 3 && matches!(startup.scenario(), 1 | 2) {
            receive(handles[2], buffers.request, 6);
            let request = record::decode(buffers.request).unwrap();
            assert_eq!(request.kind(), operation::save);
            assert_eq!(request.text(), state.text());
            observe_store(store, startup.seed(), 1);
            if startup.scenario() == 1 {
                // SAFETY: Exact service-only before-commit fault gate/UD2;
                // request remains delivered and no reply has been submitted.
                unsafe { anomalous_native_document_client_arm_before(1, 0) }
            }
            let reply = store.execute(startup.authority(), &request).unwrap();
            assert_eq!(reply.kind(), operation::saved);
            observe_store(store, request.text(), 2);
            // SAFETY: Exact after-commit fault; the semantic result has never
            // entered the response route, and transport ACK is still withheld.
            unsafe { anomalous_native_document_client_arm_after(2, 0) }
        }
        if sequence <= 4 {
            if sequence == 3 {
                state.begin_save().unwrap();
            }
            let action = command(sequence);
            application_input_v1::try_new(sequence, action)
                .unwrap()
                .encode_into(buffers.input)
                .unwrap();
            send(handles[1], buffers.input, sequence);
            poison(buffers.input);
            apply(state, action);
            wait_ack(handles[1], sequence);
        } else if sequence == 5 {
            let saved = serve_document(startup, 6, store, buffers);
            accept_save(state, saved);
            assert!(state.is_dirty());
        } else if sequence == 6 {
            let read = serve_document(startup, 7, store, buffers);
            assert_content(read, operation::read_result, 1, 2, state.saved_text());
            state.begin_save().unwrap();
            let saved = serve_document(startup, 8, store, buffers);
            accept_save(state, saved);
            assert!(!state.is_dirty());
        } else if sequence == 7 {
            assert_eq!(
                serve_document(startup, 9, store, buffers).kind(),
                operation::closed
            );
            assert_eq!(
                serve_document(startup, 10, store, buffers).status(),
                Some(service_error::closed_handle)
            );
            let opened = serve_document(startup, 11, store, buffers);
            *state = opened_editor_state::from_open(
                opened.document(),
                opened.generation(),
                opened.version(),
                opened.text(),
            )
            .unwrap();
            assert_eq!(
                serve_document(startup, 12, store, buffers).status(),
                Some(service_error::stale_handle)
            );
            assert_content(
                serve_document(startup, 13, store, buffers),
                operation::read_result,
                2,
                3,
                state.text(),
            );
            assert_eq!(
                serve_document(startup, 14, store, buffers).kind(),
                operation::closed
            );
            assert!(!store.is_open());
        } else {
            // The final copied public frame remains owned after editor exit.
            yield_peer();
            let surface = application_surface_v1::decode(buffers.packet).unwrap();
            stage_final_frame(surface.pixels(), buffers.row);
            exit();
        }
    }
    panic!("service fell through");
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
extern "C" fn anomalous_native_document_client_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: Exact launch-admitted private descriptor span, copied before use.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    // SAFETY: Admitted private RW/NX guarded arena, disjoint from descriptor,
    // stack/code/data. Sole entry creates one mutable borrow for its lifetime.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_byte_count as usize)
    };
    for byte in storage.iter() {
        // SAFETY: Every byte is retained readable RAM; observe before writing.
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
    let handles = startup.handles();
    let retired_handle = if startup.scenario() == 0 {
        handles[0] + 0x100
    } else {
        handles[0] - 0x100
    };
    assert!(!handles.contains(&retired_handle));
    expect_status(
        call(
            1,
            retired_handle,
            buffers.request.as_ptr() as u64,
            record_size,
            1,
        ),
        2,
    );
    if startup.role() == 0 {
        source(&startup, &mut buffers, state_slot)
    } else {
        // SAFETY: Correctly sized/aligned disjoint allocations. Initialize each
        // before forming its unique typed reference; no aliases escape service.
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
    // Any unarmed panic remains a fatal qualification failure.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
