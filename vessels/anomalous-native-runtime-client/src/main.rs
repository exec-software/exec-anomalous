#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental native Rust frontend for the bounded Antimatter editor contract.
//! Startup supplies document text and opaque handles. Both private tasks use
//! their precommitted arenas for editor state and transport/render buffers.
//! The service drives the same bounded trace, not physical input or a filesystem.

use anomalous_abi::application::{
    application_input_action, application_input_v1, application_input_v1_size,
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{
    frame_byte_count, frame_width, loaded_editor_state, render_loaded, theme,
};
use anomalous_native_runtime::{
    arena, arena_byte_count, arena_error, startup_descriptor, startup_size,
};
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
    fn anomalous_native_runtime_client_call(record: *mut call_record);
    fn anomalous_native_runtime_client_arm_guard(target: u64) -> !;
    fn anomalous_native_runtime_client_arm_nx(target: u64) -> !;
}

// References point to disjoint arena allocations; no bulk buffer lives on the
// 8 KiB task stack or in a hidden static allocator.
struct client_buffers<'a> {
    pixels: &'a mut [u8; frame_byte_count],
    packet: &'a mut [u8; application_surface_v1_size],
    input: &'a mut [u8; application_input_v1_size],
    row: &'a mut [u8; scaled_row_bytes],
    canary: &'a mut [u8; 32],
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
    unsafe { anomalous_native_runtime_client_call(&mut record) };
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

fn apply(state: &mut loaded_editor_state, action: application_input_action<'_>) {
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
        3 => application_input_action::set_theme(application_theme::light),
        4 => application_input_action::set_theme(application_theme::dark),
        5 => application_input_action::set_selection { anchor: 8, head: 8 },
        6 => application_input_action::undo,
        _ => panic!("unknown command sequence"),
    }
}

fn encode_surface(state: &loaded_editor_state, sequence: u64, buffers: &mut client_buffers<'_>) {
    let metadata = render_loaded(state, buffers.pixels);
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
}

fn source(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    state: &mut loaded_editor_state,
) -> ! {
    let scenario = startup.scenario();
    let surface_channel = startup.outbound_handle();
    let input_channel = startup.inbound_handle();
    // Role violations precede copy; failed copy must not consume sequence one.
    expect_status(
        call(2, surface_channel, 0, application_surface_v1_size, 0),
        2,
    );
    expect_status(call(1, input_channel, 0, application_input_v1_size, 1), 2);
    expect_status(
        call(1, surface_channel, 0, application_surface_v1_size, 1),
        3,
    );
    for sequence in 1..=7 {
        check_canary(buffers);
        encode_surface(state, sequence, buffers);
        send(surface_channel, buffers.packet, sequence);
        // A successful send owns a kernel snapshot. Deliberately corrupt every
        // source byte before the peer can run; the service must see the snapshot.
        for byte in buffers.packet.iter_mut() {
            // SAFETY: Each byte is uniquely borrowed, mapped writable normal
            // memory. Volatile makes the deliberate corruption observable even
            // though the next iteration will replace the entire packet again.
            unsafe { ptr::write_volatile(byte, 0xa5) };
        }
        if scenario >= 2 {
            // SAFETY: The exact scenario-specific gate arms only this producer,
            // queued packet and retained arena guard/NX target. RSI survives INT.
            unsafe {
                if scenario == 2 {
                    anomalous_native_runtime_client_arm_guard(startup.arena_base() - 8)
                } else {
                    anomalous_native_runtime_client_arm_nx(startup.arena_base())
                }
            }
        }
        wait_ack(surface_channel, sequence);
        if sequence == 7 {
            exit();
        }
        receive(input_channel, buffers.input, sequence);
        let input = application_input_v1::decode(buffers.input).unwrap();
        assert_eq!(input.sequence(), sequence);
        apply(state, input.action());
        acknowledge(input_channel, sequence);
    }
    panic!("source fell through");
}

fn service(
    startup: &startup_descriptor,
    buffers: &mut client_buffers<'_>,
    expected: &mut loaded_editor_state,
) -> ! {
    let surface_channel = startup.inbound_handle();
    let input_channel = startup.outbound_handle();
    if startup.scenario() >= 2 {
        buffers.packet.fill(0x5a);
        expect_status(
            call(
                2,
                surface_channel,
                buffers.packet.as_mut_ptr() as u64,
                application_surface_v1_size,
                0,
            ),
            5,
        );
        assert!(buffers.packet.iter().all(|byte| *byte == 0x5a));
        expect_status(call(3, surface_channel, 0, 0, 1), 5);
        expect_status(
            call(1, surface_channel, 0, application_surface_v1_size, 2),
            2,
        );
        expect_status(call(2, input_channel, 0, application_input_v1_size, 0), 2);
        exit();
    }
    // Source has queued a packet and yielded while awaiting its acknowledgement.
    // Neither a failed write nor insufficient capacity may consume that packet.
    expect_status(
        call(2, surface_channel, 0, application_surface_v1_size, 0),
        3,
    );
    expect_status(
        call(
            2,
            surface_channel,
            buffers.packet.as_mut_ptr() as u64,
            application_surface_v1_size - 1,
            0,
        ),
        4,
    );
    for sequence in 1..=7 {
        check_canary(buffers);
        receive(surface_channel, buffers.packet, sequence);
        let surface = application_surface_v1::decode(buffers.packet).unwrap();
        let state = surface.state();
        let metadata = render_loaded(expected, buffers.pixels);
        assert_eq!(state.request_sequence, sequence);
        assert_eq!(state.document_revision, metadata.revision);
        assert_eq!(state.text, expected.text());
        assert_eq!(
            (state.selection_anchor, state.selection_head),
            (metadata.anchor as u8, metadata.head as u8)
        );
        assert_eq!(state.dirty, metadata.dirty);
        assert_eq!(state.theme, wire_theme(metadata.appearance));
        assert_eq!(state.undo_depth, metadata.undo_depth);
        assert_eq!(surface.pixels(), buffers.pixels.as_slice());
        acknowledge(surface_channel, sequence);
        if sequence == 7 {
            // Retain the independent received packet while its producer observes
            // the final acknowledgement and exits. No channel storage is needed
            // to render the final retained frame after producer retirement.
            yield_peer();
            stage_final_frame(surface.pixels(), buffers.row);
            exit();
        }
        let action = command(sequence);
        application_input_v1::try_new(sequence, action)
            .unwrap()
            .encode_into(buffers.input)
            .unwrap();
        send(input_channel, buffers.input, sequence);
        apply(expected, action);
        wait_ack(input_channel, sequence);
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
extern "C" fn anomalous_native_runtime_client_run(pointer: *const u8, length: usize) -> ! {
    // Startup admission checked these exact registers; validate shape before
    // reading and retain a local copy independent of writable descriptor bytes.
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, startup_size);
    let mut bytes = [0; startup_size];
    // SAFETY: The loader reserves this exact128-byte descriptor in private RW,
    // fills it before task publication, and startup admits its exact pointer.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), startup_size) };
    let startup = startup_descriptor::decode(&bytes).unwrap();
    assert_eq!(startup.arena_length(), arena_byte_count);
    // SAFETY: The admitted launch retains this private RW+NX, guard-bounded
    // arena. It is disjoint from code, data, descriptor and stack; sole task
    // entry creates its unique borrow once, and no pointers outlive task exit.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_byte_count as usize)
    };
    // Observe all bytes before any allocation or application write, including
    // a fresh launch following a task-local fault in the previous scenario.
    for byte in storage.iter() {
        // SAFETY: Complete retained readable arena; volatile prevents elision.
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let mut allocator = arena::new(storage);
    let state_bytes = allocator
        .allocate(core::mem::size_of::<loaded_editor_state>(), 64)
        .unwrap();
    assert_eq!(state_bytes.as_ptr() as usize % 64, 0);
    let state_pointer = state_bytes.as_mut_ptr().cast::<loaded_editor_state>();
    // SAFETY: This disjoint allocation is large and aligned for the type; every
    // field is initialized before creating the only mutable typed reference.
    let state = unsafe {
        state_pointer.write(loaded_editor_state::from_text(startup.document()).unwrap());
        &mut *state_pointer
    };
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
    };
    assert_eq!(buffers.packet.as_ptr() as usize % 64, 0);
    assert_eq!(buffers.canary.as_ptr() as usize % 256, 0);
    buffers.canary.fill(0x39);
    let used = allocator.used();
    assert_eq!(allocator.allocate(0, 1), Err(arena_error::zero_length));
    assert_eq!(
        allocator.allocate(1, 3),
        Err(arena_error::invalid_alignment)
    );
    assert_eq!(
        allocator.allocate(arena_byte_count as usize, 1),
        Err(arena_error::exhausted)
    );
    assert_eq!(
        allocator.allocate(usize::MAX, 1),
        Err(arena_error::address_overflow)
    );
    assert_eq!(allocator.used(), used);
    check_canary(&buffers);
    let remaining = allocator.remaining();
    let tail = allocator.allocate(remaining, 1).unwrap();
    tail.fill(0x62);
    assert_eq!(allocator.allocate(1, 1), Err(arena_error::exhausted));
    assert_eq!(allocator.used(), arena_byte_count as usize);
    check_canary(&buffers);
    assert!(!state.is_dirty());
    // A handle from the alternate launch layout grants no authority, even with
    // a valid private packet span and a sequence that can subsequently succeed.
    let alternate_handle = if startup.scenario().is_multiple_of(2) {
        0x91
    } else {
        0x31
    };
    expect_status(
        call(
            1,
            alternate_handle,
            buffers.packet.as_ptr() as u64,
            application_surface_v1_size,
            1,
        ),
        2,
    );
    if startup.role() == 0 {
        source(&startup, &mut buffers, state)
    } else {
        service(&startup, &mut buffers, state)
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // Unarmed faults are fatal proof failures, never the expected source fault.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
