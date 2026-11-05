#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

use anomalous_abi::{
    id_128, rite_call, rite_exit_payload, rite_operation, rite_payload_kind, rite_reply,
    rite_rights, rite_semantic_surface_request_v1, rite_semantic_surface_snapshot_v1_size,
    rite_status,
};
use anomalous_desktop_transport::{surface_snapshot_v1, surface_snapshot_v1_byte_count};
use core::arch::{asm, global_asm};
use core::mem;
use core::panic::PanicInfo;
use core::ptr;

mod layout;
mod oracle;

use oracle::{
    expected_binding, expected_command_count, expected_commands, expected_labels, expected_state,
};

const call_offset: usize = 0;
const payload_offset: usize = mem::size_of::<rite_call>();
const request_size: u32 = mem::size_of::<rite_semantic_surface_request_v1>() as u32;
const snapshot_size: u32 = rite_semantic_surface_snapshot_v1_size;
const exit_size: u32 = mem::size_of::<rite_exit_payload>() as u32;

const submit_denied_trace: u8 = 0x21;
const malformed_read_trace: u8 = 0x22;
const delivery_trace: u8 = 0x23;
const empty_trace: u8 = 0x24;
const exit_trace: u8 = 0x25;

const _: () = {
    assert!(mem::size_of::<rite_call>() == 40);
    assert!(mem::size_of::<rite_reply>() == 40);
    assert!(request_size == 16);
    assert!(snapshot_size == 528);
    assert!(surface_snapshot_v1_byte_count == 528);
    assert!(payload_offset + surface_snapshot_v1_byte_count <= layout::page_size as usize);
};

#[derive(Clone, Copy, Eq, PartialEq)]
struct binding_oracle {
    surface_slot: u64,
    surface_generation: u64,
    scene_slot: u64,
    scene_generation: u64,
    base_revision: u64,
    revision: u64,
    renderer_slot: u64,
    renderer_generation: u64,
    attempt: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct state_oracle {
    primary_enabled: bool,
    secondary_enabled: bool,
    focus: u8,
    disclosure: u8,
    depth: u8,
    motion: u8,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct command_oracle {
    kind: u8,
    shape: u8,
    color: u8,
    secondary_color: u8,
    slot: u8,
    state_active: bool,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

global_asm!(
    include_str!("entry.S"),
    rite_gate_vector = const layout::rite_gate_vector,
    user_mailbox_address = const layout::user_mailbox_address,
    initial_user_flags = const layout::initial_user_flags,
    options(att_syntax)
);

unsafe extern "C" {
    fn anomalous_semantic_surface_consumer_submit_denied_rite();
    fn anomalous_semantic_surface_consumer_malformed_read_rite();
    fn anomalous_semantic_surface_consumer_delivery_rite();
    fn anomalous_semantic_surface_consumer_empty_rite();
    fn anomalous_semantic_surface_consumer_exit_rite();
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_semantic_surface_consumer_run() -> u64 {
    if submit_denied().is_err()
        || malformed_read().is_err()
        || receive_delivery().is_err()
        || observe_empty().is_err()
        || prepare_exit().is_err()
    {
        return 1;
    }

    // SAFETY: The shim preserves Rust callee-saved registers, clears every
    // register other than the fixed mailbox pointer, and enters the one Rite
    // gate. A successful task exit is terminal; any return reaches the target's
    // deliberate failure path.
    unsafe { anomalous_semantic_surface_consumer_exit_rite() };
    1
}

fn submit_denied() -> Result<(), ()> {
    let trace = trace(submit_denied_trace);
    write_call(rite_call::new(
        rite_operation::semantic_surface_submit,
        rite_payload_kind::semantic_surface_snapshot,
        rite_rights::semantic_surface_submit,
        snapshot_size,
        snapshot_size,
        trace,
    ));
    clear_body();
    // SAFETY: Admission must reject this complete outer call against the
    // consumer's read-only grant before consulting its deliberately empty body.
    unsafe { anomalous_semantic_surface_consumer_submit_denied_rite() };
    validate_empty_reply(rite_status::denied, 1, trace)
}

fn malformed_read() -> Result<(), ()> {
    let request = rite_semantic_surface_request_v1::from_raw_parts(16, 2, 0, 528, 0);
    issue_read(
        &request,
        malformed_read_trace,
        anomalous_semantic_surface_consumer_malformed_read_rite,
    );
    validate_empty_reply(rite_status::bad_payload, 2, trace(malformed_read_trace))
}

fn receive_delivery() -> Result<(), ()> {
    let request = rite_semantic_surface_request_v1::new();
    issue_read(
        &request,
        delivery_trace,
        anomalous_semantic_surface_consumer_delivery_rite,
    );
    let trace = trace(delivery_trace);
    let reply = read_reply();
    if !reply.has_supported_header()
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::semantic_surface_snapshot
        || reply.payload_size != snapshot_size
        || reply.sequence != 3
        || reply.trace_id != trace
    {
        return Err(());
    }
    let bytes = read_snapshot_body();
    let snapshot = surface_snapshot_v1::try_from_bytes(bytes).map_err(|_| ())?;
    validate_snapshot(&snapshot)
}

fn observe_empty() -> Result<(), ()> {
    let request = rite_semantic_surface_request_v1::new();
    issue_read(
        &request,
        empty_trace,
        anomalous_semantic_surface_consumer_empty_rite,
    );
    validate_empty_reply(rite_status::would_block, 4, trace(empty_trace))?;
    body_is_clear().then_some(()).ok_or(())
}

fn issue_read(
    request: &rite_semantic_surface_request_v1,
    trace_byte: u8,
    invoke: unsafe extern "C" fn(),
) {
    write_call(rite_call::new(
        rite_operation::semantic_surface_read,
        rite_payload_kind::semantic_surface_request,
        rite_rights::semantic_surface_read,
        request_size,
        request_size,
        trace(trace_byte),
    ));
    write_bytes(payload_offset, &request.encode_le());
    // SAFETY: Every selected shim has the same register-preserving Rite-call
    // contract and the fixed mailbox contains the complete request.
    unsafe { invoke() };
}

fn validate_snapshot(snapshot: &surface_snapshot_v1) -> Result<(), ()> {
    let binding = snapshot.binding();
    let actual_binding = binding_oracle {
        surface_slot: binding.surface().slot(),
        surface_generation: binding.surface().generation(),
        scene_slot: binding.scene().slot(),
        scene_generation: binding.scene().generation(),
        base_revision: binding.base_revision(),
        revision: binding.revision(),
        renderer_slot: binding.renderer().slot(),
        renderer_generation: binding.renderer().generation(),
        attempt: binding.attempt(),
    };
    let state = snapshot.state();
    let actual_state = state_oracle {
        primary_enabled: state.primary_enabled(),
        secondary_enabled: state.secondary_enabled(),
        focus: state.focus().raw(),
        disclosure: state.disclosure().raw(),
        depth: state.depth().raw(),
        motion: state.motion().raw(),
    };
    if actual_binding != expected_binding
        || actual_state != expected_state
        || snapshot.command_count() != expected_command_count
    {
        return Err(());
    }
    for (index, expected) in expected_labels.iter().enumerate() {
        let actual = snapshot.label(index).ok_or(())?;
        if actual.raw() != *expected {
            return Err(());
        }
    }
    if snapshot.label(expected_labels.len()).is_some() {
        return Err(());
    }
    for (index, expected) in expected_commands.iter().enumerate() {
        let actual = snapshot.command(index).ok_or(())?;
        let bounds = actual.bounds();
        let actual = command_oracle {
            kind: actual.kind().raw(),
            shape: actual.shape().raw(),
            color: actual.color().raw(),
            secondary_color: actual.secondary_color().map_or(0, |color| color.raw()),
            slot: actual.slot().raw(),
            state_active: actual.state_active(),
            x: bounds.x(),
            y: bounds.y(),
            width: bounds.width(),
            height: bounds.height(),
        };
        if actual != *expected {
            return Err(());
        }
    }
    if snapshot.command(expected_command_count).is_some() {
        return Err(());
    }
    Ok(())
}

fn prepare_exit() -> Result<(), ()> {
    let trace = trace(exit_trace);
    let payload = rite_exit_payload::new(0);
    if !payload.has_supported_body() {
        return Err(());
    }
    write_call(rite_call::new(
        rite_operation::task_exit,
        rite_payload_kind::exit_status,
        rite_rights::task_exit,
        exit_size,
        exit_size,
        trace,
    ));
    write_bytes(payload_offset, &payload.encode_le());
    Ok(())
}

fn validate_empty_reply(
    expected_status: rite_status,
    sequence: u64,
    trace: id_128,
) -> Result<(), ()> {
    let reply = read_reply();
    if reply.has_supported_header()
        && reply.status == expected_status
        && reply.payload_kind == rite_payload_kind::none
        && reply.payload_size == 0
        && reply.sequence == sequence
        && reply.trace_id == trace
    {
        Ok(())
    } else {
        Err(())
    }
}

fn clear_body() {
    let mut index = 0;
    while index < surface_snapshot_v1_byte_count {
        write_mailbox_byte(payload_offset + index, 0);
        index += 1;
    }
}

fn body_is_clear() -> bool {
    let mut index = 0;
    while index < surface_snapshot_v1_byte_count {
        if read_mailbox_byte(payload_offset + index) != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn read_snapshot_body() -> [u8; surface_snapshot_v1_byte_count] {
    let mut bytes = [0; surface_snapshot_v1_byte_count];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = read_mailbox_byte(payload_offset + index);
        index += 1;
    }
    bytes
}

const fn trace(value: u8) -> id_128 {
    id_128::from_bytes([value; 16])
}

fn write_call(call: rite_call) {
    write_bytes(call_offset, &call.encode_le());
}

fn write_bytes(offset: usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        write_mailbox_byte(offset + index, bytes[index]);
        index += 1;
    }
}

fn read_reply() -> rite_reply {
    let mut bytes = [0; mem::size_of::<rite_reply>()];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = read_mailbox_byte(call_offset + index);
        index += 1;
    }
    rite_reply::decode_le(&bytes)
}

fn write_mailbox_byte(offset: usize, byte: u8) {
    assert!(offset < layout::page_size as usize);
    let target = (layout::user_mailbox_address as *mut u8).wrapping_add(offset);
    // SAFETY: The fixed address names this task's sole mapped RW+NX mailbox.
    unsafe { ptr::write_volatile(target, byte) };
}

fn read_mailbox_byte(offset: usize) -> u8 {
    assert!(offset < layout::page_size as usize);
    let source = (layout::user_mailbox_address as *const u8).wrapping_add(offset);
    // SAFETY: The fixed address names this task's sole mapped RW+NX mailbox.
    unsafe { ptr::read_volatile(source) }
}

#[panic_handler]
fn panic(_information: &PanicInfo<'_>) -> ! {
    // SAFETY: Violated target invariants terminate at a supervisor-verifiable
    // wrong-site invalid opcode and never return to Rust.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
