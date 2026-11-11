#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Native program console transport over the existing ANPG launch contract.
//! The client checks real replies and adversarial raw requests. Edition handles
//! remain inert until source delivery and semantic-manifest admission exist.

use anomalous_native_runtime::{
    arena, console_error, console_transport, console_writer, granted_handle, maximum_write_bytes,
    program_descriptor, program_startup_size, program_status, service,
};
use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));

/// The arena offset the kernel reads the proof record back from, and the
/// number of words in it. Offset zero: a program's first act is to state what
/// it was given, before it has had a chance to do anything else.
pub const proof_offset: usize = 0;
pub const proof_words: usize = 6;

// Deliberately unaligned and crossing a page boundary for actual user-copy.
const console_buffer_offset: usize = 4095;

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
    fn anomalous_native_program_client_call(record: *mut call_record);
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
    // SAFETY: Assembly preserves SysV callee-saved registers and writes only
    // the three result fields in this live private record. The stub never
    // dereferences the pointer field; kernel copy admission validates it.
    unsafe { anomalous_native_program_client_call(&mut record) };
    record
}

fn exit() -> ! {
    call(5, 0, 0, 0, 0);
    panic!("terminal exit returned");
}

struct native_console<'a> {
    scratch: &'a mut [u8],
}

impl console_transport for native_console<'_> {
    fn write(&mut self, channel: u64, bytes: &[u8], sequence: u64) -> Result<usize, console_error> {
        if bytes.len() > self.scratch.len() {
            return Err(console_error::length);
        }
        self.scratch[..bytes.len()].copy_from_slice(bytes);
        let reply = call(
            1,
            channel,
            self.scratch.as_ptr() as u64,
            bytes.len(),
            sequence,
        );
        if reply.status != program_status::ok
            || reply.returned_sequence != sequence
            || reply.returned_length != bytes.len() as u64
        {
            return Err(console_error::transport);
        }
        Ok(bytes.len())
    }
}

fn refused(handle: u64, address: u64, length: usize, sequence: u64, expected: u64) {
    let reply = call(1, handle, address, length, sequence);
    assert_eq!(reply.status, expected);
    assert_eq!(reply.returned_sequence, 0);
    assert_eq!(reply.returned_length, 0);
}

fn exercise_console(startup: &program_descriptor, scratch: &mut [u8]) {
    let address = scratch.as_ptr() as u64;
    let Some(handle) = startup.console() else {
        // The previous incarnation's valid number must confer no authority.
        for handle in [0x62, 0] {
            refused(handle, address, 1, 1, program_status::denied);
        }
        return;
    };
    let raw = handle.raw();
    for (channel, pointer, length, sequence, status) in [
        (0, address, 1, 1, program_status::denied),
        (
            startup.edition_handle(),
            address,
            1,
            1,
            program_status::denied,
        ),
        (raw ^ 1, address, 1, 1, program_status::denied),
        (raw, address, 1, 0, program_status::invalid),
        (raw, address, 1, 2, program_status::invalid),
        (raw, address, 0, 1, program_status::invalid),
        (
            raw,
            address,
            maximum_write_bytes + 1,
            1,
            program_status::invalid,
        ),
        (raw, u64::MAX, 4, 1, program_status::bad_address),
        (
            raw,
            startup.arena_base() + startup.arena_length() - 1,
            2,
            1,
            program_status::bad_address,
        ),
        (raw, 0x400000, 1, 1, program_status::bad_address),
    ] {
        refused(channel, pointer, length, sequence, status);
    }
    for (word, length) in [(0xff_u16, 1), (0x1b, 1), (0x9bc2, 2)] {
        scratch[..length].copy_from_slice(&word.to_le_bytes()[..length]);
        refused(raw, address, length, 1, program_status::invalid);
    }
    let mut writer = console_writer::new(handle, native_console { scratch }).unwrap();
    writer
        .write("Native console: ready — ✓\n".as_bytes())
        .unwrap();
    assert_eq!(writer.written(), 1);
    // A replay cannot republish the first line or consume sequence two.
    refused(raw, address, 30, 1, program_status::invalid);
    writer.write(b"Native console: reply checked\n").unwrap();
    assert_eq!(writer.written(), 2);
}

/// What this task may reach, as the launch stated it.
///
/// Read back by the kernel, so every value here is one the kernel can compare
/// against what it wrote. The console handle is reported as present or absent
/// rather than by value: a handle is the kernel's live binding, and echoing
/// one proves nothing about authority.
fn record_grant(startup: &program_descriptor, console: Option<granted_handle>, into: &mut [u8]) {
    for (index, value) in [
        startup.arena_length(),
        startup.edition_length(),
        startup.granted_services(),
        startup.arguments().len() as u64,
        u64::from(console.is_some()),
        startup.edition_handle(),
    ]
    .into_iter()
    .enumerate()
    {
        into[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
    }
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_native_program_client_run(pointer: *const u8, length: usize) -> ! {
    // Startup admission checked these exact registers. Validate the shape
    // before reading, and keep a local copy independent of the writable
    // descriptor bytes: a descriptor is launch data, never a capability, and
    // nothing may re-read it after the decisions below are made.
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, program_startup_size);
    let mut bytes = [0; program_startup_size];
    // SAFETY: The loader reserves this exact 256-byte region in private RW,
    // fills it before task publication, and startup admits its exact pointer.
    unsafe { ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), program_startup_size) };
    let startup = program_descriptor::decode(&bytes).unwrap();

    // A console handle exists exactly when the grant that justifies it does.
    // decode already refuses one without the other, so reaching this means the
    // launch paired them; it is checked again because this is the side of the
    // boundary that would be lied to.
    let console = startup.console();
    assert_eq!(console.is_some(), startup.grants(service::console_write));
    // A ceiling is not a wish list. Nothing outside the vocabulary is granted,
    // and no grant at all is not a grant.
    assert!(!startup.grants(0));
    assert!(!startup.grants(service::known + 1));
    for family in [
        service::console_write,
        service::file_read,
        service::clock_read,
    ] {
        assert_eq!(
            startup.grants(family),
            startup.granted_services() & family == family
        );
    }

    // SAFETY: The admitted launch retains this private RW+NX, guard-bounded
    // arena. It is disjoint from code, data, descriptor and stack; sole task
    // entry creates its unique borrow once, and no pointer outlives task exit.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(
            startup.arena_base() as *mut u8,
            startup.arena_length() as usize,
        )
    };
    // The complete arena is fresh; no edition is mapped by this profile.
    for byte in storage.iter() {
        // SAFETY: Complete retained readable arena; volatile prevents elision.
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }

    let mut allocator = arena::new(storage);
    let record = allocator.allocate(proof_words * 8, 8).unwrap();
    assert_eq!(
        record.as_ptr() as u64,
        startup.arena_base() + proof_offset as u64
    );
    record_grant(&startup, console, record);

    exercise_console(
        &startup,
        &mut storage[console_buffer_offset..console_buffer_offset + maximum_write_bytes],
    );

    exit();
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // An unarmed fault is a proof failure, never an expected outcome.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
