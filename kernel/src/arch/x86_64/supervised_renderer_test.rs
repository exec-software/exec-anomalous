//! Fixed two-generation supervised semantic-renderer composition proof.

use anomalous_abi::{
    id_128, rite_call, rite_operation, rite_payload_kind, rite_reply, rite_rights, rite_status,
};
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use crate::boot_omen::{color_channel, framebuffer_info};
use crate::circle::{circle_accounting, circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::serial_port;
use crate::display::{
    framebuffer_presenter, rgb8_frame_byte_length, rgb8_frame_height, rgb8_frame_row_bytes,
    rgb8_frame_stage, rgb8_frame_width,
};
use crate::identity::{circle_id, fixture_principal_id, supervision_id, task_identity};
use crate::memory;
use crate::memory::physical::{
    frame_accounting, frame_allocator, frame_lease, frame_purpose, frame_size, physical_range,
};
use crate::rite::{rite_channel, rite_decision};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::supervision::{
    supervision_observation, supervision_outcome, supervision_phase, supervision_replacement,
    supervision_table,
};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};
use crate::time::monotonic_instant;

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_display_patch,
    decode_exit_payload, display_patch_pixels, read_call, reply_size, rite_scratch,
    scratch_is_zero, write_reply,
};
use super::scheduler_test::{
    attach_task_space, cr0_task_switched, frame_address_matches_stack, initial_user_flags,
    invalid_opcode_vector, prepare_task_space, privilege_stack_pages, read_mailbox_word,
    release_task_space, relocated_user_address, resume_flag, running_user_flags, scheduler_frame,
    scheduler_gate_vector, timer_vector, user_mailbox_address as supervisor_mailbox_address,
    user_stack_top as supervisor_stack_top,
};
use super::{descriptors, exceptions, interrupts, paging};

const page_size: u64 = 4096;
const renderer_image_address: u64 = 0x0000_6000_0000_1000;
const renderer_generation_2_address: u64 = renderer_image_address + 0x200;
const renderer_mailbox_address: u64 = 0x0000_6000_0008_0000;
const renderer_stack_address: u64 = 0x0000_6000_0009_0000;
const renderer_stack_pages: usize = 4;
const renderer_stack_top: u64 = renderer_stack_address + renderer_stack_pages as u64 * page_size;
const renderer_image_pages: usize = 2;
const renderer_image_page_limit: usize = 64;
const stage_frame_count: usize = 576;
const renderer_task_count: usize = 2;

const rite_gate_vector: u64 = 0xf3;
const renderer_grant: u64 = rite_rights::display_update | rite_rights::task_exit;
const display_payload_size: u32 = 3112;
const exit_payload_size: u32 = 8;
const generation_1_rows: usize = 384;
const generation_2_rows: usize = 768;
const expected_exit_code: i32 = 0;
const expected_fault_flags: u64 = 0x46;
const restart_status: u64 = 1;
const completion_status: u64 = 2;

const mailbox_canary_a_offset: u64 = 0;
const mailbox_canary_b_offset: u64 = 8;
const mailbox_arm_offset: u64 = 16;
const generation_1_canary_a: u64 = 0x72d1_5ca8_4e30_b691;
const generation_1_canary_b: u64 = 0xc438_9fe2_17a6_5bd0;
const generation_1_arm: u64 = 0xa671_3dc9_e254_8b0f;

const supervisor_value_a_offset: u64 = 0;
const supervisor_value_b_offset: u64 = 8;
const supervisor_state_offset: u64 = 16;
const supervisor_value_a: u64 = 0x196b_f3a0_4d72_c85e;
const supervisor_value_b: u64 = 0xd4a7_0e5b_832c_61f9;
const supervisor_continuity: u64 = 0x5ec1_28a7_f40d_936b;
const operation_renderer_ready: u64 = 1;
const operation_supervisor_await_fault: u64 = 2;
const operation_supervisor_restart: u64 = 3;
const operation_supervisor_complete: u64 = 5;
const wake_fault: u64 = 0x41;
const wake_completion: u64 = 0x42;

const _: () = {
    assert!(frame_size == page_size);
    assert!(privilege_stack_pages == 4);
    assert!(descriptors::privilege_stack_size == 16 * 1024);
    assert!(renderer_stack_pages == 4);
    assert!(renderer_grant == 0x14);
    assert!(rgb8_frame_width == 1024);
    assert!(rgb8_frame_height == 768);
    assert!(rgb8_frame_row_bytes == 3072);
    assert!(rgb8_frame_byte_length == stage_frame_count * page_size as usize);
    assert!(generation_1_rows == rgb8_frame_height / 2);
    assert!(generation_2_rows == rgb8_frame_height);
    assert!(display_payload_size as usize == 40 + rgb8_frame_row_bytes);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum runtime_role {
    renderer,
    supervisor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum rite_phase {
    generation_1_row(u32),
    generation_1_fault_pending,
    generation_2_row(u32),
    generation_2_exit,
    idle,
}

impl rite_phase {
    const fn incarnation(self) -> Option<u64> {
        match self {
            Self::generation_1_row(_) | Self::generation_1_fault_pending => Some(1),
            Self::generation_2_row(_) | Self::generation_2_exit => Some(2),
            Self::idle => None,
        }
    }

    const fn row(self) -> Option<u32> {
        match self {
            Self::generation_1_row(row) | Self::generation_2_row(row) => Some(row),
            Self::generation_1_fault_pending | Self::generation_2_exit | Self::idle => None,
        }
    }

    const fn generation(self) -> Option<u8> {
        match self {
            Self::generation_1_row(_) | Self::generation_1_fault_pending => Some(1),
            Self::generation_2_row(_) | Self::generation_2_exit => Some(2),
            Self::idle => None,
        }
    }

    const fn next_after_row(self) -> Option<Self> {
        match self {
            Self::generation_1_row(row) if row + 1 < generation_1_rows as u32 => {
                Some(Self::generation_1_row(row + 1))
            }
            Self::generation_1_row(_) => Some(Self::generation_1_fault_pending),
            Self::generation_2_row(row) if row + 1 < generation_2_rows as u32 => {
                Some(Self::generation_2_row(row + 1))
            }
            Self::generation_2_row(_) => Some(Self::generation_2_exit),
            Self::generation_1_fault_pending | Self::generation_2_exit | Self::idle => None,
        }
    }
}

struct renderer_task_space {
    root: address_space,
    image_physical: physical_range,
    mailbox_physical: physical_range,
    stack_physical: physical_range,
    privilege_stack: frame_lease,
    privilege_stack_range: physical_range,
    frame: *mut scheduler_frame,
    payload_length: usize,
}

struct renderer_task {
    circle: circle_id,
    identity: task_identity,
    space: renderer_task_space,
    ready_seen: bool,
    incarnation: u64,
}

struct supervisor_task {
    circle: circle_id,
    identity: task_identity,
    space: super::scheduler_test::task_space,
    ready_seen: bool,
}

struct renderer_stage {
    lease: frame_lease,
    stage: Option<rgb8_frame_stage<'static>>,
    task: task_identity,
    channel_generation: u64,
    generation: u64,
}

#[derive(Clone, Copy)]
struct renderer_extents {
    image: physical_range,
    mailbox: physical_range,
    stack: physical_range,
    root: u64,
    privilege_stack: physical_range,
    stage: physical_range,
}

#[derive(Clone, Copy)]
struct renderer_trace {
    dispatches: [u64; 6],
    preemptions: [u64; 1],
    tick: u64,
    rows: [usize; 2],
    fault: Option<exceptions::exception_snapshot>,
    fault_observation: Option<supervision_observation>,
    completion_observation: Option<supervision_observation>,
    replacement: Option<supervision_replacement>,
    block_count: usize,
    wake_count: usize,
    observation_count: usize,
    fault_count: usize,
    restart_count: usize,
    completion_count: usize,
    channel_retire_count: usize,
    stage_clear_count: usize,
    body_copy_count: usize,
    body_bytes_copied: usize,
    reply_copy_count: usize,
    terminal_exit_count: usize,
    terminal_reply_copied: bool,
    flags_normalized: bool,
    surface_unchanged_generation_1: bool,
    surface_unchanged_generation_2: bool,
    surface_withdrawn: bool,
    preflight_complete: bool,
    frame_commit_count: usize,
    fresh_replacement: bool,
    supervisor_continued: bool,
    kernel_continuation_restored: bool,
}

impl renderer_trace {
    const fn empty() -> Self {
        Self {
            dispatches: [u64::MAX; 6],
            preemptions: [u64::MAX],
            tick: 0,
            rows: [0; 2],
            fault: None,
            fault_observation: None,
            completion_observation: None,
            replacement: None,
            block_count: 0,
            wake_count: 0,
            observation_count: 0,
            fault_count: 0,
            restart_count: 0,
            completion_count: 0,
            channel_retire_count: 0,
            stage_clear_count: 0,
            body_copy_count: 0,
            body_bytes_copied: 0,
            reply_copy_count: 0,
            terminal_exit_count: 0,
            terminal_reply_copied: false,
            flags_normalized: false,
            surface_unchanged_generation_1: false,
            surface_unchanged_generation_2: false,
            surface_withdrawn: false,
            preflight_complete: false,
            frame_commit_count: 0,
            fresh_replacement: false,
            supervisor_continued: false,
            kernel_continuation_restored: false,
        }
    }
}

struct renderer_runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    supervisions: supervision_table,
    relationship: supervision_id,
    renderer: Option<renderer_task>,
    supervisor: supervisor_task,
    active_role: Option<runtime_role>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    channel: Option<rite_channel>,
    channel_generation: u64,
    stage: Option<renderer_stage>,
    next_stage_generation: u64,
    rite_phase: rite_phase,
    scratch: rite_scratch,
    trace: renderer_trace,
}

#[derive(Clone, Copy)]
struct renderer_evidence {
    free_frames: usize,
}

static active_runtime: AtomicPtr<renderer_runtime> = AtomicPtr::new(ptr::null_mut());
static switch_active: AtomicBool = AtomicBool::new(false);
static gate_active: AtomicBool = AtomicBool::new(false);
static rite_gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_supervised_renderer_payload_start: u8;
    static x86_64_supervised_renderer_payload_end: u8;
    static x86_64_supervised_renderer_generation_1_entry: u8;
    static x86_64_supervised_renderer_generation_1_ready_return: u8;
    static x86_64_supervised_renderer_generation_1_preemption_wait: u8;
    static x86_64_supervised_renderer_generation_1_preemption_resume: u8;
    static x86_64_supervised_renderer_generation_1_fault: u8;
    static x86_64_supervised_renderer_generation_1_failed: u8;
    static x86_64_supervised_renderer_generation_2_entry: u8;
    static x86_64_supervised_renderer_generation_2_exit_return: u8;
    static x86_64_supervised_renderer_generation_2_failed: u8;
    static x86_64_supervised_renderer_submit_display_rite: u8;
    static x86_64_supervised_renderer_display_rite_return: u8;
    static x86_64_supervised_renderer_submit_exit_rite: u8;
    static x86_64_supervised_renderer_task_exit_rite_return: u8;

    static x86_64_supervised_renderer_supervisor_start: u8;
    static x86_64_supervised_renderer_supervisor_fault_return: u8;
    static x86_64_supervised_renderer_supervisor_completion_return: u8;
    static x86_64_supervised_renderer_supervisor_complete_return: u8;
    static x86_64_supervised_renderer_supervisor_end: u8;
}

pub(super) fn run(framebuffer: framebuffer_info) -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_renderer_state_invalid");
    }
    if framebuffer_aperture_length(framebuffer).is_none() {
        fail("renderer_framebuffer_invalid");
    }

    let kernel_cr3 = paging::read_cr3();
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(move |allocator| {
        run_with_allocator(
            allocator,
            kernel_cr3,
            bootstrap_privilege_stack_top,
            framebuffer,
        )
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
    framebuffer: framebuffer_info,
) -> Result<renderer_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("renderer_setup_state_invalid");
    }
    validate_payload_contract()?;
    let initial = allocator.accounting();
    let required_frames = renderer_image_pages + 600;
    if initial.free < required_frames
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("renderer_initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let renderer_circle = circles
        .create()
        .map_err(|_| "renderer_circle_create_failed")?;
    let supervisor_circle = circles
        .create()
        .map_err(|_| "renderer_supervisor_circle_create_failed")?;
    let renderer_space = prepare_renderer_space(allocator, kernel_cr3, 1, framebuffer)?;
    let supervisor_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_start),
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_end),
    )?;
    attach_task_space(&mut circles, renderer_circle)?;
    attach_task_space(&mut circles, supervisor_circle)?;
    let renderer_identity = create_renderer_task(
        &mut circles,
        &mut tasks,
        renderer_circle,
        1,
        &renderer_space,
    )?;
    let supervisor_identity = super::scheduler_test::create_task(
        &mut circles,
        &mut tasks,
        supervisor_circle,
        2,
        supervisor_space.payload_length,
    )?;
    let channel = rite_channel::try_new(renderer_identity, renderer_grant)
        .map_err(|_| "renderer_channel_create_failed")?;
    let stage = allocate_stage(allocator, renderer_identity, 1, 1)?;
    validate_stage_is_supervisor_only(&renderer_space.root, &stage)?;
    let mut scheduler = scheduler_table::new(renderer_task_count)
        .map_err(|_| "renderer_scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, renderer_identity)
        .map_err(|_| "renderer_task_admission_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, supervisor_identity)
        .map_err(|_| "renderer_supervisor_admission_failed")?;
    let mut supervisions = supervision_table::new();
    let relationship = supervisions
        .create(&mut circles, &tasks, supervisor_identity, renderer_identity)
        .map_err(|_| "renderer_supervision_create_failed")?;

    validate_ready_ownership(
        initial,
        allocator.accounting(),
        &circles,
        &tasks,
        &scheduler,
        &supervisions,
        relationship,
    )?;
    seed_framebuffer_baseline(framebuffer)?;

    let mut runtime = renderer_runtime {
        circles,
        tasks,
        scheduler,
        supervisions,
        relationship,
        renderer: Some(renderer_task {
            circle: renderer_circle,
            identity: renderer_identity,
            space: renderer_space,
            ready_seen: false,
            incarnation: 1,
        }),
        supervisor: supervisor_task {
            circle: supervisor_circle,
            identity: supervisor_identity,
            space: supervisor_space,
            ready_seen: false,
        },
        active_role: None,
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        channel: Some(channel),
        channel_generation: 1,
        stage: Some(stage),
        next_stage_generation: 2,
        rite_phase: rite_phase::generation_1_row(0),
        scratch: [0; page_size as usize],
        trace: renderer_trace::empty(),
    };

    let first = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "renderer_initial_dispatch_failed")?;
    if first.task() != renderer_identity
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        return Err("renderer_initial_dispatch_invalid");
    }
    runtime.trace.dispatches[0] = first.sequence();
    let first_frame = runtime.select_role(runtime_role::renderer, false)?;
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
        || runtime.trace.rows[0] != generation_1_rows
        || runtime.rite_phase != rite_phase::generation_1_fault_pending
        || runtime.trace.fault_count != 1
        || runtime.trace.restart_count != 1
        || runtime.trace.block_count != 2
        || runtime.trace.wake_count != 1
        || runtime.trace.observation_count != 1
        || !runtime.trace.flags_normalized
        || !interrupts::scheduler_interrupts_are_masked()
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("renderer_restart_restore_failed");
    }
    if !framebuffer_baseline_is_intact(framebuffer)? {
        return Err("renderer_generation_1_surface_changed");
    }
    runtime.trace.surface_unchanged_generation_1 = true;
    runtime.saved_kernel_stack = 0;
    runtime.original_task_switched = 0;
    compiler_fence(Ordering::SeqCst);

    let generation_1_identity = runtime.renderer_ref()?.identity;
    let retired_extents = runtime.renderer_extents()?;
    withdraw_and_destroy_renderer(allocator, &mut runtime, generation_1_identity, 1)?;
    let generation_2_identity =
        create_replacement_renderer(allocator, &mut runtime, retired_extents, framebuffer)?;
    let admission = runtime
        .supervisions
        .prepare_admit_replacement(
            &runtime.circles,
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            generation_2_identity,
        )
        .map_err(|_| "renderer_replacement_preflight_failed")?;
    if admission.incarnation() != generation_2_identity || admission.restart_sequence() != 1 {
        return Err("renderer_replacement_admission_invalid");
    }
    runtime
        .scheduler
        .admit(
            &mut runtime.circles,
            &mut runtime.tasks,
            generation_2_identity,
        )
        .map_err(|_| "renderer_replacement_scheduler_admission_failed")?;
    admission.commit();
    let relationship = runtime
        .supervisions
        .snapshot(runtime.relationship)
        .map_err(|_| "renderer_replacement_snapshot_failed")?;
    if relationship.phase() != supervision_phase::replacement_active
        || relationship.active_incarnation() != Some(generation_2_identity)
        || relationship.restart_budget_remaining() != 0
    {
        return Err("renderer_replacement_relationship_invalid");
    }
    runtime.trace.fresh_replacement = true;

    let second = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "renderer_replacement_dispatch_failed")?;
    if second.task() != generation_2_identity
        || second.sequence() != 4
        || second.context() != scheduler_context::initial
    {
        return Err("renderer_replacement_dispatch_invalid");
    }
    runtime.trace.dispatches[4] = second.sequence();
    let second_frame = runtime.select_role(runtime_role::renderer, false)?;
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
        || runtime.trace.rows != [generation_1_rows, generation_2_rows]
        || runtime.rite_phase != rite_phase::idle
        || runtime.trace.completion_count != 1
        || runtime.trace.wake_count != 2
        || runtime.trace.observation_count != 2
        || runtime.trace.terminal_exit_count != 1
        || runtime.trace.terminal_reply_copied
        || !runtime.trace.supervisor_continued
        || !runtime.trace.fresh_replacement
        || !interrupts::scheduler_interrupts_are_masked()
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("renderer_completion_restore_failed");
    }
    runtime.saved_kernel_stack = 0;
    compiler_fence(Ordering::SeqCst);

    if !framebuffer_baseline_is_intact(framebuffer)? {
        return Err("renderer_generation_2_surface_changed");
    }
    runtime.trace.surface_unchanged_generation_2 = true;
    teardown_completed_runtime(allocator, &mut runtime)?;
    validate_prepublication_teardown(allocator, &runtime, initial)?;
    publish_complete_frame(allocator, &mut runtime, framebuffer)?;

    let final_accounting = allocator.accounting();
    if final_accounting != initial
        || runtime.stage.is_some()
        || runtime.trace.stage_clear_count != 2
        || runtime.trace.frame_commit_count != 1
        || !runtime.trace.surface_withdrawn
        || !runtime.trace.preflight_complete
    {
        return Err("renderer_final_restoration_failed");
    }
    let evidence = renderer_evidence {
        free_frames: final_accounting.free,
    };
    retire_runtime_metadata(&mut runtime);
    Ok(evidence)
}

fn validate_prepublication_teardown(
    allocator: &frame_allocator<'_>,
    runtime: &renderer_runtime,
    initial: frame_accounting,
) -> Result<(), &'static str> {
    let retained = allocator.accounting();
    let final_circles = runtime.circles.accounting();
    let final_scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "renderer_final_scheduler_invalid")?;
    let stage = runtime.stage.as_ref().ok_or("renderer_stage_missing")?;
    let completion_observation = runtime
        .trace
        .completion_observation
        .ok_or("renderer_completion_observation_missing")?;
    if initial.free.checked_sub(stage_frame_count) != Some(retained.free)
        || retained.data_owned != stage_frame_count
        || retained.table_owned != 0
        || retained.active_leases != 1
        || retained.bound_leases != 0
        || final_circles != empty_circle_accounting()
        || runtime.tasks.active_count() != 0
        || runtime.tasks.attached_context_count() != 0
        || runtime.supervisions.active_count() != 0
        || final_scheduler.admitted() != 0
        || final_scheduler.runnable() != 0
        || final_scheduler.current().is_some()
        || runtime.trace.channel_retire_count != 2
        || runtime.trace.stage_clear_count != 1
        || runtime.trace.frame_commit_count != 0
        || runtime.trace.dispatches != [0, 1, 2, 3, 4, 5]
        || runtime.trace.preemptions != [0]
        || runtime.trace.tick == 0
        || runtime.trace.rows != [generation_1_rows, generation_2_rows]
        || runtime.trace.fault.is_none()
        || runtime.trace.fault_observation.is_none()
        || runtime.trace.replacement.is_none()
        || runtime.trace.block_count != 2
        || runtime.trace.wake_count != 2
        || runtime.trace.observation_count != 2
        || runtime.trace.fault_count != 1
        || runtime.trace.restart_count != 1
        || runtime.trace.completion_count != 1
        || runtime.trace.body_copy_count != generation_1_rows + generation_2_rows + 1
        || runtime.trace.body_bytes_copied
            != (generation_1_rows + generation_2_rows) * display_payload_size as usize
                + exit_payload_size as usize
        || runtime.trace.reply_copy_count != generation_1_rows + generation_2_rows
        || runtime.trace.terminal_exit_count != 1
        || runtime.trace.terminal_reply_copied
        || !runtime.trace.flags_normalized
        || !runtime.trace.surface_unchanged_generation_1
        || !runtime.trace.surface_unchanged_generation_2
        || runtime.trace.surface_withdrawn
        || runtime.trace.preflight_complete
        || !runtime.trace.fresh_replacement
        || !runtime.trace.supervisor_continued
        || !runtime.trace.kernel_continuation_restored
        || runtime.renderer.is_some()
        || runtime.channel.is_some()
        || runtime.active_role.is_some()
        || runtime.rite_phase != rite_phase::idle
        || !scratch_is_zero(&runtime.scratch)
        || !interrupts::scheduler_interrupts_are_masked()
        || paging::read_cr3() != runtime.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || paging::read_cr0() & cr0_task_switched != runtime.original_task_switched
        || !matches!(runtime.original_task_switched, 0 | cr0_task_switched)
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || runtime.next_stage_generation != 3
        || stage.channel_generation != 2
        || stage.generation != 2
        || stage.task != completion_observation.incarnation()
        || completion_observation.event_sequence() != 2
        || completion_observation.outcome() != supervision_outcome::successful_exit
        || stage
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != generation_2_rows)
        || allocator
            .is_bound(&stage.lease)
            .map_err(|_| "renderer_stage_binding_state_invalid")?
    {
        return Err("renderer_prepublication_teardown_invalid");
    }
    Ok(())
}

fn validate_payload_contract() -> Result<(), &'static str> {
    let start = ptr::addr_of!(x86_64_supervised_renderer_payload_start) as usize;
    let end = ptr::addr_of!(x86_64_supervised_renderer_payload_end) as usize;
    let length = end
        .checked_sub(start)
        .ok_or("renderer_payload_extent_invalid")?;
    if length == 0
        || length.div_ceil(page_size as usize) != renderer_image_pages
        || length > renderer_image_page_limit * page_size as usize
        || linked_user_address(ptr::addr_of!(x86_64_supervised_renderer_generation_1_entry))
            != Some(renderer_image_address)
        || linked_user_address(ptr::addr_of!(x86_64_supervised_renderer_generation_2_entry))
            != Some(renderer_generation_2_address)
    {
        return Err("renderer_payload_contract_invalid");
    }
    let ready_return =
        linked_user_address(&raw const x86_64_supervised_renderer_generation_1_ready_return);
    let preemption_wait =
        linked_user_address(&raw const x86_64_supervised_renderer_generation_1_preemption_wait);
    let preemption_resume =
        linked_user_address(&raw const x86_64_supervised_renderer_generation_1_preemption_resume);
    let fault = linked_user_address(&raw const x86_64_supervised_renderer_generation_1_fault);
    if !matches!(
        (ready_return, preemption_wait, preemption_resume, fault),
        (Some(ready), Some(wait), Some(resume), Some(fault))
            if ready.checked_add(6) == Some(wait) && wait < resume && resume < fault
    ) {
        return Err("renderer_preemption_symbol_order_invalid");
    }
    for symbol in [
        ptr::addr_of!(x86_64_supervised_renderer_generation_1_ready_return),
        ptr::addr_of!(x86_64_supervised_renderer_generation_1_preemption_wait),
        ptr::addr_of!(x86_64_supervised_renderer_generation_1_preemption_resume),
        ptr::addr_of!(x86_64_supervised_renderer_generation_1_fault),
        ptr::addr_of!(x86_64_supervised_renderer_generation_1_failed),
        ptr::addr_of!(x86_64_supervised_renderer_generation_2_exit_return),
        ptr::addr_of!(x86_64_supervised_renderer_generation_2_failed),
        ptr::addr_of!(x86_64_supervised_renderer_submit_display_rite),
        ptr::addr_of!(x86_64_supervised_renderer_display_rite_return),
        ptr::addr_of!(x86_64_supervised_renderer_submit_exit_rite),
        ptr::addr_of!(x86_64_supervised_renderer_task_exit_rite_return),
    ] {
        if linked_user_address(symbol).is_none() {
            return Err("renderer_linked_symbol_invalid");
        }
    }
    Ok(())
}

fn payload_length() -> Option<usize> {
    (ptr::addr_of!(x86_64_supervised_renderer_payload_end) as usize)
        .checked_sub(ptr::addr_of!(x86_64_supervised_renderer_payload_start) as usize)
}

fn linked_user_address(symbol: *const u8) -> Option<u64> {
    let start = ptr::addr_of!(x86_64_supervised_renderer_payload_start) as usize;
    let offset = (symbol as usize).checked_sub(start)?;
    if offset >= payload_length()? {
        return None;
    }
    renderer_image_address.checked_add(offset as u64)
}

fn allocate_data(
    allocator: &mut frame_allocator<'_>,
    count: usize,
    alignment: usize,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    unsafe {
        allocator.allocate_with(count, alignment, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

fn prepare_renderer_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    incarnation: u64,
    framebuffer: framebuffer_info,
) -> Result<renderer_task_space, &'static str> {
    let payload_length = payload_length().ok_or("renderer_payload_extent_invalid")?;
    let image_page_count = payload_length.div_ceil(page_size as usize);
    if image_page_count != renderer_image_pages {
        return Err("renderer_payload_page_count_invalid");
    }
    let image = allocate_data(
        allocator,
        image_page_count,
        1,
        "renderer_image_allocation_failed",
    )?;
    let mailbox = allocate_data(allocator, 1, 1, "renderer_mailbox_allocation_failed")?;
    let stack = allocate_data(
        allocator,
        renderer_stack_pages,
        renderer_stack_pages,
        "renderer_stack_allocation_failed",
    )?;
    let privilege_stack = allocate_data(
        allocator,
        privilege_stack_pages,
        privilege_stack_pages,
        "renderer_privilege_stack_allocation_failed",
    )?;
    let image_physical = image.range().ok_or("renderer_image_extent_missing")?;
    let mailbox_physical = mailbox.range().ok_or("renderer_mailbox_extent_missing")?;
    let stack_physical = stack.range().ok_or("renderer_stack_extent_missing")?;
    let privilege_stack_range = privilege_stack
        .range()
        .ok_or("renderer_privilege_stack_extent_missing")?;
    if image_physical.length != image_page_count as u64 * page_size
        || mailbox_physical.length != page_size
        || stack_physical.length != renderer_stack_pages as u64 * page_size
        || privilege_stack_range.length != descriptors::privilege_stack_size as u64
        || privilege_stack_range.base & (descriptors::privilege_stack_size as u64 - 1) != 0
        || [
            image_physical,
            mailbox_physical,
            stack_physical,
            privilege_stack_range,
        ]
        .iter()
        .any(|range| !range_is_zero(*range))
    {
        return Err("renderer_task_frames_invalid");
    }
    copy_renderer_payload(image_physical)?;

    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "renderer_address_space_create_failed")?;
    root.map_owned_extent(
        allocator,
        renderer_image_address,
        image,
        paging::page_permissions {
            user: true,
            writable: false,
            executable: true,
        },
    )
    .map_err(|_| "renderer_image_mapping_failed")?;
    root.map_owned(
        allocator,
        renderer_mailbox_address,
        mailbox,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "renderer_mailbox_mapping_failed")?;
    root.map_owned_extent(
        allocator,
        renderer_stack_address,
        stack,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "renderer_stack_mapping_failed")?;

    for page in 0..renderer_image_pages {
        validate_mapping(
            root.validate_mapping(renderer_image_address + page as u64 * page_size)
                .map_err(|_| "renderer_image_mapping_walk_failed")?,
            image_physical.base + page as u64 * page_size,
            false,
            true,
        )?;
    }
    validate_mapping(
        root.validate_mapping(renderer_mailbox_address)
            .map_err(|_| "renderer_mailbox_mapping_walk_failed")?,
        mailbox_physical.base,
        true,
        false,
    )?;
    for page in 0..renderer_stack_pages {
        validate_mapping(
            root.validate_mapping(renderer_stack_address + page as u64 * page_size)
                .map_err(|_| "renderer_stack_mapping_walk_failed")?,
            stack_physical.base + page as u64 * page_size,
            true,
            false,
        )?;
    }
    for guard in [
        renderer_image_address - page_size,
        renderer_image_address + image_page_count as u64 * page_size,
        renderer_mailbox_address - page_size,
        renderer_mailbox_address + page_size,
        renderer_stack_address - page_size,
        renderer_stack_top,
    ] {
        root.validate_absent(guard)
            .map_err(|_| "renderer_guard_mapping_present")?;
    }
    let aperture_length = framebuffer_aperture_length(framebuffer)
        .ok_or("renderer_framebuffer_extent_invalid")? as u64;
    root.validate_borrowed_supervisor_range(framebuffer.address, aperture_length)
        .map_err(|_| "renderer_framebuffer_user_accessible")?;
    if !renderer_payload_matches(image_physical) {
        return Err("renderer_payload_copy_mismatch");
    }

    allocator
        .bind(&privilege_stack)
        .map_err(|_| "renderer_privilege_stack_binding_failed")?;
    let privilege_stack_top = privilege_stack_range
        .end()
        .ok_or("renderer_privilege_stack_extent_invalid")?;
    let frame_address = privilege_stack_top
        .checked_sub(super::scheduler_test::context_size)
        .ok_or("renderer_initial_frame_address_invalid")?;
    if frame_address < privilege_stack_range.base
        || frame_address & (mem::align_of::<u64>() as u64 - 1) != 0
    {
        return Err("renderer_initial_frame_address_invalid");
    }
    let entry = match incarnation {
        1 => linked_user_address(ptr::addr_of!(x86_64_supervised_renderer_generation_1_entry)),
        2 => linked_user_address(ptr::addr_of!(x86_64_supervised_renderer_generation_2_entry)),
        _ => None,
    }
    .ok_or("renderer_entry_invalid")?;
    let frame = frame_address as usize as *mut scheduler_frame;
    let mut initial_frame = scheduler_frame::initial(entry);
    initial_frame.stack_pointer = renderer_stack_top;
    unsafe { ptr::write_volatile(frame, initial_frame) };
    compiler_fence(Ordering::SeqCst);

    Ok(renderer_task_space {
        root,
        image_physical,
        mailbox_physical,
        stack_physical,
        privilege_stack,
        privilege_stack_range,
        frame,
        payload_length,
    })
}

fn copy_renderer_payload(destination: physical_range) -> Result<(), &'static str> {
    let length = payload_length().ok_or("renderer_payload_extent_invalid")?;
    if destination.length != renderer_image_pages as u64 * page_size
        || length as u64 > destination.length
    {
        return Err("renderer_payload_destination_invalid");
    }
    let destination = usize::try_from(destination.base)
        .map_err(|_| "renderer_payload_destination_invalid")? as *mut u8;
    unsafe {
        ptr::copy_nonoverlapping(
            ptr::addr_of!(x86_64_supervised_renderer_payload_start),
            destination,
            length,
        )
    };
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

fn renderer_payload_matches(destination: physical_range) -> bool {
    let Some(length) = payload_length() else {
        return false;
    };
    if length as u64 > destination.length {
        return false;
    }
    let Ok(base) = usize::try_from(destination.base) else {
        return false;
    };
    for offset in 0..length {
        let expected = unsafe {
            ptr::read(ptr::addr_of!(x86_64_supervised_renderer_payload_start).add(offset))
        };
        let actual = unsafe { ptr::read_volatile((base as *const u8).add(offset)) };
        if expected != actual {
            return false;
        }
    }
    (length..destination.length as usize)
        .all(|offset| unsafe { ptr::read_volatile((base as *const u8).add(offset)) == 0 })
}

fn range_is_zero(range: physical_range) -> bool {
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    let Ok(length) = usize::try_from(range.length) else {
        return false;
    };
    (0..length).all(|offset| unsafe { ptr::read_volatile((base as *const u8).add(offset)) == 0 })
}

fn validate_mapping(
    evidence: mapping_evidence,
    physical_address: u64,
    writable: bool,
    executable: bool,
) -> Result<(), &'static str> {
    if evidence.physical_address != physical_address
        || !evidence.permissions.present
        || !evidence.permissions.user
        || evidence.permissions.writable != writable
        || evidence.permissions.executable != executable
    {
        return Err("renderer_mapping_permissions_invalid");
    }
    Ok(())
}

fn create_renderer_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    principal: u64,
    space: &renderer_task_space,
) -> Result<task_identity, &'static str> {
    let code_end = renderer_image_address
        .checked_add(space.payload_length as u64)
        .ok_or("renderer_task_code_extent_invalid")?;
    let entry = unsafe { ptr::read_volatile(space.frame) }.instruction_pointer;
    let bounds = task_bounds::new(
        renderer_image_address,
        code_end,
        entry,
        renderer_stack_address,
        renderer_stack_top,
        renderer_stack_top,
    )
    .ok_or("renderer_task_bounds_invalid")?;
    let principal =
        fixture_principal_id::new(principal).ok_or("renderer_task_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(
                circle,
                principal,
                synthetic_origin_metadata::receiver_role_fixture,
                bounds,
            ),
        )
        .map_err(|_| "renderer_task_create_failed")
}

fn allocate_stage(
    allocator: &mut frame_allocator<'_>,
    task: task_identity,
    channel_generation: u64,
    generation: u64,
) -> Result<renderer_stage, &'static str> {
    if channel_generation == 0 || generation == 0 {
        return Err("renderer_stage_identity_invalid");
    }
    let lease = allocate_data(
        allocator,
        stage_frame_count,
        1,
        "renderer_stage_allocation_failed",
    )?;
    let range = lease.range().ok_or("renderer_stage_extent_missing")?;
    if range.length != rgb8_frame_byte_length as u64 || !range_is_zero(range) {
        return Err("renderer_stage_extent_invalid");
    }
    allocator
        .bind(&lease)
        .map_err(|_| "renderer_stage_binding_failed")?;
    let base = usize::try_from(range.base).map_err(|_| "renderer_stage_address_invalid")?;
    let storage =
        unsafe { core::slice::from_raw_parts_mut(base as *mut u8, rgb8_frame_byte_length) };
    let stage =
        rgb8_frame_stage::try_new(storage).map_err(|_| "renderer_stage_construction_failed")?;
    Ok(renderer_stage {
        lease,
        stage: Some(stage),
        task,
        channel_generation,
        generation,
    })
}

fn validate_stage_is_supervisor_only(
    root: &address_space,
    stage: &renderer_stage,
) -> Result<(), &'static str> {
    let range = stage.lease.range().ok_or("renderer_stage_extent_missing")?;
    root.validate_borrowed_supervisor_range(range.base, range.length)
        .map_err(|_| "renderer_stage_user_accessible")
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
    let expected_data = renderer_image_pages + 592;
    let expected_total = renderer_image_pages + 600;
    if initial.free.checked_sub(expected_total) != Some(ready.free)
        || ready.data_owned != expected_data
        || ready.table_owned != 8
        || ready.active_leases != 11
        || ready.bound_leases != 11
    {
        return Err("renderer_ready_frame_accounting_invalid");
    }
    let circle_accounting = circles.accounting();
    let scheduler_snapshot = scheduler
        .snapshot(tasks)
        .map_err(|_| "renderer_ready_scheduler_invalid")?;
    let relationship_snapshot = supervisions
        .snapshot(relationship)
        .map_err(|_| "renderer_ready_relationship_invalid")?;
    if circle_accounting.circles != 2
        || circle_accounting.tasks != 2
        || circle_accounting.task_contexts != 2
        || circle_accounting.supervisions != 1
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
        return Err("renderer_ready_object_accounting_invalid");
    }
    Ok(())
}

fn publish_runtime(runtime: *mut renderer_runtime) -> Result<(), &'static str> {
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("renderer_context_publication_failed");
    }
    Ok(())
}

impl renderer_runtime {
    fn renderer_ref(&self) -> Result<&renderer_task, &'static str> {
        self.renderer.as_ref().ok_or("renderer_task_missing")
    }

    fn renderer_mut(&mut self) -> Result<&mut renderer_task, &'static str> {
        self.renderer.as_mut().ok_or("renderer_task_missing")
    }

    fn identity_for_role(&self, role: runtime_role) -> Result<task_identity, &'static str> {
        match role {
            runtime_role::renderer => Ok(self.renderer_ref()?.identity),
            runtime_role::supervisor => Ok(self.supervisor.identity),
        }
    }

    fn role_for_identity(&self, identity: task_identity) -> Option<runtime_role> {
        if self
            .renderer
            .as_ref()
            .is_some_and(|renderer| renderer.identity == identity)
        {
            Some(runtime_role::renderer)
        } else if self.supervisor.identity == identity {
            Some(runtime_role::supervisor)
        } else {
            None
        }
    }

    fn current_role(&self) -> Option<runtime_role> {
        let current = self.scheduler.snapshot(&self.tasks).ok()?.current()?;
        let role = self.role_for_identity(current)?;
        (self.active_role == Some(role)).then_some(role)
    }

    fn renderer_extents(&self) -> Result<renderer_extents, &'static str> {
        let renderer = self.renderer_ref()?;
        let stage = self.stage.as_ref().ok_or("renderer_stage_missing")?;
        Ok(renderer_extents {
            image: renderer.space.image_physical,
            mailbox: renderer.space.mailbox_physical,
            stack: renderer.space.stack_physical,
            root: renderer
                .space
                .root
                .root_address()
                .map_err(|_| "renderer_root_missing")?,
            privilege_stack: renderer.space.privilege_stack_range,
            stage: stage.lease.range().ok_or("renderer_stage_extent_missing")?,
        })
    }

    fn select_role(
        &mut self,
        role: runtime_role,
        require_task_switched: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let (root, stack_top, frame) = match role {
            runtime_role::renderer => {
                let renderer = self.renderer_ref()?;
                (
                    &renderer.space.root,
                    renderer
                        .space
                        .privilege_stack_range
                        .end()
                        .ok_or("renderer_privilege_stack_extent_invalid")?,
                    renderer.space.frame,
                )
            }
            runtime_role::supervisor => (
                &self.supervisor.space.root,
                self.supervisor
                    .space
                    .privilege_stack_range
                    .end()
                    .ok_or("renderer_supervisor_stack_extent_invalid")?,
                self.supervisor.space.frame,
            ),
        };
        let root_address = root
            .root_address()
            .map_err(|_| "renderer_selected_root_missing")?;
        unsafe { root.activate() }.map_err(|_| "renderer_root_activation_failed")?;
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("renderer_privilege_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root_address
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || self
                .frame_validation_error(frame, role, require_task_switched)
                .is_some()
        {
            return Err("renderer_selected_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_validation_error(
        &self,
        frame: *const scheduler_frame,
        role: runtime_role,
        require_task_switched: bool,
    ) -> Option<&'static str> {
        let (expected_frame, privilege_stack, root, identity, expected_stack_top) = match role {
            runtime_role::renderer => {
                let Some(renderer) = self.renderer.as_ref() else {
                    return Some("renderer_frame_task_missing");
                };
                (
                    renderer.space.frame,
                    renderer.space.privilege_stack_range,
                    &renderer.space.root,
                    renderer.identity,
                    renderer_stack_top,
                )
            }
            runtime_role::supervisor => (
                self.supervisor.space.frame,
                self.supervisor.space.privilege_stack_range,
                &self.supervisor.space.root,
                self.supervisor.identity,
                supervisor_stack_top,
            ),
        };
        if frame != expected_frame {
            return Some("renderer_frame_pointer_invalid");
        }
        if !frame_address_matches_stack(frame, privilege_stack) {
            return Some("renderer_frame_stack_address_invalid");
        }
        if root.validate_all_mappings().is_err() {
            return Some("renderer_frame_mappings_invalid");
        }
        let Ok(snapshot) = self.tasks.snapshot(identity) else {
            return Some("renderer_frame_snapshot_invalid");
        };
        if snapshot.state() != task_state::running || !snapshot.context_attached() {
            return Some("renderer_frame_task_state_invalid");
        }
        if root.root_address().ok() != Some(paging::entry_address(paging::read_cr3())) {
            return Some("renderer_frame_root_not_selected");
        }
        if privilege_stack.end() != Some(unsafe { descriptors::selected_privilege_stack_top() }) {
            return Some("renderer_frame_stack_not_selected");
        }
        if require_task_switched && paging::read_cr0() & cr0_task_switched == 0 {
            return Some("renderer_frame_ts_not_set");
        }
        let saved = unsafe { ptr::read(frame) };
        let bounds = snapshot.bounds();
        if saved.error_code != 0
            || saved.code_segment != u64::from(descriptors::user_code_selector)
            || saved.stack_segment != u64::from(descriptors::user_data_selector)
            || saved.flags & (1 << 1) == 0
            || saved.flags & (3 << 12) != 0
            || saved.flags & (1 << 10) != 0
            || saved.flags & (1 << 14) != 0
            || !bounds.contains_code(saved.instruction_pointer)
            || !bounds.contains_stack_pointer(saved.stack_pointer)
        {
            return Some("renderer_frame_return_fields_invalid");
        }
        match role {
            runtime_role::renderer
                if saved.stack_pointer <= renderer_stack_address
                    || saved.stack_pointer > expected_stack_top =>
            {
                Some("renderer_user_stack_pointer_invalid")
            }
            runtime_role::supervisor if saved.stack_pointer != expected_stack_top => {
                Some("renderer_supervisor_stack_pointer_invalid")
            }
            _ => None,
        }
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: runtime_role,
        require_task_switched: bool,
    ) -> bool {
        self.frame_validation_error(frame, role, require_task_switched)
            .is_none()
    }
}

fn withdraw_and_destroy_renderer(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut renderer_runtime,
    identity: task_identity,
    incarnation: u64,
) -> Result<(), &'static str> {
    retire_channel(runtime, identity, incarnation)?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, identity)
        .map_err(|_| "renderer_context_withdrawal_failed")?;
    release_stage(allocator, runtime, identity, incarnation)?;
    let mut renderer = runtime.renderer.take().ok_or("renderer_task_missing")?;
    release_renderer_space(
        allocator,
        &mut runtime.circles,
        renderer.circle,
        &mut renderer.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, identity)
        .map_err(|_| "renderer_task_destroy_failed")?;
    runtime
        .circles
        .destroy(renderer.circle)
        .map_err(|_| "renderer_circle_destroy_failed")?;
    Ok(())
}

fn retire_channel(
    runtime: &mut renderer_runtime,
    identity: task_identity,
    incarnation: u64,
) -> Result<(), &'static str> {
    if runtime.renderer_ref()?.identity != identity
        || runtime.renderer_ref()?.incarnation != incarnation
        || runtime.channel_generation != incarnation
        || runtime.channel.take().is_none()
        || runtime.trace.channel_retire_count + 1 != incarnation as usize
    {
        return Err("renderer_channel_retirement_invalid");
    }
    runtime.channel_generation = 0;
    runtime.trace.channel_retire_count += 1;
    Ok(())
}

fn release_stage(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut renderer_runtime,
    identity: task_identity,
    generation: u64,
) -> Result<(), &'static str> {
    let mut owner = runtime.stage.take().ok_or("renderer_stage_missing")?;
    if owner.task != identity
        || owner.channel_generation != generation
        || owner.generation != generation
        || runtime.trace.stage_clear_count + 1 != generation as usize
    {
        return Err("renderer_stage_retirement_invalid");
    }
    let stage = owner.stage.take().ok_or("renderer_stage_owner_missing")?;
    if !stage.clear_and_verify() {
        return Err("renderer_stage_clear_failed");
    }
    allocator
        .unbind(&owner.lease)
        .map_err(|_| "renderer_stage_unbind_failed")?;
    unsafe { allocator.release_with(&mut owner.lease, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "renderer_stage_release_failed")?;
    runtime.trace.stage_clear_count += 1;
    Ok(())
}

fn release_renderer_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: circle_id,
    space: &mut renderer_task_space,
) -> Result<(), &'static str> {
    let mut image = space
        .root
        .unmap_owned(allocator, renderer_image_address)
        .map_err(|_| "renderer_image_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, renderer_mailbox_address)
        .map_err(|_| "renderer_mailbox_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, renderer_stack_address)
        .map_err(|_| "renderer_stack_withdrawal_failed")?;
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "renderer_address_space_destroy_failed")?;
    unsafe { allocator.release_with(&mut image, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "renderer_image_release_failed")?;
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "renderer_mailbox_release_failed")?;
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "renderer_stack_release_failed")?;
    allocator
        .unbind(&space.privilege_stack)
        .map_err(|_| "renderer_privilege_stack_unbind_failed")?;
    unsafe {
        allocator.release_with(&mut space.privilege_stack, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "renderer_privilege_stack_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "renderer_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "renderer_root_detachment_failed")?;
    retire_renderer_space_metadata(space);
    Ok(())
}

fn retire_renderer_space_metadata(space: &mut renderer_task_space) {
    debug_assert!(space.privilege_stack.range().is_none());
    debug_assert!(space.root.root_address().is_err());
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(space.image_physical),
            physical_range::new(0, 0),
        );
        ptr::write_volatile(
            ptr::addr_of_mut!(space.mailbox_physical),
            physical_range::new(0, 0),
        );
        ptr::write_volatile(
            ptr::addr_of_mut!(space.stack_physical),
            physical_range::new(0, 0),
        );
        ptr::write_volatile(
            ptr::addr_of_mut!(space.privilege_stack_range),
            physical_range::new(0, 0),
        );
        ptr::write_volatile(ptr::addr_of_mut!(space.frame), ptr::null_mut());
        ptr::write_volatile(ptr::addr_of_mut!(space.payload_length), 0);
    }
}

fn create_replacement_renderer(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut renderer_runtime,
    expected: renderer_extents,
    framebuffer: framebuffer_info,
) -> Result<task_identity, &'static str> {
    if runtime.renderer.is_some()
        || runtime.channel.is_some()
        || runtime.stage.is_some()
        || runtime.channel_generation != 0
        || runtime.rite_phase != rite_phase::generation_1_fault_pending
        || runtime.next_stage_generation != 2
    {
        return Err("renderer_replacement_state_invalid");
    }
    let circle = runtime
        .circles
        .create()
        .map_err(|_| "renderer_replacement_circle_create_failed")?;
    let space = prepare_renderer_space(allocator, runtime.kernel_cr3, 2, framebuffer)?;
    if space.image_physical != expected.image
        || space.mailbox_physical != expected.mailbox
        || space.stack_physical != expected.stack
        || space
            .root
            .root_address()
            .map_err(|_| "renderer_replacement_root_missing")?
            != expected.root
        || space.privilege_stack_range != expected.privilege_stack
    {
        return Err("renderer_replacement_storage_reuse_invalid");
    }
    attach_task_space(&mut runtime.circles, circle)?;
    let identity =
        create_renderer_task(&mut runtime.circles, &mut runtime.tasks, circle, 1, &space)?;
    let channel = rite_channel::try_new(identity, renderer_grant)
        .map_err(|_| "renderer_replacement_channel_create_failed")?;
    let stage = allocate_stage(allocator, identity, 2, runtime.next_stage_generation)?;
    if stage.lease.range() != Some(expected.stage) {
        return Err("renderer_replacement_stage_reuse_invalid");
    }
    validate_stage_is_supervisor_only(&space.root, &stage)?;
    runtime.next_stage_generation = runtime
        .next_stage_generation
        .checked_add(1)
        .ok_or("renderer_stage_generation_exhausted")?;
    runtime.renderer = Some(renderer_task {
        circle,
        identity,
        space,
        ready_seen: false,
        incarnation: 2,
    });
    runtime.channel = Some(channel);
    runtime.channel_generation = 2;
    runtime.stage = Some(stage);
    runtime.rite_phase = rite_phase::generation_2_row(0);
    Ok(identity)
}

fn teardown_completed_runtime(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut renderer_runtime,
) -> Result<(), &'static str> {
    let renderer_identity = runtime.renderer_ref()?.identity;
    let supervisor_identity = runtime.supervisor.identity;
    retire_channel(runtime, renderer_identity, 2)?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, renderer_identity)
        .map_err(|_| "renderer_replacement_context_withdrawal_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            supervisor_identity,
        )
        .map_err(|_| "renderer_supervisor_context_withdrawal_failed")?;
    runtime
        .supervisions
        .withdraw_incarnation(
            &runtime.tasks,
            runtime.relationship,
            supervisor_identity,
            renderer_identity,
        )
        .map_err(|_| "renderer_relationship_incarnation_withdrawal_failed")?;
    let mut renderer = runtime
        .renderer
        .take()
        .ok_or("renderer_replacement_task_missing")?;
    release_renderer_space(
        allocator,
        &mut runtime.circles,
        renderer.circle,
        &mut renderer.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, renderer_identity)
        .map_err(|_| "renderer_replacement_task_destroy_failed")?;
    runtime
        .circles
        .destroy(renderer.circle)
        .map_err(|_| "renderer_replacement_circle_destroy_failed")?;
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
        .map_err(|_| "renderer_relationship_supervisor_withdrawal_failed")?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, supervisor_identity)
        .map_err(|_| "renderer_supervisor_task_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.supervisor.circle)
        .map_err(|_| "renderer_supervisor_circle_destroy_failed")?;
    runtime
        .supervisions
        .release(&runtime.circles, &runtime.tasks, runtime.relationship)
        .map_err(|_| "renderer_relationship_release_failed")?;
    let stage = runtime.stage.as_ref().ok_or("renderer_stage_missing")?;
    allocator
        .unbind(&stage.lease)
        .map_err(|_| "renderer_final_stage_unbind_failed")?;
    Ok(())
}

fn publish_complete_frame(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut renderer_runtime,
    framebuffer: framebuffer_info,
) -> Result<(), &'static str> {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || runtime.active_role.is_some()
        || !interrupts::scheduler_interrupts_are_masked()
        || !framebuffer_baseline_is_intact(framebuffer)?
        || runtime.trace.rows[1] != generation_2_rows
        || runtime.trace.frame_commit_count != 0
    {
        return Err("renderer_publication_state_invalid");
    }
    let withdrawn = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .ok_or("renderer_failure_surface_missing")?;
    if !framebuffer_info_matches(withdrawn, framebuffer) {
        return Err("renderer_failure_surface_mismatch");
    }
    runtime.trace.surface_withdrawn = true;

    let completion_observation = runtime
        .trace
        .completion_observation
        .ok_or("renderer_completion_observation_missing")?;
    let stage_owner = runtime.stage.as_ref().ok_or("renderer_stage_missing")?;
    if stage_owner.task != completion_observation.incarnation()
        || stage_owner.channel_generation != 2
        || stage_owner.generation != 2
        || stage_owner
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != generation_2_rows)
    {
        return Err("renderer_complete_stage_invalid");
    }
    let mut presenter = unsafe { framebuffer_presenter::try_new(withdrawn) }
        .map_err(|_| "renderer_presenter_construction_failed")?;
    if presenter.width() != rgb8_frame_width || presenter.height() != rgb8_frame_height {
        return Err("renderer_presenter_geometry_invalid");
    }
    let mut stage_owner = runtime.stage.take().ok_or("renderer_stage_missing")?;
    let stage_range = stage_owner
        .lease
        .range()
        .ok_or("renderer_stage_extent_missing")?;
    let mut stage = stage_owner
        .stage
        .take()
        .ok_or("renderer_stage_owner_missing")?;
    let completion = stage
        .completion()
        .map_err(|_| "renderer_stage_completion_failed")?;
    let prepared = presenter
        .prepare_complete_rgb8_frame(completion)
        .map_err(|_| "renderer_frame_preflight_failed")?;
    let prepared_release = allocator
        .prepare_release(&mut stage_owner.lease)
        .map_err(|_| "renderer_stage_release_preflight_failed")?;
    if prepared_release.range() != stage_range {
        return Err("renderer_stage_release_extent_changed");
    }
    runtime.trace.preflight_complete = true;
    prepared.commit();
    runtime.trace.frame_commit_count = 1;
    let mut stage = Some(stage);
    let release_result = unsafe {
        prepared_release.clear_and_release_with(|range| {
            range == stage_range && stage.take().is_some_and(rgb8_frame_stage::clear_and_verify)
        })
    };
    if release_result.is_err() {
        fail("renderer_published_stage_quarantined");
    }
    runtime.trace.stage_clear_count = 2;
    Ok(())
}

fn framebuffer_aperture_length(info: framebuffer_info) -> Option<usize> {
    if info.width != rgb8_frame_width
        || info.height != rgb8_frame_height
        || info.address == 0
        || !(24..=32).contains(&info.bits_per_pixel)
        || !color_channel_is_valid(info.red, info.bits_per_pixel)
        || !color_channel_is_valid(info.green, info.bits_per_pixel)
        || !color_channel_is_valid(info.blue, info.bits_per_pixel)
        || color_channels_overlap(info.red, info.green)
        || color_channels_overlap(info.red, info.blue)
        || color_channels_overlap(info.green, info.blue)
    {
        return None;
    }
    let bytes_per_pixel = usize::from(info.bits_per_pixel).div_ceil(8);
    let minimum_pitch = info.width.checked_mul(bytes_per_pixel)?;
    let length = info.pitch.checked_mul(info.height)?;
    let end = info.address.checked_add(length as u64)?;
    if info.pitch < minimum_pitch
        || length > isize::MAX as usize
        || end > paging::identity_map_limit
    {
        return None;
    }
    Some(length)
}

const fn color_channel_is_valid(channel: color_channel, bits_per_pixel: u8) -> bool {
    channel.size != 0
        && channel.size <= 16
        && channel.position < bits_per_pixel
        && match channel.position.checked_add(channel.size) {
            Some(end) => end <= bits_per_pixel,
            None => false,
        }
}

const fn color_channels_overlap(left: color_channel, right: color_channel) -> bool {
    let left_end = left.position + left.size;
    let right_end = right.position + right.size;
    left.position < right_end && right.position < left_end
}

fn framebuffer_info_matches(left: framebuffer_info, right: framebuffer_info) -> bool {
    left.address == right.address
        && left.pitch == right.pitch
        && left.width == right.width
        && left.height == right.height
        && left.bits_per_pixel == right.bits_per_pixel
        && color_channel_matches(left.red, right.red)
        && color_channel_matches(left.green, right.green)
        && color_channel_matches(left.blue, right.blue)
}

const fn color_channel_matches(left: color_channel, right: color_channel) -> bool {
    left.position == right.position && left.size == right.size
}

fn seed_framebuffer_baseline(info: framebuffer_info) -> Result<(), &'static str> {
    unsafe { crate::display::initialize_supervised_renderer_baseline(info) }
        .map_err(|_| "renderer_framebuffer_baseline_seed_failed")
}

fn framebuffer_baseline_is_intact(info: framebuffer_info) -> Result<bool, &'static str> {
    unsafe { crate::display::supervised_renderer_baseline_is_intact(info) }
        .map_err(|_| "renderer_framebuffer_baseline_check_failed")
}

fn retire_runtime_metadata(runtime: &mut renderer_runtime) {
    debug_assert!(runtime.renderer.is_none());
    debug_assert!(runtime.stage.is_none());
    debug_assert!(runtime.supervisor.space.frame.is_null());
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(runtime.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.bootstrap_privilege_stack_top), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.original_task_switched), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.channel_generation), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.next_stage_generation), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.rite_phase), rite_phase::idle);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.scratch), [0; page_size as usize]);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.trace), renderer_trace::empty());
    }
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

pub(super) fn supervised_renderer_is_active() -> bool {
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
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        return Err("renderer_timer_without_runtime");
    }
    let runtime = unsafe { &mut *runtime_pointer };
    if runtime.current_role() != Some(runtime_role::renderer)
        || runtime.renderer_ref()?.incarnation != 1
        || !runtime.renderer_ref()?.ready_seen
        || runtime.trace.preemptions != [u64::MAX]
        || runtime.trace.rows[0] != 0
        || runtime.rite_phase != rite_phase::generation_1_row(0)
        || !runtime.frame_is_valid_for_role(frame, runtime_role::renderer, true)
        || !renderer_mailbox_matches(runtime.renderer_ref()?, 0)
        || observed_at.timebase().slot() != 0
        || observed_at.timebase().generation() != crate::time::boot_timebase_generation
    {
        return Err("renderer_timer_state_invalid");
    }
    let incoming = unsafe { ptr::read(frame) };
    let ready_return = linked_user_address(ptr::addr_of!(
        x86_64_supervised_renderer_generation_1_ready_return
    ))
    .ok_or("renderer_ready_return_invalid")?;
    let preemption_resume = linked_user_address(ptr::addr_of!(
        x86_64_supervised_renderer_generation_1_preemption_resume
    ))
    .ok_or("renderer_preemption_resume_invalid")?;
    if incoming.vector != timer_vector
        || incoming.flags & (1 << 9) == 0
        || incoming.instruction_pointer < ready_return
        || incoming.instruction_pointer >= preemption_resume
        || incoming.stack_pointer + (mem::size_of::<u64>() as u64) < renderer_stack_top
        || incoming.stack_pointer > renderer_stack_top
    {
        return Err("renderer_timer_observation_invalid");
    }
    write_renderer_mailbox_word(
        runtime.renderer_ref()?.space.mailbox_physical.base,
        mailbox_arm_offset,
        generation_1_arm,
    )?;
    unsafe { ptr::addr_of_mut!((*frame).flags).write(initial_user_flags) };
    runtime.trace.flags_normalized = true;
    let renderer_identity = runtime.renderer_ref()?.identity;
    let transition = runtime
        .scheduler
        .preempt_current(&mut runtime.tasks, renderer_identity)
        .map_err(|_| "renderer_preemption_failed")?;
    if transition.task() != renderer_identity
        || transition.sequence() != 0
        || transition.dispatch().task() != runtime.supervisor.identity
        || transition.dispatch().sequence() != 1
        || transition.dispatch().context() != scheduler_context::initial
    {
        return Err("renderer_preemption_transition_invalid");
    }
    runtime.trace.preemptions[0] = transition.sequence();
    runtime.trace.dispatches[1] = transition.dispatch().sequence();
    runtime.trace.tick = observed_at.tick();
    runtime.select_role(runtime_role::supervisor, true)
}

pub(super) unsafe fn gate_dispatch(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("renderer_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("renderer_gate_without_runtime");
    }
    let runtime = unsafe { &mut *runtime_pointer };
    let role = runtime
        .current_role()
        .unwrap_or_else(|| fail("renderer_gate_without_current_task"));
    if let Some(reason) = runtime.frame_validation_error(frame, role, true) {
        fail(reason);
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != scheduler_gate_vector || incoming.error_code != 0 {
        fail("renderer_gate_vector_invalid");
    }
    let result = match incoming.accumulator {
        operation_renderer_ready if role == runtime_role::renderer => {
            handle_renderer_ready(runtime, frame, incoming)
        }
        operation_supervisor_await_fault if role == runtime_role::supervisor => {
            handle_supervisor_await_fault(runtime, incoming)
        }
        operation_supervisor_restart if role == runtime_role::supervisor => {
            finish_supervisor_restart(runtime, incoming)
        }
        operation_supervisor_complete if role == runtime_role::supervisor => {
            finish_supervisor_completion(runtime, incoming)
        }
        _ => Err("renderer_gate_operation_invalid"),
    };
    match result {
        Ok(next) => {
            gate_active.store(false, Ordering::Release);
            next
        }
        Err(reason) => fail(reason),
    }
}

fn handle_renderer_ready(
    runtime: &mut renderer_runtime,
    frame: *mut scheduler_frame,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = linked_user_address(ptr::addr_of!(
        x86_64_supervised_renderer_generation_1_ready_return
    ))
    .ok_or("renderer_ready_return_invalid")?;
    if runtime.renderer_ref()?.incarnation != 1
        || runtime.renderer_ref()?.ready_seen
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != renderer_stack_top
        || !gate_registers_are_exact(incoming, operation_renderer_ready, renderer_mailbox_address)
        || runtime.rite_phase != rite_phase::generation_1_row(0)
        || !renderer_mailbox_matches(runtime.renderer_ref()?, 0)
    {
        return Err("renderer_ready_gate_invalid");
    }
    runtime.renderer_mut()?.ready_seen = true;
    unsafe { ptr::addr_of_mut!((*frame).flags).write(running_user_flags) };
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn handle_supervisor_await_fault(
    runtime: &mut renderer_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_start),
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_fault_return),
        runtime.supervisor.space.payload_length,
    )
    .ok_or("renderer_supervisor_fault_return_invalid")?;
    if runtime.supervisor.ready_seen
        || runtime.trace.preemptions != [0]
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != supervisor_stack_top
        || !gate_registers_are_exact(
            incoming,
            operation_supervisor_await_fault,
            supervisor_mailbox_address,
        )
        || !supervisor_mailbox_matches(&runtime.supervisor, 0)
    {
        return Err("renderer_supervisor_await_gate_invalid");
    }
    let renderer_identity = runtime.renderer_ref()?.identity;
    let wait = runtime
        .supervisions
        .prepare_wait(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            renderer_identity,
        )
        .map_err(|_| "renderer_fault_wait_preflight_failed")?;
    let transition = runtime
        .scheduler
        .block_current_and_dispatch(
            &mut runtime.tasks,
            runtime.supervisor.identity,
            renderer_identity,
        )
        .map_err(|_| "renderer_fault_wait_block_failed")?;
    if transition.task() != runtime.supervisor.identity
        || transition.dispatch().task() != renderer_identity
        || transition.dispatch().sequence() != 2
        || transition.dispatch().context() != scheduler_context::saved
        || wait.supervisor() != runtime.supervisor.identity
        || wait.incarnation() != renderer_identity
        || wait.expected_outcome() != supervision_outcome::invalid_instruction_fault
    {
        return Err("renderer_fault_wait_transition_invalid");
    }
    wait.commit();
    runtime.supervisor.ready_seen = true;
    runtime.trace.dispatches[2] = transition.dispatch().sequence();
    runtime.trace.block_count = 1;
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("renderer_interrupt_mask_failed");
    }
    runtime.select_role(runtime_role::renderer, true)
}

fn finish_supervisor_restart(
    runtime: &mut renderer_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_start),
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_completion_return),
        runtime.supervisor.space.payload_length,
    )
    .ok_or("renderer_supervisor_completion_return_invalid")?;
    if runtime.trace.fault_count != 1
        || runtime.trace.wake_count != 1
        || runtime.trace.observation_count != 1
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != supervisor_stack_top
        || !gate_registers_are_exact(
            incoming,
            operation_supervisor_restart,
            supervisor_mailbox_address,
        )
        || !supervisor_mailbox_matches(&runtime.supervisor, supervisor_continuity)
    {
        return Err("renderer_supervisor_restart_gate_invalid");
    }
    let renderer_identity = runtime.renderer_ref()?.identity;
    let wait = runtime
        .supervisions
        .prepare_wait(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            renderer_identity,
        )
        .map_err(|_| "renderer_completion_wait_preflight_failed")?;
    let park = runtime
        .scheduler
        .park_current(&mut runtime.tasks, runtime.supervisor.identity)
        .map_err(|_| "renderer_supervisor_restart_park_failed")?;
    if park.task() != runtime.supervisor.identity
        || wait.expected_outcome() != supervision_outcome::successful_exit
    {
        return Err("renderer_supervisor_restart_park_invalid");
    }
    wait.commit();
    let replacement = runtime
        .supervisions
        .begin_replacement(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            renderer_identity,
        )
        .map_err(|_| "renderer_replacement_begin_failed")?;
    if replacement.restart_sequence() != 1 || replacement.retired_incarnation() != renderer_identity
    {
        return Err("renderer_replacement_sequence_invalid");
    }
    runtime.trace.replacement = Some(replacement);
    runtime.trace.block_count = 2;
    runtime.trace.restart_count = 1;
    resume_kernel(runtime, restart_status)
}

fn finish_supervisor_completion(
    runtime: &mut renderer_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_start),
        ptr::addr_of!(x86_64_supervised_renderer_supervisor_complete_return),
        runtime.supervisor.space.payload_length,
    )
    .ok_or("renderer_supervisor_complete_return_invalid")?;
    if runtime.trace.completion_count != 1
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != supervisor_stack_top
        || !gate_registers_are_exact(
            incoming,
            operation_supervisor_complete,
            supervisor_mailbox_address,
        )
        || !supervisor_mailbox_matches(&runtime.supervisor, supervisor_continuity)
    {
        return Err("renderer_supervisor_completion_gate_invalid");
    }
    let completion = runtime
        .supervisions
        .prepare_supervisor_completion(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
            runtime.renderer_ref()?.identity,
        )
        .map_err(|_| "renderer_supervisor_completion_preflight_failed")?;
    runtime
        .scheduler
        .complete_current(&mut runtime.tasks, runtime.supervisor.identity)
        .map_err(|_| "renderer_supervisor_completion_transition_failed")?;
    if completion.supervisor() != runtime.supervisor.identity {
        return Err("renderer_supervisor_completion_identity_invalid");
    }
    completion.commit();
    runtime.trace.supervisor_continued = true;
    resume_kernel(runtime, completion_status)
}

fn resume_kernel(
    runtime: &mut renderer_runtime,
    status: u64,
) -> Result<*mut scheduler_frame, &'static str> {
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("renderer_resume_interrupt_mask_failed");
    }
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3 {
        return Err("renderer_kernel_root_restore_failed");
    }
    if !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
    {
        return Err("renderer_tss_restore_failed");
    }
    let saved_kernel_stack = runtime.saved_kernel_stack;
    let original_task_switched = runtime.original_task_switched;
    if saved_kernel_stack == 0
        || !(saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(original_task_switched, 0 | cr0_task_switched)
        || paging::read_cr0() & cr0_task_switched == 0
    {
        return Err("renderer_kernel_continuation_invalid");
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

fn renderer_mailbox_matches(task: &renderer_task, arm: u64) -> bool {
    task.incarnation == 1
        && read_mailbox_word(task.space.mailbox_physical.base, mailbox_canary_a_offset)
            == Some(generation_1_canary_a)
        && read_mailbox_word(task.space.mailbox_physical.base, mailbox_canary_b_offset)
            == Some(generation_1_canary_b)
        && read_mailbox_word(task.space.mailbox_physical.base, mailbox_arm_offset) == Some(arm)
}

fn write_renderer_mailbox_word(base: u64, offset: u64, value: u64) -> Result<(), &'static str> {
    super::scheduler_test::write_mailbox_word(base, offset, value)
}

fn supervisor_mailbox_matches(task: &supervisor_task, state: u64) -> bool {
    read_mailbox_word(task.space.physical_pages[1], supervisor_value_a_offset)
        == Some(supervisor_value_a)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_value_b_offset)
            == Some(supervisor_value_b)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_state_offset) == Some(state)
}

fn gate_registers_are_exact(frame: scheduler_frame, operation: u64, mailbox: u64) -> bool {
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
        && frame.register_15 == mailbox
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
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        return Err("renderer_fault_without_runtime");
    }
    let runtime = unsafe { &mut *runtime_pointer };
    let stage = runtime.stage.as_ref().ok_or("renderer_stage_missing")?;
    if runtime.current_role() != Some(runtime_role::renderer)
        || runtime.renderer_ref()?.incarnation != 1
        || runtime.trace.preemptions != [0]
        || runtime.rite_phase != rite_phase::generation_1_fault_pending
        || runtime.trace.rows[0] != generation_1_rows
        || stage.task != runtime.renderer_ref()?.identity
        || stage.channel_generation != 1
        || stage.generation != 1
        || stage
            .stage
            .as_ref()
            .is_none_or(|stage| stage.rows_staged() != generation_1_rows || stage.is_complete())
        || !runtime.frame_is_valid_for_role(frame, runtime_role::renderer, true)
    {
        return Err("renderer_fault_state_invalid");
    }
    let expected_instruction =
        linked_user_address(ptr::addr_of!(x86_64_supervised_renderer_generation_1_fault))
            .ok_or("renderer_fault_instruction_invalid")?;
    let registers = unsafe { ptr::read(frame) };
    if !expected_invalid_opcode_matches(snapshot, registers, expected_instruction) {
        return Err("renderer_unexpected_user_exception");
    }
    let renderer_identity = runtime.renderer_ref()?.identity;
    let publication = runtime
        .supervisions
        .prepare_publication(
            &runtime.tasks,
            runtime.relationship,
            renderer_identity,
            supervision_outcome::invalid_instruction_fault,
        )
        .map_err(|_| "renderer_fault_publication_preflight_failed")?;
    let transition = runtime
        .scheduler
        .fault_current_and_wake(
            &mut runtime.tasks,
            renderer_identity,
            runtime.supervisor.identity,
        )
        .map_err(|_| "renderer_fault_wake_failed")?;
    if transition.task() != renderer_identity
        || transition.dispatch().task() != runtime.supervisor.identity
        || transition.dispatch().sequence() != 3
        || transition.dispatch().context() != scheduler_context::saved
        || publication.supervisor() != runtime.supervisor.identity
        || publication.observation().event_sequence() != 1
    {
        return Err("renderer_fault_wake_invalid");
    }
    publication.commit();
    let observation = runtime
        .supervisions
        .take_observation(
            &runtime.tasks,
            runtime.relationship,
            runtime.supervisor.identity,
        )
        .map_err(|_| "renderer_fault_observation_consume_failed")?;
    if observation.event_sequence() != 1
        || observation.incarnation() != renderer_identity
        || observation.outcome() != supervision_outcome::invalid_instruction_fault
    {
        return Err("renderer_fault_observation_invalid");
    }
    runtime.trace.dispatches[3] = transition.dispatch().sequence();
    runtime.trace.fault = Some(snapshot);
    runtime.trace.fault_observation = Some(observation);
    runtime.trace.fault_count = 1;
    runtime.trace.wake_count = 1;
    runtime.trace.observation_count = 1;
    let next = runtime.select_role(runtime_role::supervisor, true)?;
    unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_fault) };
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

fn expected_invalid_opcode_matches(
    snapshot: exceptions::exception_snapshot,
    registers: scheduler_frame,
    expected_instruction: u64,
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
        && snapshot.instruction_pointer == expected_instruction
        && snapshot.code_segment == u64::from(descriptors::user_code_selector)
        // Fault-class exceptions may set RF in the saved return flags.
        && (snapshot.flags == expected_fault_flags
            || snapshot.flags == expected_fault_flags | resume_flag)
        && snapshot.stack_pointer == renderer_stack_top
        && snapshot.stack_segment == Some(u64::from(descriptors::user_data_selector))
        && snapshot.fault_address.is_none()
}

pub(super) fn reject_unexpected_exception() -> ! {
    fail("renderer_unexpected_exception")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_supervised_renderer_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || rite_gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("renderer_rite_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("renderer_rite_gate_without_runtime");
    }
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
    runtime: &mut renderer_runtime,
    frame: *mut scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let phase = runtime.rite_phase;
    let renderer_identity = runtime.renderer_ref()?.identity;
    let incarnation = runtime.renderer_ref()?.incarnation;
    if runtime.current_role() != Some(runtime_role::renderer)
        || phase.incarnation() != Some(incarnation)
        || runtime.channel_generation != incarnation
        || !runtime.frame_is_valid_for_role(frame, runtime_role::renderer, true)
    {
        return Err("renderer_rite_runtime_state_invalid");
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || !gate_registers_are_exact(incoming, 0, renderer_mailbox_address)
        || expected_rite_return(phase) != Some(incoming.instruction_pointer)
        || incoming.stack_pointer <= renderer_stack_address
        || incoming.stack_pointer >= renderer_stack_top
    {
        return Err("renderer_rite_frame_invalid");
    }
    let mailbox_alias = runtime.renderer_ref()?.space.mailbox_physical.base;
    let call = unsafe { read_call(mailbox_alias) }?;
    validate_rite_call(phase, call)?;

    let admission = match runtime
        .channel
        .as_mut()
        .ok_or("renderer_rite_channel_missing")?
        .assess(&runtime.tasks, renderer_identity, call)
        .map_err(|_| "renderer_rite_assessment_failed")?
    {
        rite_decision::admitted(admission) => admission,
        rite_decision::rejected(_) => return Err("renderer_fixed_rite_rejected"),
    };
    unsafe {
        copy_admitted_body(
            mailbox_alias,
            renderer_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copy_count,
            &mut runtime.trace.body_bytes_copied,
        )
    }?;

    if let Some(row) = phase.row() {
        let patch = decode_display_patch(&runtime.scratch);
        let pixels = display_patch_pixels(&runtime.scratch, admission.payload_size())?;
        if !patch.has_supported_body()
            || patch.x() != 0
            || patch.y() != row
            || patch.width() != rgb8_frame_width as u32
            || patch.height() != 1
            || patch.row_bytes() != rgb8_frame_row_bytes as u32
            || patch.pixel_byte_length() != rgb8_frame_row_bytes as u32
            || pixels.len() != rgb8_frame_row_bytes
        {
            return Err("renderer_display_patch_invalid");
        }
        let generation = phase.generation().ok_or("renderer_generation_missing")?;
        let stage_owner = runtime.stage.as_mut().ok_or("renderer_stage_missing")?;
        if stage_owner.task != renderer_identity
            || stage_owner.channel_generation != incarnation
            || stage_owner.generation != incarnation
            || usize::try_from(row).ok()
                != stage_owner
                    .stage
                    .as_ref()
                    .map(rgb8_frame_stage::rows_staged)
        {
            return Err("renderer_stage_correlation_invalid");
        }
        let stage = stage_owner
            .stage
            .as_mut()
            .ok_or("renderer_stage_owner_missing")?;
        let reply = admission
            .finish_ok(&mut runtime.tasks, |_, committed_renderer| {
                if committed_renderer != renderer_identity {
                    fail("renderer_display_identity_changed");
                }
                stage
                    .stage_row(row as usize, pixels)
                    .unwrap_or_else(|_| fail("renderer_stage_commit_failed"));
            })
            .map_err(|_| "renderer_display_completion_failed")?;
        validate_rite_reply(phase, reply)?;
        if !clear_and_verify_bytes(&mut runtime.scratch) {
            return Err("renderer_display_scratch_clear_failed");
        }
        let row_count = row as usize + 1;
        runtime.trace.rows[generation as usize - 1] = row_count;
        runtime.rite_phase = phase
            .next_after_row()
            .ok_or("renderer_rite_phase_overflow")?;
        unsafe { write_reply(mailbox_alias, reply) }?;
        runtime.trace.reply_copy_count += 1;
        compiler_fence(Ordering::SeqCst);
        return Ok(frame);
    }

    if phase != rite_phase::generation_2_exit {
        return Err("renderer_rite_idle_phase");
    }
    let payload = decode_exit_payload(&runtime.scratch)?;
    let stage_owner = runtime.stage.as_ref().ok_or("renderer_stage_missing")?;
    if !payload.has_supported_body()
        || payload.code != expected_exit_code
        || stage_owner.task != renderer_identity
        || stage_owner.channel_generation != 2
        || stage_owner.generation != 2
        || stage_owner
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != generation_2_rows)
    {
        return Err("renderer_exit_or_stage_invalid");
    }
    let supervisor_identity = runtime.supervisor.identity;
    let relationship = runtime.relationship;
    let publication = runtime
        .supervisions
        .prepare_publication(
            &runtime.tasks,
            relationship,
            renderer_identity,
            supervision_outcome::successful_exit,
        )
        .map_err(|_| "renderer_completion_publication_preflight_failed")?;
    let expected_observation = publication.observation();
    if publication.supervisor() != supervisor_identity
        || expected_observation.event_sequence() != 2
        || expected_observation.incarnation() != renderer_identity
        || expected_observation.outcome() != supervision_outcome::successful_exit
    {
        return Err("renderer_completion_publication_invalid");
    }
    let scheduler = &mut runtime.scheduler;
    let mut terminal_dispatch_sequence = None;
    let reply_copies_before = runtime.trace.reply_copy_count;
    let reply = admission
        .finish_ok(&mut runtime.tasks, |tasks, committed_renderer| {
            if committed_renderer != renderer_identity {
                fail("renderer_exit_identity_changed");
            }
            let transition = scheduler
                .complete_current_and_wake(tasks, committed_renderer, supervisor_identity)
                .unwrap_or_else(|_| fail("renderer_completion_wake_failed"));
            if transition.task() != renderer_identity
                || transition.dispatch().task() != supervisor_identity
                || transition.dispatch().sequence() != 5
                || transition.dispatch().context() != scheduler_context::saved
            {
                fail("renderer_completion_wake_invalid");
            }
            terminal_dispatch_sequence = Some(transition.dispatch().sequence());
            publication.commit();
        })
        .map_err(|_| "renderer_exit_completion_failed")?;
    validate_rite_reply(phase, reply)?;
    if terminal_dispatch_sequence != Some(5)
        || !clear_and_verify_bytes(&mut runtime.scratch)
        || runtime.trace.reply_copy_count != reply_copies_before
    {
        return Err("renderer_terminal_commit_invalid");
    }
    let observation = runtime
        .supervisions
        .take_observation(&runtime.tasks, relationship, supervisor_identity)
        .map_err(|_| "renderer_completion_observation_consume_failed")?;
    if observation != expected_observation {
        return Err("renderer_completion_observation_invalid");
    }
    runtime.rite_phase = rite_phase::idle;
    runtime.trace.completion_observation = Some(observation);
    runtime.trace.dispatches[5] = 5;
    runtime.trace.wake_count = 2;
    runtime.trace.observation_count = 2;
    runtime.trace.completion_count = 1;
    runtime.trace.terminal_exit_count = 1;
    runtime.trace.terminal_reply_copied = false;
    let next = runtime.select_role(runtime_role::supervisor, true)?;
    if next == frame {
        return Err("renderer_terminal_frame_reused");
    }
    unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_completion) };
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

fn expected_rite_return(phase: rite_phase) -> Option<u64> {
    match phase {
        rite_phase::generation_1_row(_) | rite_phase::generation_2_row(_) => linked_user_address(
            ptr::addr_of!(x86_64_supervised_renderer_display_rite_return),
        ),
        rite_phase::generation_2_exit => linked_user_address(ptr::addr_of!(
            x86_64_supervised_renderer_task_exit_rite_return
        )),
        rite_phase::generation_1_fault_pending | rite_phase::idle => None,
    }
}

fn validate_rite_call(phase: rite_phase, call: rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.reserved != 0
        || !super::rite_transport::payload_span_fits(renderer_mailbox_address, call.payload_size)
    {
        return Err("renderer_rite_call_header_invalid");
    }
    let exact = if let Some(row) = phase.row() {
        call.operation == rite_operation::display_update
            && call.payload_kind == rite_payload_kind::display_patch
            && call.rights_present == rite_rights::display_update
            && call.payload_size == display_payload_size
            && call.payload_limit == display_payload_size
            && call.trace_id
                == display_trace(
                    phase.generation().ok_or("renderer_generation_missing")?,
                    row,
                )
    } else if phase == rite_phase::generation_2_exit {
        call.operation == rite_operation::task_exit
            && call.payload_kind == rite_payload_kind::exit_status
            && call.rights_present == rite_rights::task_exit
            && call.payload_size == exit_payload_size
            && call.payload_limit == exit_payload_size
            && call.trace_id == exit_trace()
    } else {
        false
    };
    if !exact {
        return Err("renderer_rite_call_shape_invalid");
    }
    Ok(())
}

fn validate_rite_reply(phase: rite_phase, reply: rite_reply) -> Result<(), &'static str> {
    let (sequence, trace) = if let Some(row) = phase.row() {
        (
            u64::from(row) + 1,
            display_trace(
                phase.generation().ok_or("renderer_generation_missing")?,
                row,
            ),
        )
    } else if phase == rite_phase::generation_2_exit {
        (generation_2_rows as u64 + 1, exit_trace())
    } else {
        return Err("renderer_rite_reply_phase_invalid");
    };
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence != sequence
        || reply.trace_id != trace
    {
        return Err("renderer_rite_reply_invalid");
    }
    Ok(())
}

fn display_trace(generation: u8, row: u32) -> id_128 {
    let encoded_row = row.to_le_bytes();
    id_128::from_bytes([
        b'r',
        b'e',
        b'n',
        b'd',
        generation,
        0,
        0,
        0,
        encoded_row[0],
        encoded_row[1],
        encoded_row[2],
        encoded_row[3],
        b'r',
        b'o',
        b'w',
        0,
    ])
}

const fn exit_trace() -> id_128 {
    id_128::from_bytes(*b"renderer-exit-v2")
}

fn emit_evidence(evidence: renderer_evidence) {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.ready","level":"info","arch":"x86_64","grant":20,"image_pages":2,"mappings":3,"mailbox_pages":1,"stack_pages":4,"stage_frames":576,"supervisors":1,"renderer_limit":1,"restart_budget":1,"timer_budget":1,"nmi_masked":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.renderer.entered","level":"info","generation":1,"origin":"user","image_pages":2,"mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true,"stage_generation":1,"sequence_start":1}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.renderer.preempted","level":"info","generation":1,"timer_interrupts":1,"timer_masked":true,"flags_normalized":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.generation.staged","level":"info","generation":1,"rows":384,"complete":false,"surface_unchanged":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.generation.faulted","level":"info","generation":1,"vector":6,"kind":"invalid_opcode","contained":true,"supervisor_woken":true,"fault_site_verified":true,"saved_frame_verified":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.generation.released","level":"info","generation":1,"stage_cleared":true,"channel_retired":true,"mappings_removed":true,"root_withdrawn":true,"privilege_stack_wiped":true,"mailbox_wiped":true,"image_wiped":true,"scratch_wiped":true,"task_destroyed":true,"circle_destroyed":true,"surface_unchanged":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.renderer.restarted","level":"info","generation":2,"fresh_task":true,"fresh_circle":true,"fresh_channel":true,"fresh_stage":true,"sequence_start":1}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.generation.staged","level":"info","generation":2,"rows":768,"complete":true,"surface_unchanged":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.renderer.exited","level":"info","generation":2,"terminal":true,"replied":false,"supervisor_woken":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.supervisor.completed","level":"info","observations":2,"faults":1,"restarts":1,"completions":1,"success":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.frame.committed","level":"info","frames":1,"rows":768,"surface_withdrawn":true,"preflight_complete":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.released","level":"info","stages":2,"tasks":2,"circles":2,"channels":2,"free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_leases":0,"bound_leases":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_endpoints":0,"active_sigils":0,"active_bindings":0,"active_lineages":0,"active_supervisions":0,"supervision_children":0,"scheduler_depth":0,"attached_contexts":0,"active_rite_channels":0,"stage_cleared":true,"resources_restored":true,"interrupts_masked":true,"supervisor_retired":true,"channels_retired":true,"mailboxes_wiped":true,"stacks_wiped":true,"images_wiped":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}}"#,
        evidence.free_frames
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.passed","level":"info","generations":2,"timer_interrupts":1,"rows_generation_1":384,"rows_generation_2":768,"faults":1,"restarts":1,"terminal_exits":1,"frame_commits":1,"resources_restored":true}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.supervised_renderer_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
