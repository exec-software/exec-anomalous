//! Bounded CPL0 activation proof; not a process loader or permanent kernel map.

use core::{arch::asm, fmt::Write, ptr};

use super::mapping_plan::{
    mapping_budget, mapping_plan, mapping_request, reserved_request, user_address_limit,
};
use super::memory_test::fail;
use super::owned_mapping::{mapping_memory, staged_mapping};
use super::paging::{self, effective_permissions, page_permissions, page_size, present};
use super::physical_access::bootstrap_physical_access;
use super::resident_plan::{resident_plan, resident_request};
use super::resident_probe::{self, probe_kind};
use crate::console::serial_port;
use crate::memory::physical::{frame_allocator, frame_purpose, physical_range};

const regions: [(u64, u64, bool, bool); 4] = [
    (0x400000, 1, false, true),
    (0x5ff000, 2, false, false),
    (0x800000, 2, true, false),
    (user_address_limit - 4 * page_size, 2, true, false),
];
const collision_address: u64 = 0x400000;
const collision_marker: u64 = 0xc011_1510_aa55_55aa;

unsafe extern "C" {
    static kernel_image_start: u8;
    static kernel_text_start: u8;
    static kernel_text_end: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
    static kernel_data_start: u8;
    static kernel_data_end: u8;
    static kernel_bss_start: u8;
    static kernel_memory_end: u8;
}

// Linker section markers can share an address. Read their numeric addresses
// through assembly so Rust cannot treat distinct nonzero-sized extern statics
// as disjoint objects and fold an adjacency comparison to false.
macro_rules! linker_address {
    ($symbol:ident) => {{
        let address: u64;
        // SAFETY: LEA only materializes the linked marker; it dereferences no RAM.
        unsafe {
            asm!("lea {address}, [rip + {symbol}]", address = out(reg) address,
                symbol = sym $symbol, options(nomem, nostack, preserves_flags));
        }
        address
    }};
}

pub(super) fn run(allocator: &mut frame_allocator<'_>, kernel_cr3: u64) {
    let initial = allocator.accounting();
    require(
        initial.active_leases == 0 && initial.bound_leases == 0,
        "resident_initial_leases",
    );
    let mut access = bootstrap_physical_access::new();
    // SAFETY: This proof is alone on the BSP, before IRQ setup, with IF clear
    // and NMI masked. Threshold's reserved tables survive every owned root.
    require(
        unsafe { access.install() }.is_ok(),
        "resident_alias_install",
    );
    require(
        access.bootstrap_root() == kernel_cr3,
        "resident_bootstrap_root",
    );
    let stack = access
        .bootstrap_stack()
        .unwrap_or_else(|| fail("resident_stack_geometry"));
    let mut first = make_stage(&access, stack);
    let mut second = make_stage(&access, stack);

    // SAFETY: The retained alias independently covers allocator-authorized RAM.
    // This unbound collision lease stays owned until bootstrap is restored.
    let mut collision = unsafe {
        allocator.allocate_with(1, 1024, frame_purpose::data, |range| {
            access.clear_and_verify(range)
        })
    }
    .unwrap_or_else(|_| fail("resident_collision_allocate"));
    require(
        collision.range() == Some(physical_range::new(collision_address, page_size)),
        "resident_collision_extent",
    );
    // SAFETY: The collision lease owns this exact physical word.
    require(
        unsafe { access.write_word(collision_address, collision_marker) },
        "resident_collision_write",
    );
    for stage in [&mut first, &mut second] {
        // SAFETY: All mappings are inactive and owned; residency/alias backing
        // is borrowed from reserved boot storage for this whole function.
        require(
            unsafe { stage.build(allocator, &mut access) }.is_ok(),
            "resident_build",
        );
        inspect(stage, &mut access, stack);
    }
    for (ordinal, stage) in [&first, &second].into_iter().enumerate() {
        initialize(stage, &access, ordinal as u64);
    }
    let owned = allocator.accounting();
    require(
        owned.free + 35 == initial.free
            && owned.data_owned == 15
            && owned.table_owned == 20
            && owned.active_leases == 11
            && owned.bound_leases == 10,
        "resident_owned_accounting",
    );
    require(
        first.root_address() != second.root_address(),
        "resident_same_root",
    );
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.resident.ready","level":"info","roots":2,"user_regions":4,"data_frames":14,"table_frames":20,"probe_frames":1,"active_leases":11,"bound_leases":10,"kernel_permissions_checked":true,"descriptor_residency_checked":true,"supervisor_stack_checked":true}}"#
    );

    {
        let mut session = root_session::new([&mut first, &mut second], &mut access, stack);
        // SAFETY: make_stage/inspect checked live code, section, descriptor and
        // full stack residency. No asynchronous mutation or hidden consumer is
        // possible; the session retains both owners and the physical aperture.
        require(
            unsafe { session.begin(allocator) }.is_ok(),
            "resident_begin",
        );
        for ordinal in 0..2 {
            // SAFETY: All code, stack and exception paths remain resident.
            require(
                unsafe { session.select(ordinal) }.is_ok(),
                "resident_select",
            );
            exercise(
                session.stages[ordinal],
                session.access,
                ordinal as u64,
                false,
            );
            for (kind, target) in [
                (probe_kind::read_only_write, regions[1].0),
                (probe_kind::guard_read, user_address_limit - 5 * page_size),
                (probe_kind::no_execute, regions[2].0),
            ] {
                // SAFETY: A single exact fault is armed within the retained
                // resident stack/root and resumes only its fixed assembly stub.
                require(
                    unsafe {
                        resident_probe::run(
                            kind,
                            target,
                            session.roots[ordinal],
                            stack.base,
                            stack.base + stack.length,
                        )
                    },
                    "resident_permission_fault",
                );
            }
        }
        // SAFETY: Same admitted session; revisit A to prove B did not alter it.
        require(unsafe { session.select(0) }.is_ok(), "resident_reselect");
        exercise(session.stages[0], session.access, 0, true);
        // SAFETY: Restores bootstrap and flushes before detaching any borrower.
        require(unsafe { session.finish() }.is_ok(), "resident_finish");
        require(session.switches == 4, "resident_switch_count");
    }
    require(
        paging::read_cr3() == kernel_cr3 && access.is_current(),
        "resident_restore_root",
    );
    for stage in [&mut first, &mut second] {
        // SAFETY: Both roots are detached, with all translations flushed. Only
        // architecturally permitted accessed/dirty changes may differ now.
        require(
            unsafe { stage.validate(allocator, &mut access) }.is_ok(),
            "resident_after_hardware_validation",
        );
        inspect(stage, &mut access, stack);
    }
    // SAFETY: The independently retained collision lease remains live.
    require(
        unsafe { access.read_word(collision_address) } == Some(collision_marker),
        "resident_physical_collision_changed",
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.resident.verified","level":"info","switches":4,"low_code":"0x0000000000400000","physical_collision":"0x0000000000400000","isolated_values":true,"physical_alias_preserved":true,"read_only_faults":2,"guard_faults":2,"no_execute_faults":2,"exact_fault_resume":true,"accessed_dirty_validated":true,"bootstrap_restored":true}}"#
    );
    for stage in [&mut first, &mut second] {
        // SAFETY: The session ended after restoring/flushing and unregistering
        // both roots. No hardware or software consumer can use these mappings.
        require(
            unsafe { stage.cleanup(allocator, &mut access) }.is_ok(),
            "resident_cleanup",
        );
        require(
            stage.retained_leases() == 0,
            "resident_retained_after_cleanup",
        );
    }
    // SAFETY: This lease is unbound and no low virtual alias ever addressed its
    // physical storage. Its only high-alias consumer ended above.
    require(
        unsafe { allocator.release_with(&mut collision, |range| access.clear_and_verify(range)) }
            .is_ok(),
        "resident_collision_release",
    );
    // SAFETY: Every owned dependent is detached and completely reclaimed.
    require(unsafe { access.remove() }.is_ok(), "resident_alias_remove");
    require(
        allocator.accounting() == initial
            && paging::read_cr3() == kernel_cr3
            && !access.is_current(),
        "resident_final_accounting",
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.resident.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"window_removed":true,"kernel_cr3_restored":true}}"#,
        initial.free, initial.free
    );
}

fn make_stage(access: &bootstrap_physical_access, stack: physical_range) -> staged_mapping {
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
    let reserved = [
        reserved_request {
            virtual_address: 0,
            page_count: 512,
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
    let plan = mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages: 7,
            table_frames: 9,
        },
    )
    .unwrap_or_else(|_| fail("resident_user_plan"));
    make_resident_stage(plan, access, stack)
}

pub(super) fn make_resident_stage(
    plan: mapping_plan,
    access: &bootstrap_physical_access,
    stack: physical_range,
) -> staged_mapping {
    let text = (
        linker_address!(kernel_text_start),
        linker_address!(kernel_text_end),
    );
    let rodata = (
        linker_address!(kernel_rodata_start),
        linker_address!(kernel_rodata_end),
    );
    let data = (
        linker_address!(kernel_data_start),
        linker_address!(kernel_memory_end),
    );
    require(
        linker_address!(kernel_image_start) == 0x100000
            && text.0 == 0x100000
            && text.1 == rodata.0
            && rodata.1 == data.0
            && data.1 <= 0x200000
            && linker_address!(kernel_data_end) <= linker_address!(kernel_bss_start)
            && linker_address!(kernel_bss_start) <= data.1,
        "resident_linker_geometry",
    );
    require(
        stack.base + stack.length <= text.0 && stack_is_resident(stack),
        "resident_live_stack",
    );
    let instruction = run as *const () as u64;
    require(
        instruction >= text.0 && instruction < text.1,
        "resident_live_instruction",
    );
    // SAFETY: Descriptor initialization completed before this feature runs;
    // IF is clear and no other CPU can change its tables or stack selectors.
    require(
        unsafe { super::descriptors::resident_state_is_valid(data.0, data.1) },
        "resident_descriptor_state",
    );
    let resident = [
        (text.0, text.1, false, true),
        (rodata.0, rodata.1, false, false),
        (data.0, data.1, true, false),
        (stack.base, stack.base + stack.length, true, false),
    ]
    .map(|(start, end, writable, executable)| {
        require(
            end > start && start.is_multiple_of(page_size) && end.is_multiple_of(page_size),
            "resident_section_alignment",
        );
        resident_request {
            virtual_address: start,
            page_count: (end - start) / page_size,
            permissions: page_permissions {
                user: false,
                writable,
                executable,
            },
        }
    });
    let residency = resident_plan::new(
        &plan,
        &resident,
        access
            .alias_entry()
            .unwrap_or_else(|| fail("resident_alias_missing")),
        plan.tables().len() + 1,
    )
    .unwrap_or_else(|_| fail("resident_plan"));
    staged_mapping::new_resident(plan, residency).unwrap_or_else(|_| fail("resident_owner"))
}

fn stack_is_resident(stack: physical_range) -> bool {
    let rsp: u64;
    // SAFETY: Observe this BSP's current stack without modifying it.
    unsafe {
        asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    }
    rsp >= stack.base + 16 * 1024 && rsp < stack.base + stack.length
}

/// A separate, non-copyable transition owner. Failed methods retain every
/// registration and both mapping owners; there is no fallible automatic Drop.
#[must_use = "restore bootstrap and finish before releasing the retained mappings"]
pub(super) struct root_session<'a> {
    pub(super) stages: [&'a mut staged_mapping; 2],
    pub(super) access: &'a mut bootstrap_physical_access,
    pub(super) roots: [u64; 2],
    registered: [bool; 2],
    begun: bool,
    stack: physical_range,
    pub(super) switches: usize,
}

impl<'a> root_session<'a> {
    pub(super) fn new(
        stages: [&'a mut staged_mapping; 2],
        access: &'a mut bootstrap_physical_access,
        stack: physical_range,
    ) -> Self {
        Self {
            stages,
            access,
            roots: [0; 2],
            registered: [false; 2],
            begun: false,
            stack,
            switches: 0,
        }
    }

    /// Caller proves resident paths, stacks, descriptors and borrowed lifetimes.
    pub(super) unsafe fn begin(&mut self, allocator: &frame_allocator<'_>) -> Result<(), ()> {
        if self.begun
            || self.registered != [false; 2]
            || paging::read_cr3() != self.access.bootstrap_root()
        {
            return Err(());
        }
        for index in 0..2 {
            // SAFETY: The caller supplies residency; owners are borrowed for
            // the entire session and no target is selected during admission.
            self.roots[index] =
                unsafe { self.stages[index].prepare_activation(allocator, self.access) }
                    .map_err(|_| ())?;
        }
        for index in 0..2 {
            // SAFETY: The completely validated owned roots keep the exact
            // borrowed aperture until finish; each success is recorded at once.
            unsafe { self.access.register_root(self.roots[index]) }.map_err(|_| ())?;
            self.registered[index] = true;
        }
        self.begun = true;
        Ok(())
    }

    /// The caller must keep every resident code/stack/descriptor path unchanged.
    pub(super) unsafe fn select(&mut self, index: usize) -> Result<(), ()> {
        if index >= 2
            || !self.begun
            || !self.registered[index]
            || !self.access.is_current()
            || !stack_is_resident(self.stack)
            || self.stages[index].root_address() != Some(self.roots[index])
        {
            return Err(());
        }
        // SAFETY: begin validated both complete trees, and mutable owner borrows
        // exclude cleanup. Reserved resident mappings cover this return path.
        unsafe {
            paging::write_cr3(self.roots[index]);
        }
        self.switches += 1;
        if !self.access.is_current() || paging::read_cr3() != self.roots[index] {
            return Err(());
        }
        Ok(())
    }

    /// No CPU except this BSP or hidden consumer may retain these translations.
    pub(super) unsafe fn finish(&mut self) -> Result<(), ()> {
        if !self.access.is_current() {
            return Err(());
        }
        if paging::read_cr3() != self.access.bootstrap_root() {
            // SAFETY: The preserved bootstrap root covers all resident paths;
            // no PCID/global translations exist, so this also flushes them.
            unsafe {
                paging::write_cr3(self.access.bootstrap_root());
            }
            self.switches += 1;
        }
        if !self.access.is_current() || paging::read_cr3() != self.access.bootstrap_root() {
            return Err(());
        }
        for index in 0..2 {
            if self.registered[index] {
                // SAFETY: The root is detached and this session cannot select
                // again once the registrations are removed.
                unsafe { self.access.unregister_root(self.roots[index]) }.map_err(|_| ())?;
                self.registered[index] = false;
            }
        }
        self.begun = false;
        Ok(())
    }
}

fn marker(root: u64, region: usize, page: u64, offset: u64, modified: bool) -> u64 {
    // Every NX target starts with RET: missing NX faults return safely and fail
    // the single-use probe instead of interpreting arbitrary marker bytes.
    0x5a00_0000_0000_00c3
        | (root << 48)
        | ((region as u64) << 40)
        | (page << 24)
        | (offset << 8)
        | (u64::from(modified) << 56)
}

fn initialize(stage: &staged_mapping, access: &bootstrap_physical_access, root: u64) {
    for (region, &(_, pages, _, _)) in regions.iter().enumerate() {
        let data = stage
            .data_range(region)
            .unwrap_or_else(|| fail("resident_data_missing"));
        for page in 0..pages {
            for offset in [0, page_size - 8] {
                // SAFETY: Each word is in a retained private data lease, and
                // roots are inactive while initial contents are installed.
                require(
                    unsafe {
                        access.write_word(
                            data.base + page * page_size + offset,
                            marker(root, region, page, offset, false),
                        )
                    },
                    "resident_initialize",
                );
            }
        }
    }
}

fn exercise(stage: &staged_mapping, access: &bootstrap_physical_access, root: u64, revisit: bool) {
    require(
        stage.root_address() == Some(paging::read_cr3()),
        "resident_exercise_root",
    );
    for (region, &(address, pages, writable, _)) in regions.iter().enumerate() {
        let data = stage
            .data_range(region)
            .unwrap_or_else(|| fail("resident_exercise_data"));
        for page in 0..pages {
            for offset in [0, page_size - 8] {
                let pointer = (address + page * page_size + offset) as *mut u64;
                // SAFETY: The active, admitted stage maps this entire word to
                // its retained data lease; only writable leaves are mutated.
                unsafe {
                    require(
                        ptr::read_volatile(pointer)
                            == marker(root, region, page, offset, revisit && writable),
                        "resident_virtual_marker",
                    );
                    if writable {
                        ptr::write_volatile(pointer, marker(root, region, page, offset, true));
                    }
                    require(
                        access.read_word(data.base + page * page_size + offset)
                            == Some(marker(root, region, page, offset, writable)),
                        "resident_virtual_physical_disagree",
                    );
                }
            }
        }
    }
    // SAFETY: The collision lease is independently retained by run(). In this
    // root low VA 4 MiB contains code data from a different physical frame.
    require(
        unsafe { access.read_word(collision_address) } == Some(collision_marker),
        "resident_alias_collision",
    );
}

fn inspect(stage: &staged_mapping, access: &mut bootstrap_physical_access, stack: physical_range) {
    let arena = stage
        .table_range()
        .unwrap_or_else(|| fail("resident_arena_missing"));
    require(arena.length == 10 * page_size, "resident_arena_size");
    let residency = stage
        .residency()
        .unwrap_or_else(|| fail("resident_profile_missing"));
    for page in 0..512 {
        let address = page * page_size;
        let expected = residency
            .regions()
            .find(|region| address >= region.start() && address < region.end());
        let actual = walk(access, arena, address);
        if let Some(region) = expected {
            let permissions = region.permissions();
            require(
                actual
                    == Some((
                        address,
                        effective_permissions {
                            present: true,
                            user: false,
                            writable: permissions.writable,
                            executable: permissions.executable,
                        },
                    )),
                "resident_kernel_permissions",
            );
        } else {
            require(actual.is_none(), "resident_island_gap");
        }
    }
    require(stack_is_resident(stack), "resident_inspect_stack");
    for (index, &(address, pages, writable, executable)) in regions.iter().enumerate() {
        let data = stage
            .data_range(index)
            .unwrap_or_else(|| fail("resident_inspect_data"));
        require(
            data.base != address && data.length == pages * page_size,
            "resident_data_identity_alias",
        );
        for page in 0..pages {
            for offset in [0, page_size - 8] {
                require(
                    walk(access, arena, address + page * page_size + offset)
                        == Some((
                            data.base + page * page_size + offset,
                            effective_permissions {
                                present: true,
                                user: true,
                                writable,
                                executable,
                            },
                        )),
                    "resident_user_permissions",
                );
            }
        }
    }
    for guard in [
        0x401000,
        0x5fe000,
        0x601000,
        0x802000,
        user_address_limit - 5 * page_size,
        user_address_limit - 2 * page_size,
    ] {
        require(
            walk(access, arena, guard).is_none(),
            "resident_guard_present",
        );
    }
}

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
            "resident_walk_outside_owner",
        );
        // SAFETY: Each next table is bounds checked within the retained arena.
        let entry = unsafe { access.read_entry(table, index) }
            .unwrap_or_else(|| fail("resident_walk_access"));
        if entry & present == 0 {
            return None;
        }
        require(entry & paging::huge_page == 0, "resident_walk_large_leaf");
        path[level] = entry;
        table = paging::entry_address(entry);
    }
    Some((
        table + address % page_size,
        paging::effective_permissions_from_path(&path),
    ))
}

fn require(condition: bool, reason: &'static str) {
    if !condition {
        fail(reason);
    }
}
