#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

use anomalous_abi::{
    id_128, rite_call, rite_exit_payload, rite_operation, rite_payload_kind, rite_reply,
    rite_rights, rite_semantic_surface_request_v1, rite_semantic_surface_snapshot_v1_size,
    rite_status,
};
use core::arch::{asm, global_asm};
use core::mem;
use core::panic::PanicInfo;
use core::ptr;

mod layout;

const call_offset: usize = 0;
const payload_offset: usize = mem::size_of::<rite_call>();
const request_size: u32 = mem::size_of::<rite_semantic_surface_request_v1>() as u32;
const snapshot_size: u32 = rite_semantic_surface_snapshot_v1_size;
const exit_size: u32 = mem::size_of::<rite_exit_payload>() as u32;

const read_denied_trace: u8 = 0x11;
const malformed_submit_trace: u8 = 0x12;
const freshness_denied_trace: u8 = 0x13;
const submit_trace: u8 = 0x14;
const backpressured_trace: u8 = 0x15;
const exit_trace: u8 = 0x16;

const surface_generation_offset: usize = 40;

const _: () = {
    assert!(mem::size_of::<rite_call>() == 40);
    assert!(mem::size_of::<rite_reply>() == 40);
    assert!(request_size == 16);
    assert!(snapshot_size == 528);
    assert!(payload_offset + snapshot_size as usize <= layout::page_size as usize);
};

#[used]
#[unsafe(link_section = ".rodata.semantic_surface_snapshot")]
static embedded_surface_snapshot: [u8; 528] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/semantic_surface_snapshot.bin"));

global_asm!(
    include_str!("entry.S"),
    rite_gate_vector = const layout::rite_gate_vector,
    user_mailbox_address = const layout::user_mailbox_address,
    initial_user_flags = const layout::initial_user_flags,
    options(att_syntax)
);

unsafe extern "C" {
    fn anomalous_semantic_surface_source_read_denied_rite();
    fn anomalous_semantic_surface_source_malformed_submit_rite();
    fn anomalous_semantic_surface_source_freshness_denied_rite();
    fn anomalous_semantic_surface_source_submit_rite();
    fn anomalous_semantic_surface_source_backpressured_rite();
    fn anomalous_semantic_surface_source_exit_rite();
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_semantic_surface_source_run() -> u64 {
    if read_denied().is_err()
        || malformed_submit().is_err()
        || freshness_denied().is_err()
        || submit().is_err()
        || backpressured().is_err()
        || prepare_exit().is_err()
    {
        return 1;
    }

    // SAFETY: The shim preserves Rust callee-saved registers, clears every
    // register other than the fixed mailbox pointer, and enters the one Rite
    // gate. A successful task exit is terminal; any return reaches the target's
    // deliberate failure path.
    unsafe { anomalous_semantic_surface_source_exit_rite() };
    1
}

fn read_denied() -> Result<(), ()> {
    let request = rite_semantic_surface_request_v1::new();
    let trace = trace(read_denied_trace);
    write_call(rite_call::new(
        rite_operation::semantic_surface_read,
        rite_payload_kind::semantic_surface_request,
        rite_rights::semantic_surface_read,
        request_size,
        request_size,
        trace,
    ));
    write_bytes(payload_offset, &request.encode_le());
    // SAFETY: The fixed mailbox contains the complete call and request body.
    unsafe { anomalous_semantic_surface_source_read_denied_rite() };
    validate_empty_reply(rite_status::denied, 1, trace)
}

fn malformed_submit() -> Result<(), ()> {
    let mut bytes = embedded_surface_snapshot;
    bytes[0] = 0;
    submit_snapshot(
        &bytes,
        malformed_submit_trace,
        2,
        rite_status::bad_payload,
        anomalous_semantic_surface_source_malformed_submit_rite,
    )
}

fn freshness_denied() -> Result<(), ()> {
    let mut bytes = embedded_surface_snapshot;
    bytes[surface_generation_offset] = bytes[surface_generation_offset].wrapping_add(1);
    submit_snapshot(
        &bytes,
        freshness_denied_trace,
        3,
        rite_status::denied,
        anomalous_semantic_surface_source_freshness_denied_rite,
    )
}

fn submit() -> Result<(), ()> {
    submit_snapshot(
        &embedded_surface_snapshot,
        submit_trace,
        4,
        rite_status::ok,
        anomalous_semantic_surface_source_submit_rite,
    )
}

fn backpressured() -> Result<(), ()> {
    submit_snapshot(
        &embedded_surface_snapshot,
        backpressured_trace,
        5,
        rite_status::would_block,
        anomalous_semantic_surface_source_backpressured_rite,
    )
}

fn submit_snapshot(
    bytes: &[u8; 528],
    trace_byte: u8,
    sequence: u64,
    expected_status: rite_status,
    invoke: unsafe extern "C" fn(),
) -> Result<(), ()> {
    let trace = trace(trace_byte);
    write_call(rite_call::new(
        rite_operation::semantic_surface_submit,
        rite_payload_kind::semantic_surface_snapshot,
        rite_rights::semantic_surface_submit,
        snapshot_size,
        snapshot_size,
        trace,
    ));
    write_bytes(payload_offset, bytes);
    // SAFETY: Each selected shim has the same register-preserving Rite-call
    // contract and the fixed mailbox contains the complete 528-byte body.
    unsafe { invoke() };
    validate_empty_reply(expected_status, sequence, trace)
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
