//! Fixed scheduled, supervised shell-vessel rite composition proof.

use anomalous_abi::{
    id_128, rite_call, rite_operation, rite_payload_kind, rite_reply, rite_rights, rite_status,
};
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use crate::circle::{circle_accounting, circle_table};
use crate::console::{screen_console, serial_port};
use crate::identity::{circle_id, supervision_id, task_identity};
use crate::input;
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator, physical_range};
use crate::rite::{rite_channel, rite_decision};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::supervision::{
    supervision_observation, supervision_outcome, supervision_phase, supervision_replacement,
    supervision_table,
};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_exit_payload, read_call,
    reply_size, rite_scratch, scratch_is_zero, write_reply,
};
use super::scheduler_test::{
    all_general_registers_zero, attach_task_space, cr0_task_switched, create_task,
    frame_address_matches_stack, initial_user_flags, invalid_opcode_vector, mailbox_is_zero,
    prepare_task_space, privilege_stack_pages, read_mailbox_word, release_task_space,
    relocated_user_address, resume_flag, running_user_flags, scheduler_frame,
    scheduler_gate_vector, timer_vector, user_mailbox_address, user_return_fields_are_valid,
    user_stack_top, write_mailbox_word,
};
use super::{descriptors, exceptions, interrupts, paging};

const shell_vessel_task_count: usize = 2;
const page_size: usize = 4096;
const restart_status: u64 = 1;
const completion_status: u64 = 2;
const supervisor_value_a_offset: u64 = 0;
const supervisor_value_b_offset: u64 = 8;
const supervisor_state_offset: u64 = 16;
const vessel_stack_marker_a_offset: u64 = 0x100;
const vessel_stack_marker_b_offset: u64 = 0x108;
const vessel_stack_arm_offset: u64 = 0x110;
const vessel_generation_1_marker_a: u64 = 0x8f12_c6a4_7bd9_30e1;
const vessel_generation_1_marker_b: u64 = 0x17e4_b9c2_6a5d_803f;
const vessel_generation_1_arm: u64 = 0xa55a_c33c_96e1_7b4d;
const supervisor_value_a: u64 = 0x6d31_a8f2_c47b_950e;
const supervisor_value_b: u64 = 0xe20c_759a_13bf_486d;
const supervisor_continuity: u64 = 0x4b9e_d170_a62c_f835;
const operation_vessel_ready: u64 = 1;
const operation_supervisor_await_fault: u64 = 2;
const operation_supervisor_restart: u64 = 3;
const operation_supervisor_complete: u64 = 5;
const wake_fault: u64 = 0x41;
const wake_completion: u64 = 0x42;
const rite_gate_vector: u64 = 0xf3;
const private_console_text: [u8; 17] = *b"Anomalous ready.\n";
const expected_exit_code: i32 = 0;
const rite_call_count: usize = 5;
const returning_rite_call_count: usize = 4;
const expected_body_copy_count: usize = 3;
const expected_body_bytes: usize = private_console_text.len() * 2 + 8;

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
            Self::vessel => "shell_vessel_fixture",
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum rite_phase {
    generation_1_console,
    generation_1_input,
    generation_1_waiting,
    generation_2_console,
    generation_2_input,
    generation_2_exit,
    idle,
}

impl rite_phase {
    const fn trace(self) -> Option<id_128> {
        let bytes = match self {
            Self::generation_1_console => [
                0xd0, 0xb8, 0x15, 0x63, 0x2e, 0x7a, 0xc4, 0x91, 0x4b, 0x18, 0x7e, 0xc3, 0x05, 0xa9,
                0x6d, 0x2f,
            ],
            Self::generation_1_input => [
                0xe1, 0x90, 0xc6, 0x54, 0x3f, 0x18, 0x7d, 0xa2, 0x5c, 0x29, 0x8f, 0xd4, 0x16, 0xba,
                0x7e, 0x3b,
            ],
            Self::generation_2_console => [
                0xf2, 0xa1, 0xd7, 0x65, 0x40, 0x29, 0x8e, 0xb3, 0x6d, 0x3a, 0x90, 0xe5, 0x27, 0xcb,
                0x8f, 0x4c,
            ],
            Self::generation_2_input => [
                0x03, 0xb2, 0xe8, 0x76, 0x51, 0x3a, 0x9f, 0xc4, 0x7e, 0x4b, 0xa1, 0xf6, 0x38, 0xdc,
                0xa0, 0x5d,
            ],
            Self::generation_2_exit => [
                0x14, 0xc3, 0xf9, 0x87, 0x62, 0x4b, 0xa0, 0xd5, 0x8f, 0x5c, 0xb2, 0x07, 0x49, 0xed,
                0xb1, 0x6e,
            ],
            Self::generation_1_waiting | Self::idle => return None,
        };
        Some(id_128::from_bytes(bytes))
    }

    const fn sequence(self) -> Option<u64> {
        match self {
            Self::generation_1_console | Self::generation_2_console => Some(1),
            Self::generation_1_input | Self::generation_2_input => Some(2),
            Self::generation_2_exit => Some(3),
            Self::generation_1_waiting | Self::idle => None,
        }
    }

    const fn expected_status(self) -> Option<rite_status> {
        match self {
            Self::generation_1_console | Self::generation_2_console | Self::generation_2_exit => {
                Some(rite_status::ok)
            }
            Self::generation_1_input | Self::generation_2_input => Some(rite_status::would_block),
            Self::generation_1_waiting | Self::idle => None,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::generation_1_console => Some(Self::generation_1_input),
            Self::generation_1_input => Some(Self::generation_1_waiting),
            Self::generation_2_console => Some(Self::generation_2_input),
            Self::generation_2_input => Some(Self::generation_2_exit),
            Self::generation_2_exit => Some(Self::idle),
            Self::generation_1_waiting | Self::idle => None,
        }
    }

    const fn incarnation(self) -> Option<u64> {
        match self {
            Self::generation_1_console | Self::generation_1_input | Self::generation_1_waiting => {
                Some(1)
            }
            Self::generation_2_console | Self::generation_2_input | Self::generation_2_exit => {
                Some(2)
            }
            Self::idle => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct console_semantics {
    operation: rite_operation,
    payload_kind: rite_payload_kind,
    rights_present: u64,
    payload_size: u32,
    payload_limit: u32,
    status: rite_status,
}

impl console_semantics {
    const fn from_call(call: rite_call, status: rite_status) -> Self {
        Self {
            operation: call.operation,
            payload_kind: call.payload_kind,
            rights_present: call.rights_present,
            payload_size: call.payload_size,
            payload_limit: call.payload_limit,
            status,
        }
    }
}

#[derive(Clone, Copy)]
struct rite_observation {
    incarnation: u64,
    sequence: u64,
    status: rite_status,
}

impl rite_observation {
    const fn empty() -> Self {
        Self {
            incarnation: 0,
            sequence: 0,
            status: rite_status::faulted,
        }
    }
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
    rite_observations: [rite_observation; rite_call_count],
    rite_observation_count: usize,
    body_copy_count: usize,
    body_bytes_copied: usize,
    reply_copy_count: usize,
    console_commit_count: usize,
    would_block_count: usize,
    terminal_exit_count: usize,
    channel_retire_count: usize,
    transport_wipe_count: usize,
    semantic_equal: bool,
    terminal_reply_copied: bool,
    terminal_supervisor_selected: bool,
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
            rite_observations: [rite_observation::empty(); rite_call_count],
            rite_observation_count: 0,
            body_copy_count: 0,
            body_bytes_copied: 0,
            reply_copy_count: 0,
            console_commit_count: 0,
            would_block_count: 0,
            terminal_exit_count: 0,
            channel_retire_count: 0,
            transport_wipe_count: 0,
            semantic_equal: false,
            terminal_reply_copied: false,
            terminal_supervisor_selected: false,
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
    channel: Option<rite_channel>,
    screen: screen_console,
    rite_phase: rite_phase,
    scratch: rite_scratch,
    generation_1_semantics: Option<console_semantics>,
    generation_1_console: [u8; private_console_text.len()],
    generation_2_console: [u8; private_console_text.len()],
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
static rite_gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_shell_vessel_generation_1_start: u8;
    static x86_64_shell_vessel_generation_1_console_return: u8;
    static x86_64_shell_vessel_generation_1_input_return: u8;
    static x86_64_shell_vessel_generation_1_ready_return: u8;
    static x86_64_shell_vessel_generation_1_fault_instruction: u8;
    static x86_64_shell_vessel_generation_1_end: u8;
    static x86_64_shell_vessel_supervisor_start: u8;
    static x86_64_shell_vessel_supervisor_fault_return: u8;
    static x86_64_shell_vessel_supervisor_completion_return: u8;
    static x86_64_shell_vessel_supervisor_complete_return: u8;
    static x86_64_shell_vessel_supervisor_end: u8;
    static x86_64_shell_vessel_generation_2_start: u8;
    static x86_64_shell_vessel_generation_2_console_return: u8;
    static x86_64_shell_vessel_generation_2_input_return: u8;
    static x86_64_shell_vessel_generation_2_exit_return: u8;
    static x86_64_shell_vessel_generation_2_end: u8;
}

pub(super) fn run(mut screen: screen_console) -> ! {
    screen.initialize();
    if !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_shell_vessel_state_invalid");
    }

    let kernel_cr3 = paging::read_cr3();
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(move |allocator| {
        run_with_allocator(allocator, kernel_cr3, bootstrap_privilege_stack_top, screen)
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
    screen: screen_console,
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
        ptr::addr_of!(x86_64_shell_vessel_generation_1_start),
        ptr::addr_of!(x86_64_shell_vessel_generation_1_end),
    )?;
    let supervisor_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_shell_vessel_supervisor_start),
        ptr::addr_of!(x86_64_shell_vessel_supervisor_end),
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
    let channel = rite_channel::try_new(vessel_identity, rite_rights::shell_minimum)
        .map_err(|_| "shell_vessel_channel_create_failed")?;
    let mut scheduler = scheduler_table::new(shell_vessel_task_count)
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
        channel: Some(channel),
        screen,
        rite_phase: rite_phase::generation_1_console,
        scratch: [0; page_size],
        generation_1_semantics: None,
        generation_1_console: [0; private_console_text.len()],
        generation_2_console: [0; private_console_text.len()],
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
        || rite_gate_active.load(Ordering::Acquire)
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
        || runtime.rite_phase != rite_phase::generation_1_waiting
        || runtime.trace.rite_observation_count != 2
        || runtime.trace.rite_observations[0].incarnation != 1
        || runtime.trace.rite_observations[0].sequence != 1
        || runtime.trace.rite_observations[0].status != rite_status::ok
        || runtime.trace.rite_observations[1].incarnation != 1
        || runtime.trace.rite_observations[1].sequence != 2
        || runtime.trace.rite_observations[1].status != rite_status::would_block
        || runtime.trace.body_copy_count != 1
        || runtime.trace.body_bytes_copied != private_console_text.len()
        || runtime.trace.reply_copy_count != 2
        || runtime.trace.console_commit_count != 1
        || runtime.trace.would_block_count != 1
        || runtime.trace.terminal_exit_count != 0
        || runtime.trace.channel_retire_count != 0
        || !scratch_is_zero(&runtime.scratch)
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
        || rite_gate_active.load(Ordering::Acquire)
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
        || runtime.rite_phase != rite_phase::idle
        || runtime.trace.rite_observation_count != rite_call_count
        || runtime.trace.body_copy_count != expected_body_copy_count
        || runtime.trace.body_bytes_copied != expected_body_bytes
        || runtime.trace.reply_copy_count != returning_rite_call_count
        || runtime.trace.console_commit_count != 2
        || runtime.trace.would_block_count != 2
        || runtime.trace.terminal_exit_count != 1
        || runtime.trace.channel_retire_count != 1
        || runtime.trace.transport_wipe_count != 1
        || !runtime.trace.semantic_equal
        || runtime.trace.terminal_reply_copied
        || !runtime.trace.terminal_supervisor_selected
        || runtime.generation_1_console != private_console_text
        || runtime.generation_2_console != private_console_text
        || !scratch_is_zero(&runtime.scratch)
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
    if runtime.trace.channel_retire_count != 2
        || runtime.trace.transport_wipe_count != 2
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("shell_vessel_final_privacy_state_invalid");
    }
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
    retire_rite_channel(runtime, identity, 1)?;
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
    runtime.trace.transport_wipe_count += 1;
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

fn retire_rite_channel(
    runtime: &mut supervision_runtime,
    identity: task_identity,
    incarnation: u64,
) -> Result<(), &'static str> {
    if runtime.vessel_ref()?.identity != identity
        || runtime.vessel_ref()?.incarnation != incarnation
        || runtime.channel.take().is_none()
        || runtime.trace.channel_retire_count + 1 != incarnation as usize
    {
        return Err("shell_vessel_channel_retirement_invalid");
    }
    runtime.trace.channel_retire_count += 1;
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
        ptr::addr_of!(x86_64_shell_vessel_generation_2_start),
        ptr::addr_of!(x86_64_shell_vessel_generation_2_end),
    )?;
    if space.physical_pages != expected_extents.physical_pages
        || space
            .root
            .root_address()
            .map_err(|_| "replacement_root_missing")?
            != expected_extents.root
        || space.privilege_stack_range != expected_extents.privilege_stack
        || !replacement_transport_is_zero(&space)
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
    let channel = rite_channel::try_new(identity, rite_rights::shell_minimum)
        .map_err(|_| "replacement_rite_channel_create_failed")?;
    if runtime.channel.is_some() || runtime.rite_phase != rite_phase::generation_1_waiting {
        return Err("replacement_rite_state_invalid");
    }
    runtime.vessel = Some(supervised_task {
        circle,
        identity,
        space,
        ready_seen: false,
        incarnation: 2,
        role: supervision_role::vessel,
    });
    runtime.channel = Some(channel);
    runtime.rite_phase = rite_phase::generation_2_console;
    Ok(identity)
}

fn replacement_transport_is_zero(space: &super::scheduler_test::task_space) -> bool {
    let mailbox_zero = (0..8)
        .map(|index| index * mem::size_of::<u64>() as u64)
        .all(|offset| read_mailbox_word(space.physical_pages[1], offset) == Some(0));
    let stack_zero = [
        vessel_stack_marker_a_offset,
        vessel_stack_marker_b_offset,
        vessel_stack_arm_offset,
    ]
    .into_iter()
    .all(|offset| read_mailbox_word(space.physical_pages[2], offset) == Some(0));
    mailbox_is_zero(space) && mailbox_zero && stack_zero
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
    retire_rite_channel(runtime, vessel_identity, 2)?;
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
    runtime.trace.transport_wipe_count += 1;
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
        ptr::write_volatile(ptr::addr_of_mut!(runtime.rite_phase), rite_phase::idle);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.generation_1_semantics), None);
        ptr::write_volatile(
            ptr::addr_of_mut!(runtime.generation_1_console),
            [0; private_console_text.len()],
        );
        ptr::write_volatile(
            ptr::addr_of_mut!(runtime.generation_2_console),
            [0; private_console_text.len()],
        );
        ptr::write_volatile(ptr::addr_of_mut!(runtime.scratch), [0; page_size]);
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

pub(super) fn shell_vessel_is_active() -> bool {
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
        || runtime.rite_phase != rite_phase::generation_1_waiting
        || !vessel_stack_matches(runtime.vessel_ref()?, 0)
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
        runtime.vessel_ref()?.space.physical_pages[2],
        vessel_stack_arm_offset,
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
        || runtime.rite_phase != rite_phase::generation_1_waiting
        || !vessel_stack_matches(runtime.vessel_ref()?, vessel_generation_1_arm)
    {
        return Err("supervision_fault_state_invalid");
    }
    let expected_instruction = relocated_user_address(
        ptr::addr_of!(x86_64_shell_vessel_generation_1_start),
        ptr::addr_of!(x86_64_shell_vessel_generation_1_fault_instruction),
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
                ptr::addr_of!(x86_64_shell_vessel_generation_1_start),
                ptr::addr_of!(x86_64_shell_vessel_generation_1_ready_return),
                runtime.vessel_ref()?.space.payload_length,
            )
            .ok_or("vessel_ready_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_vessel_ready)
        || runtime.rite_phase != rite_phase::generation_1_waiting
        || !vessel_stack_matches(runtime.vessel_ref()?, 0)
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
                ptr::addr_of!(x86_64_shell_vessel_supervisor_start),
                ptr::addr_of!(x86_64_shell_vessel_supervisor_fault_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_fault_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_await_fault)
        || !supervisor_mailbox_matches(&runtime.supervisor, 0)
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
                ptr::addr_of!(x86_64_shell_vessel_supervisor_start),
                ptr::addr_of!(x86_64_shell_vessel_supervisor_completion_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_completion_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_restart)
        || !supervisor_mailbox_matches(&runtime.supervisor, supervisor_continuity)
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

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_shell_vessel_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || rite_gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("shell_vessel_rite_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("shell_vessel_rite_gate_without_runtime");
    }
    // SAFETY: The published pointer names the unique live kernel-stack runtime.
    // The interrupt gate cleared IF, NMI remains masked, and the separate rite
    // latch excludes both reentry and concurrent mutation on this BSP.
    let runtime = unsafe { &mut *runtime_pointer };
    match dispatch_rite(runtime, frame) {
        Ok(next) => {
            rite_gate_active.store(false, Ordering::Release);
            next
        }
        Err(reason) => fail(reason),
    }
}

fn dispatch_rite(
    runtime: &mut supervision_runtime,
    frame: *mut scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let phase = runtime.rite_phase;
    let vessel = runtime.vessel_ref()?.identity;
    let incarnation = runtime.vessel_ref()?.incarnation;
    if runtime.current_role() != Some(supervision_role::vessel)
        || phase.incarnation() != Some(incarnation)
        || !runtime.frame_is_valid_for_role(frame, supervision_role::vessel, true)
    {
        return Err("shell_vessel_rite_runtime_state_invalid");
    }
    // SAFETY: The complete frame belongs to the validated current vessel and
    // remains live until the selected return frame is handed to assembly.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, 0)
        || expected_rite_return(runtime, phase) != Some(incoming.instruction_pointer)
    {
        return Err("shell_vessel_rite_frame_invalid");
    }

    let mailbox_alias = runtime.vessel_ref()?.space.physical_pages[1];
    // SAFETY: The validated current task owns this live one-page mailbox; the
    // gate excludes user execution and all other mutation for the exact copy.
    let call = unsafe { read_call(mailbox_alias) }?;
    validate_rite_call(phase, call)?;
    let reply_copies_before = runtime.trace.reply_copy_count;

    let (reply, terminal_observation) = {
        let channel = runtime
            .channel
            .as_mut()
            .ok_or("shell_vessel_rite_channel_missing")?;
        let decision = channel
            .assess(&runtime.tasks, vessel, call)
            .map_err(|_| "shell_vessel_rite_assessment_failed")?;
        let admission = match decision {
            rite_decision::admitted(admission) => admission,
            rite_decision::rejected(_) => {
                return Err("shell_vessel_fixed_rite_was_rejected");
            }
        };

        match phase {
            rite_phase::generation_1_console | rite_phase::generation_2_console => {
                // SAFETY: Admission fixed the copied prefix before body access.
                // The physical alias is the current vessel's allocator-owned
                // mailbox and remains mutation-excluded by the gate.
                unsafe {
                    copy_admitted_body(
                        mailbox_alias,
                        user_mailbox_address,
                        admission.payload_size(),
                        &mut runtime.scratch,
                        &mut runtime.trace.body_copy_count,
                        &mut runtime.trace.body_bytes_copied,
                    )
                }?;
                let length = usize::try_from(admission.payload_size())
                    .map_err(|_| "shell_vessel_console_length_invalid")?;
                let body = runtime
                    .scratch
                    .get(..length)
                    .ok_or("shell_vessel_console_body_out_of_bounds")?;
                let text =
                    core::str::from_utf8(body).map_err(|_| "shell_vessel_console_utf8_invalid")?;
                if text.as_bytes() != private_console_text
                    || !text.as_bytes()[..private_console_text.len() - 1]
                        .iter()
                        .all(|byte| (b' '..=b'~').contains(byte))
                    || text.as_bytes()[private_console_text.len() - 1] != b'\n'
                {
                    return Err("shell_vessel_console_body_invalid");
                }

                let semantics = console_semantics::from_call(call, rite_status::ok);
                match phase {
                    rite_phase::generation_1_console => {
                        if runtime.generation_1_semantics.is_some()
                            || runtime.generation_1_console != [0; private_console_text.len()]
                        {
                            return Err("shell_vessel_human_projection_repeated");
                        }
                    }
                    rite_phase::generation_2_console => {
                        if runtime.generation_1_semantics != Some(semantics)
                            || runtime.generation_1_console.as_slice() != text.as_bytes()
                            || runtime.generation_2_console != [0; private_console_text.len()]
                        {
                            return Err("shell_vessel_machine_projection_not_equal");
                        }
                    }
                    _ => return Err("shell_vessel_console_phase_invalid"),
                }

                let exact_vessel = admission.vessel();
                let destination = match phase {
                    rite_phase::generation_1_console => &mut runtime.generation_1_console,
                    rite_phase::generation_2_console => &mut runtime.generation_2_console,
                    _ => return Err("shell_vessel_console_phase_invalid"),
                };
                if runtime.trace.console_commit_count >= 2 {
                    return Err("shell_vessel_console_commit_overflow");
                }
                let screen = &mut runtime.screen;
                let console_commit_count = &mut runtime.trace.console_commit_count;
                let reply = admission
                    .finish_ok(&mut runtime.tasks, |_, committed_vessel| {
                        if committed_vessel != exact_vessel {
                            fail("shell_vessel_console_identity_changed");
                        }
                        destination.copy_from_slice(text.as_bytes());
                        if screen.write_str(text).is_err() {
                            fail("shell_vessel_console_commit_failed");
                        }
                        *console_commit_count += 1;
                    })
                    .map_err(|_| "shell_vessel_console_completion_failed")?;
                if phase == rite_phase::generation_1_console {
                    runtime.generation_1_semantics = Some(semantics);
                } else {
                    runtime.trace.semantic_equal = true;
                }
                if !clear_and_verify_bytes(&mut runtime.scratch) {
                    return Err("shell_vessel_console_scratch_clear_failed");
                }
                (reply, None)
            }
            rite_phase::generation_1_input | rite_phase::generation_2_input => {
                // SAFETY: The gate holds IF clear on the only processor and the
                // proof masked IRQ1 before user execution, so readiness cannot
                // change during this empty-input decision.
                if unsafe { input::ps2_delivery_ready_if_disabled() } {
                    return Err("shell_vessel_input_not_empty");
                }
                let reply = admission
                    .finish_would_block(&runtime.tasks)
                    .map_err(|_| "shell_vessel_input_completion_failed")?;
                runtime.trace.would_block_count += 1;
                (reply, None)
            }
            rite_phase::generation_2_exit => {
                // SAFETY: The admitted fixed eight-byte body remains in the
                // current allocator-owned mailbox under the same gate exclusion.
                unsafe {
                    copy_admitted_body(
                        mailbox_alias,
                        user_mailbox_address,
                        admission.payload_size(),
                        &mut runtime.scratch,
                        &mut runtime.trace.body_copy_count,
                        &mut runtime.trace.body_bytes_copied,
                    )
                }?;
                let payload = decode_exit_payload(&runtime.scratch)?;
                if !payload.has_supported_body() || payload.code != expected_exit_code {
                    return Err("shell_vessel_exit_body_invalid");
                }

                let supervisor = runtime.supervisor.identity;
                let relationship = runtime.relationship;
                let publication = runtime
                    .supervisions
                    .prepare_publication(
                        &runtime.tasks,
                        relationship,
                        vessel,
                        supervision_outcome::successful_exit,
                    )
                    .map_err(|_| "shell_vessel_completion_publication_preflight_failed")?;
                let expected_observation = publication.observation();
                if publication.supervisor() != supervisor
                    || expected_observation.event_sequence() != 2
                    || expected_observation.incarnation() != vessel
                    || expected_observation.outcome() != supervision_outcome::successful_exit
                {
                    return Err("shell_vessel_completion_publication_invalid");
                }

                let scheduler = &mut runtime.scheduler;
                let mut terminal_dispatch_sequence = None;
                let reply = admission
                    .finish_ok(&mut runtime.tasks, |tasks, committed_vessel| {
                        if committed_vessel != vessel {
                            fail("shell_vessel_exit_identity_changed");
                        }
                        let transition = scheduler
                            .complete_current_and_wake(tasks, committed_vessel, supervisor)
                            .unwrap_or_else(|_| fail("shell_vessel_completion_wake_failed"));
                        if transition.task() != vessel
                            || transition.dispatch().task() != supervisor
                            || transition.dispatch().sequence() != 5
                            || transition.dispatch().context() != scheduler_context::saved
                        {
                            fail("shell_vessel_completion_wake_invalid");
                        }
                        terminal_dispatch_sequence = Some(transition.dispatch().sequence());
                        publication.commit();
                    })
                    .map_err(|_| "shell_vessel_exit_completion_failed")?;
                if terminal_dispatch_sequence != Some(5)
                    || !clear_and_verify_bytes(&mut runtime.scratch)
                {
                    return Err("shell_vessel_terminal_commit_invalid");
                }
                let observation = runtime
                    .supervisions
                    .take_observation(&runtime.tasks, relationship, supervisor)
                    .map_err(|_| "completion_observation_consume_failed")?;
                if observation != expected_observation {
                    return Err("completion_observation_invalid");
                }
                (reply, Some(observation))
            }
            rite_phase::generation_1_waiting | rite_phase::idle => {
                return Err("shell_vessel_rite_idle_phase");
            }
        }
    };

    validate_rite_reply(phase, reply)?;
    record_rite_observation(&mut runtime.trace, phase, reply)?;
    runtime.rite_phase = phase.next().ok_or("shell_vessel_rite_phase_overflow")?;

    if let Some(observation) = terminal_observation {
        runtime.trace.terminal_reply_copied = runtime.trace.reply_copy_count != reply_copies_before;
        if runtime.trace.terminal_reply_copied
            || runtime.rite_phase != rite_phase::idle
            || runtime
                .tasks
                .snapshot(vessel)
                .map_err(|_| "shell_vessel_terminal_snapshot_failed")?
                .state()
                != task_state::exited
        {
            return Err("shell_vessel_terminal_state_invalid");
        }
        runtime.trace.completion_observation = Some(observation);
        runtime.trace.dispatches[5] = 5;
        runtime.trace.wake_count = 2;
        runtime.trace.observation_count = 2;
        runtime.trace.resume_count = 3;
        runtime.trace.completion_count = 1;
        runtime.trace.terminal_exit_count = 1;
        let next = runtime.select_role(supervision_role::supervisor, true)?;
        if next == frame {
            return Err("shell_vessel_terminal_frame_reused");
        }
        // SAFETY: The selected frame belongs to the exact awakened supervisor;
        // the fixed wake result is its only register-level observation.
        unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_completion) };
        runtime.trace.terminal_supervisor_selected = true;
        compiler_fence(Ordering::SeqCst);
        return Ok(next);
    }

    // SAFETY: This is the same live, exclusive physical mailbox alias validated
    // for the request. Every reply byte, including reserved_2, was initialized.
    unsafe { write_reply(mailbox_alias, reply) }?;
    runtime.trace.reply_copy_count += 1;
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn expected_rite_return(runtime: &supervision_runtime, phase: rite_phase) -> Option<u64> {
    let (start, target) = match phase {
        rite_phase::generation_1_console => (
            ptr::addr_of!(x86_64_shell_vessel_generation_1_start),
            ptr::addr_of!(x86_64_shell_vessel_generation_1_console_return),
        ),
        rite_phase::generation_1_input => (
            ptr::addr_of!(x86_64_shell_vessel_generation_1_start),
            ptr::addr_of!(x86_64_shell_vessel_generation_1_input_return),
        ),
        rite_phase::generation_2_console => (
            ptr::addr_of!(x86_64_shell_vessel_generation_2_start),
            ptr::addr_of!(x86_64_shell_vessel_generation_2_console_return),
        ),
        rite_phase::generation_2_input => (
            ptr::addr_of!(x86_64_shell_vessel_generation_2_start),
            ptr::addr_of!(x86_64_shell_vessel_generation_2_input_return),
        ),
        rite_phase::generation_2_exit => (
            ptr::addr_of!(x86_64_shell_vessel_generation_2_start),
            ptr::addr_of!(x86_64_shell_vessel_generation_2_exit_return),
        ),
        rite_phase::generation_1_waiting | rite_phase::idle => return None,
    };
    relocated_user_address(
        start,
        target,
        runtime.vessel_ref().ok()?.space.payload_length,
    )
}

fn validate_rite_call(phase: rite_phase, call: rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.reserved != 0
        || !super::rite_transport::payload_span_fits(user_mailbox_address, call.payload_size)
        || call.trace_id != phase.trace().ok_or("shell_vessel_rite_trace_missing")?
    {
        return Err("shell_vessel_rite_call_header_invalid");
    }
    let exact = match phase {
        rite_phase::generation_1_console | rite_phase::generation_2_console => {
            call.operation == rite_operation::console_write
                && call.payload_kind == rite_payload_kind::utf8_text
                && call.rights_present == rite_rights::console_write
                && call.payload_size == private_console_text.len() as u32
                && call.payload_limit == private_console_text.len() as u32
        }
        rite_phase::generation_1_input | rite_phase::generation_2_input => {
            call.operation == rite_operation::input_read
                && call.payload_kind == rite_payload_kind::input_request
                && call.rights_present == rite_rights::input_read
                && call.payload_size == 0
                && call.payload_limit == 0
        }
        rite_phase::generation_2_exit => {
            call.operation == rite_operation::task_exit
                && call.payload_kind == rite_payload_kind::exit_status
                && call.rights_present == rite_rights::task_exit
                && call.payload_size == 8
                && call.payload_limit == 8
        }
        rite_phase::generation_1_waiting | rite_phase::idle => false,
    };
    if !exact {
        return Err("shell_vessel_rite_call_shape_invalid");
    }
    Ok(())
}

fn validate_rite_reply(phase: rite_phase, reply: rite_reply) -> Result<(), &'static str> {
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status
            != phase
                .expected_status()
                .ok_or("shell_vessel_rite_status_missing")?
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence
            != phase
                .sequence()
                .ok_or("shell_vessel_rite_sequence_missing")?
        || reply.trace_id != phase.trace().ok_or("shell_vessel_rite_trace_missing")?
    {
        return Err("shell_vessel_rite_reply_invalid");
    }
    Ok(())
}

fn record_rite_observation(
    trace: &mut supervision_trace,
    phase: rite_phase,
    reply: rite_reply,
) -> Result<(), &'static str> {
    let expected_index = match phase {
        rite_phase::generation_1_console => 0,
        rite_phase::generation_1_input => 1,
        rite_phase::generation_2_console => 2,
        rite_phase::generation_2_input => 3,
        rite_phase::generation_2_exit => 4,
        rite_phase::generation_1_waiting | rite_phase::idle => {
            return Err("shell_vessel_rite_observation_phase_invalid");
        }
    };
    if trace.rite_observation_count != expected_index {
        return Err("shell_vessel_rite_observation_order_invalid");
    }
    trace.rite_observations[expected_index] = rite_observation {
        incarnation: phase
            .incarnation()
            .ok_or("shell_vessel_rite_incarnation_missing")?,
        sequence: reply.sequence,
        status: reply.status,
    };
    trace.rite_observation_count += 1;
    Ok(())
}

fn finish_supervisor_completion(
    runtime: &mut supervision_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.trace.completion_count != 1
        || incoming.instruction_pointer
            != relocated_user_address(
                ptr::addr_of!(x86_64_shell_vessel_supervisor_start),
                ptr::addr_of!(x86_64_shell_vessel_supervisor_complete_return),
                runtime.supervisor.space.payload_length,
            )
            .ok_or("supervisor_complete_return_invalid")?
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !gate_registers_are_exact(incoming, operation_supervisor_complete)
        || !supervisor_mailbox_matches(&runtime.supervisor, supervisor_continuity)
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

fn supervisor_mailbox_matches(task: &supervised_task, state: u64) -> bool {
    task.role == supervision_role::supervisor
        && task.incarnation == 1
        && read_mailbox_word(task.space.physical_pages[1], supervisor_value_a_offset)
            == Some(supervisor_value_a)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_value_b_offset)
            == Some(supervisor_value_b)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_state_offset) == Some(state)
}

fn vessel_stack_matches(task: &supervised_task, arm: u64) -> bool {
    task.role == supervision_role::vessel
        && task.incarnation == 1
        && read_mailbox_word(task.space.physical_pages[2], vessel_stack_marker_a_offset)
            == Some(vessel_generation_1_marker_a)
        && read_mailbox_word(task.space.physical_pages[2], vessel_stack_marker_b_offset)
            == Some(vessel_generation_1_marker_b)
        && read_mailbox_word(task.space.physical_pages[2], vessel_stack_arm_offset) == Some(arm)
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
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.ready","level":"info","arch":"x86_64","policy":"fixed_supervised_shell","supervision_capacity":1,"relationships":1,"circles":2,"tasks":2,"queue_capacity":2,"restart_budget":1,"observation_capacity":1,"scheduler_gate_vector":242,"rite_gate_vector":243,"rite_grant":7,"transport_limit":4056,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.created","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":1,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":false,"restart_budget_remaining":1,"rite_grant":7,"rite_sequence_start":1,"transport_zero":true,"initial_if":false}}"#,
        evidence.vessel_generation_1_circle, evidence.vessel_generation_1_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.supervisor.ready","level":"info","role":"{}","supervision_generation":1,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}}"#,
        supervision_role::supervisor.label(),
        evidence.supervisor_circle,
        evidence.supervisor_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"shell_vessel_fixture","incarnation":1,"dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[0]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.rite.console.committed","level":"info","incarnation":{},"sequence":{},"status":"ok","bytes":{},"projection":"fixed_human_source"}}"#,
        evidence.trace.rite_observations[0].incarnation,
        evidence.trace.rite_observations[0].sequence,
        private_console_text.len()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.rite.input.empty","level":"info","incarnation":{},"sequence":{},"status":"would_block","state_unchanged":true}}"#,
        evidence.trace.rite_observations[1].incarnation,
        evidence.trace.rite_observations[1].sequence
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.preempted","level":"info","role":"shell_vessel_fixture","incarnation":1,"preemption":{},"tick":{},"state":"suspended","frame_retained":true,"eoi_sent":true}}"#,
        evidence.trace.preemptions[0], evidence.trace.tick
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"supervisor_fixture","dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[1]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.supervisor.blocked","level":"info","role":"supervisor_fixture","supervision_generation":1,"block":0,"wait":"await_fault","state":"blocked","frame_retained":true,"vessel_resumed":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"shell_vessel_fixture","incarnation":1,"dispatch":{},"resume":0,"state":"running","context_preserved":true,"frame_owned":true}}"#,
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
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.faulted","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":1,"state":"faulted","outcome":"{}","event_sequence":{},"vector":6,"error_code":0,"observation_published":true,"supervisor_woken":true,"frame_retained":true}}"#,
        supervision_outcome::invalid_instruction_fault.label(),
        fault_observation.event_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":{},"resume":1,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[3]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":{},"incarnation":1,"outcome":"{}","single_use":true,"pending":false}}"#,
        fault_observation.event_sequence(),
        fault_observation.outcome().label()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.restart.requested","level":"info","role":"supervisor_fixture","supervision_generation":1,"restart_sequence":{},"incarnation":1,"block":1,"wait":"restart_checkpoint","state":"blocked","restart_budget_remaining":0,"scheduler_empty":true,"kernel_continuation_restored":true}}"#,
        replacement.restart_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.withdrawn","level":"info","role":"shell_vessel_fixture","incarnation":1,"circle_generation":{},"task_generation":{},"outcome":"invalid_instruction_fault","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true,"channel_retired":true,"transport_wiped":true}}"#,
        evidence.vessel_generation_1_circle, evidence.vessel_generation_1_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.created","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":2,"circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"storage_zero":true,"replacement":true,"restart_budget_remaining":0,"rite_grant":7,"rite_sequence_start":1,"transport_zero":true,"initial_if":false}}"#,
        evidence.vessel_generation_2_circle, evidence.vessel_generation_2_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.dispatched","level":"info","role":"shell_vessel_fixture","incarnation":2,"dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[4]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.rite.console.committed","level":"info","incarnation":{},"sequence":{},"status":"ok","bytes":{},"projection":"machine_direct","semantic_equal":{}}}"#,
        evidence.trace.rite_observations[2].incarnation,
        evidence.trace.rite_observations[2].sequence,
        private_console_text.len(),
        evidence.trace.semantic_equal
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.rite.input.empty","level":"info","incarnation":{},"sequence":{},"status":"would_block","state_unchanged":true}}"#,
        evidence.trace.rite_observations[3].incarnation,
        evidence.trace.rite_observations[3].sequence
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.rite.task.exited","level":"info","incarnation":{},"sequence":{},"status":"ok","state":"exited","returned_from_user":true,"reply_copied":{},"iret_to_exited_vessel":false,"supervisor_selected":{}}}"#,
        evidence.trace.rite_observations[4].incarnation,
        evidence.trace.rite_observations[4].sequence,
        evidence.trace.terminal_reply_copied,
        evidence.trace.terminal_supervisor_selected
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.completed","level":"info","role":"shell_vessel_fixture","supervision_generation":1,"incarnation":2,"state":"exited","outcome":"{}","event_sequence":{},"completion_gate":true,"observation_published":true,"supervisor_woken":true}}"#,
        completion_observation.outcome().label(),
        completion_observation.event_sequence()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.task.resumed","level":"info","role":"supervisor_fixture","dispatch":{},"resume":2,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[5]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.observation.consumed","level":"info","role":"supervisor_fixture","supervision_generation":1,"event_sequence":{},"incarnation":2,"outcome":"{}","single_use":true,"pending":false}}"#,
        completion_observation.event_sequence(),
        completion_observation.outcome().label()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.supervisor.completed","level":"info","role":"supervisor_fixture","state":"exited","completion_gate":true,"vessel_completed":true,"kernel_continuation_restored":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.vessel.generation.withdrawn","level":"info","role":"shell_vessel_fixture","incarnation":2,"circle_generation":{},"task_generation":{},"outcome":"successful_exit","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"task_identity_stale":true,"circle_identity_stale":true,"channel_retired":true,"transport_wiped":true}}"#,
        evidence.vessel_generation_2_circle, evidence.vessel_generation_2_task
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.supervisor.withdrawn","level":"info","role":"supervisor_fixture","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true,"relationship_detached":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"active_rite_channels":0,"observation_cleared":true,"wait_cleared":true,"relationship_cleared":true,"channels_retired":true,"transport_wiped":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}}"#,
        evidence.initial.free, evidence.initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.passed","level":"info","dispatches":6,"preemptions":1,"resumes":{},"blocks":{},"wakes":{},"observations":{},"faults":{},"restarts":{},"completions":{},"rite_calls":5,"console_commits":2,"would_block":2,"terminal_exits":1,"sequence_reset":true,"clean_replacement":{},"supervisor_continued":{}}}"#,
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
        r#"{{"schema":1,"event":"kernel.shell_vessel_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
