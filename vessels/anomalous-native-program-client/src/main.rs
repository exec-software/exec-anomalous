#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! A task launched with a program to run rather than a document to edit.
//!
//! The launch is an `ANPG` descriptor: a bounded arena range, an edition to
//! run, an optional console, and `granted_services` as a ceiling. There is no
//! interpreter yet, so this runs no edition. What it proves is the part that
//! has to be true first -- that the launch contract survives the boundary
//! intact, and that a task cannot talk itself into authority the launch did
//! not grant.
//!
//! The proof is written where the kernel can read it. A task can only exit,
//! so an assertion here would be indistinguishable from any other fault;
//! instead the grant this task actually received is copied into the head of
//! its arena and the kernel compares it against what it granted.

use anomalous_native_runtime::{
    arena, granted_handle, program_descriptor, program_startup_size, service,
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
    // Observe every byte before writing one. A launch that reused a previous
    // task's arena would show here, and nowhere else.
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
    exit();
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // An unarmed fault is a proof failure, never an expected outcome.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
