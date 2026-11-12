#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Read one external immutable package and admit its semantic manifest in
//! userspace. This loader does not interpret or execute the admitted edition.
//! A fixed console marker proves an accepted console witness reached the helper;
//! it is a loader diagnostic, never the edition's authored output.

use anomalous_native_runtime::{
    console_error, console_transport, console_writer, edition_error, edition_reader,
    edition_receipt, edition_transport, manifest_report as wire, maximum_read_bytes,
    program_descriptor, program_startup_size, program_status, service,
};
use anomalous_program_manifest::{admit, admitted_edition, authority, error, maximum_image_bytes};
use core::{
    arch::{asm, global_asm},
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));

#[repr(C)]
struct call_record {
    operation: u64,
    channel: u64,
    pointer: u64,
    length: u64,
    sequence: u64,
    offset: u64,
    status: u64,
    returned_sequence: u64,
    returned_length: u64,
}

unsafe extern "C" {
    fn anomalous_native_manifest_loader_call(record: *mut call_record);
}

fn call(
    operation: u64,
    channel: u64,
    pointer: u64,
    length: usize,
    sequence: u64,
    offset: u64,
) -> call_record {
    let mut record = call_record {
        operation,
        channel,
        pointer,
        length: length as u64,
        sequence,
        offset,
        status: u64::MAX,
        returned_sequence: 0,
        returned_length: 0,
    };
    // SAFETY: The inspected stub preserves SysV callee-saved registers and
    // writes only the three reply words in this live record. The kernel
    // independently validates every service pointer and its retained owner.
    unsafe { anomalous_native_manifest_loader_call(&mut record) };
    record
}

struct native_edition;

impl edition_transport for native_edition {
    fn read_at(
        &mut self,
        handle: u64,
        offset: u64,
        destination: &mut [u8],
        sequence: u64,
    ) -> Result<edition_receipt, edition_error> {
        let result = call(
            2,
            handle,
            destination.as_mut_ptr() as u64,
            destination.len(),
            sequence,
            offset,
        );
        if result.status != program_status::ok {
            return Err(edition_error::transport);
        }
        // edition_reader validates exact sequence and planned committed length.
        Ok(edition_receipt {
            sequence: result.returned_sequence,
            length: result.returned_length,
        })
    }
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
        let result = call(
            1,
            channel,
            self.scratch.as_ptr() as u64,
            bytes.len(),
            sequence,
            0,
        );
        self.scratch.fill(0);
        if result.status != program_status::ok
            || result.returned_sequence != sequence
            || result.returned_length != bytes.len() as u64
        {
            return Err(console_error::transport);
        }
        Ok(bytes.len())
    }
}

fn put_u32(report: &mut [u8], offset: usize, value: u32) {
    report[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(report: &mut [u8], offset: usize, value: u64) {
    report[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn begin_report(report: &mut [u8], ceiling: u64, image_length: usize, reads: u64, bytes: u64) {
    assert_eq!(report.len(), wire::report_bytes);
    report.fill(0);
    report[..8].copy_from_slice(wire::magic);
    report[wire::offset::version..wire::offset::version + 2]
        .copy_from_slice(&wire::version.to_le_bytes());
    report[wire::offset::length..wire::offset::length + 2]
        .copy_from_slice(&(wire::report_bytes as u16).to_le_bytes());
    put_u64(report, wire::offset::ceiling, ceiling);
    put_u64(report, wire::offset::image_length, image_length as u64);
    put_u64(report, wire::offset::read_calls, reads);
    put_u64(report, wire::offset::read_bytes, bytes);
}

/// Only a complete admission witness can reach this publication helper.
/// Its image remains immutably borrowed throughout the marker and report.
fn accepted(
    witness: &admitted_edition<'_>,
    startup: &program_descriptor,
    report: &mut [u8],
    scratch: &mut [u8],
) {
    let console_calls = if witness.granted_services() & authority::console_write != 0 {
        let mut writer =
            console_writer::new(startup.console().unwrap(), native_console { scratch }).unwrap();
        writer.write(wire::console_marker).unwrap();
        writer.written()
    } else {
        0
    };
    put_u32(report, wire::offset::status, wire::status::admitted);
    put_u64(
        report,
        wire::offset::required_services,
        witness.required_services(),
    );
    put_u64(
        report,
        wire::offset::granted_services,
        witness.granted_services(),
    );
    put_u64(
        report,
        wire::offset::edition_length,
        witness.edition().len() as u64,
    );
    put_u64(report, wire::offset::console_calls, console_calls);
    report[wire::offset::identity..wire::offset::identity + 32]
        .copy_from_slice(&witness.identity());
    let intent = witness.intent().as_bytes();
    put_u32(report, wire::offset::intent_length, intent.len() as u32);
    report[wire::offset::intent..wire::offset::intent + intent.len()].copy_from_slice(intent);
}

fn refused(report: &mut [u8], failure: error) {
    let reason = match failure {
        error::edition => wire::reason::edition,
        error::syntax => wire::reason::syntax,
        error::unsupported => wire::reason::unsupported,
        error::intent => wire::reason::intent,
        error::limit => wire::reason::limit,
        error::image => wire::reason::image,
        error::encoding => wire::reason::encoding,
        error::version => wire::reason::version,
        error::profile => wire::reason::profile,
        error::layout => wire::reason::layout,
        error::identity => wire::reason::identity,
        error::mismatch => wire::reason::mismatch,
        error::ceiling => wire::reason::ceiling,
        error::output => wire::reason::output,
    };
    // No semantic claims are copied from a refused package. The untouched
    // zero fields distinguish unavailable claims from admitted zero authority.
    put_u32(report, wire::offset::status, wire::status::refused);
    put_u32(report, wire::offset::reason, reason);
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_native_manifest_loader_run(pointer: *const u8, length: usize) -> ! {
    assert_eq!(pointer as u64, 0x801000);
    assert_eq!(length, program_startup_size);
    let mut descriptor_bytes = [0; program_startup_size];
    // SAFETY: Startup admits this exact private, initialized ANPG span. Copy
    // it once; subsequent service and policy decisions use only the snapshot.
    unsafe { ptr::copy_nonoverlapping(pointer, descriptor_bytes.as_mut_ptr(), length) };
    let startup = program_descriptor::decode(&descriptor_bytes).unwrap();
    assert_eq!(
        startup.granted_services(),
        service::file_read | service::console_write
    );
    let ceiling = match startup.arguments() {
        b"ceiling=0" => 0,
        b"ceiling=1" => authority::console_write,
        _ => panic!("manifest ceiling argument"),
    };
    let image_length = usize::try_from(startup.edition_length()).unwrap();
    assert!((1..=maximum_image_bytes).contains(&image_length));
    let arena_length = usize::try_from(startup.arena_length()).unwrap();
    assert!(arena_length >= wire::package_offset + maximum_image_bytes);
    // SAFETY: The kernel retains the exact descriptor-named private RW+NX
    // arena. Sole task entry creates this unique borrow once. The report page
    // and package are split into disjoint slices; none escape terminal exit.
    let storage =
        unsafe { core::slice::from_raw_parts_mut(startup.arena_base() as *mut u8, arena_length) };
    for byte in storage.iter() {
        // SAFETY: Every byte is in the retained readable arena; volatile keeps
        // the fresh-storage check observable before any service publication.
        assert_eq!(unsafe { ptr::read_volatile(byte) }, 0);
    }
    let (report_page, package_storage) = storage.split_at_mut(wire::package_offset);
    let (report_storage, scratch) = report_page.split_at_mut(wire::console_scratch_offset);
    let report = &mut report_storage[..wire::report_bytes];
    let image = &mut package_storage[..image_length];
    let mut reader = edition_reader::new(
        startup.edition().unwrap(),
        startup.edition_length(),
        native_edition,
    )
    .unwrap();
    let mut offset = 0;
    for chunk in image.chunks_mut(maximum_read_bytes) {
        assert_eq!(reader.read_at(offset, chunk).unwrap(), chunk.len());
        offset += chunk.len() as u64;
    }
    assert_eq!(reader.bytes_read(), image_length as u64);
    begin_report(
        report,
        ceiling,
        image_length,
        reader.reads(),
        reader.bytes_read(),
    );
    // Complete transport precedes hashing/derivation, and the admitted witness
    // borrows this immutable snapshot until all dependent actions finish.
    match admit(image, ceiling) {
        Ok(witness) => accepted(&witness, &startup, report, scratch),
        Err(failure) => refused(report, failure),
    }
    // The fixed diagnostic remains in the private arena for the terminal gate
    // to copy. Opcode5 revokes services; it cannot return into this task.
    call(5, 0, 0, 0, 0, 0);
    panic!("terminal exit returned");
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // An unarmed fault is a failed proof, never a manifest refusal.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
