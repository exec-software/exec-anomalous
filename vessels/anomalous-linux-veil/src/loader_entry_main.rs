#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Bounded userspace loader policy and immutable regular-file service.
//! Native source reads and staged caller copyout publish Linux results through
//! exact delayed acknowledgement; unsupported operations retire the caller.

use anomalous_veil_protocol::{
    caller_read_operation, complete_operation, completion_action, invocation,
    loader_policy::{self, access_outcome, read_error, record_size, startup_policy},
    native_status, request_operation, request_size,
    source_read::{self, source_binding, source_report},
};
use core::{arch::global_asm, panic::PanicInfo, ptr};

mod loader_entry_files;

global_asm!(include_str!("entry.S"), options(att_syntax));

#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.elf_canary")]
static anomalous_linux_veil_data_canary: u64 = 0x454c_465f_4441_5441;

// The host fills only this dedicated inert binding after independently checking
// the original zero record, preserved policy and canary. Volatile reads below
// observe provisioned data; numeric fields alone convey no source authority.
#[unsafe(no_mangle)]
#[unsafe(link_section = ".data.file_source")]
static anomalous_linux_veil_file_source: [u8; source_read::binding_size] =
    [0; source_read::binding_size];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.file_source_report")]
static mut anomalous_linux_veil_file_source_report: [u8; source_read::report_size] =
    [0; source_read::report_size];

#[unsafe(no_mangle)]
#[unsafe(link_section = ".bss.file_source_buffer")]
static mut anomalous_linux_veil_file_source_buffer: [u8; source_read::source_chunk_limit] =
    [0; source_read::source_chunk_limit];

const source_boundary_witness: u64 = 0x801ff8;

#[repr(C)]
struct call_record {
    operation: u64,
    arguments: [u64; 6],
    status: u64,
    value: u64,
    auxiliary: u64,
}

unsafe extern "C" {
    fn anomalous_linux_veil_call(record: *mut call_record);
    fn anomalous_linux_veil_failure_instruction() -> !;
}

fn call(operation: u64, arguments: [u64; 6]) -> call_record {
    let mut record = call_record {
        operation,
        arguments,
        status: u64::MAX,
        value: u64::MAX,
        auxiliary: u64::MAX,
    };
    // SAFETY: The assembly stub preserves the callee-saved registers and writes
    // only the result words of this live, private stack record.
    unsafe { anomalous_linux_veil_call(&mut record) };
    record
}

fn startup(role: u64) -> startup_policy {
    let mut bytes = [0; record_size];
    for (index, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: The complete 128-byte prefix is inside this task's
        // admitted initialized RW mapping. Volatile reads witness copied data.
        *byte = unsafe { ptr::read_volatile((0x800000 + index) as *const u8) };
    }
    startup_policy::decode(&bytes, role).expect("invalid userspace policy record")
}

fn request() -> invocation {
    let mut bytes = [0; request_size];
    let result = call(
        request_operation,
        [bytes.as_mut_ptr() as u64, request_size as u64, 0, 0, 0, 0],
    );
    assert_eq!(result.status, native_status::ok as u64);
    let request = invocation::decode(&bytes).expect("invalid copied Linux invocation");
    assert_eq!(
        (result.value, result.auxiliary),
        (request.sequence(), request_size as u64)
    );
    request
}

fn return_result(sequence: u64, result: u64) {
    let completed = call(
        complete_operation,
        [
            sequence,
            result,
            completion_action::return_to_caller as u64,
            0,
            0,
            0,
        ],
    );
    // The same veil resumes here only after the caller reaches a fresh Linux
    // boundary. The prior native completion has no staged effect or result data.
    assert_eq!(
        (completed.status, completed.value, completed.auxiliary),
        (native_status::ok as u64, sequence, 0)
    );
}

fn retire(sequence: u64) -> ! {
    call(
        complete_operation,
        [
            sequence,
            (-38i64) as u64,
            completion_action::retire_caller as u64,
            0,
            0,
            0,
        ],
    );
    // The controller retires this veil and the blocked caller without another
    // userspace return. This is deliberately not Linux exit_group/exit success.
    panic!("unsupported loader retirement returned");
}

fn copy_probes(sequence: u64, source: u64) {
    let mut witness = [0xa5; 24];
    let destination = witness.as_mut_ptr() as u64 + 8;
    let cases = [
        ([0, source, destination, 1, 0, 0], native_status::invalid),
        (
            [sequence - 1, source, destination, 1, 0, 0],
            native_status::invalid,
        ),
        (
            [sequence, source, destination, 0, 0, 0],
            native_status::invalid,
        ),
        (
            [sequence, source, destination, 1, 1, 0],
            native_status::invalid,
        ),
        (
            [sequence, u64::MAX, destination, 1, 0, 0],
            native_status::bad_copy,
        ),
        (
            [sequence, source, 0x400000, 1, 0, 0],
            native_status::bad_copy,
        ),
    ];
    for (arguments, status) in cases {
        let record = call(caller_read_operation, arguments);
        assert_eq!(
            (record.status, record.value, record.auxiliary),
            (status as u64, 0, 0)
        );
        for byte in &witness {
            // SAFETY: Volatile access observes the retained private destination
            // after the gate; optimizer assumptions cannot erase this witness.
            assert_eq!(unsafe { ptr::read_volatile(byte) }, 0xa5);
        }
    }
}

fn caller_byte(sequence: u64, address: u64) -> Result<u8, read_error> {
    let mut byte = [0; 1];
    let record = call(
        caller_read_operation,
        [sequence, address, byte.as_mut_ptr() as u64, 1, 0, 0],
    );
    if record.status == native_status::ok as u64 {
        assert_eq!((record.value, record.auxiliary), (1, 0));
        Ok(byte[0])
    } else {
        assert_eq!((record.value, record.auxiliary), (0, 0));
        if record.status == native_status::bad_copy as u64 {
            Err(read_error::inaccessible)
        } else {
            Err(read_error::unsupported)
        }
    }
}

fn file_binding() -> source_binding {
    assert_eq!(
        ptr::addr_of!(anomalous_linux_veil_file_source) as u64,
        0x800000 + source_read::binding_offset as u64
    );
    let mut bytes = [0; source_read::binding_size];
    for (index, byte) in bytes.iter_mut().enumerate() {
        // SAFETY: Exact initialized binding retained in this task's RW mapping.
        *byte = unsafe {
            ptr::read_volatile(
                ptr::addr_of!(anomalous_linux_veil_file_source)
                    .cast::<u8>()
                    .add(index),
            )
        };
    }
    source_binding::decode(&bytes).expect("invalid file source binding")
}

fn source_buffer() -> *mut u8 {
    ptr::addr_of_mut!(anomalous_linux_veil_file_source_buffer).cast()
}

fn fill_source_buffer(value: u8) {
    for index in 0..source_read::source_chunk_limit {
        // SAFETY: This task exclusively owns the exact linker-bounded BSS
        // buffer. No Rust reference or source reader survives a native call.
        unsafe { ptr::write_volatile(source_buffer().add(index), value) };
    }
}

fn check_source_buffer(start: usize, value: u8) {
    assert!(start <= source_read::source_chunk_limit);
    for index in start..source_read::source_chunk_limit {
        // SAFETY: Observe the complete retained BSS suffix after each gate.
        assert_eq!(
            unsafe { ptr::read_volatile(source_buffer().add(index)) },
            value
        );
    }
}

fn set_boundary_witness(value: u8) {
    for index in 0..8 {
        // SAFETY: The admitted native RW region covers two complete pages;
        // linker assertions reserve its final eight bytes outside static data.
        unsafe { ptr::write_volatile((source_boundary_witness + index) as *mut u8, value) };
    }
}

fn check_source_canaries(witness: u8) {
    // SAFETY: This admitted RX entry begins with CLD (0xfc), independently
    // inspected by the host. Witness the actual rejected RX destination too.
    assert_eq!(unsafe { ptr::read_volatile(0x400000 as *const u8) }, 0xfc);
    // SAFETY: Existing initialized canary and explicitly reserved RW tail.
    assert_eq!(
        unsafe { ptr::read_volatile(ptr::addr_of!(anomalous_linux_veil_data_canary)) },
        0x454c_465f_4441_5441
    );
    for index in 0..8 {
        assert_eq!(
            unsafe { ptr::read_volatile((source_boundary_witness + index) as *const u8) },
            witness
        );
    }
}

fn source_probe(arguments: [u64; 6], expected: native_status) {
    let record = call(source_read::source_read_operation, arguments);
    assert_eq!(
        (record.status, record.value, record.auxiliary),
        (expected as u64, 0, 0)
    );
    check_source_buffer(0, 0xa5);
    check_source_canaries(0xa5);
}

fn source_probes(sequence: u64, binding: source_binding) {
    assert!(sequence > 1 && binding.length() >= source_read::stream_first_chunk as u64);
    let grant = binding.grant();
    let destination = source_buffer() as u64;
    fill_source_buffer(0xa5);
    set_boundary_witness(0xa5);
    source_probe([0, grant, 0, destination, 1, 0], native_status::invalid);
    source_probe(
        [sequence - 1, grant, 0, destination, 1, 0],
        native_status::invalid,
    );
    source_probe(
        [sequence, grant + 1, 0, destination, 1, 0],
        native_status::denied,
    );
    source_probe(
        [sequence, grant, 0, destination, 0, 0],
        native_status::invalid,
    );
    source_probe(
        [
            sequence,
            grant,
            0,
            destination,
            source_read::source_chunk_limit as u64 + 1,
            0,
        ],
        native_status::invalid,
    );
    source_probe(
        [sequence, grant, u64::MAX, destination, 2, 0],
        native_status::invalid,
    );
    source_probe(
        [sequence, grant, binding.length(), destination, 1, 0],
        native_status::invalid,
    );
    source_probe(
        [sequence, grant, 0, 0x400000, 1, 0],
        native_status::bad_copy,
    );
    source_probe(
        [sequence, grant, 0, source_boundary_witness, 16, 0],
        native_status::bad_copy,
    );
    source_probe(
        [sequence, grant, 0, destination, 1, 1],
        native_status::invalid,
    );
    fill_source_buffer(0);
    set_boundary_witness(0);
    check_source_buffer(0, 0);
    check_source_canaries(0);
}

fn publish_source_report(length: u64, fnv64: u64, chunks: u64) {
    let bytes = source_report::new(length, fnv64, chunks)
        .expect("invalid consumed source report")
        .encode();
    for (index, byte) in bytes.iter().enumerate() {
        // SAFETY: The first dedicated BSS record belongs exclusively to this
        // task. The supervisor observes it only while this task is suspended.
        unsafe {
            ptr::write_volatile(
                ptr::addr_of_mut!(anomalous_linux_veil_file_source_report)
                    .cast::<u8>()
                    .add(index),
                *byte,
            );
        }
    }
}

fn consume_file_source(sequence: u64) {
    let binding = file_binding();
    assert_eq!(
        ptr::addr_of!(anomalous_linux_veil_file_source_report) as u64,
        0x800000 + source_read::report_offset as u64
    );
    assert_eq!(source_buffer() as u64, 0x8000d0);
    source_probes(sequence, binding);
    let mut offset = 0u64;
    let mut hash = source_read::fnv_offset;
    let mut chunks = 0u64;
    while offset < binding.length() {
        let capacity = if offset == 0 {
            source_read::stream_first_chunk
        } else {
            source_read::source_chunk_limit
        };
        let length = (binding.length() - offset).min(capacity as u64) as usize;
        check_source_buffer(0, 0);
        let record = call(
            source_read::source_read_operation,
            [
                sequence,
                binding.grant(),
                offset,
                source_buffer() as u64,
                length as u64,
                0,
            ],
        );
        assert_eq!(
            (record.status, record.value, record.auxiliary),
            (native_status::ok as u64, length as u64, 0)
        );
        for index in 0..length {
            // SAFETY: Successful exact source copy initialized this retained
            // private buffer; hash only bytes actually observed after the gate.
            hash = source_read::hash_byte(hash, unsafe {
                ptr::read_volatile(source_buffer().add(index))
            });
        }
        check_source_buffer(length, 0);
        check_source_canaries(0);
        offset = offset
            .checked_add(length as u64)
            .expect("source offset exhausted");
        chunks = chunks.checked_add(1).expect("source chunks exhausted");
        publish_source_report(offset, hash, chunks);
        fill_source_buffer(0);
    }
    assert_eq!(offset, binding.length());
    assert_eq!(hash, binding.fnv64());
    assert_eq!(
        Some(chunks),
        source_read::stream_chunk_count(binding.length())
    );
    check_source_buffer(0, 0);
    check_source_canaries(0);
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_linux_veil_run(role: u64, reserved: u64) -> ! {
    assert!(matches!(role, 1 | 2) && reserved <= 7);
    let policy = startup(role);
    // SAFETY: This is the inspected initialized canary after the policy prefix.
    assert_eq!(
        unsafe { ptr::read_volatile(ptr::addr_of!(anomalous_linux_veil_data_canary)) },
        0x454c_465f_4441_5441
    );
    if role == 1 {
        call(5, [0; 6]);
        panic!("native peer exit returned");
    }
    let first = request();
    // Only arg0 belongs to brk; unused argument registers are not constrained.
    assert_eq!((first.number(), first.arguments()[0]), (12, 0));
    return_result(first.sequence(), policy.initial_break());
    let second = request();
    assert!(second.sequence() > first.sequence());
    assert_eq!((second.number(), second.arguments()[1]), (21, 4));
    copy_probes(second.sequence(), second.arguments()[0]);
    match loader_policy::evaluate_access(policy, second, |address| {
        caller_byte(second.sequence(), address)
    }) {
        access_outcome::returned(result) => return_result(second.sequence(), result as u64),
        access_outcome::unsupported => retire(second.sequence()),
    }
    let third = request();
    assert!(third.sequence() > second.sequence());
    assert_eq!(third.number(), 257);
    copy_probes(third.sequence(), third.arguments()[1]);
    match loader_policy::evaluate_openat(policy, third, |address| {
        caller_byte(third.sequence(), address)
    }) {
        access_outcome::returned(result) => return_result(third.sequence(), result as u64),
        access_outcome::unsupported => retire(third.sequence()),
    }
    let fourth = request();
    assert!(fourth.sequence() > third.sequence());
    // Each probe and subsequent byte read authenticates this new suspended
    // invocation. No source snapshot can reconstruct its live stack contents.
    copy_probes(fourth.sequence(), fourth.arguments()[1]);
    let mut path = [0; loader_policy::pathname_read_budget];
    let mut copied = 0;
    let length = loader_policy::capture_openat_path(fourth, |address| {
        let byte = caller_byte(fourth.sequence(), address)?;
        path[copied] = byte;
        copied += 1;
        Ok(byte)
    })
    .expect("incomplete live loader pathname capture");
    consume_file_source(fourth.sequence());
    loader_entry_files::run(fourth, &mut path, length - 1, reserved)
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    // SAFETY: An unexpected path reaches the separately attributed failure UD2.
    unsafe { anomalous_linux_veil_failure_instruction() }
}
