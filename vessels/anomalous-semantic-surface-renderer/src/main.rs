#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

use anomalous_abi::{
    id_128, rite_call, rite_display_patch_v1, rite_exit_payload, rite_operation, rite_payload_kind,
    rite_reply, rite_rights, rite_semantic_surface_request_v1,
    rite_semantic_surface_snapshot_v1_size, rite_status,
};
use anomalous_desktop_renderer::{
    frame_height, frame_width, row_byte_count, transport_render_snapshot,
};
use anomalous_desktop_transport::{surface_snapshot_v1, surface_snapshot_v1_byte_count};
use core::arch::{asm, global_asm};
use core::mem;
use core::panic::PanicInfo;
use core::ptr;

mod layout;

const call_offset: usize = 0;
const payload_offset: usize = mem::size_of::<rite_call>();
const display_header_size: usize = mem::size_of::<rite_display_patch_v1>();
const display_pixels_offset: usize = payload_offset + display_header_size;
const read_request_size: u32 = mem::size_of::<rite_semantic_surface_request_v1>() as u32;
const snapshot_size: u32 = rite_semantic_surface_snapshot_v1_size;
const display_payload_size: u32 = display_header_size as u32 + row_byte_count as u32;
const exit_payload_size: u32 = mem::size_of::<rite_exit_payload>() as u32;

const _: () = {
    assert!(mem::size_of::<rite_call>() == 40);
    assert!(mem::size_of::<rite_reply>() == 40);
    assert!(read_request_size == 16);
    assert!(snapshot_size == 528);
    assert!(surface_snapshot_v1_byte_count == 528);
    assert!(frame_width == 1024);
    assert!(frame_height == 768);
    assert!(row_byte_count == 3072);
    assert!(display_header_size == 40);
    assert!(display_payload_size == 3112);
    assert!(display_pixels_offset + row_byte_count <= layout::page_size as usize);
};

global_asm!(
    include_str!("entry.S"),
    rite_gate_vector = const layout::rite_gate_vector,
    user_mailbox_address = const layout::user_mailbox_address,
    initial_user_flags = const layout::initial_user_flags,
    options(att_syntax)
);

unsafe extern "C" {
    fn anomalous_semantic_surface_renderer_submit_denied_rite();
    fn anomalous_semantic_surface_renderer_delivery_rite();
    fn anomalous_semantic_surface_renderer_display_rite();
    fn anomalous_semantic_surface_renderer_exit_rite();
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_semantic_surface_renderer_run() -> u64 {
    if render_surface().is_err() {
        return 1;
    }

    // SAFETY: The shim preserves Rust callee-saved registers, clears all
    // register-carried state except the fixed mailbox pointer, and enters the
    // sole Rite gate. Successful task exit is terminal.
    unsafe { anomalous_semantic_surface_renderer_exit_rite() };
    1
}

fn render_surface() -> Result<(), ()> {
    submit_denied()?;
    let snapshot = read_snapshot()?;
    validate_binding(&snapshot)?;
    let renderer = transport_render_snapshot::try_new(&snapshot).map_err(|_| ())?;
    let mut pixels = [0_u8; row_byte_count];
    let mut row = 0;
    while row < frame_height {
        renderer.render_row(row, &mut pixels).map_err(|_| ())?;
        submit_display_row(row, &pixels)?;
        row += 1;
    }
    prepare_exit()
}

fn submit_denied() -> Result<(), ()> {
    let trace = submit_denied_trace();
    write_call(rite_call::new(
        rite_operation::semantic_surface_submit,
        rite_payload_kind::semantic_surface_snapshot,
        rite_rights::semantic_surface_submit,
        snapshot_size,
        snapshot_size,
        trace,
    ));

    // SAFETY: Admission must reject this operation against the renderer's
    // read/display/exit grant before consulting the absent snapshot body.
    unsafe { anomalous_semantic_surface_renderer_submit_denied_rite() };

    let reply = read_reply();
    if reply.has_supported_header()
        && reply.status == rite_status::denied
        && reply.payload_kind == rite_payload_kind::none
        && reply.payload_size == 0
        && reply.sequence == 1
        && reply.trace_id == trace
    {
        Ok(())
    } else {
        Err(())
    }
}

fn read_snapshot() -> Result<surface_snapshot_v1, ()> {
    let trace = read_trace();
    let request = rite_semantic_surface_request_v1::new();
    if !request.has_supported_body() {
        return Err(());
    }
    write_call(rite_call::new(
        rite_operation::semantic_surface_read,
        rite_payload_kind::semantic_surface_request,
        rite_rights::semantic_surface_read,
        read_request_size,
        read_request_size,
        trace,
    ));
    write_bytes(payload_offset, &request.encode_le());

    // SAFETY: The complete request is in the fixed mailbox and the shim
    // preserves Rust callee-saved registers while clearing ambient GPR state.
    unsafe { anomalous_semantic_surface_renderer_delivery_rite() };

    let reply = read_reply();
    if !reply.has_supported_header()
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::semantic_surface_snapshot
        || reply.payload_size != snapshot_size
        || reply.sequence != 2
        || reply.trace_id != trace
    {
        return Err(());
    }
    let mut bytes = [0_u8; surface_snapshot_v1_byte_count];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = read_mailbox_byte(payload_offset + index);
        index += 1;
    }
    surface_snapshot_v1::try_from_bytes(bytes).map_err(|_| ())
}

fn validate_binding(snapshot: &surface_snapshot_v1) -> Result<(), ()> {
    let binding = snapshot.binding();
    if binding.surface().slot() == 51
        && binding.surface().generation() == 3
        && binding.scene().slot() == 41
        && binding.scene().generation() == 7
        && binding.base_revision() == 1
        && binding.revision() == 2
        && binding.renderer().slot() == 61
        && binding.renderer().generation() == 1
        && binding.attempt() == 1
    {
        Ok(())
    } else {
        Err(())
    }
}

fn submit_display_row(row: u32, pixels: &[u8; row_byte_count]) -> Result<(), ()> {
    let patch = rite_display_patch_v1::try_new(0, row, frame_width, 1).ok_or(())?;
    if !patch.has_supported_body() || patch.pixel_byte_length() as usize != row_byte_count {
        return Err(());
    }
    let trace = display_trace(row);
    write_call(rite_call::new(
        rite_operation::display_update,
        rite_payload_kind::display_patch,
        rite_rights::display_update,
        display_payload_size,
        display_payload_size,
        trace,
    ));
    write_bytes(payload_offset, &encode_display_patch(&patch));
    write_bytes(display_pixels_offset, pixels);

    // SAFETY: The complete row patch is in the fixed mailbox and the shim
    // exposes no register-carried authority beyond the mailbox pointer.
    unsafe { anomalous_semantic_surface_renderer_display_rite() };
    validate_reply(u64::from(row) + 3, trace)
}

fn prepare_exit() -> Result<(), ()> {
    let payload = rite_exit_payload::new(0);
    if !payload.has_supported_body() {
        return Err(());
    }
    write_call(rite_call::new(
        rite_operation::task_exit,
        rite_payload_kind::exit_status,
        rite_rights::task_exit,
        exit_payload_size,
        exit_payload_size,
        exit_trace(),
    ));
    write_bytes(payload_offset, &payload.encode_le());
    Ok(())
}

fn validate_reply(sequence: u64, trace: id_128) -> Result<(), ()> {
    let reply = read_reply();
    if reply.has_supported_header()
        && reply.status == rite_status::ok
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

fn encode_display_patch(patch: &rite_display_patch_v1) -> [u8; 40] {
    let mut bytes = [0_u8; 40];
    bytes[0..2].copy_from_slice(&patch.structure_size().to_le_bytes());
    bytes[2..4].copy_from_slice(&patch.version().to_le_bytes());
    bytes[4..8].copy_from_slice(&patch.flags().to_le_bytes());
    bytes[8..12].copy_from_slice(&patch.x().to_le_bytes());
    bytes[12..16].copy_from_slice(&patch.y().to_le_bytes());
    bytes[16..20].copy_from_slice(&patch.width().to_le_bytes());
    bytes[20..24].copy_from_slice(&patch.height().to_le_bytes());
    bytes[24..28].copy_from_slice(&patch.row_bytes().to_le_bytes());
    bytes[28..30].copy_from_slice(&patch.pixel_format().raw().to_le_bytes());
    bytes[30..32].copy_from_slice(&patch.reserved().to_le_bytes());
    bytes[32..36].copy_from_slice(&patch.pixel_byte_length().to_le_bytes());
    bytes[36..40].copy_from_slice(&patch.reserved_2().to_le_bytes());
    bytes
}

const fn read_trace() -> id_128 {
    id_128::from_bytes(*b"surface-read-v1!")
}

const fn submit_denied_trace() -> id_128 {
    id_128::from_bytes(*b"surface-deny-v1!")
}

fn display_trace(row: u32) -> id_128 {
    let encoded_row = row.to_le_bytes();
    id_128::from_bytes([
        b's',
        b'u',
        b'r',
        b'f',
        1,
        0,
        0,
        0,
        encoded_row[0],
        encoded_row[1],
        encoded_row[2],
        encoded_row[3],
        b'r',
        b'o',
        b'w',
        0,
    ])
}

const fn exit_trace() -> id_128 {
    id_128::from_bytes(*b"surface-exit-v1!")
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
    let mut bytes = [0_u8; mem::size_of::<rite_reply>()];
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
    // SAFETY: The fixed address is this task's sole kernel-provided RW+NX
    // mailbox and the bound above keeps the write inside its one page.
    unsafe { ptr::write_volatile(target, byte) };
}

fn read_mailbox_byte(offset: usize) -> u8 {
    assert!(offset < layout::page_size as usize);
    let source = (layout::user_mailbox_address as *const u8).wrapping_add(offset);
    // SAFETY: The fixed address is this task's sole kernel-provided RW+NX
    // mailbox and the bound above keeps the read inside its one page.
    unsafe { ptr::read_volatile(source) }
}

#[panic_handler]
fn panic(_information: &PanicInfo<'_>) -> ! {
    // SAFETY: A violated target invariant terminates at a supervisor-visible
    // invalid opcode and returns to no Rust frame.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
