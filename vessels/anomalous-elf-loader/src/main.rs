#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Unprivileged adapter for one bounded immutable ELF deployment image.
//! Parsing creates only an inert plan. The kernel independently admits its
//! source, geometry, ownership and lifetime before creating application tasks.

use anomalous_elf_adapter::{encoded_plan_size, load_plan, parse};
use core::{
    arch::{asm, global_asm},
    hint::black_box,
    panic::PanicInfo,
    ptr,
};

global_asm!(include_str!("entry.S"), options(att_syntax));

const deployment: &[u8] = include_bytes!(env!("ANOMALOUS_ELF_DEPLOYMENT"));

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
    fn anomalous_elf_loader_call(record: *mut call_record);
}

fn call(operation: u64, pointer: u64, length: usize) -> call_record {
    let mut record = call_record {
        operation,
        channel: 0,
        pointer,
        length: length as u64,
        sequence: 0,
        status: u64::MAX,
        returned_sequence: u64::MAX,
        returned_length: u64::MAX,
    };
    // SAFETY: The assembly stub preserves the SysV callee-saved registers and
    // writes only this live record's result fields. User packet pointers remain
    // opaque here; the kernel validates them against the current owned mapping.
    unsafe { anomalous_elf_loader_call(&mut record) };
    record
}

fn expect_offer(pointer: u64, length: usize, status: u64) {
    let reply = call(9, pointer, length);
    assert!(reply.status == status);
    assert!(reply.returned_sequence == 0 && reply.returned_length == 0);
}

fn reject_mutation(encoded: &[u8; encoded_plan_size], offset: usize, value: u64) {
    let mut changed = *encoded;
    changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    expect_offer(changed.as_ptr() as u64, changed.len(), 4);
}

fn loader(plan: load_plan, image: &[u8]) {
    // Each call observes a distinct shortened immutable slice, including an
    // otherwise complete header and a missing final file-backed segment byte.
    for length in [0, 15, 63, 175, image.len() - 1] {
        assert!(parse(black_box(&image[..length])).is_err());
    }
    let encoded = plan.encode();
    assert!(load_plan::decode(&encoded) == Ok(plan));

    expect_offer(0, encoded.len(), 3);
    expect_offer(encoded.as_ptr() as u64, encoded.len() - 1, 4);
    let mut bad_version = encoded;
    bad_version[4..6].copy_from_slice(&2u16.to_le_bytes());
    expect_offer(bad_version.as_ptr() as u64, bad_version.len(), 4);
    reject_mutation(&encoded, 8, plan.entry() + 1);
    reject_mutation(&encoded, 16, plan.rx().file_offset() + 1);
    reject_mutation(&encoded, 80, plan.image_length() + 1);
    reject_mutation(&encoded, 40, u64::MAX);

    // Geometry alone grants no authority: this different but decodable request
    // must still fail the kernel's exact immutable-source manifest binding.
    let memory_size = if plan.rw().memory_size() == 8192 {
        plan.rw().memory_size() - 1
    } else {
        plan.rw().memory_size() + 1
    };
    assert!(memory_size >= plan.rw().file_size() && memory_size <= 8192);
    let mut unauthorized = encoded;
    unauthorized[72..80].copy_from_slice(&memory_size.to_le_bytes());
    let Ok(altered) = load_plan::decode(&unauthorized) else {
        panic!("unauthorized probe is not a valid plan");
    };
    assert!(altered != plan);
    expect_offer(unauthorized.as_ptr() as u64, unauthorized.len(), 4);

    // All failures above leave admission available for this exact valid retry.
    expect_offer(encoded.as_ptr() as u64, encoded.len(), 0);
    expect_offer(encoded.as_ptr() as u64, encoded.len(), 4);
}

#[unsafe(no_mangle)]
extern "C" fn anomalous_elf_loader_run() -> ! {
    // SAFETY: The retained loader owner reserves these boot arguments outside
    // Rust objects, initializes them before publication, and never rewrites
    // them while either independently mapped task runs.
    let role = unsafe { ptr::read_volatile(0x800000 as *const u64) };
    let scenario = unsafe { ptr::read_volatile(0x800008 as *const u64) };
    assert!(role <= 1 && scenario == 0);

    // Preserve runtime parsing rather than specialize header checks to the
    // compiler-known embedded bytes. The slice remains immutable RX storage.
    let image = black_box(deployment);
    let Ok(plan) = parse(image) else {
        panic!("deployment ELF rejected");
    };
    if role == 0 {
        loader(plan, image);
    } else {
        let encoded = plan.encode();
        // A valid plan grants nothing to a task without the loader role.
        expect_offer(encoded.as_ptr() as u64, encoded.len(), 2);
    }
    call(5, 0, 0);
    panic!("terminal loader exit returned");
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // No failure is expected in this loader session; any UD2 fails its proof.
    unsafe { asm!("ud2", options(noreturn, nomem, nostack)) }
}
