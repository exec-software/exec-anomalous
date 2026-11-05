//! Fixed vessel-supervision and destructive-restart proof.

use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use crate::circle::{circle_accounting, circle_table};
use crate::console::serial_port;
use crate::identity::{circle_id, supervision_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator, physical_range};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::supervision::{
    supervision_observation, supervision_outcome, supervision_phase, supervision_replacement,
    supervision_table,
};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

use super::scheduler_test::{
    all_general_registers_zero, attach_task_space, cr0_task_switched, create_task,
    frame_address_matches_stack, initial_user_flags, invalid_opcode_vector, mailbox_is_zero,
    prepare_task_space, privilege_stack_pages, read_mailbox_word, release_task_space,
    relocated_user_address, resume_flag, running_user_flags, scheduler_frame,
    scheduler_gate_vector, timer_vector, user_mailbox_address, user_return_fields_are_valid,
    user_stack_top, write_mailbox_word,
};
use super::{descriptors, exceptions, interrupts, paging};

const supervision_task_count: usize = 2;
const restart_status: u64 = 1;
const completion_status: u64 = 2;
const mailbox_value_a_offset: u64 = 0;
const mailbox_value_b_offset: u64 = 8;
const mailbox_state_offset: u64 = 16;
const vessel_generation_1_value_a: u64 = 0xa110_ca11_0001_0001;
const vessel_generation_1_value_b: u64 = 0xa110_ca11_0001_0002;
const vessel_generation_1_arm: u64 = 0xa110_a11d_0001_0001;
const supervisor_value_a: u64 = 0x5a9e_ca11_0001_0001;
const supervisor_value_b: u64 = 0x5a9e_ca11_0001_0002;
const supervisor_continuity: u64 = 0x5a9e_c017_0001_0001;
const vessel_generation_2_value_a: u64 = 0xa110_ca11_0002_0001;
const vessel_generation_2_value_b: u64 = 0xa110_ca11_0002_0002;
const vessel_generation_2_completion: u64 = 0xa110_d09e_0002_0001;
const operation_vessel_ready: u64 = 1;
const operation_supervisor_await_fault: u64 = 2;
const operation_supervisor_restart: u64 = 3;
const operation_vessel_complete: u64 = 4;
const operation_supervisor_complete: u64 = 5;
const wake_fault: u64 = 0x41;
const wake_completion: u64 = 0x42;

const _: () = assert!(privilege_stack_pages == 4);
const _: () = assert!(descriptors::privilege_stack_size == 16 * 1024);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum supervision_role {
    vessel,
    supervisor,
}

impl supervision_role {
    const fn label(self) -> &'static str {
        match self {
            Self::vessel => "vessel_fixture",
            Self::supervisor => "supervisor_fixture",
        }
    }
}

struct supervised_task {
    circle: circle_id,
    identity: task_identity,
    space: super::scheduler_test::task_space,
    ready_seen: bool,
    incarnation: u64,
    role: supervision_role,
}

#[derive(Clone, Copy)]
struct owned_extents {
    physical_pages: [u64; 3],
    root: u64,
    privilege_stack: physical_range,
}

#[derive(Clone, Copy)]
struct supervision_trace {
    dispatches: [u64; 6],
    preemptions: [u64; 1],
    tick: u64,
    fault: Option<exceptions::exception_snapshot>,
    fault_observation: Option<supervision_observation>,
    completion_observation: Option<supervision_observation>,
    replacement: Option<supervision_replacement>,
    resume_count: usize,
    block_count: usize,
    wake_count: usize,
    observation_count: usize,
    fault_count: usize,
    restart_count: usize,
    completion_count: usize,
    clean_replacement: bool,
    supervisor_continued: bool,
    kernel_continuation_restored: bool,
}

impl supervision_trace {
    const fn empty() -> Self {
        Self {
            dispatches: [u64::MAX; 6],
            preemptions: [u64::MAX; 1],
            tick: 0,
            fault: None,
            fault_observation: None,
            completion_observation: None,
            replacement: None,
            resume_count: 0,
            block_count: 0,
            wake_count: 0,
            observation_count: 0,
            fault_count: 0,
            restart_count: 0,
            completion_count: 0,
            clean_replacement: false,
            supervisor_continued: false,
            kernel_continuation_restored: false,
        }
    }
}

struct supervision_runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    supervisions: supervision_table,
    relationship: supervision_id,
    vessel: Option<supervised_task>,
    supervisor: supervised_task,
    active_role: Option<supervision_role>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    trace: supervision_trace,
}

#[derive(Clone, Copy)]
struct supervision_evidence {
    initial: frame_accounting,
    vessel_generation_1_circle: u64,
    vessel_generation_1_task: u64,
    vessel_generation_2_circle: u64,
    vessel_generation_2_task: u64,
    supervisor_circle: u64,
    supervisor_task: u64,
    trace: supervision_trace,
}

static active_runtime: AtomicPtr<supervision_runtime> = AtomicPtr::new(ptr::null_mut());
static switch_active: AtomicBool = AtomicBool::new(false);
static gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_supervision_vessel_generation_1_start: u8;
    static x86_64_supervision_vessel_generation_1_ready_return: u8;
    static x86_64_supervision_vessel_generation_1_fault_instruction: u8;
    static x86_64_supervision_vessel_generation_1_end: u8;
    static x86_64_supervision_supervisor_start: u8;
    static x86_64_supervision_supervisor_fault_return: u8;
    static x86_64_supervision_supervisor_completion_return: u8;
    static x86_64_supervision_supervisor_complete_return: u8;
    static x86_64_supervision_supervisor_end: u8;
    static x86_64_supervision_vessel_generation_2_start: u8;
    static x86_64_supervision_vessel_generation_2_completion_return: u8;
    static x86_64_supervision_vessel_generation_2_end: u8;
}

pub(super) fn run() -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_supervision_state_invalid");
    }

    let kernel_cr3 = paging::read_cr3();
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        run_with_allocator(allocator, kernel_cr3, bootstrap_privilege_stack_top)
    });

    if paging::read_cr3() != kernel_cr3 {
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    let evidence = match result {
        Ok(Ok(evidence)) => evidence,
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    };
    emit_evidence(evidence);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
) -> Result<supervision_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("supervision_setup_state_invalid");
    }
    let initial = allocator.accounting();
    if initial.free < 22
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("supervision_initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let vessel_circle = circles
        .create()
        .map_err(|_| "vessel_circle_create_failed")?;
    let supervisor_circle = circles
        .create()
        .map_err(|_| "supervisor_circle_create_failed")?;
    let vessel_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_supervision_vessel_generation_1_start),
        ptr::addr_of!(x86_64_supervision_vessel_generation_1_end),
    )?;
    let supervisor_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_supervision_supervisor_start),
        ptr::addr_of!(x86_64_supervision_supervisor_end),
    )?;
    attach_task_space(&mut circles, vessel_circle)?;
    attach_task_space(&mut circles, supervisor_circle)?;
    let vessel_identity = create_task(
        &mut circles,
        &mut tasks,
        vessel_circle,
        1,
        vessel_space.payload_length,
    )?;
    let supervisor_identity = create_task(
        &mut circles,
        &mut tasks,
        supervisor_circle,
        2,
        supervisor_space.payload_length,
    )?;
    let mut scheduler = scheduler_table::new(supervision_task_count)
        .map_err(|_| "supervision_scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, vessel_identity)
        .map_err(|_| "vessel_task_admission_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, supervisor_identity)
        .map_err(|_| "supervisor_task_admission_failed")?;
    let mut supervisions = supervision_table::new();
    let relationship = supervisions
        .create(&mut circles, &tasks, supervisor_identity, vessel_identity)
        .map_err(|_| "supervision_relationship_create_failed")?;

    let ready = allocator.accounting();
    validate_ready_ownership(
        initial,
        ready,
        &circles,
        &tasks,
        &scheduler,
        &supervisions,
        relationship,
    )?;

    let mut runtime = supervision_runtime {
        circles,
        tasks,
        scheduler,
        supervisions,
        relationship,
        vessel: Some(supervised_task {
            circle: vessel_circle,
            identity: vessel_identity,
            space: vessel_space,
            ready_seen: false,
            incarnation: 1,
            role: supervision_role::vessel,
        }),
        supervisor: supervised_task {
            circle: supervisor_circle,
            identity: supervisor_identity,
            space: supervisor_space,
            ready_seen: false,
            incarnation: 1,
            role: supervision_role::supervisor,
        },
        active_role: None,
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        trace: supervision_trace::empty(),
    };

    let first = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "supervision_initial_dispatch_failed")?;
    if first.task() != runtime.vessel_ref()?.identity
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        return Err("supervision_initial_dispatch_invalid");
    }
    runtime.trace.dispatches[0] = first.sequence();
    let first_frame = runtime.select_role(supervision_role::vessel, false)?;
    let runtime_pointer = ptr::from_mut(&mut runtime);
    publish_runtime(runtime_pointer)?;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    let status = unsafe {
        super::scheduler_test::x86_64_scheduler_enter(
            first_frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };

    active_runtime.store(ptr::null_mut(), Ordering::Release);
    if status != restart_status
        || runtime.original_task_switched != original_task_switched
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || paging::read_cr3() != kernel_cr3
        || interrupts::interrupts_enabled()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_privilege_stack_top
        || runtime.trace.dispatches[..4] != [0, 1, 2, 3]
        || runtime.trace.preemptions != [0]
        || runtime.trace.tick == 0
        || runtime.trace.resume_count != 2
        || runtime.trace.block_count != 2
        || runtime.trace.wake_count != 1
        || runtime.trace.observation_count != 1
        || runtime.trace.fault_count != 1
        || runtime.trace.restart_count != 1
        || !interrupts::scheduler_interrupts_are_masked()
    {
        return Err("supervision_restart_restore_failed");
    }
    compiler_fence(Ordering::SeqCst);
    runtime.saved_kernel_stack = 0;
    runtime.original_task_switched = 0;

    let vessel_generation_1 = runtime.vessel_ref()?.identity;
    let vessel_generation_1_circle = vessel_generation_1.circle().generation_value();
    let vessel_generation_1_task = vessel_generation_1.task().generation_value();
    let old_extents = runtime.vessel_extents()?;
    withdraw_and_destroy_vessel(allocator, &mut runtime, vessel_generation_1)?;

    let vessel_generation_2 = create_replacement_vessel(allocator, &mut runtime, old_extents)?;
    let vessel_generation_2_circle = vessel_generation_2.circle().generation_value();
    let vessel_generation_2_task = vessel_generation_2.task().generation_value();
    let admission = runtime
        .supervisions
        .prepare_admit_replacement(
            &runtime.circles,
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            vessel_generation_2,
        )
        .map_err(replacement_preflight_reason)?;
    if admission.incarnation() != vessel_generation_2 || admission.restart_sequence() != 1 {
        return Err("replacement_admission_invalid");
    }
    runtime
        .scheduler
        .admit(
            &mut runtime.circles,
            &mut runtime.tasks,
            vessel_generation_2,
        )
        .map_err(|_| "replacement_scheduler_admission_failed")?;
    admission.commit();
    let snapshot = runtime
        .supervisions
        .snapshot(runtime.relationship)
        .map_err(|_| "replacement_snapshot_failed")?;
    if snapshot.phase() != supervision_phase::replacement_active
        || snapshot.active_incarnation() != Some(vessel_generation_2)
        || snapshot.restart_budget_remaining() != 0
    {
        return Err("replacement_relationship_invalid");
    }
    runtime.trace.clean_replacement = true;

    let second = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "replacement_dispatch_failed")?;
    if second.task() != vessel_generation_2
        || second.sequence() != 4
        || second.context() != scheduler_context::initial
    {
        return Err("replacement_dispatch_invalid");
    }
    runtime.trace.dispatches[4] = second.sequence();
    let second_frame = runtime.select_role(supervision_role::vessel, false)?;
    publish_runtime(runtime_pointer)?;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    let status = unsafe {
        super::scheduler_test::x86_64_scheduler_enter(
            second_frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };

    active_runtime.store(ptr::null_mut(), Ordering::Release);
    runtime.trace.kernel_continuation_restored = true;
    if status != completion_status
        || runtime.original_task_switched != original_task_switched
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || paging::read_cr3() != kernel_cr3
        || interrupts::interrupts_enabled()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_privilege_stack_top
        || runtime.trace.dispatches != [0, 1, 2, 3, 4, 5]
        || runtime.trace.preemptions != [0]
        || runtime.trace.resume_count != 3
        || runtime.trace.block_count != 2
        || runtime.trace.wake_count != 2
        || runtime.trace.observation_count != 2
        || runtime.trace.fault_count != 1
        || runtime.trace.restart_count != 1
        || runtime.trace.completion_count != 1
        || !runtime.trace.clean_replacement
        || !runtime.trace.supervisor_continued
        || !interrupts::scheduler_interrupts_are_masked()
    {
        return Err("supervision_completion_restore_failed");
    }
    compiler_fence(Ordering::SeqCst);
    runtime.saved_kernel_stack = 0;
    runtime.original_task_switched = 0;

    let supervisor_circle_generation = runtime.supervisor.circle.generation_value();
    let supervisor_task_generation = runtime.supervisor.identity.task().generation_value();
    teardown_completed_runtime(allocator, &mut runtime)?;
    let final_frames = allocator.accounting();
    let final_circles = runtime.circles.accounting();
    let final_scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "supervision_final_scheduler_invalid")?;
    if final_frames != initial
        || final_circles != empty_circle_accounting()
        || runtime.tasks.active_count() != 0
        || runtime.tasks.attached_context_count() != 0
        || runtime.supervisions.active_count() != 0
        || final_scheduler.admitted() != 0
        || final_scheduler.runnable() != 0
        || final_scheduler.current().is_some()
    {
        return Err("supervision_final_accounting_invalid");
    }

    let evidence = supervision_evidence {
        initial,
        vessel_generation_1_circle,
        vessel_generation_1_task,
        vessel_generation_2_circle,
        vessel_generation_2_task,
        supervisor_circle: supervisor_circle_generation,
        supervisor_task: supervisor_task_generation,
        trace: runtime.trace,
    };
    retire_runtime_metadata(&mut runtime);
    Ok(evidence)
}

fn publish_runtime(runtime: *mut supervision_runtime) -> Result<(), &'static str> {
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("supervision_context_publication_failed");
    }
    Ok(())
}

fn validate_ready_ownership(
    initial: frame_accounting,
    ready: frame_accounting,
    circles: &circle_table,
    tasks: &task_table,
    scheduler: &scheduler_table,
    supervisions: &supervision_table,
    relationship: supervision_id,
) -> Result<(), &'static str> {
    if initial.free.checked_sub(22) != Some(ready.free)
        || ready.data_owned != 14
        || ready.table_owned != 8
        || ready.active_leases != 10
        || ready.bound_leases != 10
    {
        return Err("supervision_ready_frame_accounting_invalid");
    }
    let circle_accounting = circles.accounting();
    let scheduler_snapshot = scheduler
        .snapshot(tasks)
        .map_err(|_| "supervision_ready_scheduler_invalid")?;
    let relationship_snapshot = supervisions
        .snapshot(relationship)
        .map_err(|_| "supervision_ready_relationship_invalid")?;
    if circle_accounting.circles != 2
        || circle_accounting.tasks != 2
        || circle_accounting.task_contexts != 2
        || circle_accounting.supervisions != 1
        || circle_accounting.endpoints != 0
        || circle_accounting.sigils != 0
        || circle_accounting.address_spaces != 2
        || circle_accounting.mappings != 6
        || tasks.active_count() != 2
        || tasks.attached_context_count() != 2
        || scheduler_snapshot.admitted() != 2
        || scheduler_snapshot.runnable() != 2
        || scheduler_snapshot.current().is_some()
        || scheduler_snapshot.next_dispatch_sequence() != Some(0)
        || scheduler_snapshot.next_preemption_sequence() != Some(0)
        || relationship_snapshot.phase() != supervision_phase::active_incarnation
        || relationship_snapshot.restart_budget_remaining() != 1
        || relationship_snapshot.next_event_sequence() != Some(1)
        || relationship_snapshot.next_restart_sequence() != Some(1)
        || relationship_snapshot.pending_observation().is_some()
        || !relationship_snapshot.supervisor_circle_owns_relationship()
    {
        return Err("supervision_ready_object_accounting_invalid");
    }
    Ok(())
}

fn withdraw_and_destroy_vessel(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut supervision_runtime,
    identity: task_identity,
) -> Result<(), &'static str> {
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, identity)
        .map_err(|_| "vessel_context_withdrawal_failed")?;
    let mut vessel = runtime.vessel.take().ok_or("vessel_missing")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        vessel.circle,
        &mut vessel.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, identity)
        .map_err(|_| "vessel_task_destroy_failed")?;
    runtime
        .circles
        .destroy(vessel.circle)
        .map_err(|_| "vessel_circle_destroy_failed")?;
    Ok(())
}

fn create_replacement_vessel(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut supervision_runtime,
    expected_extents: owned_extents,
) -> Result<task_identity, &'static str> {
    let circle = runtime
        .circles
        .create()
        .map_err(|_| "replacement_circle_create_failed")?;
    let space = prepare_task_space(
        allocator,
        runtime.kernel_cr3,
        ptr::addr_of!(x86_64_supervision_vessel_generation_2_start),
        ptr::addr_of!(x86_64_supervision_vessel_generation_2_end),
    )?;
    if space.physical_pages != expected_extents.physical_pages
        || space
            .root
            .root_address()
            .map_err(|_| "replacement_root_missing")?
            != expected_extents.root
        || space.privilege_stack_range != expected_extents.privilege_stack
        || !mailbox_is_zero(&space)
    {
        return Err("replacement_storage_reuse_invalid");
    }
    attach_task_space(&mut runtime.circles, circle)?;
    let identity = create_task(
        &mut runtime.circles,
        &mut runtime.tasks,
        circle,
        1,
        space.payload_length,
    )?;
    runtime.vessel = Some(supervised_task {
        circle,
        identity,
        space,
        ready_seen: false,
        incarnation: 2,
        role: supervision_role::vessel,
    });
    Ok(identity)
}

fn replacement_preflight_reason(error: crate::supervision::supervision_error) -> &'static str {
    match error {
        crate::supervision::supervision_error::invalid_identity => {
            "replacement_preflight_invalid_identity"
        }
        crate::supervision::supervision_error::stale_identity => {
            "replacement_preflight_stale_identity"
        }
        crate::supervision::supervision_error::capacity_exhausted => {
            "replacement_preflight_capacity_exhausted"
        }
        crate::supervision::supervision_error::generation_exhausted => {
            "replacement_preflight_generation_exhausted"
        }
        crate::supervision::supervision_error::corrupt_record => {
            "replacement_preflight_corrupt_record"
        }
        crate::supervision::supervision_error::same_task => "replacement_preflight_same_task",
        crate::supervision::supervision_error::same_circle => "replacement_preflight_same_circle",
        crate::supervision::supervision_error::wrong_supervisor => {
            "replacement_preflight_wrong_supervisor"
        }
        crate::supervision::supervision_error::wrong_incarnation => {
            "replacement_preflight_wrong_incarnation"
        }
        crate::supervision::supervision_error::invalid_phase => {
            "replacement_preflight_invalid_phase"
        }
        crate::supervision::supervision_error::invalid_supervisor_state => {
            "replacement_preflight_invalid_supervisor_state"
        }
        crate::supervision::supervision_error::invalid_incarnation_state => {
            "replacement_preflight_invalid_incarnation_state"
        }
        crate::supervision::supervision_error::context_missing => {
            "replacement_preflight_context_missing"
        }
        crate::supervision::supervision_error::context_still_attached => {
            "replacement_preflight_context_still_attached"
        }
        crate::supervision::supervision_error::observation_pending => {
            "replacement_preflight_observation_pending"
        }
        crate::supervision::supervision_error::observation_missing => {
            "replacement_preflight_observation_missing"
        }
        crate::supervision::supervision_error::wrong_outcome => {
            "replacement_preflight_wrong_outcome"
        }
        crate::supervision::supervision_error::event_sequence_exhausted => {
            "replacement_preflight_event_sequence_exhausted"
        }
        crate::supervision::supervision_error::restart_sequence_exhausted => {
            "replacement_preflight_restart_sequence_exhausted"
        }
        crate::supervision::supervision_error::restart_budget_exhausted => {
            "replacement_preflight_restart_budget_exhausted"
        }
        crate::supervision::supervision_error::retired_incarnation_live => {
            "replacement_preflight_retired_incarnation_live"
        }
        crate::supervision::supervision_error::retired_circle_live => {
            "replacement_preflight_retired_circle_live"
        }
        crate::supervision::supervision_error::replacement_not_fresh => {
            "replacement_preflight_not_fresh"
        }
        crate::supervision::supervision_error::circle_accounting_mismatch => {
            "replacement_preflight_circle_accounting_mismatch"
        }
        crate::supervision::supervision_error::record_clearing_failed => {
            "replacement_preflight_record_clearing_failed"
        }
        crate::supervision::supervision_error::task(_) => "replacement_preflight_task",
        crate::supervision::supervision_error::circle(_) => "replacement_preflight_circle",
    }
}

fn teardown_completed_runtime(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut supervision_runtime,
) -> Result<(), &'static str> {
    let vessel_identity = runtime.vessel_ref()?.identity;
    let supervisor_identity = runtime.supervisor.identity;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, vessel_identity)
        .map_err(|_| "replacement_context_withdrawal_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            supervisor_identity,
        )
        .map_err(|_| "supervisor_context_withdrawal_failed")?;
    runtime
        .supervisions
        .withdraw_incarnation(
            &runtime.tasks,
            runtime.relationship,
            supervisor_identity,
            vessel_identity,
        )
        .map_err(|_| "relationship_incarnation_withdrawal_failed")?;
    let vessel = runtime
        .vessel
        .as_mut()
        .ok_or("replacement_vessel_missing")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        vessel.circle,
        &mut vessel.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, vessel_identity)
        .map_err(|_| "replacement_task_destroy_failed")?;
    runtime
        .circles
        .destroy(vessel.circle)
        .map_err(|_| "replacement_circle_destroy_failed")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        runtime.supervisor.circle,
        &mut runtime.supervisor.space,
    )?;
    runtime
        .supervisions
        .withdraw_supervisor(
            &mut runtime.circles,
            &runtime.tasks,
            runtime.relationship,
            supervisor_identity,
        )
        .map_err(|_| "relationship_supervisor_withdrawal_failed")?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, supervisor_identity)
        .map_err(|_| "supervisor_task_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.supervisor.circle)
        .map_err(|_| "supervisor_circle_destroy_failed")?;
    runtime
        .supervisions
        .release(&runtime.circles, &runtime.tasks, runtime.relationship)
        .map_err(|_| "relationship_release_failed")?;
    Ok(())
}

fn retire_runtime_metadata(runtime: &mut supervision_runtime) {
    debug_assert!(
        runtime
            .vessel
            .as_ref()
            .is_none_or(|task| task.space.frame.is_null())
    );
    debug_assert!(runtime.supervisor.space.frame.is_null());
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(runtime.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.bootstrap_privilege_stack_top), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.original_task_switched), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.trace), supervision_trace::empty());
    }
}

impl supervision_runtime {
    fn vessel_ref(&self) -> Result<&supervised_task, &'static str> {
        self.vessel.as_ref().ok_or("vessel_missing")
    }

    fn vessel_mut(&mut self) -> Result<&mut supervised_task, &'static str> {
        self.vessel.as_mut().ok_or("vessel_missing")
    }

    fn task(&self, role: supervision_role) -> Result<&supervised_task, &'static str> {
        match role {
            supervision_role::vessel => self.vessel_ref(),
            supervision_role::supervisor => Ok(&self.supervisor),
        }
    }

    fn role_for_identity(&self, identity: task_identity) -> Option<supervision_role> {
        if self
            .vessel
            .as_ref()
            .is_some_and(|task| task.identity == identity)
        {
            Some(supervision_role::vessel)
        } else if self.supervisor.identity == identity {
            Some(supervision_role::supervisor)
        } else {
            None
        }
    }

    fn current_role(&self) -> Option<supervision_role> {
        let current = self.scheduler.snapshot(&self.tasks).ok()?.current()?;
        let role = self.role_for_identity(current)?;
        (self.active_role == Some(role)).then_some(role)
    }

    fn vessel_extents(&self) -> Result<owned_extents, &'static str> {
        let vessel = self.vessel_ref()?;
        Ok(owned_extents {
            physical_pages: vessel.space.physical_pages,
            root: vessel
                .space
                .root
                .root_address()
                .map_err(|_| "vessel_root_missing")?,
            privilege_stack: vessel.space.privilege_stack_range,
        })
    }

    fn select_role(
        &mut self,
        role: supervision_role,
        require_task_switched: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let task = self.task(role)?;
        let root = task
            .space
            .root
            .root_address()
            .map_err(|_| "selected_supervision_root_missing")?;
        let stack_top = task
            .space
            .privilege_stack_range
            .end()
            .ok_or("selected_supervision_stack_extent_invalid")?;
        let frame = task.space.frame;
        unsafe { task.space.root.activate() }
            .map_err(|_| "selected_supervision_root_activation_failed")?;
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("selected_supervision_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || !self.frame_is_valid_for_role(frame, role, require_task_switched)
        {
            return Err("selected_supervision_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_validation_error(
        &self,
        frame: *const scheduler_frame,
        role: supervision_role,
        require_task_switched: bool,
    ) -> Option<&'static str> {
        let Ok(task) = self.task(role) else {
            return Some("supervision_frame_task_missing");
        };
        if frame != task.space.frame {
            return Some("supervision_frame_pointer_invalid");
        }
        if task.role != role {
            return Some("supervision_frame_role_invalid");
        }
        if !frame_address_matches_stack(frame, task.space.privilege_stack_range) {
            return Some("supervision_frame_stack_address_invalid");
        }
        if task.space.privilege_stack.frame_count() != privilege_stack_pages
            || task.space.privilege_stack.range() != Some(task.space.privilege_stack_range)
        {
            return Some("supervision_frame_privilege_stack_invalid");
        }
        if task.space.root.validate_all_mappings().is_err() {
            return Some("supervision_frame_mappings_invalid");
        }
        let Ok(snapshot) = self.tasks.snapshot(task.identity) else {
            return Some("supervision_frame_snapshot_invalid");
        };
        if snapshot.state() != task_state::running {
            return Some("supervision_frame_task_not_running");
        }
        if !snapshot.context_attached() {
            return Some("supervision_frame_context_missing");
        }
        let root = task.space.root.root_address().ok();
        if root != Some(paging::entry_address(paging::read_cr3())) {
            return Some("supervision_frame_root_not_selected");
        }
        let stack_top = task.space.privilege_stack_range.end();
        if stack_top != Some(unsafe { descriptors::selected_privilege_stack_top() }) {
            return Some("supervision_frame_stack_not_selected");
        }
        if require_task_switched && paging::read_cr0() & cr0_task_switched == 0 {
            return Some("supervision_frame_ts_not_set");
        }
        let frame = unsafe { ptr::read(frame) };
        let bounds = snapshot.bounds();
        if frame.error_code != 0 {
            return Some("supervision_frame_error_code_invalid");
        }
        if frame.code_segment != u64::from(descriptors::user_code_selector) {
            return Some("supervision_frame_code_segment_invalid");
        }
        if frame.stack_segment != u64::from(descriptors::user_data_selector) {
            return Some("supervision_frame_stack_segment_invalid");
        }
        if frame.stack_pointer != user_stack_top {
            return Some("supervision_frame_stack_pointer_invalid");
        }
        if frame.flags & (1 << 1) == 0
            || frame.flags & (3 << 12) != 0
            || frame.flags & (1 << 10) != 0
            || frame.flags & (1 << 14) != 0
        {
            return Some("supervision_frame_flags_invalid");
        }
        if !bounds.contains_code(frame.instruction_pointer) {
            return Some("supervision_frame_instruction_pointer_out_of_bounds");
        }
        if !bounds.contains_stack_pointer(frame.stack_pointer) {
            return Some("supervision_frame_stack_pointer_out_of_bounds");
        }
        if !user_return_fields_are_valid(frame, bounds) {
            return Some("supervision_frame_return_fields_invalid");
        }
        None
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: supervision_role,
        require_task_switched: bool,
    ) -> bool {
        self.frame_validation_error(frame, role, require_task_switched)
            .is_none()
    }
}

pub(super) fn supervision_is_active() -> bool {
    !active_runtime.load(Ordering::Acquire).is_null()
}

pub(super) fn user_interrupt_frame_is_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return false;
    }
    let runtime = unsafe { &*runtime };
    let Some(role) = runtime.current_role() else {
        return false;
    };
    runtime.frame_is_valid_for_role(frame.cast(), role, true)
}

pub(super) fn dispatch_timer_interrupt(
    frame: *mut interrupts::interrupt_frame,
    observed_at: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    if switch_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    let result = dispatch_timer(frame.cast(), observed_at).ok();
    switch_active.store(false, Ordering::Release);
    result.map(|frame| frame.cast())
}

fn dispatch_timer(
    frame: *mut scheduler_frame,
    observed_at: monotonic_instant,
) -> Result<*mut scheduler_frame, &'static str> {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return Err("supervision_timer_without_runtime");
    }
    let runtime = unsafe { &mut *runtime };
    if runtime.current_role() != Some(supervision_role::vessel)
        || runtime.vessel_ref()?.incarnation != 1
        || runtime.trace.preemptions != [u64::MAX]
        || !runtime.vessel_ref()?.ready_seen
        || !runtime.frame_is_valid_for_role(frame, supervision_role::vessel, true)
        || !mailbox_matches(runtime.vessel_ref()?, 0)
        || observed_at.timebase().slot() != 0
        || observed_at.timebase().generation() != crate::time::boot_timebase_generation
    {
        return Err("supervision_timer_state_invalid");
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != timer_vector || incoming.flags & (1 << 9) == 0 {
        return Err("supervision_timer_observation_invalid");
    }
    write_mailbox_word(
        runtime.vessel_ref()?.space.physical_pages[1],
        mailbox_state_offset,
        vessel_generation_1_arm,
    )?;
    let vessel_identity = runtime.vessel_ref()?.identity;
    let transition = runtime
        .scheduler
        .preempt_current(&mut runtime.tasks, vessel_identity)
        .map_err(|_| "vessel_preemption_failed")?;
    if transition.task() != vessel_identity
        || transition.sequence() != 0
        || transition.dispatch().task() != runtime.supervisor.identity
        || transition.dispatch().sequence() != 1
        || transition.dispatch().context() != scheduler_context::initial
    {
        return Err("vessel_preemption_transition_invalid");
    }
    runtime.trace.preemptions[0] = transition.sequence();
    runtime.trace.dispatches[1] = transition.dispatch().sequence();
    runtime.trace.tick = observed_at.tick();
    runtime.select_role(supervision_role::supervisor, true)
}

pub(super) fn contain_expected_user_exception(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    if switch_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    let result = contain_user_exception(frame.cast(), snapshot).ok();
    switch_active.store(false, Ordering::Release);
    result.map(|frame| frame.cast())
}

fn contain_user_exception(
    frame: *mut scheduler_frame,
    snapshot: exceptions::exception_snapshot,
) -> Result<*mut scheduler_frame, &'static str> {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return Err("supervision_fault_without_runtime");
    }
    let runtime = unsafe { &mut *runtime };
    if runtime.current_role() != Some(supervision_role::vessel)
        || runtime.vessel_ref()?.incarnation != 1
        || runtime.trace.preemptions != [0]
        || runtime.trace.resume_count != 1
        || !runtime.frame_is_valid_for_role(frame, supervision_role::vessel, true)
        || !mailbox_matches(runtime.vessel_ref()?, vessel_generation_1_arm)
    {
        return Err("supervision_fault_state_invalid");
    }
    let expected_instruction = relocated_user_address(
        ptr::addr_of!(x86_64_supervision_vessel_generation_1_start),
        ptr::addr_of!(x86_64_supervision_vessel_generation_1_fault_instruction),
        runtime.vessel_ref()?.space.payload_length,
    )
    .ok_or("supervision_fault_instruction_invalid")?;
    let registers = unsafe { ptr::read(frame) };
    if !expected_invalid_opcode_matches(snapshot, registers, expected_instruction) {
        return Err("supervision_unexpected_user_exception");
    }
    let vessel_identity = runtime.vessel_ref()?.identity;
    let publication = runtime
        .supervisions
        .prepare_publication(
            &runtime.tasks,
            runtime.relationship,
            vessel_identity,
            supervision_outcome::invalid_instruction_fault,
        )
        .map_err(|_| "supervision_fault_publication_preflight_failed")?;
    let transition = runtime
        .scheduler
        .fault_current_and_wake(
            &mut runtime.tasks,
            vessel_identity,
            runtime.supervisor.identity,
        )
        .map_err(|_| "supervision_fault_wake_failed")?;
    if transition.task() != vessel_identity
        || transition.dispatch().task() != runtime.supervisor.identity
        || transition.dispatch().sequence() != 3
        || transition.dispatch().context() != scheduler_context::saved
        || publication.supervisor() != runtime.supervisor.identity
        || publication.observation().event_sequence() != 1
    {
        return Err("supervision_fault_wake_invalid");
    }
    publication.commit();
    let observation = runtime
        .supervisions
        .take_observation(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
        )
        .map_err(|_| "fault_observation_consume_failed")?;
    if observation.event_sequence() != 1
        || observation.incarnation() != vessel_identity
        || observation.outcome() != supervision_outcome::invalid_instruction_fault
    {
        return Err("fault_observation_invalid");
    }
    runtime.trace.dispatches[3] = transition.dispatch().sequence();
    runtime.trace.fault = Some(snapshot);
    runtime.trace.fault_observation = Some(observation);
    runtime.trace.fault_count = 1;
    runtime.trace.wake_count = 1;
    runtime.trace.observation_count = 1;
    runtime.trace.resume_count = 2;
    let next = runtime.select_role(supervision_role::supervisor, true)?;
    unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_fault) };
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

pub(super) fn reject_unexpected_exception() -> ! {
    fail("supervision_unexpected_exception")
}

pub(super) unsafe fn gate_dispatch(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    if gate_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        fail("supervision_gate_reentry");
    }
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        fail("supervision_gate_without_runtime");
    }
    let runtime = unsafe { &mut *runtime };
    let role = runtime
        .current_role()
        .unwrap_or_else(|| fail("supervision_gate_without_current_task"));
    if let Some(reason) = runtime.frame_validation_error(frame, role, true) {
        fail(reason);
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != scheduler_gate_vector || incoming.error_code != 0 {
        fail("supervision_gate_vector_invalid");
    }

    let result = match incoming.accumulator {
        operation_vessel_ready if role == supervision_role::vessel => {
            handle_vessel_ready(runtime, frame, incoming)
        }
        operation_supervisor_await_fault if role == supervision_role::supervisor => {
            handle_supervisor_await_fault(runtime, incoming)
        }
        operation_supervisor_restart if role == supervision_role::supervisor => {
            finish_supervisor_restart(runtime, incoming)
        }
        operation_vessel_complete if role == supervision_role::vessel => {
            handle_vessel_completion(runtime, incoming)
        }
        operation_supervisor_complete if role == supervision_role::supervisor => {
            finish_supervisor_completion(runtime, incoming)
        }
        _ => Err("supervision_gate_operation_invalid"),
    };
    match result {
        Ok(next) => {
            gate_active.store(false, Ordering::Release);
            next
        }
        Err(reason) => fail(reason),
    }
}

fn handle_vessel_ready(
    runtime: &mut supervision_runtime,
    frame: *mut scheduler_frame,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.vessel_ref()?.incarnation != 1
        || runtime.vessel_ref()?.ready_seen
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_supervision_vessel_generation_1_start),
                ptr::addr_of!(x86_64_supervision_vessel_generation_1_ready_return),
                runtime.vessel_ref()?.space.payload_length,
            )
            .ok_or("vessel_ready_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_vessel_ready)
        || !mailbox_matches(runtime.vessel_ref()?, 0)
    {
        return Err("vessel_ready_gate_invalid");
    }
    runtime.vessel_mut()?.ready_seen = true;
    unsafe { ptr::addr_of_mut!((*frame).flags).write(running_user_flags) };
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn handle_supervisor_await_fault(
    runtime: &mut supervision_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.supervisor.ready_seen
        || runtime.trace.preemptions != [0]
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_supervision_supervisor_start),
                ptr::addr_of!(x86_64_supervision_supervisor_fault_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_fault_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_await_fault)
        || !mailbox_matches(&runtime.supervisor, 0)
    {
        return Err("supervisor_await_gate_invalid");
    }
    let vessel_identity = runtime.vessel_ref()?.identity;
    let wait = runtime
        .supervisions
        .prepare_wait(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            vessel_identity,
        )
        .map_err(|_| "supervision_fault_wait_preflight_failed")?;
    let transition = runtime
        .scheduler
        .block_current_and_dispatch(
            &mut runtime.tasks,
            runtime.supervisor.identity,
            vessel_identity,
        )
        .map_err(|_| "supervisor_fault_wait_block_failed")?;
    if transition.task() != runtime.supervisor.identity
        || transition.dispatch().task() != vessel_identity
        || transition.dispatch().sequence() != 2
        || transition.dispatch().context() != scheduler_context::saved
        || wait.supervisor() != runtime.supervisor.identity
        || wait.incarnation() != vessel_identity
        || wait.expected_outcome() != supervision_outcome::invalid_instruction_fault
    {
        return Err("supervisor_fault_wait_transition_invalid");
    }
    wait.commit();
    runtime.supervisor.ready_seen = true;
    runtime.trace.dispatches[2] = transition.dispatch().sequence();
    runtime.trace.block_count = 1;
    runtime.trace.resume_count = 1;
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("supervision_interrupt_mask_failed");
    }
    runtime.select_role(supervision_role::vessel, true)
}

fn finish_supervisor_restart(
    runtime: &mut supervision_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.trace.fault_count != 1
        || runtime.trace.wake_count != 1
        || runtime.trace.observation_count != 1
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_supervision_supervisor_start),
                ptr::addr_of!(x86_64_supervision_supervisor_completion_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_completion_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_restart)
        || !mailbox_matches(&runtime.supervisor, supervisor_continuity)
    {
        return Err("supervisor_restart_gate_invalid");
    }
    let vessel_identity = runtime.vessel_ref()?.identity;
    let wait = runtime
        .supervisions
        .prepare_wait(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            vessel_identity,
        )
        .map_err(|_| "supervision_completion_wait_preflight_failed")?;
    let park = runtime
        .scheduler
        .park_current(&mut runtime.tasks, runtime.supervisor.identity)
        .map_err(|_| "supervisor_restart_park_failed")?;
    if park.task() != runtime.supervisor.identity
        || wait.expected_outcome() != supervision_outcome::successful_exit
    {
        return Err("supervisor_restart_park_invalid");
    }
    wait.commit();
    let replacement = runtime
        .supervisions
        .begin_replacement(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            vessel_identity,
        )
        .map_err(|_| "supervision_replacement_begin_failed")?;
    if replacement.restart_sequence() != 1 || replacement.retired_incarnation() != vessel_identity {
        return Err("supervision_replacement_sequence_invalid");
    }
    runtime.trace.replacement = Some(replacement);
    runtime.trace.block_count = 2;
    runtime.trace.restart_count = 1;
    resume_kernel(runtime, restart_status)
}

fn handle_vessel_completion(
    runtime: &mut supervision_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.vessel_ref()?.incarnation != 2
        || runtime.trace.restart_count != 1
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_supervision_vessel_generation_2_start),
                ptr::addr_of!(x86_64_supervision_vessel_generation_2_completion_return),
                runtime.vessel_ref()?.space.payload_length,
            )
            .ok_or("vessel_completion_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_vessel_complete)
        || !mailbox_matches(runtime.vessel_ref()?, vessel_generation_2_completion)
    {
        return Err("vessel_completion_gate_invalid");
    }
    let vessel_identity = runtime.vessel_ref()?.identity;
    let publication = runtime
        .supervisions
        .prepare_publication(
            &runtime.tasks,
            runtime.relationship,
            vessel_identity,
            supervision_outcome::successful_exit,
        )
        .map_err(|_| "supervision_completion_publication_preflight_failed")?;
    let transition = runtime
        .scheduler
        .complete_current_and_wake(
            &mut runtime.tasks,
            vessel_identity,
            runtime.supervisor.identity,
        )
        .map_err(|_| "supervision_completion_wake_failed")?;
    if transition.task() != vessel_identity
        || transition.dispatch().task() != runtime.supervisor.identity
        || transition.dispatch().sequence() != 5
        || transition.dispatch().context() != scheduler_context::saved
        || publication.observation().event_sequence() != 2
    {
        return Err("supervision_completion_wake_invalid");
    }
    publication.commit();
    let observation = runtime
        .supervisions
        .take_observation(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
        )
        .map_err(|_| "completion_observation_consume_failed")?;
    if observation.event_sequence() != 2
        || observation.incarnation() != vessel_identity
        || observation.outcome() != supervision_outcome::successful_exit
    {
        return Err("completion_observation_invalid");
    }
    runtime.trace.completion_observation = Some(observation);
    runtime.trace.dispatches[5] = transition.dispatch().sequence();
    runtime.trace.wake_count = 2;
    runtime.trace.observation_count = 2;
    runtime.trace.resume_count = 3;
    runtime.trace.completion_count = 1;
    let next = runtime.select_role(supervision_role::supervisor, true)?;
    unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_completion) };
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

fn finish_supervisor_completion(
    runtime: &mut supervision_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.trace.completion_count != 1
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_supervision_supervisor_start),
                ptr::addr_of!(x86_64_supervision_supervisor_complete_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_complete_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_complete)
        || !mailbox_matches(&runtime.supervisor, supervisor_continuity)
    {
        return Err("supervisor_completion_gate_invalid");
    }
    let completion = runtime
        .supervisions
        .prepare_supervisor_completion(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            runtime.vessel_ref()?.identity,
        )
        .map_err(|_| "supervisor_completion_preflight_failed")?;
    runtime
        .scheduler
        .complete_current(&mut runtime.tasks, runtime.supervisor.identity)
        .map_err(|_| "supervisor_completion_transition_failed")?;
    if completion.supervisor() != runtime.supervisor.identity {
        return Err("supervisor_completion_identity_invalid");
    }
    completion.commit();
    runtime.trace.supervisor_continued = true;
    resume_kernel(runtime, completion_status)
}

fn resume_kernel(
    runtime: &mut supervision_runtime,
    status: u64,
) -> Result<*mut scheduler_frame, &'static str> {
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("supervision_resume_interrupt_mask_failed");
    }
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3 {
        return Err("supervision_kernel_root_restore_failed");
    }
    if !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
    {
        return Err("supervision_tss_restore_failed");
    }
    let saved_kernel_stack = runtime.saved_kernel_stack;
    let original_task_switched = runtime.original_task_switched;
    if saved_kernel_stack == 0
        || !(saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(original_task_switched, 0 | cr0_task_switched)
        || paging::read_cr0() & cr0_task_switched == 0
    {
        return Err("supervision_continuation_invalid");
    }
    runtime.active_role = None;
    gate_active.store(false, Ordering::Release);
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    unsafe {
        super::scheduler_test::x86_64_scheduler_resume_kernel(
            status,
            saved_kernel_stack,
            original_task_switched,
        )
    }
}

fn mailbox_matches(task: &supervised_task, state: u64) -> bool {
    let (value_a, value_b) = match (task.role, task.incarnation) {
        (supervision_role::vessel, 1) => (vessel_generation_1_value_a, vessel_generation_1_value_b),
        (supervision_role::vessel, 2) => (vessel_generation_2_value_a, vessel_generation_2_value_b),
        (supervision_role::supervisor, 1) => (supervisor_value_a, supervisor_value_b),
        _ => return false,
    };
    read_mailbox_word(task.space.physical_pages[1], mailbox_value_a_offset) == Some(value_a)
        && read_mailbox_word(task.space.physical_pages[1], mailbox_value_b_offset) == Some(value_b)
        && read_mailbox_word(task.space.physical_pages[1], mailbox_state_offset) == Some(state)
}

fn gate_registers_are_exact(frame: scheduler_frame, operation: u64) -> bool {
    frame.accumulator == operation
        && frame.base == 0
        && frame.counter == 0
        && frame.data == 0
        && frame.frame_pointer == 0
        && frame.source_index == 0
        && frame.destination_index == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
        && frame.register_15 == user_mailbox_address
}

fn expected_invalid_opcode_matches(
    snapshot: exceptions::exception_snapshot,
    registers: scheduler_frame,
    instruction_pointer: u64,
) -> bool {
    registers.accumulator == snapshot.accumulator
        && registers.vector == snapshot.vector
        && registers.error_code == snapshot.error_code
        && registers.instruction_pointer == snapshot.instruction_pointer
        && registers.code_segment == snapshot.code_segment
        && registers.flags == snapshot.flags
        && registers.stack_pointer == snapshot.stack_pointer
        && Some(registers.stack_segment) == snapshot.stack_segment
        && snapshot.vector == invalid_opcode_vector
        && snapshot.error_code == 0
        && snapshot.instruction_pointer == instruction_pointer
        && snapshot.code_segment == u64::from(descriptors::user_code_selector)
        && snapshot.stack_pointer == user_stack_top
        && snapshot.stack_segment == Some(u64::from(descriptors::user_data_selector))
        && snapshot.fault_address.is_none()
        && (snapshot.flags == running_user_flags
            || snapshot.flags == running_user_flags | resume_flag)
        && snapshot.accumulator == 0
        && all_general_registers_zero(registers)
}

fn empty_circle_accounting() -> circle_accounting {
    circle_accounting {
        circles: 0,
        tasks: 0,
        task_contexts: 0,
        endpoints: 0,
        sigils: 0,
        supervisions: 0,
        address_spaces: 0,
        mappings: 0,
    }
}

fn emit_evidence(evidence: supervision_evidence) {
    let fault = evidence
        .trace
        .fault
        .unwrap_or_else(|| fail("supervision_fault_evidence_missing"));
    let stack_segment = fault
        .stack_segment
        .unwrap_or_else(|| fail("supervision_fault_stack_missing"));
    let replacement = evidence
        .trace
        .replacement
        .unwrap_or_else(|| fail("supervision_replacement_evidence_missing"));
    let fault_observation = evidence
        .trace
        .fault_observation
        .unwrap_or_else(|| fail("fault_observation_evidence_missing"));
    let completion_observation = evidence
        .trace
        .completion_observation
        .unwrap_or_else(|| fail("completion_observation_evidence_missing"));
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.ready","level":"info","policy":"fixed_supervision","supervision_capacity":1,"relationships":1,"circles":2,"tasks":2,"queue_capacity":2,"restart_budget":1,"observation_capacity":1,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.created","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":1,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":false,"restart_budget_remaining":1,"initial_if":false}}"#,
        evidence.vessel_generation_1_circle, evidence.vessel_generation_1_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.supervisor.ready","level":"info","role":"{}","supervision_generation":1,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}}"#,
        supervision_role::supervisor.label(),
        evidence.supervisor_circle,
        evidence.supervisor_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"vessel_fixture","incarnation":1,"dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[0]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.preempted","level":"info","role":"vessel_fixture","incarnation":1,"preemption":{},"tick":{},"state":"suspended","frame_retained":true,"eoi_sent":true}}"#,
        evidence.trace.preemptions[0], evidence.trace.tick
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"supervisor_fixture","dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[1]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.supervisor.blocked","level":"info","role":"supervisor_fixture","supervision_generation":1,"block":0,"wait":"await_fault","state":"blocked","frame_retained":true,"vessel_resumed":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"vessel_fixture","incarnation":1,"dispatch":{},"resume":0,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[2]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.exception","level":"info","vector":{},"kind":"invalid_opcode","error_code":"{:#018x}","instruction_pointer":"{:#018x}","stack_pointer":"{:#018x}","stack_segment":"{:#06x}","stack_switched":true,"code_segment":"{:#06x}","flags":"{:#018x}","origin":"user","disposition":"contained_test","fault_address":null}}"#,
        fault.vector,
        fault.error_code,
        fault.instruction_pointer,
        fault.stack_pointer,
        stack_segment,
        fault.code_segment,
        fault.flags
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.faulted","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":1,"state":"faulted","outcome":"{}","event_sequence":{},"vector":6,"error_code":0,"observation_published":true,"supervisor_woken":true,"frame_retained":true}}"#,
        supervision_outcome::invalid_instruction_fault.label(),
        fault_observation.event_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":{},"resume":1,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[3]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":{},"incarnation":1,"outcome":"{}","single_use":true,"pending":false}}"#,
        fault_observation.event_sequence(),
        fault_observation.outcome().label()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.restart.requested","level":"info","role":"supervisor_fixture","supervision_generation":1,"restart_sequence":{},"incarnation":1,"block":1,"wait":"restart_checkpoint","state":"blocked","restart_budget_remaining":0,"scheduler_empty":true,"kernel_continuation_restored":true}}"#,
        replacement.restart_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.withdrawn","level":"info","role":"vessel_fixture","incarnation":1,"circle_generation":{},"task_generation":{},"outcome":"invalid_instruction_fault","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true}}"#,
        evidence.vessel_generation_1_circle, evidence.vessel_generation_1_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.created","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":2,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":true,"restart_budget_remaining":0,"initial_if":false}}"#,
        evidence.vessel_generation_2_circle, evidence.vessel_generation_2_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.dispatched","level":"info","role":"vessel_fixture","incarnation":2,"dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[4]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.completed","level":"info","role":"vessel_fixture","supervision_generation":1,"incarnation":2,"state":"exited","outcome":"{}","event_sequence":{},"completion_gate":true,"observation_published":true,"supervisor_woken":true}}"#,
        completion_observation.outcome().label(),
        completion_observation.event_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":{},"resume":2,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[5]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":{},"incarnation":2,"outcome":"{}","single_use":true,"pending":false}}"#,
        completion_observation.event_sequence(),
        completion_observation.outcome().label()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.supervisor.completed","level":"info","role":"supervisor_fixture","state":"exited","completion_gate":true,"vessel_completed":true,"kernel_continuation_restored":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.vessel.generation.withdrawn","level":"info","role":"vessel_fixture","incarnation":2,"circle_generation":{},"task_generation":{},"outcome":"successful_exit","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true}}"#,
        evidence.vessel_generation_2_circle, evidence.vessel_generation_2_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.supervisor.withdrawn","level":"info","role":"supervisor_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"relationship_detached":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"observation_cleared":true,"wait_cleared":true,"relationship_cleared":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}}"#,
        evidence.initial.free, evidence.initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.passed","level":"info","dispatches":6,"preemptions":1,"resumes":{},"blocks":{},"wakes":{},"observations":{},"faults":{},"restarts":{},"completions":{},"clean_replacement":{},"supervisor_continued":{}}}"#,
        evidence.trace.resume_count,
        evidence.trace.block_count,
        evidence.trace.wake_count,
        evidence.trace.observation_count,
        evidence.trace.fault_count,
        evidence.trace.restart_count,
        evidence.trace.completion_count,
        evidence.trace.clean_replacement,
        evidence.trace.supervisor_continued
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervision_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
