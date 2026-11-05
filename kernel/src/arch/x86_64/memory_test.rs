use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use super::address_space::{address_space, address_space_error, mapping_evidence};
use super::paging::{self, page_permissions};
use crate::console::serial_port;
use crate::memory;
use crate::memory::physical::{
    frame_allocator, frame_error, frame_lease, frame_purpose, physical_range,
};

const test_virtual_address: u64 = 0x0000_5000_0000_0000;
const marker_a: u64 = 0xa11c_a11c_a11c_a11c;
const marker_b: u64 = 0xb22d_b22d_b22d_b22d;

pub(super) fn run() -> ! {
    let kernel_cr3 = paging::read_cr3();
    let result =
        match memory::with_frame_allocator(|allocator| run_with_allocator(allocator, kernel_cr3)) {
            Ok(result) => result,
            Err(error) => Err(error.reason()),
        };

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact live kernel root captured before the proof.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    if paging::read_cr3() != kernel_cr3 {
        fail("kernel_root_restore_failed");
    }
    if let Err(reason) = result {
        fail(reason);
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.passed","level":"info","roots":2,"switches":3,"reused_frames":2}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    let initial = allocator.accounting();
    if initial.free == 0
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("initial_accounting_invalid");
    }

    let data_a = allocate_data(allocator, "data_a_allocation_failed")?;
    let data_b = allocate_data(allocator, "data_b_allocation_failed")?;
    let data_a_range = data_a.range().ok_or("data_a_extent_missing")?;
    let data_b_range = data_b.range().ok_or("data_b_extent_missing")?;
    if data_a_range == data_b_range || !range_is_zero(data_a_range) || !range_is_zero(data_b_range)
    {
        return Err("data_lease_evidence_invalid");
    }
    allocator
        .bind(&data_a)
        .map_err(|_| "duplicate_binding_setup_failed")?;
    if allocator.bind(&data_a) != Err(frame_error::lease_already_bound) {
        return Err("duplicate_binding_not_rejected");
    }
    allocator
        .unbind(&data_a)
        .map_err(|_| "duplicate_binding_cleanup_failed")?;

    // SAFETY: The allocator exclusively grants every callback extent, and the
    // identity-mapped clearer consumes the whole range synchronously.
    let mut space_a = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "root_a_creation_failed")?;
    // SAFETY: The same allocator/clearer contract holds for B's private tables.
    let mut space_b = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "root_b_creation_failed")?;
    let root_a = space_a
        .root_address()
        .map_err(|_| "root_a_extent_missing")?;
    let root_b = space_b
        .root_address()
        .map_err(|_| "root_b_extent_missing")?;
    let owned = allocator.accounting();
    if root_a == root_b
        || owned.data_owned != 2
        || owned.table_owned != 8
        || owned.active_leases != 4
        || owned.bound_leases != 2
    {
        return Err("lease_accounting_invalid");
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.leases.ready","level":"info","data_a":"{:#018x}","data_b":"{:#018x}","root_a":"{:#018x}","root_b":"{:#018x}","data_frames":2,"table_frames":8,"zeroed":true,"distinct":true}}"#,
        data_a_range.base, data_b_range.base, root_a, root_b
    );

    space_a
        .map_owned(
            allocator,
            test_virtual_address,
            data_a,
            page_permissions::supervisor_read_write,
        )
        .map_err(|_| "mapping_a_failed")?;
    space_b
        .map_owned(
            allocator,
            test_virtual_address,
            data_b,
            page_permissions::supervisor_read_write,
        )
        .map_err(|_| "mapping_b_failed")?;
    let evidence_a = space_a
        .validate_mapping(test_virtual_address)
        .map_err(|_| "mapping_a_walk_failed")?;
    let evidence_b = space_b
        .validate_mapping(test_virtual_address)
        .map_err(|_| "mapping_b_walk_failed")?;
    if !valid_mapping_evidence(evidence_a, data_a_range.base)
        || !valid_mapping_evidence(evidence_b, data_b_range.base)
        || evidence_a.physical_address == evidence_b.physical_address
        || allocator.accounting().bound_leases != 4
    {
        return Err("mapping_evidence_invalid");
    }
    let overlap_probe = allocate_data(allocator, "overlap_probe_allocation_failed")?;
    let mut overlap_probe = match space_a.map_owned(
        allocator,
        test_virtual_address,
        overlap_probe,
        page_permissions::supervisor_read_write,
    ) {
        Err((address_space_error::mapping_exists, lease)) => lease,
        _ => return Err("virtual_overlap_not_rejected"),
    };
    // SAFETY: The rejected mapping returned the still-unbound exclusive lease.
    unsafe { allocator.release_with(&mut overlap_probe, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "overlap_probe_release_failed")?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.roots.ready","level":"info","virtual_address":"{:#018x}","writable":true,"executable":false,"supervisor":true,"exact_walk":true,"duplicate_binding_rejected":true,"overlap_rejected":true}}"#,
        test_virtual_address
    );

    switch_and_verify(allocator, &mut space_a, &space_b, kernel_cr3)?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.switch.verified","level":"info","sequence":"a_b_a","distinct":true,"marker_a":"{:#018x}","marker_b":"{:#018x}","active_root_rejected":true}}"#,
        marker_a, marker_b
    );

    let mut data_a = space_a
        .unmap_owned(allocator, test_virtual_address)
        .map_err(|_| "mapping_a_withdrawal_failed")?;
    let mut data_b = space_b
        .unmap_owned(allocator, test_virtual_address)
        .map_err(|_| "mapping_b_withdrawal_failed")?;
    // SAFETY: Both roots are inactive, empty, and exclusively allocator-owned.
    unsafe { space_a.destroy(allocator, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "root_a_destruction_failed")?;
    // SAFETY: B meets the same inactive, empty, exclusive teardown contract.
    unsafe { space_b.destroy(allocator, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "root_b_destruction_failed")?;
    if paging::read_cr3() != kernel_cr3
        || allocator.accounting().table_owned != 0
        || allocator.accounting().bound_leases != 0
    {
        return Err("root_withdrawal_evidence_invalid");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.roots.withdrawn","level":"info","kernel_cr3_restored":true,"mappings_removed":true,"roots_inactive":true}}"#
    );

    // SAFETY: Withdrawal returned both exclusive, unbound data leases.
    unsafe { allocator.release_with(&mut data_a, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "data_a_release_failed")?;
    // SAFETY: Withdrawal returned B's exclusive, unbound data lease.
    unsafe { allocator.release_with(&mut data_b, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "data_b_release_failed")?;
    let mut reused_a = allocate_data(allocator, "data_a_reuse_failed")?;
    let mut reused_b = allocate_data(allocator, "data_b_reuse_failed")?;
    let reused_a_range = reused_a.range().ok_or("reused_data_a_extent_missing")?;
    let reused_b_range = reused_b.range().ok_or("reused_data_b_extent_missing")?;
    if reused_a_range != data_a_range
        || reused_b_range != data_b_range
        || !range_is_zero(reused_a_range)
        || !range_is_zero(reused_b_range)
    {
        return Err("clean_reuse_evidence_invalid");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.reuse.verified","level":"info","data_a":"{:#018x}","data_b":"{:#018x}","zeroed":true,"same_frames":true}}"#,
        reused_a_range.base, reused_b_range.base
    );

    // SAFETY: The proof never mapped either reused exclusive data lease.
    unsafe { allocator.release_with(&mut reused_a, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "reused_data_a_release_failed")?;
    // SAFETY: The same exclusive release contract holds for reused B.
    unsafe { allocator.release_with(&mut reused_b, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "reused_data_b_release_failed")?;
    let final_accounting = allocator.accounting();
    if final_accounting != initial {
        return Err("final_accounting_mismatch");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0}}"#,
        final_accounting.free, initial.free
    );
    super::owned_mapping_test::run(allocator, kernel_cr3);
    super::active_mapping_test::run(allocator, kernel_cr3);
    super::user_mapping_test::run(allocator, kernel_cr3);
    Ok(())
}

fn allocate_data<'allocator>(
    allocator: &mut frame_allocator<'allocator>,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator is the sole authority for the callback range and
    // the identity-mapped clearer synchronously clears and verifies all of it.
    unsafe {
        allocator.allocate_with(1, 1, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

fn valid_mapping_evidence(evidence: mapping_evidence, physical_address: u64) -> bool {
    evidence.virtual_address == test_virtual_address
        && evidence.physical_address == physical_address
        && evidence.permissions.present
        && !evidence.permissions.user
        && evidence.permissions.writable
        && !evidence.permissions.executable
}

fn switch_and_verify(
    allocator: &mut frame_allocator<'_>,
    space_a: &mut address_space,
    space_b: &address_space,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    // SAFETY: Both spaces have exact validated low kernel branches and one
    // owned high mapping. The original root remains live until restoration.
    unsafe { space_a.activate() }.map_err(|_| "root_a_activation_failed")?;
    // SAFETY: The callback meets the release contract, though the active-root
    // guard must reject this attempt before invoking it.
    if !matches!(
        unsafe { space_a.destroy(allocator, |range| { memory::clear_and_verify(range) }) },
        Err(address_space_error::active_root)
    ) {
        return Err("active_root_destruction_not_rejected");
    }
    // SAFETY: The selected root maps this aligned test address RW+NX to A.
    unsafe { ptr::write_volatile(test_virtual_address as *mut u64, marker_a) };
    compiler_fence(Ordering::SeqCst);
    // SAFETY: The same validated A mapping remains active.
    if unsafe { ptr::read_volatile(test_virtual_address as *const u64) } != marker_a {
        return Err("marker_a_write_failed");
    }

    // SAFETY: B borrows the same live low branch and maps the test VA to its
    // distinct owned data frame.
    unsafe { space_b.activate() }.map_err(|_| "root_b_activation_failed")?;
    // SAFETY: The selected B mapping is present and readable.
    if unsafe { ptr::read_volatile(test_virtual_address as *const u64) } != 0 {
        return Err("mapping_b_not_zero");
    }
    // SAFETY: The selected root maps the test address RW+NX to B.
    unsafe { ptr::write_volatile(test_virtual_address as *mut u64, marker_b) };
    compiler_fence(Ordering::SeqCst);
    // SAFETY: The same validated B mapping remains active.
    if unsafe { ptr::read_volatile(test_virtual_address as *const u64) } != marker_b {
        return Err("marker_b_write_failed");
    }

    // SAFETY: A remains owned and unchanged while B was active.
    unsafe { space_a.activate() }.map_err(|_| "root_a_reactivation_failed")?;
    // SAFETY: The selected A mapping is present and readable.
    if unsafe { ptr::read_volatile(test_virtual_address as *const u64) } != marker_a {
        return Err("marker_a_not_preserved");
    }
    // SAFETY: Restore the exact root captured before the experiment.
    unsafe { paging::write_cr3(kernel_cr3) };
    if paging::read_cr3() != kernel_cr3 {
        return Err("kernel_root_restore_failed");
    }
    Ok(())
}

fn range_is_zero(range: physical_range) -> bool {
    let Some(end) = range.end() else {
        return false;
    };
    if range.length == 0 || end > memory::address_limit {
        return false;
    }
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    let Ok(length) = usize::try_from(range.length) else {
        return false;
    };
    let word_count = length / core::mem::size_of::<u64>();
    // SAFETY: The allocator owns the checked identity-mapped extent while this
    // proof holds its live lease.
    unsafe {
        let words = base as *const u64;
        for index in 0..word_count {
            if ptr::read_volatile(words.add(index)) != 0 {
                return false;
            }
        }
    }
    true
}

pub(super) fn fail(reason: &str) -> ! {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
