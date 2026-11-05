//! Native construction proof. No staged root is ever selected.

use core::fmt::Write;

use super::mapping_plan::{
    mapping_budget, mapping_plan, mapping_request, reserved_request, user_address_limit,
};
use super::memory_test::fail;
use super::owned_mapping::{mapping_memory, staged_mapping};
use super::paging::{self, effective_permissions, page_permissions, page_size, present};
use super::physical_access::{bootstrap_physical_access, proof_window_base};
use crate::console::serial_port;
use crate::memory::physical::{frame_accounting, frame_allocator, physical_range};

const regions: [(u64, u64, bool, bool); 4] = [
    (0x400000, 1, false, true),
    (0x5ff000, 2, false, false),
    (0x800000, 2, true, false),
    (user_address_limit - 4 * page_size, 2, true, false),
];

pub(super) fn run(allocator: &mut frame_allocator<'_>, kernel_cr3: u64) {
    let initial = allocator.accounting();
    require(
        initial.active_leases == 0 && initial.bound_leases == 0,
        "staged_initial_leases",
    );
    let mut access = bootstrap_physical_access::new();
    // SAFETY: The feature runs alone on the BSP with IF clear. Threshold's
    // reserved tables and original root remain live throughout this function.
    require(
        unsafe { access.install() }.is_ok(),
        "physical_window_install",
    );
    require(
        paging::read_cr3() == kernel_cr3 && access.is_current(),
        "physical_window_root",
    );
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.physical_window.ready","level":"info","base":"{proof_window_base:#018x}","length":4294967296,"supervisor":true,"writable":true,"executable":false,"bootstrap_root_unchanged":true}}"#
    );

    let mut stage = staged_mapping::new(plan());
    // SAFETY: The global allocator grants exclusive usable RAM. The retained
    // alias remains live; no staged root is selected or exposed to hardware.
    require(
        unsafe { stage.build(allocator, &mut access) }.is_ok(),
        "staged_build",
    );
    require(
        stage.is_ready() && stage.retained_leases() == 5,
        "staged_ready_owner",
    );
    let owned = allocator.accounting();
    require(
        owned.free + 16 == initial.free
            && owned.data_owned == 7
            && owned.table_owned == 9
            && owned.active_leases == 5
            && owned.bound_leases == 5,
        "staged_owned_accounting",
    );
    // SAFETY: Exact same inactive owner and independently live access contract.
    require(
        unsafe { stage.validate(allocator, &mut access) }.is_ok(),
        "staged_exact_validation",
    );
    inspect(&stage, &mut access);
    require(
        paging::read_cr3() == kernel_cr3,
        "staged_unexpected_activation",
    );
    // SAFETY: The root has never been active; all accesses below are through the
    // independently retained bootstrap alias and leases remain in this owner.
    require(
        unsafe { stage.cleanup(allocator, &mut access) }.is_ok(),
        "staged_cleanup",
    );
    restored(allocator, &stage, initial, kernel_cr3);

    // One table arena and four data allocations. Failed initialization must
    // publish no new lease, and all earlier allocations remain reachable.
    for fail_at in 0..5 {
        let mut stage = staged_mapping::new(plan());
        let mut injected = fault_access::new(&mut access);
        injected.fail_clear = Some(fail_at);
        // SAFETY: Faults can deny or partially clear authorized ranges only;
        // they never falsely claim success or change the active root.
        require(
            unsafe { stage.build(allocator, &mut injected) }.is_err(),
            "staged_allocation_injection",
        );
        require(
            !stage.is_ready() && allocator.accounting().active_leases == stage.retained_leases(),
            "staged_allocation_owner_lost",
        );
        injected.fail_clear = None;
        // SAFETY: Every surviving lease remains in the inactive stage.
        require(
            unsafe { stage.cleanup(allocator, &mut injected) }.is_ok(),
            "staged_allocation_retry",
        );
        restored(allocator, &stage, initial, kernel_cr3);
    }

    let mut stage = staged_mapping::new(plan());
    let mut injected = fault_access::new(&mut access);
    injected.fail_write = Some(3);
    // SAFETY: This injected write failure retains the same exclusive resources.
    require(
        unsafe { stage.build(allocator, &mut injected) }.is_err(),
        "staged_population_injection",
    );
    require(
        !stage.is_ready() && stage.retained_leases() == 5,
        "staged_population_owner_lost",
    );
    injected.fail_write = None;
    // SAFETY: Cleanup relies on retained leases, not the partially written tree.
    require(
        unsafe { stage.cleanup(allocator, &mut injected) }.is_ok(),
        "staged_population_retry",
    );
    restored(allocator, &stage, initial, kernel_cr3);

    // Whole-arena withdrawal, four data releases, then table release. Every
    // clear failure must retain a quarantine owner and permit a later retry.
    for fail_at in 0..6 {
        let mut stage = staged_mapping::new(plan());
        let mut injected = fault_access::new(&mut access);
        // SAFETY: Same authorized inactive construction as the successful case.
        require(
            unsafe { stage.build(allocator, &mut injected) }.is_ok(),
            "staged_retry_setup",
        );
        injected.clears = 0;
        injected.fail_clear = Some(fail_at);
        // SAFETY: A partial clear is allowed to report failure; storage remains
        // owned and this proof never chooses the damaged root.
        require(
            unsafe { stage.cleanup(allocator, &mut injected) }.is_err(),
            "staged_cleanup_injection",
        );
        require(
            !stage.is_ready()
                && stage.retained_leases() > 0
                && allocator.accounting().active_leases == stage.retained_leases(),
            "staged_quarantine_owner_lost",
        );
        injected.fail_clear = None;
        // SAFETY: Retry uses retained handles and the still-installed alias.
        require(
            unsafe { stage.cleanup(allocator, &mut injected) }.is_ok(),
            "staged_cleanup_retry",
        );
        restored(allocator, &stage, initial, kernel_cr3);
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.staged.verified","level":"info","regions":4,"data_frames":7,"table_frames":9,"low_code":"0x0000000000400000","exact_walk":true,"guards_absent":true,"distinct_ownership":true,"inactive":true,"allocation_failures":5,"population_failures":1,"cleanup_failures":6,"retry_restored":true}}"#
    );

    // SAFETY: Every staged owner above was fully cleaned. No physical pointer
    // escaped, and no callback survives this final alias withdrawal.
    require(unsafe { access.remove() }.is_ok(), "physical_window_remove");
    require(
        !access.is_current()
            && paging::read_cr3() == kernel_cr3
            && allocator.accounting() == initial,
        "staged_final_accounting",
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.staged.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"window_removed":true,"kernel_cr3_restored":true}}"#,
        initial.free, initial.free
    );
}

fn plan() -> mapping_plan {
    let requests =
        regions.map(
            |(virtual_address, page_count, writable, executable)| mapping_request {
                virtual_address,
                page_count,
                permissions: page_permissions {
                    user: true,
                    writable,
                    executable,
                },
            },
        );
    let exclusions = [
        reserved_request {
            virtual_address: 0,
            page_count: 1,
        },
        reserved_request {
            virtual_address: 0x100000,
            page_count: 128,
        },
        reserved_request {
            virtual_address: user_address_limit - 5 * page_size,
            page_count: 1,
        },
        reserved_request {
            virtual_address: user_address_limit - 2 * page_size,
            page_count: 1,
        },
    ];
    mapping_plan::new(
        &requests,
        &exclusions,
        mapping_budget {
            data_pages: 7,
            table_frames: 9,
        },
    )
    .unwrap_or_else(|_| fail("staged_plan"))
}

fn inspect(stage: &staged_mapping, access: &mut bootstrap_physical_access) {
    let root = stage
        .root_address()
        .unwrap_or_else(|| fail("staged_root_missing"));
    let arena = physical_range::new(root, 9 * page_size);
    for (index, &(virtual_address, pages, writable, executable)) in regions.iter().enumerate() {
        let data = stage
            .data_range(index)
            .unwrap_or_else(|| fail("staged_data_missing"));
        require(
            data.length == pages * page_size && data.base != virtual_address,
            "staged_data_shape",
        );
        require(!overlap(data, arena), "staged_data_table_alias");
        for other in 0..index {
            let previous = stage
                .data_range(other)
                .unwrap_or_else(|| fail("staged_prior_data"));
            require(!overlap(data, previous), "staged_data_alias");
        }
        for page in 0..pages {
            for offset in [0, page_size - 8] {
                let expected_physical = data.base + page * page_size + offset;
                let (physical, permissions) =
                    walk(access, arena, virtual_address + page * page_size + offset)
                        .unwrap_or_else(|| fail("staged_walk_missing"));
                require(
                    physical == expected_physical
                        && permissions
                            == effective_permissions {
                                present: true,
                                user: true,
                                writable,
                                executable,
                            },
                    "staged_walk_permissions",
                );
                // SAFETY: These words are in the retained exclusive data lease;
                // all reads/writes use the independently live high alias.
                unsafe {
                    require(
                        access.read_word(physical) == Some(0),
                        "staged_data_not_zero",
                    );
                    let marker = 0x5a00_0000_0000_0000 | physical;
                    require(access.write_word(physical, marker), "staged_marker_write");
                    require(
                        access.read_word(physical) == Some(marker),
                        "staged_marker_read",
                    );
                }
            }
        }
    }
    for address in [
        0,
        0x100000,
        0x401000,
        0x5fe000,
        0x601000,
        0x802000,
        user_address_limit - 5 * page_size,
        user_address_limit - 2 * page_size,
        proof_window_base,
    ] {
        require(
            walk(access, arena, address).is_none(),
            "staged_guard_present",
        );
    }
}

/// Independent hardware-style four-level walk. Each intermediate address is
/// checked against the retained private arena before any physical read.
fn walk(
    access: &mut bootstrap_physical_access,
    arena: physical_range,
    address: u64,
) -> Option<(u64, effective_permissions)> {
    let indexes = paging::indexes(address);
    let mut table = arena.base;
    let mut path = [0; 4];
    for (level, index) in [
        indexes.pml4,
        indexes.directory_pointer,
        indexes.directory,
        indexes.table,
    ]
    .into_iter()
    .enumerate()
    {
        require(
            table.is_multiple_of(page_size)
                && table >= arena.base
                && table + page_size <= arena.base + arena.length,
            "staged_walk_outside_owner",
        );
        // SAFETY: The retained owner owns the entire checked table arena.
        let entry = unsafe { access.read_entry(table, index) }
            .unwrap_or_else(|| fail("staged_walk_access"));
        if entry & present == 0 {
            return None;
        }
        path[level] = entry;
        table = paging::entry_address(entry);
    }
    Some((
        table + address % page_size,
        paging::effective_permissions_from_path(&path),
    ))
}

fn overlap(a: physical_range, b: physical_range) -> bool {
    a.base < b.base + b.length && b.base < a.base + a.length
}

fn restored(
    allocator: &frame_allocator<'_>,
    stage: &staged_mapping,
    initial: frame_accounting,
    root: u64,
) {
    require(
        allocator.accounting() == initial
            && stage.retained_leases() == 0
            && !stage.is_ready()
            && stage.root_address().is_none()
            && paging::read_cr3() == root,
        "staged_restoration",
    );
}

fn require(condition: bool, reason: &'static str) {
    if !condition {
        fail(reason);
    }
}

struct fault_access<'a> {
    access: &'a mut bootstrap_physical_access,
    clears: usize,
    writes: usize,
    fail_clear: Option<usize>,
    fail_write: Option<usize>,
}
impl<'a> fault_access<'a> {
    fn new(access: &'a mut bootstrap_physical_access) -> Self {
        Self {
            access,
            clears: 0,
            writes: 0,
            fail_clear: None,
            fail_write: None,
        }
    }
}
// SAFETY: Faults only deny operations or partially clear an authorized extent;
// every successful operation delegates to the validated live alias.
unsafe impl mapping_memory for fault_access<'_> {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        let ordinal = self.clears;
        self.clears += 1;
        if self.fail_clear == Some(ordinal) {
            // SAFETY: This word belongs to the allocator-authorized clear range.
            let _ = unsafe { self.access.write_word(range.base, 0) };
            return false;
        }
        // SAFETY: Forward exactly the caller's exclusive clear authority.
        unsafe { self.access.clear_and_verify(range) }
    }
    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        // SAFETY: Forward the caller's retained-table authority.
        unsafe { self.access.read_entry(table, index) }
    }
    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
        let ordinal = self.writes;
        self.writes += 1;
        if self.fail_write == Some(ordinal) {
            return false;
        }
        // SAFETY: Forward the caller's retained-table authority.
        unsafe { self.access.write_entry(table, index, value) }
    }
    fn active_root(&self) -> u64 {
        self.access.active_root()
    }
}
