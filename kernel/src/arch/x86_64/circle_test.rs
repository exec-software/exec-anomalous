use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicU8, Ordering, compiler_fence};

use super::exceptions::{self, exception_frame, exception_snapshot};
use super::{descriptors, paging};
use crate::console::serial_port;

const page_size: usize = 4096;
const table_page_count: usize = 10;
const private_page_count: usize = 3;
const kernel_directory_count: usize = 4;
const entries_per_table: usize = 512;
const kernel_identity_limit: u64 = 4 * 1024 * 1024 * 1024;
const two_mib: u64 = 2 * 1024 * 1024;

const user_window_address: u64 = 0x0000_4000_0000_0000;
const user_code_address: u64 = user_window_address + 0x1000;
const user_data_address: u64 = user_window_address + 0x3000;
const user_stack_address: u64 = user_window_address + 0x5000;
const user_stack_top: u64 = user_window_address + 0x6000;

const pml4_table: usize = 0;
const kernel_pdpt_table: usize = 1;
const kernel_low_page_table: usize = 2;
const kernel_directory_first_table: usize = 3;
const user_pdpt_table: usize = 7;
const user_directory_table: usize = 8;
const user_page_table: usize = 9;

const present: u64 = 1 << 0;
const writable: u64 = 1 << 1;
const user_accessible: u64 = 1 << 2;
const huge_page: u64 = 1 << 7;
const no_execute: u64 = 1 << 63;
const page_address_mask: u64 = 0x000f_ffff_ffff_f000;

const cr0_task_switched: u64 = 1 << 3;

const page_fault_vector: u64 = 14;
const user_read_protection_error: u64 = 0x5;
const user_instruction_protection_error: u64 = 0x15;
const sanitized_flags: u64 = 0x2;
const resume_flag: u64 = 1 << 16;

const generation_1_marker: u64 = 0x1616_1616_1616_1616;
const generation_2_marker: u64 = 0x2626_2626_2626_2626;
const generation_3_marker: u64 = 0x3636_3636_3636_3636;

const report_generation_1_entered: u64 = 1;
const report_after_supervisor_read: u64 = 2;
const report_generation_2_entered: u64 = 3;
const report_after_nx_fetch: u64 = 4;
const report_generation_3_entered: u64 = 5;
const report_after_terminal: u64 = 6;
const report_dirty_state: u64 = 7;

const phase_idle: u8 = 0;
const phase_generation_1_report: u8 = 1;
const phase_generation_1_denial: u8 = 2;
const phase_generation_2_report: u8 = 3;
const phase_generation_2_denial: u8 = 4;
const phase_generation_3_report: u8 = 5;
const phase_recovering: u8 = 6;

const status_generation_1_denied: u64 = 1;
const status_generation_2_denied: u64 = 2;
const status_generation_3_complete: u64 = 3;

#[repr(C, align(4096))]
struct page_table_storage([u8; table_page_count * page_size]);

#[repr(C, align(4096))]
struct private_page_storage([u8; private_page_count * page_size]);

#[used]
#[unsafe(link_section = ".bss.circle_test.tables")]
static mut circle_tables: page_table_storage =
    page_table_storage([0; table_page_count * page_size]);

#[used]
#[unsafe(link_section = ".bss.circle_test.private_pages")]
static mut circle_private_pages: private_page_storage =
    private_page_storage([0; private_page_count * page_size]);

#[used]
#[unsafe(no_mangle)]
static circle_kernel_canary: u64 = 0x9d31_b783_44f2_a6c5;

static circle_phase: AtomicU8 = AtomicU8::new(phase_idle);
static mut saved_kernel_stack: u64 = 0;
static mut saved_task_switched: u64 = 0;

unsafe extern "C" {
    fn x86_64_enter_circle(
        instruction_pointer: u64,
        stack_pointer: u64,
        data_selector: u64,
        code_selector: u64,
        saved_stack: *mut u64,
    ) -> u64;
    fn x86_64_resume_circle(status: u64, saved_stack: u64, original_task_switched: u64) -> !;

    static x86_64_circle_generation_1_start: u8;
    static x86_64_circle_generation_1_report_return: u8;
    static x86_64_circle_generation_1_denied_read: u8;
    static x86_64_circle_generation_1_end: u8;
    static x86_64_circle_generation_2_start: u8;
    static x86_64_circle_generation_2_report_return: u8;
    static x86_64_circle_generation_2_end: u8;
    static x86_64_circle_stack_probe_start: u8;
    static x86_64_circle_stack_probe_end: u8;
    static x86_64_circle_generation_3_start: u8;
    static x86_64_circle_generation_3_report_return: u8;
    static x86_64_circle_generation_3_end: u8;

    static kernel_text_start: u8;
    static kernel_text_end: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum generation {
    first = 1,
    second = 2,
    third = 3,
}

impl generation {
    const fn number(self) -> u8 {
        self as u8
    }

    const fn marker(self) -> u64 {
        match self {
            Self::first => generation_1_marker,
            Self::second => generation_2_marker,
            Self::third => generation_3_marker,
        }
    }

    const fn report_phase(self) -> u8 {
        match self {
            Self::first => phase_generation_1_report,
            Self::second => phase_generation_2_report,
            Self::third => phase_generation_3_report,
        }
    }

    const fn report_identifier(self) -> u64 {
        match self {
            Self::first => report_generation_1_entered,
            Self::second => report_generation_2_entered,
            Self::third => report_generation_3_entered,
        }
    }

    const fn recovery_status(self) -> u64 {
        match self {
            Self::first => status_generation_1_denied,
            Self::second => status_generation_2_denied,
            Self::third => status_generation_3_complete,
        }
    }
}

#[derive(Clone, Copy)]
struct payload {
    start: *const u8,
    end: *const u8,
    report_return: *const u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct effective_permissions {
    present: bool,
    user: bool,
    writable: bool,
    executable: bool,
}

#[derive(Clone, Copy)]
struct fault_expectation {
    phase: u8,
    error_code: u64,
    instruction_pointer: u64,
    fault_address: u64,
    marker: u64,
}

pub(super) fn run() -> ! {
    let original_cr3 = paging::read_cr3();
    if !private_storage_is_zero() {
        fail("initial_storage_not_zero");
    }
    if !paging::enable_and_verify_protections() {
        fail("paging_protection_unavailable");
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.protections.ready","level":"info","nx":true,"write_protect":true,"private_root":true}}"#
    );

    run_generation(generation::first, original_cr3);
    run_generation(generation::second, original_cr3);
    run_generation(generation::third, original_cr3);

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.passed","level":"info","generations":3,"denials":2}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_generation(current: generation, original_cr3: u64) {
    if circle_phase.load(Ordering::Acquire) != phase_idle {
        fail("generation_started_while_active");
    }
    if !prepare_generation(current) {
        fail("generation_preparation_failed");
    }

    circle_phase.store(current.report_phase(), Ordering::Release);
    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The bootstrap processor is the only writer and no interrupt can
    // enter the feature-local recovery path before the following IRETQ.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(saved_task_switched),
            original_task_switched,
        )
    };
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.generation.mapped","level":"info","generation":{},"scratch_zero":true,"private_root":true,"replacement":{}}}"#,
        current.number(),
        current != generation::first
    );

    // SAFETY: The root was built and walked immediately above. It maps all
    // kernel state supervisor-only and the three fixed user pages exactly.
    unsafe { paging::write_cr3(table_address(pml4_table)) };
    // SAFETY: Descriptor and IDT setup is complete, the TSS owns a dedicated
    // RSP0 stack, and the assembly entry clears state before IRETQ.
    let status = unsafe {
        x86_64_enter_circle(
            user_code_address,
            user_stack_top,
            u64::from(descriptors::user_data_selector),
            u64::from(descriptors::user_code_selector),
            ptr::addr_of_mut!(saved_kernel_stack),
        )
    };

    // The nonlocal recovery path has restored the original kernel stack but
    // deliberately leaves the fixed private CR3 active until this instruction.
    let task_switched_restored = paging::read_cr0() & cr0_task_switched == original_task_switched;
    // SAFETY: This is the exact root captured before the experiment began.
    unsafe { paging::write_cr3(original_cr3) };
    let reached_recovery = circle_phase.load(Ordering::Acquire) == phase_recovering;
    circle_phase.store(phase_idle, Ordering::Release);

    let storage_wiped = wipe_private_storage();
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    // SAFETY: Recovery is now on the original stack and no code can use the
    // abandoned saved pointer again.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(saved_kernel_stack), 0) };
    // SAFETY: Recovery consumed the value and restored the processor state.
    unsafe { ptr::write_volatile(ptr::addr_of_mut!(saved_task_switched), 0) };
    compiler_fence(Ordering::SeqCst);

    if !reached_recovery
        || status != current.recovery_status()
        || !task_switched_restored
        || !storage_wiped
        || !transition_stack_wiped
    {
        fail("generation_teardown_failed");
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.generation.torn_down","level":"info","generation":{},"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}}"#,
        current.number()
    );
}

fn prepare_generation(current: generation) -> bool {
    if !private_storage_is_zero() {
        return false;
    }

    let selected = payload_for_generation(current);
    // SAFETY: The linker-defined template extent is checked against one page
    // before it is copied into the private code backing page.
    if !unsafe { copy_payload(selected.start, selected.end, private_page(0)) } {
        return false;
    }
    if current == generation::second {
        // SAFETY: The fixed stack probe is likewise bounded by one page.
        if !unsafe {
            copy_payload(
                ptr::addr_of!(x86_64_circle_stack_probe_start),
                ptr::addr_of!(x86_64_circle_stack_probe_end),
                private_page(2),
            )
        } {
            return false;
        }
    }

    build_private_page_tables();
    validate_private_page_tables()
        && read_scratch_marker() == 0
        && expected_user_address(selected.report_return, selected.start).is_some()
}

fn payload_for_generation(current: generation) -> payload {
    match current {
        generation::first => payload {
            start: ptr::addr_of!(x86_64_circle_generation_1_start),
            end: ptr::addr_of!(x86_64_circle_generation_1_end),
            report_return: ptr::addr_of!(x86_64_circle_generation_1_report_return),
        },
        generation::second => payload {
            start: ptr::addr_of!(x86_64_circle_generation_2_start),
            end: ptr::addr_of!(x86_64_circle_generation_2_end),
            report_return: ptr::addr_of!(x86_64_circle_generation_2_report_return),
        },
        generation::third => payload {
            start: ptr::addr_of!(x86_64_circle_generation_3_start),
            end: ptr::addr_of!(x86_64_circle_generation_3_end),
            report_return: ptr::addr_of!(x86_64_circle_generation_3_report_return),
        },
    }
}

unsafe fn copy_payload(start: *const u8, end: *const u8, destination: *mut u8) -> bool {
    let Some(length) = (end as usize).checked_sub(start as usize) else {
        return false;
    };
    if length == 0 || length > page_size {
        return false;
    }
    // SAFETY: The checked immutable template and page-aligned private backing
    // do not overlap, and the selected extent fits in the destination page.
    unsafe { ptr::copy_nonoverlapping(start, destination, length) };
    true
}

fn build_private_page_tables() {
    set_entry(
        pml4_table,
        0,
        table_address(kernel_pdpt_table) | present | writable,
    );
    for directory in 0..kernel_directory_count {
        let table = kernel_directory_first_table + directory;
        set_entry(
            kernel_pdpt_table,
            directory,
            table_address(table) | present | writable,
        );
        for entry in 0..entries_per_table {
            let physical = ((directory * entries_per_table + entry) as u64) * two_mib;
            set_entry(
                table,
                entry,
                physical | present | writable | huge_page | no_execute,
            );
        }
    }

    set_entry(
        kernel_directory_first_table,
        0,
        table_address(kernel_low_page_table) | present | writable,
    );
    let text_start = ptr::addr_of!(kernel_text_start) as usize as u64;
    let text_end = ptr::addr_of!(kernel_text_end) as usize as u64;
    let rodata_start = ptr::addr_of!(kernel_rodata_start) as usize as u64;
    let rodata_end = ptr::addr_of!(kernel_rodata_end) as usize as u64;
    for entry in 0..entries_per_table {
        let physical = (entry * page_size) as u64;
        let flags = if (text_start..text_end).contains(&physical) {
            present
        } else if (rodata_start..rodata_end).contains(&physical) {
            present | no_execute
        } else {
            present | writable | no_execute
        };
        set_entry(kernel_low_page_table, entry, physical | flags);
    }

    let user_pml4_index = page_table_index(user_window_address, 39);
    set_entry(
        pml4_table,
        user_pml4_index,
        table_address(user_pdpt_table) | present | writable | user_accessible,
    );
    set_entry(
        user_pdpt_table,
        page_table_index(user_window_address, 30),
        table_address(user_directory_table) | present | writable | user_accessible,
    );
    set_entry(
        user_directory_table,
        page_table_index(user_window_address, 21),
        table_address(user_page_table) | present | writable | user_accessible,
    );
    for entry in 0..entries_per_table {
        if let Some((page, flags)) = user_leaf_mapping(entry) {
            set_entry(user_page_table, entry, private_page_address(page) | flags);
        }
    }
}

const fn user_leaf_mapping(index: usize) -> Option<(usize, u64)> {
    match index {
        1 => Some((0, present | user_accessible)),
        3 => Some((1, present | writable | user_accessible | no_execute)),
        5 => Some((2, present | writable | user_accessible | no_execute)),
        _ => None,
    }
}

fn validate_private_page_tables() -> bool {
    for table in 0..table_page_count {
        if table_address(table) & (page_size as u64 - 1) != 0
            || table_address(table) >= kernel_identity_limit
        {
            return false;
        }
    }
    for page in 0..private_page_count {
        if private_page_address(page) & (page_size as u64 - 1) != 0
            || private_page_address(page) >= kernel_identity_limit
        {
            return false;
        }
    }

    let user_pml4_index = page_table_index(user_window_address, 39);
    for entry in 0..entries_per_table {
        if !matches!(entry, 0) && entry != user_pml4_index && read_entry(pml4_table, entry) != 0 {
            return false;
        }
    }
    let kernel_root = read_entry(pml4_table, 0);
    if kernel_root != table_address(kernel_pdpt_table) | present | writable {
        return false;
    }
    for directory in 0..kernel_directory_count {
        let entry = read_entry(kernel_pdpt_table, directory);
        if entry != table_address(kernel_directory_first_table + directory) | present | writable {
            return false;
        }
    }
    for entry in kernel_directory_count..entries_per_table {
        if read_entry(kernel_pdpt_table, entry) != 0 {
            return false;
        }
    }
    if read_entry(kernel_directory_first_table, 0)
        != table_address(kernel_low_page_table) | present | writable
    {
        return false;
    }
    let canary_address = ptr::addr_of!(circle_kernel_canary) as usize as u64;
    if canary_address >= two_mib {
        return false;
    }
    let canary_leaf = read_entry(kernel_low_page_table, page_table_index(canary_address, 12));
    if entry_address(canary_leaf) != canary_address & page_address_mask
        || canary_leaf & present == 0
        || canary_leaf & user_accessible != 0
    {
        return false;
    }

    let parents = [
        read_entry(pml4_table, page_table_index(user_window_address, 39)),
        read_entry(user_pdpt_table, page_table_index(user_window_address, 30)),
        read_entry(
            user_directory_table,
            page_table_index(user_window_address, 21),
        ),
    ];
    let expected_parent_addresses = [
        table_address(user_pdpt_table),
        table_address(user_directory_table),
        table_address(user_page_table),
    ];
    for (entry, address) in parents.into_iter().zip(expected_parent_addresses) {
        if entry != address | present | writable | user_accessible {
            return false;
        }
    }
    for entry in 1..entries_per_table {
        if read_entry(user_pdpt_table, entry) != 0 || read_entry(user_directory_table, entry) != 0 {
            return false;
        }
    }
    let code_leaf = read_entry(user_page_table, page_table_index(user_code_address, 12));
    let data_leaf = read_entry(user_page_table, page_table_index(user_data_address, 12));
    let stack_leaf = read_entry(user_page_table, page_table_index(user_stack_address, 12));
    if code_leaf != private_page_address(0) | present | user_accessible
        || data_leaf != private_page_address(1) | present | writable | user_accessible | no_execute
        || stack_leaf != private_page_address(2) | present | writable | user_accessible | no_execute
    {
        return false;
    }
    let code = effective_permissions_from_path([parents[0], parents[1], parents[2], code_leaf]);
    let data = effective_permissions_from_path([parents[0], parents[1], parents[2], data_leaf]);
    let stack = effective_permissions_from_path([parents[0], parents[1], parents[2], stack_leaf]);
    if code
        != (effective_permissions {
            present: true,
            user: true,
            writable: false,
            executable: true,
        })
        || data
            != (effective_permissions {
                present: true,
                user: true,
                writable: true,
                executable: false,
            })
        || stack != data
    {
        return false;
    }

    for entry in 0..entries_per_table {
        if !matches!(entry, 1 | 3 | 5) && read_entry(user_page_table, entry) != 0 {
            return false;
        }
    }
    true
}

fn effective_permissions_from_path(path: [u64; 4]) -> effective_permissions {
    effective_permissions {
        present: path.iter().all(|entry| entry & present != 0),
        user: path.iter().all(|entry| entry & user_accessible != 0),
        writable: path.iter().all(|entry| entry & writable != 0),
        executable: path.iter().all(|entry| entry & no_execute == 0),
    }
}

const fn entry_address(entry: u64) -> u64 {
    entry & page_address_mask
}

fn set_entry(table: usize, index: usize, value: u64) {
    debug_assert!(table < table_page_count);
    debug_assert!(index < entries_per_table);
    // SAFETY: Both indexes are bounded above and the storage is exclusively
    // owned by this single-processor test while inactive.
    unsafe { ptr::write(table_pointer(table).add(index), value) };
}

fn read_entry(table: usize, index: usize) -> u64 {
    debug_assert!(table < table_page_count);
    debug_assert!(index < entries_per_table);
    // SAFETY: Both indexes are bounded above and each entry is initialized or
    // retains its loader-provided zero value before being read.
    unsafe { ptr::read(table_pointer(table).add(index)) }
}

fn table_pointer(table: usize) -> *mut u64 {
    let base = ptr::addr_of_mut!(circle_tables).cast::<u8>();
    // SAFETY: Callers bound `table` to the ten page-sized table slots.
    unsafe { base.add(table * page_size).cast::<u64>() }
}

fn table_address(table: usize) -> u64 {
    table_pointer(table) as usize as u64
}

fn private_page(page: usize) -> *mut u8 {
    let base = ptr::addr_of_mut!(circle_private_pages).cast::<u8>();
    // SAFETY: Callers use one of the three fixed backing-page indexes.
    unsafe { base.add(page * page_size) }
}

fn private_page_address(page: usize) -> u64 {
    private_page(page) as usize as u64
}

const fn page_table_index(address: u64, shift: u32) -> usize {
    ((address >> shift) & 0x1ff) as usize
}

fn expected_user_address(symbol: *const u8, template_start: *const u8) -> Option<u64> {
    let offset = (symbol as usize).checked_sub(template_start as usize)?;
    if offset >= page_size {
        return None;
    }
    Some(user_code_address + offset as u64)
}

pub(super) fn matches_expected_fault(snapshot: exception_snapshot) -> bool {
    let phase = circle_phase.load(Ordering::Acquire);
    let expectation = match phase {
        phase_generation_1_denial => fault_expectation {
            phase,
            error_code: user_read_protection_error,
            instruction_pointer: expected_user_address(
                ptr::addr_of!(x86_64_circle_generation_1_denied_read),
                ptr::addr_of!(x86_64_circle_generation_1_start),
            )
            .unwrap_or(0),
            fault_address: ptr::addr_of!(circle_kernel_canary) as usize as u64,
            marker: generation_1_marker,
        },
        phase_generation_2_denial => fault_expectation {
            phase,
            error_code: user_instruction_protection_error,
            instruction_pointer: user_stack_address,
            fault_address: user_stack_address,
            marker: generation_2_marker,
        },
        _ => return false,
    };
    fault_matches(snapshot, phase, read_scratch_marker(), expectation)
}

fn fault_matches(
    snapshot: exception_snapshot,
    phase: u8,
    scratch_marker: u64,
    expectation: fault_expectation,
) -> bool {
    phase == expectation.phase
        && snapshot.vector == page_fault_vector
        && snapshot.error_code == expectation.error_code
        && snapshot.instruction_pointer == expectation.instruction_pointer
        && snapshot.code_segment == u64::from(descriptors::user_code_selector)
        && fault_flags_are_sanitized(snapshot.flags)
        && user_stack_contains(snapshot.stack_pointer)
        && snapshot.stack_segment == Some(u64::from(descriptors::user_data_selector))
        && snapshot.fault_address == Some(expectation.fault_address)
        && scratch_marker == expectation.marker
}

const fn fault_flags_are_sanitized(flags: u64) -> bool {
    flags & !resume_flag == sanitized_flags
}

fn user_stack_contains(stack_pointer: u64) -> bool {
    stack_pointer > user_stack_address && stack_pointer <= user_stack_top
}

pub(super) fn contain_expected_fault(snapshot: exception_snapshot, serial: &mut serial_port) -> ! {
    let (current, kind, status) = match circle_phase.load(Ordering::Acquire) {
        phase_generation_1_denial => (
            generation::first,
            "supervisor_read",
            status_generation_1_denied,
        ),
        phase_generation_2_denial => (generation::second, "nx_stack", status_generation_2_denied),
        _ => fail_with_serial(serial, "fault_phase_changed"),
    };
    if !matches_expected_fault(snapshot) {
        fail_with_serial(serial, "fault_evidence_changed");
    }

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.denial.contained","level":"info","generation":{},"kind":"{kind}","vector":14,"error_code":"0x{:x}","contained":true}}"#,
        current.number(),
        snapshot.error_code
    );
    circle_phase.store(phase_recovering, Ordering::Release);
    exceptions::reset_after_contained_fault();
    // SAFETY: The entry assembly wrote this pointer after saving the original
    // Rust call frame and before transferring to CPL 3.
    let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
    if saved_stack == 0 || saved_stack & 0x7 != 0 {
        fail_with_serial(serial, "saved_stack_invalid");
    }
    // SAFETY: Exact evidence consumed the only recoverable fault phase. This
    // abandons the TSS stack and returns through the saved kernel call frame.
    let original_task_switched = unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
    if !matches!(original_task_switched, 0 | cr0_task_switched) {
        fail_with_serial(serial, "saved_task_switched_invalid");
    }
    unsafe { x86_64_resume_circle(status, saved_stack, original_task_switched) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_circle_report_dispatch(frame: *const exception_frame) {
    // SAFETY: The feature-local assembly entry built the normalized prefix and
    // arrived through a privilege-changing interrupt gate.
    let snapshot = unsafe { exceptions::snapshot_from_frame(frame) };
    let phase = circle_phase.load(Ordering::Acquire);
    let report = snapshot.accumulator;

    if matches!(
        report,
        report_after_supervisor_read | report_after_nx_fetch | report_after_terminal
    ) {
        let mut serial = serial_port::com1();
        serial.initialize();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.circle_test.after_denial","level":"fatal","report":{report}}}"#
        );
        fail_with_serial(&mut serial, "instruction_after_boundary_executed");
    }
    if report == report_dirty_state {
        fail("user_observed_dirty_scratch");
    }

    let current = match phase {
        phase_generation_1_report => generation::first,
        phase_generation_2_report => generation::second,
        phase_generation_3_report => generation::third,
        _ => fail("report_out_of_order"),
    };
    let selected = payload_for_generation(current);
    let Some(return_address) = expected_user_address(selected.report_return, selected.start) else {
        fail("report_address_invalid");
    };
    if report != current.report_identifier()
        || snapshot.vector != exceptions::circle_report_vector as u64
        || snapshot.error_code != 0
        || snapshot.instruction_pointer != return_address
        || snapshot.code_segment != u64::from(descriptors::user_code_selector)
        || snapshot.flags != sanitized_flags
        || !user_stack_contains(snapshot.stack_pointer)
        || snapshot.stack_segment != Some(u64::from(descriptors::user_data_selector))
        || snapshot.fault_address.is_some()
        || read_scratch_marker() != current.marker()
    {
        fail("report_evidence_invalid");
    }

    let next_phase = match current {
        generation::first => phase_generation_1_denial,
        generation::second => phase_generation_2_denial,
        generation::third => phase_recovering,
    };
    if circle_phase
        .compare_exchange(phase, next_phase, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        fail("report_phase_raced");
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.generation.entered","level":"info","generation":{},"origin":"user","scratch_marker":"0x{:016x}"}}"#,
        current.number(),
        current.marker()
    );

    if current == generation::third {
        let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
        if saved_stack == 0 || saved_stack & 0x7 != 0 {
            fail("terminal_saved_stack_invalid");
        }
        // SAFETY: The exact terminal report is the only successful way out of
        // generation 3; its user frame is discarded like the denied frames.
        let original_task_switched =
            unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
        if !matches!(original_task_switched, 0 | cr0_task_switched) {
            fail("terminal_task_switched_invalid");
        }
        unsafe {
            x86_64_resume_circle(
                status_generation_3_complete,
                saved_stack,
                original_task_switched,
            )
        }
    }
}

fn read_scratch_marker() -> u64 {
    // SAFETY: The data backing page exists throughout the fixed test and is
    // naturally aligned for the single volatile u64 observation.
    unsafe { ptr::read_volatile(private_page(1).cast::<u64>()) }
}

fn wipe_private_storage() -> bool {
    volatile_clear(
        ptr::addr_of_mut!(circle_tables).cast::<u8>(),
        table_page_count * page_size,
    );
    volatile_clear(
        ptr::addr_of_mut!(circle_private_pages).cast::<u8>(),
        private_page_count * page_size,
    );
    compiler_fence(Ordering::SeqCst);
    private_storage_is_zero()
}

fn private_storage_is_zero() -> bool {
    volatile_is_zero(
        ptr::addr_of!(circle_tables).cast::<u8>(),
        table_page_count * page_size,
    ) && volatile_is_zero(
        ptr::addr_of!(circle_private_pages).cast::<u8>(),
        private_page_count * page_size,
    )
}

fn volatile_clear(address: *mut u8, length: usize) {
    for offset in 0..length {
        // SAFETY: Callers supply one live, exclusively owned static extent.
        unsafe { ptr::write_volatile(address.add(offset), 0) };
    }
}

fn volatile_is_zero(address: *const u8, length: usize) -> bool {
    for offset in 0..length {
        // SAFETY: Callers supply one live static or test extent.
        if unsafe { ptr::read_volatile(address.add(offset)) } != 0 {
            return false;
        }
    }
    true
}

fn fail(reason: &'static str) -> ! {
    let mut serial = serial_port::com1();
    serial.initialize();
    fail_with_serial(&mut serial, reason)
}

fn fail_with_serial(serial: &mut serial_port, reason: &'static str) -> ! {
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.circle_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_snapshot(expectation: fault_expectation) -> exception_snapshot {
        exception_snapshot {
            accumulator: 0,
            vector: page_fault_vector,
            error_code: expectation.error_code,
            instruction_pointer: expectation.instruction_pointer,
            code_segment: u64::from(descriptors::user_code_selector),
            flags: sanitized_flags,
            stack_pointer: user_stack_top,
            stack_segment: Some(u64::from(descriptors::user_data_selector)),
            fault_address: Some(expectation.fault_address),
        }
    }

    #[test]
    fn fixed_layout_uses_one_isolated_pml4_branch() {
        assert_eq!(page_table_index(user_window_address, 39), 128);
        assert_eq!(page_table_index(user_code_address, 12), 1);
        assert_eq!(page_table_index(user_data_address, 12), 3);
        assert_eq!(page_table_index(user_stack_address, 12), 5);
        assert_eq!(user_leaf_mapping(1), Some((0, present | user_accessible)));
        assert_eq!(user_leaf_mapping(3).map(|mapping| mapping.0), Some(1));
        assert_eq!(user_leaf_mapping(5).map(|mapping| mapping.0), Some(2));
        for guard in [0, 2, 4, 6, 511] {
            assert_eq!(user_leaf_mapping(guard), None);
        }
    }

    #[test]
    fn effective_permissions_require_every_paging_level() {
        let parents = present | writable | user_accessible;
        let code =
            effective_permissions_from_path([parents, parents, parents, present | user_accessible]);
        assert_eq!(
            code,
            effective_permissions {
                present: true,
                user: true,
                writable: false,
                executable: true,
            }
        );

        let supervisor_parent = parents & !user_accessible;
        let denied = effective_permissions_from_path([
            supervisor_parent,
            parents,
            parents,
            present | writable | user_accessible | no_execute,
        ]);
        assert!(!denied.user);
        assert!(!denied.executable);
    }

    #[test]
    fn exact_fault_predicate_rejects_each_mutated_field() {
        let expectation = fault_expectation {
            phase: phase_generation_1_denial,
            error_code: user_read_protection_error,
            instruction_pointer: 0x1234,
            fault_address: 0x5678,
            marker: generation_1_marker,
        };
        let snapshot = expected_snapshot(expectation);
        assert!(fault_matches(
            snapshot,
            expectation.phase,
            expectation.marker,
            expectation
        ));

        let mut resumed_fault = snapshot;
        resumed_fault.flags |= resume_flag;
        assert!(fault_matches(
            resumed_fault,
            expectation.phase,
            expectation.marker,
            expectation
        ));
        resumed_fault.flags |= 1 << 9;
        assert!(!fault_matches(
            resumed_fault,
            expectation.phase,
            expectation.marker,
            expectation
        ));

        let mut mutations = [snapshot; 8];
        mutations[0].vector += 1;
        mutations[1].error_code ^= 1;
        mutations[2].instruction_pointer += 1;
        mutations[3].code_segment ^= 3;
        mutations[4].flags ^= 1 << 9;
        mutations[5].stack_pointer = user_stack_address;
        mutations[6].stack_segment = Some(0x10);
        mutations[7].fault_address = Some(expectation.fault_address + 1);
        for mutation in mutations {
            assert!(!fault_matches(
                mutation,
                expectation.phase,
                expectation.marker,
                expectation
            ));
        }
        assert!(!fault_matches(
            snapshot,
            expectation.phase + 1,
            expectation.marker,
            expectation
        ));
        assert!(!fault_matches(
            snapshot,
            expectation.phase,
            expectation.marker + 1,
            expectation
        ));
    }

    #[test]
    fn nx_stack_fault_requires_present_user_instruction_denial() {
        let expectation = fault_expectation {
            phase: phase_generation_2_denial,
            error_code: user_instruction_protection_error,
            instruction_pointer: user_stack_address,
            fault_address: user_stack_address,
            marker: generation_2_marker,
        };
        let snapshot = expected_snapshot(expectation);

        assert!(fault_matches(
            snapshot,
            expectation.phase,
            expectation.marker,
            expectation
        ));
        let mut not_present = snapshot;
        not_present.error_code = 0x14;
        assert!(!fault_matches(
            not_present,
            expectation.phase,
            expectation.marker,
            expectation
        ));
    }

    #[test]
    fn volatile_clear_is_observable() {
        let mut bytes = [0xa5; 97];
        volatile_clear(bytes.as_mut_ptr(), bytes.len());
        assert!(volatile_is_zero(bytes.as_ptr(), bytes.len()));
    }
}
