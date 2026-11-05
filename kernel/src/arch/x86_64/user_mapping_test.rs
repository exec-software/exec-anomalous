//! Two admitted, unscheduled ring-3 tasks in retained low-address roots.
//! UD2 checkpoints are a private fixture protocol, not an application ABI.

use core::sync::atomic::{AtomicPtr, Ordering};
use core::{arch::asm, fmt::Write, ptr};

use super::active_mapping_test::{make_resident_stage, root_session};
use super::descriptors;
use super::exceptions::{exception_frame, exception_snapshot};
use super::mapping_plan::{
    mapping_budget, mapping_plan, mapping_request, reserved_request, user_address_limit,
};
use super::memory_test::fail;
use super::owned_mapping::{mapping_memory, staged_mapping};
use super::paging::{self, page_permissions, page_size};
use super::physical_access::bootstrap_physical_access;
use super::prepared_launch::prepared_launch;
use super::user_entry_plan::user_entry_plan;
use super::user_fault_gate::{self, user_action, user_expectation, user_observation, user_phase};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::{fixture_principal_id, task_identity};
use crate::memory::physical::{frame_allocator, physical_range};
use crate::task::{synthetic_origin_metadata, task_state, task_table};

const code_base: u64 = 0x400000;
const data_base: u64 = 0x800000;
const stack_base: u64 = user_address_limit - 4 * page_size;
const stack_top: u64 = user_address_limit - 2 * page_size;
const task_switched: u64 = 8;

unsafe extern "C" {
    fn x86_64_owned_user_enter(
        entry: u64,
        stack: u64,
        saved_stack: *mut u64,
        saved_ts: *mut u64,
    ) -> u64;
    fn x86_64_owned_user_resume(status: u64, saved_stack: u64, saved_ts: u64) -> !;
    static owned_user_payload_start: u8;
    static owned_user_payload_end: u8;
    static owned_user_report_one: u8;
    static owned_user_resume_one: u8;
    static owned_user_report_two: u8;
    static owned_user_resume_two: u8;
    static owned_user_exit: u8;
    static owned_user_denied_read: u8;
}

macro_rules! symbol_address {
    ($symbol:ident) => {{
        let address: u64;
        // SAFETY: Materialize the numeric linked marker, never its fictitious
        // Rust object. Adjacent assembler/linker markers may share an address.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) address,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        address
    }};
}

static active_context: AtomicPtr<user_context> = AtomicPtr::new(ptr::null_mut());

struct user_context {
    tasks: task_table,
    active: Option<task_identity>,
    expected: user_expectation,
    phase: Option<user_phase>,
    kernel_stack: physical_range,
    saved_stack: u64,
    saved_ts: u64,
    original_ts: u64,
    completion: u64,
    reports: usize,
}

pub(super) fn run(allocator: &mut frame_allocator<'_>, kernel_cr3: u64) {
    let initial = allocator.accounting();
    require(
        initial.active_leases == 0
            && initial.bound_leases == 0
            && active_context.load(Ordering::Acquire).is_null(),
        "user_initial_state",
    );
    require(tls_bases_are_zero(), "user_tls_profile");
    let mut access = bootstrap_physical_access::new();
    // SAFETY: The BSP still owns the reserved bootstrap tree, with IF clear
    // and NMI masked. Every dependent owner is retained until removal below.
    require(unsafe { access.install() }.is_ok(), "user_alias_install");
    let kernel_stack = access
        .bootstrap_stack()
        .unwrap_or_else(|| fail("user_kernel_stack"));
    let source = symbol_address!(owned_user_payload_start);
    let length = symbol_address!(owned_user_payload_end)
        .checked_sub(source)
        .filter(|length| *length > 0 && *length <= page_size)
        .unwrap_or_else(|| fail("user_payload_length"));
    let mut circles = circle_table::new();
    let empty_circles = circles.accounting();
    let mut tasks = task_table::new();
    prove_cancellation(
        allocator,
        &mut access,
        &mut circles,
        &mut tasks,
        kernel_stack,
        source,
        length,
    );
    let mut first = launch(&access, kernel_stack, length);
    let mut second = launch(&access, kernel_stack, length);
    for (index, owner) in [&mut first, &mut second].into_iter().enumerate() {
        // SAFETY: Every private lease and the independent high alias remain
        // retained. Successful population copies and verifies the whole payload.
        require(
            unsafe {
                owner.prepare(allocator, &mut access, &mut circles, |mapping, access| {
                    populate(mapping, access, source, length, index as u64)
                })
            }
            .is_ok(),
            "user_prepare",
        );
        require(
            owner.is_prepared() && owner.task().is_none() && tasks.active_count() == 0,
            "user_preparation_published_task",
        );
    }
    let admissions = [first.admission(), second.admission()];
    let (privilege_start, privilege_end) = descriptors::bootstrap_privilege_stack_range();
    // SAFETY: Resident admission verified the initialized private TSS and all
    // selected stacks. This proof never replaces the RSP0 selection.
    require(
        unsafe { descriptors::selected_privilege_stack_top() } == privilege_end
            && descriptors::wipe_privilege_stack(),
        "user_privilege_stack",
    );
    let identities =
        [&first, &second].map(|owner| owner.circle().unwrap_or_else(|| fail("user_circle")));
    let expected = expectation(
        admissions[0],
        first
            .mapping()
            .root_address()
            .unwrap_or_else(|| fail("user_first_root")),
        0,
        privilege_start,
        privilege_end,
        source,
    );
    let mut context = user_context {
        tasks,
        active: None,
        expected,
        phase: None,
        kernel_stack,
        saved_stack: 0,
        saved_ts: 0,
        original_ts: 0,
        completion: 0,
        reports: 0,
    };
    let tasks = [&mut first, &mut second].map(|owner| {
        owner
            .commit(
                &mut context.tasks,
                &mut circles,
                fixture_principal_id::new(if owner.circle() == Some(identities[0]) {
                    1
                } else {
                    2
                })
                .unwrap_or_else(|| fail("user_fixture_identity")),
                synthetic_origin_metadata::machine_role_fixture,
            )
            .unwrap_or_else(|_| fail("user_task_commit"))
    });
    let owned = allocator.accounting();
    require(
        owned.free + 28 == initial.free
            && owned.data_owned == 10
            && owned.table_owned == 18
            && owned.active_leases == 8
            && owned.bound_leases == 8
            && context.tasks.active_count() == 2
            && circles.accounting().mappings == 6,
        "user_accounting",
    );
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.user.ready","level":"info","roots":2,"tasks":2,"circles":2,"regions_per_root":3,"data_frames":10,"table_frames":18,"active_leases":8,"bound_leases":8,"code_base":"0x0000000000400000","guarded_stack":true,"entry_admitted":true,"privilege_stack_resident":true,"gpr_only":true}}"#
    );
    {
        // SAFETY: Committed owners remain live and cannot be cancelled while
        // their mappings are borrowed by the retained activation session.
        let stages = unsafe {
            [
                first
                    .committed_mapping_mut()
                    .unwrap_or_else(|_| fail("user_first_commit")),
                second
                    .committed_mapping_mut()
                    .unwrap_or_else(|_| fail("user_second_commit")),
            ]
        };
        let mut session = root_session::new(stages, &mut access, kernel_stack);
        // SAFETY: Both resident profiles and every descriptor/stack were checked;
        // no owner or bootstrap alias can be withdrawn during this borrow.
        require(
            unsafe { session.begin(allocator) }.is_ok(),
            "user_session_begin",
        );
        // Fault B first; A must retain its initial data and subsequently exit.
        for index in [1, 0] {
            inspect_data(session.stages[index], session.access, index as u64, 0);
            context.expected = expectation(
                admissions[index],
                session.roots[index],
                index as u64,
                privilege_start,
                privilege_end,
                source,
            );
            // SAFETY: The admitted roots keep the current Rust stack and the
            // separate privilege-transition stack resident for every exception.
            require(unsafe { session.select(index) }.is_ok(), "user_root_select");
            execute(&mut context, tasks[index]);
            let expected_state = if index == 1 {
                task_state::faulted
            } else {
                task_state::exited
            };
            require(
                context
                    .tasks
                    .snapshot(tasks[index])
                    .is_ok_and(|task| task.state() == expected_state),
                "user_terminal_state",
            );
            inspect_data(session.stages[index], session.access, index as u64, 2);
            // SAFETY: This returns on the saved bootstrap stack, after the user
            // frame was abandoned; no transition can race the wipe or TSS read.
            require(
                descriptors::wipe_privilege_stack()
                    && unsafe { descriptors::selected_privilege_stack_top() } == privilege_end,
                "user_transition_stack_cleanup",
            );
        }
        // SAFETY: All tasks are terminal; no frame/continuation can select them.
        require(
            unsafe { session.finish() }.is_ok() && session.switches == 3,
            "user_session_finish",
        );
    }
    require(
        context.reports == 4
            && context.phase.is_none()
            && context.active.is_none()
            && context.saved_stack == 0
            && context.saved_ts == 0
            && active_context.load(Ordering::Acquire).is_null()
            && paging::read_cr3() == kernel_cr3
            && tls_bases_are_zero(),
        "user_continuation_restoration",
    );
    for (index, owner) in [&first, &second].into_iter().enumerate() {
        inspect_data(owner.mapping(), &access, index as u64, 2);
    }
    for owner in [&mut first, &mut second] {
        // SAFETY: Roots are detached and flushed; only hardware A/D changes are
        // admitted by the exact retained owner's post-execution verification.
        require(
            unsafe {
                owner
                    .committed_mapping_mut()
                    .unwrap_or_else(|_| fail("user_committed_validation"))
                    .validate(allocator, &mut access)
            }
            .is_ok(),
            "user_hardware_validation",
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.user.verified","level":"info","entries":2,"resumed_reports":4,"root_switches":3,"faulted_tasks":1,"exited_tasks":1,"supervisor_read_denied":true,"peer_continued":true,"private_data_isolated":true,"noninitial_stack_return":true,"bootstrap_restored":true,"ts_restored":true,"tss_restored":true,"continuations_cleared":true}}"#
    );
    for (index, owner) in [&mut first, &mut second].into_iter().enumerate() {
        // SAFETY: Bootstrap restoration and session withdrawal completed; task
        // records remain terminal and retain their circle attachments meanwhile.
        require(
            unsafe { owner.cancel(allocator, &mut access, &mut context.tasks, &mut circles) }
                .is_ok(),
            "user_owner_cleanup",
        );
        require(
            owner.is_released()
                && context.tasks.validate(tasks[index]).is_err()
                && circles.validate(identities[index]).is_err(),
            "user_retired_identity_accepted",
        );
    }
    require(
        context.tasks.active_count() == 0
            && context.tasks.attached_context_count() == 0
            && circles.accounting() == empty_circles
            && allocator.accounting() == initial,
        "user_final_accounting",
    );
    // SAFETY: Both task owners are completely reclaimed and all borrowers gone.
    require(
        unsafe { access.remove() }.is_ok() && paging::read_cr3() == kernel_cr3,
        "user_alias_remove",
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.user.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"active_tasks":0,"active_circles":0,"attached_contexts":0,"generations_retired":true,"window_removed":true,"kernel_cr3_restored":true}}"#,
        initial.free, initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.memory_test.launch.verified","level":"info","allocation_failures":4,"table_write_failures":1,"payload_failures":1,"circle_failures":1,"prepared_cancellations":1,"unstarted_cancellations":5,"cleanup_failures":5,"cleanup_retries":5,"successful_commits":2,"prepared_without_tasks":true,"cancelled_before_cleanup":true,"commit_before_entry":true,"retry_restored":true}}"#
    );
}

fn plan() -> mapping_plan {
    let requests = [
        (code_base, 1, false, true),
        (data_base, 2, true, false),
        (stack_base, 2, true, false),
    ]
    .map(
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
    let reserved = [(0, 512), (stack_base - page_size, 1), (stack_top, 1)].map(
        |(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        },
    );
    mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages: 5,
            table_frames: 8,
        },
    )
    .unwrap_or_else(|_| fail("user_mapping_plan"))
}

fn marker(mode: u64) -> u64 {
    0x517a_0000_0000_0100 | mode
}

fn launch(
    access: &bootstrap_physical_access,
    stack: physical_range,
    length: u64,
) -> prepared_launch {
    let mapping = make_resident_stage(plan(), access, stack);
    let admission = user_entry_plan::new(mapping.plan(), 0, length, code_base, 2, stack_top)
        .unwrap_or_else(|_| fail("launch_admission"));
    prepared_launch::new(mapping, admission)
}

fn prove_cancellation(
    allocator: &mut frame_allocator<'_>,
    access: &mut bootstrap_physical_access,
    circles: &mut circle_table,
    tasks: &mut task_table,
    stack: physical_range,
    source: u64,
    length: u64,
) {
    let initial = allocator.accounting();
    let empty_circles = circles.accounting();
    let root = paging::read_cr3();
    let principal = fixture_principal_id::new(1).unwrap_or_else(|| fail("launch_principal"));
    // Four allocation clears, one table write, one partial payload, circle
    // exhaustion, prepared cancellation, then every cleanup clear after commit.
    for case in 0..13 {
        let mut owner = launch(access, stack, length);
        require(
            owner
                .commit(
                    tasks,
                    circles,
                    principal,
                    synthetic_origin_metadata::machine_role_fixture,
                )
                .is_err(),
            "launch_early_commit",
        );
        let full_circles: [_; 16] = core::array::from_fn(|_| {
            if case == 6 {
                Some(
                    circles
                        .create()
                        .unwrap_or_else(|_| fail("launch_fill_circles")),
                )
            } else {
                None
            }
        });
        let mut injected = launch_fault_access::new(access);
        if case < 4 {
            injected.fail_clear = Some(case);
        }
        if case == 4 {
            injected.fail_write = Some(3);
        }
        // SAFETY: Every fault denies an authorized operation; successful access
        // uses the retained alias. A false population result grants no commit.
        let prepared = unsafe {
            owner.prepare(allocator, &mut injected, circles, |mapping, memory| {
                if case == 5 {
                    let data = mapping
                        .data_range(1)
                        .unwrap_or_else(|| fail("launch_partial_data"));
                    let _ = memory.access.write_word(data.base, 0xa11c);
                    false
                } else {
                    populate(mapping, memory.access, source, length, 0)
                }
            })
        };
        require(
            prepared.is_ok() == (case >= 7) && tasks.active_count() == 0 && owner.task().is_none(),
            "launch_prepare_fault",
        );
        let mut cancelled_task = None;
        if case < 7 {
            require(
                owner
                    .commit(
                        tasks,
                        circles,
                        principal,
                        synthetic_origin_metadata::machine_role_fixture,
                    )
                    .is_err(),
                "launch_failed_commit",
            );
        } else if case >= 8 {
            let task = owner
                .commit(
                    tasks,
                    circles,
                    principal,
                    synthetic_origin_metadata::machine_role_fixture,
                )
                .unwrap_or_else(|_| fail("launch_cancel_commit"));
            require(
                tasks
                    .snapshot(task)
                    .is_ok_and(|task| task.state() == task_state::ready)
                    && tasks.attached_context_count() == 0,
                "launch_cancel_ready",
            );
            require(
                owner
                    .commit(
                        tasks,
                        circles,
                        principal,
                        synthetic_origin_metadata::machine_role_fixture,
                    )
                    .is_err(),
                "launch_duplicate_commit",
            );
            cancelled_task = Some(task);
        }
        injected.fail_write = None;
        injected.fail_clear = None;
        if case >= 8 {
            injected.clears = 0;
            injected.fail_clear = Some(case - 8);
            // SAFETY: No root has been registered or selected and no task has
            // started. Failed cleanup must retain all unreleased leases.
            require(
                unsafe { owner.cancel(allocator, &mut injected, tasks, circles) }.is_err(),
                "launch_cleanup_injection",
            );
            let task = cancelled_task.unwrap_or_else(|| fail("launch_cancel_identity"));
            require(
                tasks
                    .snapshot(task)
                    .is_ok_and(|task| task.state() == task_state::exited)
                    && tasks.start(task).is_err()
                    && !owner.is_committed()
                    && owner.mapping().retained_leases() > 0
                    && allocator.accounting().active_leases == owner.mapping().retained_leases(),
                "launch_cancel_ownership",
            );
            injected.fail_clear = None;
        }
        // SAFETY: Same inactive retained owner; earlier failures kept ownership.
        require(
            unsafe { owner.cancel(allocator, &mut injected, tasks, circles) }.is_ok(),
            "launch_cancel_retry",
        );
        for circle in full_circles.into_iter().flatten() {
            require(
                circles.destroy(circle).is_ok(),
                "launch_empty_circle_destroy",
            );
        }
        require(
            owner.is_released()
                && owner.mapping().retained_leases() == 0
                && allocator.accounting() == initial
                && circles.accounting() == empty_circles
                && tasks.active_count() == 0
                && tasks.attached_context_count() == 0
                && cancelled_task.is_none_or(|task| tasks.validate(task).is_err())
                && paging::read_cr3() == root,
            "launch_cancel_restoration",
        );
    }
}

fn populate(
    owner: &staged_mapping,
    access: &bootstrap_physical_access,
    source: u64,
    length: u64,
    mode: u64,
) -> bool {
    let Some(code) = owner.data_range(0) else {
        return false;
    };
    let Some(data) = owner.data_range(1) else {
        return false;
    };
    for offset in (0..length.div_ceil(8) * 8).step_by(8) {
        let mut bytes = [0; 8];
        for (index, byte) in bytes.iter_mut().enumerate() {
            if offset + index as u64 >= length {
                break;
            }
            // SAFETY: The linked, resident read-only template covers this byte.
            *byte = unsafe { ptr::read((source + offset + index as u64) as *const u8) };
        }
        let word = u64::from_le_bytes(bytes);
        // SAFETY: The checked code lease owns this aligned whole word. Its root
        // is inactive; zero padding keeps the final partial word initialized.
        if !unsafe {
            access.write_word(code.base + offset, word)
                && access.read_word(code.base + offset) == Some(word)
        } {
            return false;
        }
    }
    // SAFETY: Both words belong to the retained writable data lease.
    unsafe {
        access.write_word(data.base, mode)
            && access.write_word(data.base + 8, marker(mode))
            && access.read_word(data.base) == Some(mode)
            && access.read_word(data.base + 8) == Some(marker(mode))
            && access.read_word(data.base + 16) == Some(0)
            && access.read_word(data.base + data.length - 8) == Some(0)
    }
}

struct launch_fault_access<'a> {
    access: &'a mut bootstrap_physical_access,
    clears: usize,
    writes: usize,
    fail_clear: Option<usize>,
    fail_write: Option<usize>,
}

impl<'a> launch_fault_access<'a> {
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

// SAFETY: Faults only deny operations or partially clear authorized storage.
// Successful operations delegate to the independently live privileged alias.
unsafe impl mapping_memory for launch_fault_access<'_> {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        let ordinal = self.clears;
        self.clears += 1;
        if self.fail_clear == Some(ordinal) {
            // SAFETY: The caller exclusively owns the complete clear range.
            let _ = unsafe { self.access.write_word(range.base, 0) };
            return false;
        }
        // SAFETY: Forward the caller's exclusive clearing authority.
        unsafe { self.access.clear_and_verify(range) }
    }
    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        // SAFETY: Forward the caller's retained-table authority and bounds.
        unsafe { self.access.read_entry(table, index) }
    }
    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
        let ordinal = self.writes;
        self.writes += 1;
        if self.fail_write == Some(ordinal) {
            return false;
        }
        // SAFETY: Forward the caller's exclusive inactive-table authority.
        unsafe { self.access.write_entry(table, index, value) }
    }
    fn active_root(&self) -> u64 {
        self.access.active_root()
    }
}

fn inspect_data(
    owner: &staged_mapping,
    access: &bootstrap_physical_access,
    mode: u64,
    progress: u64,
) {
    let data = owner
        .data_range(1)
        .unwrap_or_else(|| fail("user_inspect_data"));
    // SAFETY: Retained ownership grants read access through the checked alias.
    require(
        unsafe {
            access.read_word(data.base) == Some(mode)
                && access.read_word(data.base + 8) == Some(marker(mode))
                && access.read_word(data.base + 16) == Some(progress)
                && access.read_word(data.base + data.length - 8) == Some(0)
        },
        "user_private_data",
    );
}

fn expectation(
    user: user_entry_plan,
    root: u64,
    mode: u64,
    privilege_start: u64,
    privilege_end: u64,
    source: u64,
) -> user_expectation {
    let translate = |symbol: u64| {
        code_base
            .checked_add(
                symbol
                    .checked_sub(source)
                    .unwrap_or_else(|| fail("user_symbol_before_payload")),
            )
            .filter(|address| user.bounds().contains_code(*address))
            .unwrap_or_else(|| fail("user_symbol_outside_payload"))
    };
    user_expectation {
        phase: user_phase::first_report,
        user,
        root,
        mode,
        marker: marker(mode),
        privilege_start,
        privilege_end,
        report_one: translate(symbol_address!(owned_user_report_one)),
        resume_one: translate(symbol_address!(owned_user_resume_one)),
        report_two: translate(symbol_address!(owned_user_report_two)),
        resume_two: translate(symbol_address!(owned_user_resume_two)),
        exit: translate(symbol_address!(owned_user_exit)),
        denied_read: translate(symbol_address!(owned_user_denied_read)),
    }
}

fn execute(context: &mut user_context, task: task_identity) {
    require(
        context.active.is_none()
            && context.phase.is_none()
            && context.saved_stack == 0
            && context.saved_ts == 0
            && active_context.load(Ordering::Acquire).is_null()
            && paging::read_cr3() == context.expected.root,
        "user_entry_state",
    );
    require(
        context.tasks.snapshot(task).is_ok_and(|value| {
            value.state() == task_state::ready && value.bounds() == context.expected.user.bounds()
        }),
        "user_task_binding",
    );
    require(context.tasks.start(task).is_ok(), "user_task_start");
    context.active = Some(task);
    context.phase = Some(user_phase::first_report);
    context.original_ts = paging::read_cr0() & task_switched;
    context.completion = 0;
    require(
        active_context
            .compare_exchange(
                ptr::null_mut(),
                ptr::from_mut(context),
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_ok(),
        "user_context_publish",
    );
    // SAFETY: The exact live task and admitted owner are selected; every user
    // return site/stack is checked. The independent RSP0 stack cannot overwrite
    // this Rust continuation. Only the single-use terminal handler resumes it.
    let result = unsafe {
        x86_64_owned_user_enter(
            context.expected.user.bounds().entry(),
            context.expected.user.bounds().initial_stack_pointer(),
            ptr::addr_of_mut!(context.saved_stack),
            ptr::addr_of_mut!(context.saved_ts),
        )
    };
    require(
        result == (if context.expected.mode == 1 { 1 } else { 2 })
            && context.completion == result
            && context.active.is_none()
            && context.phase.is_none()
            && active_context.load(Ordering::Acquire).is_null()
            && context.saved_stack == 0
            && context.saved_ts == 0
            && paging::read_cr0() & task_switched == context.original_ts
            && paging::read_cr3() == context.expected.root,
        "user_return_state",
    );
}

/// # Safety
/// The common handler owns the live frame and matching snapshot. On Some(true)
/// it must clear its active latch and call resume_kernel without returning to
/// this user frame. Some(false) returns this same validated frame to ring 3.
pub(super) unsafe fn contain(
    frame: *mut exception_frame,
    snapshot: exception_snapshot,
) -> Option<bool> {
    let pointer = active_context.load(Ordering::Acquire);
    if pointer.is_null() {
        return None;
    }
    // SAFETY: Published only while its kernel-stack context and both retained
    // root owners are live; this BSP handler is its sole accessor during entry.
    let context = unsafe { &mut *pointer };
    let task = context.active?;
    let record = context.tasks.snapshot(task).ok()?;
    if record.state() != task_state::running
        || record.bounds() != context.expected.user.bounds()
        || record.context_attached()
        || context.phase.is_none()
        || !continuation_is_valid(context)
    {
        return None;
    }
    let address = frame as u64;
    if address < context.expected.privilege_start
        || !address.is_multiple_of(16)
        || address.checked_add(176) != Some(context.expected.privilege_end)
    {
        return None;
    }
    // SAFETY: The complete hardware frame is inside the private resident RSP0
    // storage. No user mapping can modify the kernel-owned normalized prefix.
    let prefix = unsafe { ptr::read(frame) };
    context.expected.phase = context.phase?;
    let action = user_fault_gate::match_exception(
        context.expected,
        user_observation {
            snapshot,
            frame: address,
            root: paging::read_cr3(),
            task_switched: paging::read_cr0() & task_switched != 0,
            data_base_register: prefix.base,
            mode_register: prefix.register_12,
            marker_register: prefix.register_13,
        },
    )?;
    match action {
        user_action::resume(next) => {
            context.phase = Some(match context.phase? {
                user_phase::first_report => user_phase::second_report,
                user_phase::second_report => user_phase::terminal,
                user_phase::terminal => return None,
            });
            context.reports += 1;
            // SAFETY: The exact admitted report resumes only its adjacent UD2
            // continuation; every other register, stack word and flag is kept.
            unsafe {
                ptr::write(ptr::addr_of_mut!((*frame).instruction_pointer), next);
            }
            Some(false)
        }
        user_action::exit | user_action::fault => {
            if action == user_action::exit {
                context.tasks.exit(task).ok()?;
                context.completion = 2;
            } else {
                context.tasks.fault_unscheduled(task).ok()?;
                context.completion = 1;
            }
            context.phase = None;
            Some(true)
        }
    }
}

fn continuation_is_valid(context: &user_context) -> bool {
    context.saved_stack.is_multiple_of(8)
        && context.saved_stack >= context.kernel_stack.base
        && context
            .saved_stack
            .checked_add(56)
            .is_some_and(|end| end <= context.kernel_stack.base + context.kernel_stack.length)
        && matches!(context.saved_ts, 0 | 8)
        && context.saved_ts == context.original_ts
}

/// # Safety
/// The common exception latch was cleared after contain returned Some(true).
/// No Rust reference into the abandoned privilege stack may remain observable.
pub(super) unsafe fn resume_kernel() -> ! {
    let pointer = active_context.swap(ptr::null_mut(), Ordering::AcqRel);
    require(!pointer.is_null(), "user_resume_without_owner");
    // SAFETY: This consumes the sole published context after terminal admission.
    let context = unsafe { &mut *pointer };
    require(
        context.phase.is_none()
            && context.active.is_some()
            && matches!(context.completion, 1 | 2)
            && continuation_is_valid(context),
        "user_resume_state",
    );
    let stack = context.saved_stack;
    let ts = context.saved_ts;
    let status = context.completion;
    context.saved_stack = 0;
    context.saved_ts = 0;
    context.active = None;
    // SAFETY: Consume the exact pre-entry continuation once, restore TS and
    // kernel selectors, and return on the original independently resident stack.
    unsafe { x86_64_owned_user_resume(status, stack, ts) }
}

fn tls_bases_are_zero() -> bool {
    for register in [0xc000_0100_u32, 0xc000_0101, 0xc000_0102] {
        let low: u32;
        let high: u32;
        // SAFETY: These architectural FS/GS MSRs exist in the admitted x86-64
        // CPL0 environment. The GPR-only fixture admits no inherited TLS state.
        unsafe {
            asm!("rdmsr", in("ecx") register, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags));
        }
        if low != 0 || high != 0 {
            return false;
        }
    }
    true
}

fn require(condition: bool, reason: &'static str) {
    if !condition {
        fail(reason);
    }
}
