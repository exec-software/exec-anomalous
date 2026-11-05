#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]
#![cfg_attr(
    any(feature = "interactive_proof", feature = "remote_workspace_proof"),
    allow(dead_code, unused_imports)
)]

//! Native disk fixture: only the service owns the bounded sector interface.
//! Mount-ready follows the durable epoch flush. Recovery launch data carries
//! no expected text; the host independently checks recovered bytes and pixels.

use anomalous_abi::application::{
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_document_protocol::durable::{
    durable_error, durable_store, format_disposable, io_error, sector_io, sector_size,
};
#[cfg(feature = "failure_proof")]
use anomalous_document_protocol::durable_failure_startup::startup_descriptor;
#[cfg(not(feature = "failure_proof"))]
use anomalous_document_protocol::durable_startup::startup_descriptor;
use anomalous_document_protocol::durable_startup::{arena_byte_count, mount_ready, startup_size};
use anomalous_document_protocol::{operation, record, record_size, service_error};
use anomalous_editor_contract::{
    frame_byte_count, frame_width, opened_editor_state, render_opened, save_acknowledgement,
    save_snapshot, theme,
};
use anomalous_native_runtime::arena;
use core::{
    arch::{asm, global_asm},
    mem::MaybeUninit,
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));
#[cfg(all(
    feature = "interactive_proof",
    not(any(feature = "failure_proof", feature = "remote_workspace_proof"))
))]
mod interactive;
#[cfg(feature = "failure_proof")]
global_asm!(include_str!("failure_entry.S"), options(att_syntax));
#[cfg(feature = "failure_proof")]
mod failure;
#[cfg(feature = "failure_proof")]
use failure::{service, source};
#[cfg(all(feature = "repeated_proof", not(feature = "failure_proof")))]
mod repeated;
#[cfg(all(feature = "repeated_proof", not(feature = "failure_proof")))]
use anomalous_editor_contract::repeated_opened_editor_state as active_editor_state;
#[cfg(all(feature = "repeated_proof", not(feature = "failure_proof")))]
use repeated::{service, source};
#[cfg(any(not(feature = "repeated_proof"), feature = "failure_proof"))]
type active_editor_state = opened_editor_state;
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
    fn anomalous_native_durable_client_call(record: *mut call_record);
}

struct client_buffers<'a> {
    pixels: &'a mut [u8; frame_byte_count],
    packet: &'a mut [u8; application_surface_v1_size],
    row: &'a mut [u8; scaled_row_bytes],
    canary: &'a mut [u8; 32],
    request: &'a mut [u8; record_size],
    response: &'a mut [u8; record_size],
    sector: &'a mut [u8; sector_size],
    tail: &'a mut [u8; 256],
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
    unsafe { anomalous_native_durable_client_call(&mut record) };
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
    let reply = receive_document(handles, sequence + 1, buffers);
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

#[cfg(any(not(feature = "repeated_proof"), feature = "failure_proof"))]
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

struct gate_device {
    handle: u64,
}

impl sector_io for gate_device {
    fn read_sector(&mut self, sector: u64, out: &mut [u8; sector_size]) -> Result<(), io_error> {
        let reply = call(
            12,
            self.handle,
            out.as_mut_ptr() as u64,
            sector_size,
            sector,
        );
        if reply.status != 0 {
            return Err(io_error::failed);
        }
        assert_eq!(reply.returned_sequence, sector);
        assert_eq!(reply.returned_length, sector_size as u64);
        Ok(())
    }

    fn write_sector(&mut self, sector: u64, bytes: &[u8; sector_size]) -> Result<(), io_error> {
        let reply = call(13, self.handle, bytes.as_ptr() as u64, sector_size, sector);
        if reply.status != 0 {
            return Err(io_error::failed);
        }
        assert_eq!(reply.returned_sequence, sector);
        assert_eq!(reply.returned_length, sector_size as u64);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), io_error> {
        let reply = call(14, self.handle, 0, 0, 0);
        if reply.status != 0 {
            return Err(io_error::failed);
        }
        assert_eq!(reply.returned_sequence, 0);
        assert_eq!(reply.returned_length, 0);
        Ok(())
    }
}

fn device_probes(startup: &startup_descriptor, buffers: &mut client_buffers<'_>) {
    let handle = startup.device_handle();
    assert_ne!(handle, 0);
    buffers.sector.fill(0x6b);
    let pointer = buffers.sector.as_mut_ptr() as u64;
    // Actual authority checks precede both argument decoding and device access.
    expect_status(call(12, 0, pointer, sector_size, 0), 2);
    if startup.phase() != 0 {
        let retired = handle.checked_sub(startup.phase() * 0x100).unwrap();
        assert_ne!(retired, 0);
        expect_status(call(12, retired, pointer, sector_size, 0), 2);
    }
    for operation in [12, 13] {
        expect_status(call(operation, handle, 0, sector_size, 0), 3);
        expect_status(
            call(
                operation,
                handle,
                buffers.tail.as_mut_ptr() as u64,
                sector_size,
                0,
            ),
            3,
        );
        expect_status(call(operation, handle, pointer, sector_size - 1, 0), 4);
        expect_status(call(operation, handle, pointer, sector_size, 5), 4);
        expect_status(call(operation, handle, pointer, sector_size, u64::MAX), 4);
    }
    expect_status(call(14, handle, pointer, 0, 0), 4);
    expect_status(call(14, handle, 0, 1, 0), 4);
    expect_status(call(14, handle, 0, 0, 1), 4);
    assert!(buffers.sector.iter().all(|byte| *byte == 0x6b));
    assert!(buffers.tail.iter().all(|byte| *byte == 0));
}

fn negative_document(ready: mount_ready, sequence: u64) -> (record, service_error) {
    match sequence {
        2 => {
            let old_document = if ready.document() > 1 {
                ready.document() - 1
            } else {
                ready.document() + 1
            };
            (
                record::read(old_document, 1, 2).unwrap(),
                service_error::wrong_document,
            )
        }
        3 => (
            record::read(ready.document(), 2, 3).unwrap(),
            service_error::stale_handle,
        ),
        4 => (
            record::save(ready.document(), 1, 99, ready.version() + 1, 1, b"invalid").unwrap(),
            service_error::version_conflict,
        ),
        _ => panic!("unexpected rejection probe"),
    }
}

#[cfg(any(not(feature = "repeated_proof"), feature = "failure_proof"))]
fn initialize_state(
    slot: &mut MaybeUninit<opened_editor_state>,
    opened: record,
) -> &mut opened_editor_state {
    assert_eq!(opened.kind(), operation::opened);
    assert_eq!(opened.status(), None);
    assert_eq!(opened.generation(), 1);
    slot.write(
        opened_editor_state::from_open(
            opened.document(),
            opened.generation(),
            opened.version(),
            opened.text(),
        )
        .unwrap(),
    )
}

#[cfg(not(any(feature = "failure_proof", feature = "repeated_proof")))]
fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<opened_editor_state>,
) -> ! {
    // Knowing the service's numeric handle cannot confer its task-bound grant.
    // Use otherwise valid requests so rejection independently exercises role
    // ownership, rather than succeeding only because the handle is wrong.
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
    assert_eq!(ready.version(), if startup.phase() == 0 { 1 } else { 2 });
    let opened = exchange(
        handles,
        1,
        record::open(ready.document(), 1).unwrap(),
        buffers,
    );
    assert_eq!(opened.version(), ready.version());
    let state = initialize_state(state_slot, opened);
    if startup.phase() == 0 {
        assert!(state.text().is_empty());
    } else {
        assert!(!state.text().is_empty());
        assert!(startup.challenge().is_empty());
    }
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
        assert!(state.is_dirty());
        publish(state, handles, 2, buffers);
        let snapshot = state.begin_save().unwrap();
        assert_eq!(snapshot.request(), 1);
        accept_save(state, exchange(handles, 5, save_record(&snapshot), buffers));
        assert_eq!(state.version(), 2);
        assert!(!state.is_dirty());
        publish(state, handles, 3, buffers);
        6
    } else {
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
        2,
        state.text(),
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
    assert!(!state.is_dirty());
    check_canary(buffers);
    exit()
}

#[cfg(not(feature = "failure_proof"))]
fn serve_document(
    startup: &startup_descriptor,
    sequence: u64,
    store: &mut durable_store<'_, gate_device>,
    buffers: &mut client_buffers<'_>,
) -> (record, record) {
    let handles = startup.handles();
    receive(handles[2], buffers.request, sequence);
    let request = record::decode(buffers.request).unwrap();
    let reply = match store.execute(startup.authority(), &request) {
        Ok(reply) => reply,
        Err(durable_error::rejected(error)) => record::rejection_for(&request, error).unwrap(),
        // In particular, an uncertain write is never converted into a semantic
        // rejection or a saved acknowledgement. Panic fails this proof run.
        Err(_) => panic!("durable service failed"),
    };
    acknowledge(handles[2], sequence);
    buffers.response.copy_from_slice(&reply.encode());
    send(handles[3], buffers.response, sequence + 1);
    poison(buffers.response);
    wait_ack(handles[3], sequence + 1);
    (request, reply)
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

#[cfg(not(any(feature = "failure_proof", feature = "repeated_proof")))]
fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state_slot: &mut MaybeUninit<opened_editor_state>,
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
    assert_eq!(ready.version(), if startup.phase() == 0 { 1 } else { 2 });
    let handles = startup.handles();
    buffers.response.copy_from_slice(&ready.encode());
    send(handles[3], buffers.response, 1);
    poison(buffers.response);
    wait_ack(handles[3], 1);
    let (request, opened) = serve_document(startup, 1, &mut store, buffers);
    assert_eq!(request, record::open(ready.document(), 1).unwrap());
    let state = initialize_state(state_slot, opened);
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
        // The service receives the edited bytes through the copied surface,
        // never through a challenge-bearing service startup descriptor.
        receive(handles[0], buffers.packet, 2);
        let surface = application_surface_v1::decode(buffers.packet).unwrap();
        state.insert(surface.state().text).unwrap();
        let metadata = render_opened(state, buffers.pixels);
        assert_eq!(surface.state().request_sequence, 2);
        assert_eq!(surface.state().document_revision, metadata.revision);
        assert_eq!(surface.state().selection_anchor, metadata.anchor as u8);
        assert_eq!(surface.state().selection_head, metadata.head as u8);
        assert_eq!(surface.state().dirty, metadata.dirty);
        assert_eq!(surface.state().theme, wire_theme(metadata.appearance));
        assert_eq!(surface.state().undo_depth, metadata.undo_depth);
        assert_eq!(surface.pixels(), buffers.pixels.as_slice());
        acknowledge(handles[0], 2);
        let snapshot = state.begin_save().unwrap();
        let (request, reply) = serve_document(startup, 5, &mut store, buffers);
        assert_eq!(request, save_record(&snapshot));
        accept_save(state, reply);
        assert_eq!(store.stored_text(), state.text());
        assert_eq!(store.version(), 2);
        inspect_surface(state, 3, handles, buffers);
        6
    } else {
        5
    };
    let (request, reply) = serve_document(startup, sequence, &mut store, buffers);
    assert_eq!(request, record::read(ready.document(), 1, 2).unwrap());
    assert_content(reply, operation::read_result, 1, 2, state.text());
    let (request, reply) = serve_document(startup, sequence + 1, &mut store, buffers);
    assert_eq!(request, record::close(ready.document(), 1, 3).unwrap());
    assert_eq!(reply.kind(), operation::closed);
    assert!(!store.is_open());
    assert!(!store.is_quarantined());
    check_canary(buffers);
    stage_final_frame(buffers.pixels, buffers.row);
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

#[cfg(all(
    not(feature = "remote_workspace_proof"),
    any(not(feature = "interactive_proof"), feature = "failure_proof")
))]
#[unsafe(no_mangle)]
extern "C" fn anomalous_native_durable_client_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: Launch-admitted descriptor copied before interpretation.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    // SAFETY: Independently admitted private RW/NX arena, disjoint from code,
    // descriptor and stacks. This task forms its sole mutable borrow once.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_byte_count as usize)
    };
    for byte in storage.iter() {
        // SAFETY: Check every admitted byte before allocating any private state.
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let mut allocator = arena::new(storage);
    let state_storage = allocator
        .allocate(core::mem::size_of::<MaybeUninit<active_editor_state>>(), 64)
        .unwrap();
    // SAFETY: One aligned arena allocation of exactly this size, disjoint from
    // all following allocations. MaybeUninit permits the not-yet-written state;
    // initialize_state creates its sole initialized borrow for the task's life.
    let state_slot = unsafe {
        &mut *state_storage
            .as_mut_ptr()
            .cast::<MaybeUninit<active_editor_state>>()
    };
    let pixels = allocator
        .allocate(frame_byte_count, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let packet = allocator
        .allocate(application_surface_v1_size, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let row = allocator
        .allocate(scaled_row_bytes, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let canary = allocator.allocate(32, 256).unwrap().try_into().unwrap();
    let request = allocator
        .allocate(record_size, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let response = allocator
        .allocate(record_size, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let sector = allocator
        .allocate(sector_size, 64)
        .unwrap()
        .try_into()
        .unwrap();
    let padding = allocator.remaining().checked_sub(256).unwrap();
    if padding != 0 {
        let _ = allocator.allocate(padding, 1).unwrap();
    }
    let tail = allocator.allocate(256, 1).unwrap().try_into().unwrap();
    assert_eq!(allocator.remaining(), 0);
    let mut buffers = client_buffers {
        pixels,
        packet,
        row,
        canary,
        request,
        response,
        sector,
        tail,
    };
    buffers.canary.fill(0x39);
    if startup.role() == 0 {
        source(&startup, &mut buffers, state_slot)
    } else {
        service(&startup, &mut buffers, state_slot)
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // An unarmed panic is always a fatal qualification failure.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}

#[cfg(feature = "remote_workspace_proof")]
mod remote_workspace;
