#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

use anomalous_abi::{
    id_128, rite_call, rite_display_patch_v1, rite_exit_payload, rite_operation, rite_payload_kind,
    rite_reply, rite_rights, rite_status,
};
use anomalous_desktop_renderer::{
    capsule_byte_count, frame_height, frame_width, glyph_table_fingerprint, render_capsule,
    render_row, row_byte_count,
};
use core::arch::{asm, global_asm};
use core::mem;
use core::panic::PanicInfo;
use core::ptr;

mod layout;

const generation_1_row_count: u32 = 384;
const generation_2_row_count: u32 = frame_height;
const display_payload_size: u32 =
    mem::size_of::<rite_display_patch_v1>() as u32 + row_byte_count as u32;
const task_exit_payload_size: u32 = mem::size_of::<rite_exit_payload>() as u32;
const call_offset: usize = 0;
const payload_offset: usize = mem::size_of::<rite_call>();
const display_pixels_offset: usize = payload_offset + mem::size_of::<rite_display_patch_v1>();

const _: () = {
    assert!(frame_width == 1024);
    assert!(frame_height == 768);
    assert!(row_byte_count == 3072);
    assert!(capsule_byte_count == 416);
    assert!(display_payload_size == 3112);
    assert!(display_pixels_offset + row_byte_count <= layout::page_size as usize);
    assert!(generation_1_row_count == frame_height / 2);
};

#[used]
#[unsafe(link_section = ".rodata.renderer_capsule")]
static embedded_render_capsule: [u8; capsule_byte_count] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/supervised_renderer_capsule.bin"));

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".rodata.renderer_glyph_fingerprint")]
pub static anomalous_supervised_renderer_glyph_fingerprint: [u8; 16] = glyph_table_fingerprint;

global_asm!(
    include_str!("entry.S"),
    scheduler_gate_vector = const layout::scheduler_gate_vector,
    rite_gate_vector = const layout::rite_gate_vector,
    user_mailbox_address = const layout::user_mailbox_address,
    scheduler_ready_operation = const layout::scheduler_ready_operation,
    initial_user_flags = const layout::initial_user_flags,
    running_user_flags = const layout::running_user_flags,
    mailbox_canary_a_offset = const layout::mailbox_canary_a_offset,
    mailbox_canary_b_offset = const layout::mailbox_canary_b_offset,
    mailbox_arm_offset = const layout::mailbox_arm_offset,
    generation_1_canary_a = const layout::generation_1_canary_a,
    generation_1_canary_b = const layout::generation_1_canary_b,
    generation_1_arm = const layout::generation_1_arm,
    options(att_syntax)
);

unsafe extern "C" {
    fn anomalous_supervised_renderer_submit_display_rite();
    fn anomalous_supervised_renderer_submit_exit_rite();
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_supervised_renderer_generation_1_run() -> u64 {
    match render_generation(1, generation_1_row_count) {
        Ok(()) => 0,
        Err(()) => 1,
    }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_supervised_renderer_generation_2_run() -> u64 {
    if render_generation(2, generation_2_row_count).is_err() {
        return 1;
    }
    if prepare_task_exit().is_err() {
        return 1;
    }

    // SAFETY: The assembly shim preserves the Rust callee-saved registers,
    // supplies no register-based authority, and enters the fixed 0xf3 gate
    // with R15 naming only this task's one-page mailbox. A successful exit is
    // terminal; any return is converted to the generation-2 failure UD.
    unsafe { anomalous_supervised_renderer_submit_exit_rite() };
    1
}

fn render_generation(generation: u8, row_limit: u32) -> Result<(), ()> {
    let capsule = render_capsule::try_from_slice(&embedded_render_capsule).map_err(|_| ())?;
    let mut row_pixels = [0_u8; row_byte_count];
    let mut row = 0;
    while row < row_limit {
        render_row(&capsule, row, &mut row_pixels).map_err(|_| ())?;
        submit_display_row(generation, row, &row_pixels)?;
        row += 1;
    }
    Ok(())
}

fn submit_display_row(generation: u8, row: u32, pixels: &[u8; row_byte_count]) -> Result<(), ()> {
    let patch = rite_display_patch_v1::try_new(0, row, frame_width, 1).ok_or(())?;
    if !patch.has_supported_body() || patch.pixel_byte_length() as usize != row_byte_count {
        return Err(());
    }
    let trace = display_trace(generation, row);
    let call = rite_call::new(
        rite_operation::display_update,
        rite_payload_kind::display_patch,
        rite_rights::display_update,
        display_payload_size,
        display_payload_size,
        trace,
    );
    write_value(call_offset, &call);
    write_value(payload_offset, &patch);
    write_bytes(display_pixels_offset, pixels);

    // SAFETY: The assembly shim preserves Rust's callee-saved registers and
    // enters the fixed Rite gate with all GPRs zero except the fixed mailbox in
    // R15. The kernel copies and validates the complete call before replying.
    unsafe { anomalous_supervised_renderer_submit_display_rite() };

    let reply = read_value::<rite_reply>(call_offset);
    validate_reply(reply, u64::from(row) + 1, trace)
}

fn prepare_task_exit() -> Result<(), ()> {
    let trace = exit_trace();
    let call = rite_call::new(
        rite_operation::task_exit,
        rite_payload_kind::exit_status,
        rite_rights::task_exit,
        task_exit_payload_size,
        task_exit_payload_size,
        trace,
    );
    let payload = rite_exit_payload::new(0);
    if !payload.has_supported_body() {
        return Err(());
    }
    write_value(call_offset, &call);
    write_value(payload_offset, &payload);
    Ok(())
}

fn validate_reply(reply: rite_reply, sequence: u64, trace: id_128) -> Result<(), ()> {
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

fn display_trace(generation: u8, row: u32) -> id_128 {
    let encoded_row = row.to_le_bytes();
    id_128::from_bytes([
        0x72,
        0x65,
        0x6e,
        0x64,
        generation,
        0,
        0,
        0,
        encoded_row[0],
        encoded_row[1],
        encoded_row[2],
        encoded_row[3],
        0x72,
        0x6f,
        0x77,
        0,
    ])
}

const fn exit_trace() -> id_128 {
    id_128::from_bytes(*b"renderer-exit-v2")
}

fn write_value<T>(offset: usize, value: &T) {
    let source = ptr::from_ref(value).cast::<u8>();
    let mut index = 0;
    while index < mem::size_of::<T>() {
        // SAFETY: `source` spans the initialized byte representation of `T`,
        // and the checked fixed offsets keep every volatile write inside the
        // task's one-page mailbox mapping.
        let byte = unsafe { ptr::read(source.add(index)) };
        write_mailbox_byte(offset + index, byte);
        index += 1;
    }
}

fn write_bytes(offset: usize, bytes: &[u8]) {
    let mut index = 0;
    while index < bytes.len() {
        write_mailbox_byte(offset + index, bytes[index]);
        index += 1;
    }
}

fn write_mailbox_byte(offset: usize, byte: u8) {
    assert!(offset < layout::page_size as usize);
    let target = (layout::user_mailbox_address as *mut u8).wrapping_add(offset);
    // SAFETY: The fixed address names this task's sole kernel-provided RW+NX
    // mailbox page for the complete lifetime of either renderer incarnation.
    unsafe { ptr::write_volatile(target, byte) };
}

fn read_value<T: Copy>(offset: usize) -> T {
    assert!(offset + mem::size_of::<T>() <= layout::page_size as usize);
    let source = (layout::user_mailbox_address as *const T).wrapping_byte_add(offset);
    // SAFETY: The fixed mailbox is page-aligned, `offset` preserves `T`'s
    // alignment for the only call site, and the preceding bound covers `T`.
    unsafe { ptr::read_volatile(source) }
}

#[panic_handler]
fn panic(_information: &PanicInfo<'_>) -> ! {
    // SAFETY: `ud2` deliberately transfers any violated target invariant to
    // the supervisor as a wrong-site fault; it accesses no memory and returns
    // to no invalid Rust frame.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
