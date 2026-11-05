#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental native Rust frontend for the bounded Antimatter editor contract.
//! Two private tasks run the same image: an editor and a surface/input service.
//! The service drives a fixed trace, not physical input or desktop policy.

use anomalous_abi::application::{
    application_input_action, application_input_v1, application_input_v1_size,
    application_surface_state, application_surface_v1, application_surface_v1_size,
    application_theme,
};
use anomalous_editor_contract::{editor_state, frame_byte_count, frame_width, render, theme};
use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));

// The ELF deployment proof copies these initialized bytes independently of the
// zeroed boot arguments and BSS. Keep the default native image unchanged.
#[cfg(feature = "elf_data_test")]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.elf_canary")]
static anomalous_native_client_data_canary: u64 = 0x454c_465f_4441_5441;

const surface_channel: u64 = 1;
const input_channel: u64 = 2;
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
    fn anomalous_native_client_call(record: *mut call_record);
    fn anomalous_native_client_arm_fault() -> !;
}

// Private BSS avoids large stack frames. The two roles have independent physical
// pages, and the sole entry borrows this aggregate once for the task's lifetime.
struct client_buffers {
    pixels: [u8; frame_byte_count],
    packet: [u8; application_surface_v1_size],
    input: [u8; application_input_v1_size],
    row: [u8; scaled_row_bytes],
}
static mut buffer_storage: client_buffers = client_buffers {
    pixels: [0; frame_byte_count],
    packet: [0; application_surface_v1_size],
    input: [0; application_input_v1_size],
    row: [0; scaled_row_bytes],
};

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
    unsafe { anomalous_native_client_call(&mut record) };
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

fn apply(state: &mut editor_state, action: application_input_action<'_>) {
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

fn encode_surface(state: &editor_state, sequence: u64, buffers: &mut client_buffers) {
    let metadata = render(state, &mut buffers.pixels);
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
        &buffers.pixels,
    )
    .unwrap()
    .encode_into(&mut buffers.packet)
    .unwrap();
}

fn source(scenario: u64, buffers: &mut client_buffers) -> ! {
    let mut state = editor_state::new();
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
        encode_surface(&state, sequence, buffers);
        send(surface_channel, &buffers.packet, sequence);
        // A successful send owns a kernel snapshot. Deliberately corrupt every
        // source byte before the peer can run; the service must see the snapshot.
        for byte in &mut buffers.packet {
            // SAFETY: Each byte is uniquely borrowed, mapped writable normal
            // memory. Volatile makes the deliberate corruption observable even
            // though the next iteration will replace the entire packet again.
            unsafe { ptr::write_volatile(byte, 0xa5) };
        }
        if scenario == 1 {
            // SAFETY: The kernel arms only this exact fault instruction, only
            // for the source's queued first packet in the revocation scenario.
            unsafe { anomalous_native_client_arm_fault() }
        }
        wait_ack(surface_channel, sequence);
        if sequence == 7 {
            exit();
        }
        receive(input_channel, &mut buffers.input, sequence);
        let input = application_input_v1::decode(&buffers.input).unwrap();
        assert_eq!(input.sequence(), sequence);
        apply(&mut state, input.action());
        acknowledge(input_channel, sequence);
    }
    panic!("source fell through");
}

fn service(scenario: u64, buffers: &mut client_buffers) -> ! {
    if scenario == 1 {
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
    let mut expected = editor_state::new();
    for sequence in 1..=7 {
        receive(surface_channel, &mut buffers.packet, sequence);
        let surface = application_surface_v1::decode(&buffers.packet).unwrap();
        let state = surface.state();
        let metadata = render(&expected, &mut buffers.pixels);
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
        assert_eq!(surface.pixels(), &buffers.pixels);
        acknowledge(surface_channel, sequence);
        if sequence == 7 {
            // Retain the independent received packet while its producer observes
            // the final acknowledgement and exits. No channel storage is needed
            // to render the final retained frame after producer retirement.
            yield_peer();
            stage_final_frame(surface.pixels(), &mut buffers.row);
            exit();
        }
        let action = command(sequence);
        application_input_v1::try_new(sequence, action)
            .unwrap()
            .encode_into(&mut buffers.input)
            .unwrap();
        send(input_channel, &buffers.input, sequence);
        apply(&mut expected, action);
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
extern "C" fn anomalous_native_client_run() -> ! {
    #[cfg(feature = "elf_data_test")]
    {
        // SAFETY: The admitted ELF plan places this immutable eight-byte object
        // in private initialized RW memory. Volatile access tests the loaded
        // bytes rather than permitting constant folding of the Rust initializer.
        let canary = unsafe { ptr::read_volatile(&anomalous_native_client_data_canary) };
        assert_eq!(canary, 0x454c_465f_4441_5441);
    }
    // SAFETY: Loader contract reserves the first128 RW bytes independently of
    // every Rust global, writes these two qwords before publication, and never
    // mutates them while the task is live. Each task enters this function once.
    let (role, scenario, buffers) = unsafe {
        (
            ptr::read_volatile(0x800000 as *const u64),
            ptr::read_volatile(0x800008 as *const u64),
            &mut *ptr::addr_of_mut!(buffer_storage),
        )
    };
    assert!(role <= 1 && scenario <= 1);
    if role == 0 {
        source(scenario, buffers)
    } else {
        service(scenario, buffers)
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // Unarmed faults are fatal proof failures, never the expected source fault.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
