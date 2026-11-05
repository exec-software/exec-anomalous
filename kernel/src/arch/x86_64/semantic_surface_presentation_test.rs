//! One-attempt native semantic-surface render-and-present composition proof.
//!
//! A narrowly granted source queues one copied ordinal-61 snapshot and exits.
//! A separately linked renderer consumes that body, stages exactly one complete
//! RGB8 frame, and exits under one persistent supervisor. Delivery, stage
//! completion, and successful presentation remain distinct ownership events.

use anomalous_abi::{
    id_128, rite_call, rite_exit_payload, rite_operation, rite_payload_kind, rite_reply,
    rite_rights, rite_semantic_surface_snapshot_v1_size, rite_status,
};
use anomalous_desktop_transport::{
    surface_snapshot_binding, surface_snapshot_body_v1, surface_snapshot_v1_byte_count,
    transport_identity,
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
use crate::rite::{rite_admission, rite_channel, rite_decision};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::semantic_surface::{
    semantic_surface_binding, semantic_surface_delivery_receipt, semantic_surface_endpoint,
    semantic_surface_endpoint_identity, semantic_surface_error,
};
use crate::supervision::{
    supervision_observation, supervision_outcome, supervision_phase, supervision_table,
};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};
use crate::time::monotonic_instant;

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_display_patch,
    decode_exit_payload, decode_semantic_surface_request, display_patch_pixels, payload_span_fits,
    prepare_semantic_surface_delivery_reply, prepare_semantic_surface_empty_reply, read_call,
    reply_size, rite_scratch, scratch_is_zero, semantic_surface_request_size,
    semantic_surface_snapshot_size, staged_semantic_surface_reply, write_reply,
};
use super::scheduler_test::{
    context_size, cr0_task_switched, frame_address_matches_stack, gate_registers_are_canonical,
    initial_user_flags, prepare_task_space as prepare_supervisor_space, privilege_stack_pages,
    read_mailbox_word, release_task_space as release_supervisor_space, relocated_user_address,
    scheduler_frame, scheduler_gate_vector, user_mailbox_address as supervisor_mailbox_address,
    user_return_fields_are_valid_with_stack_top, user_return_fields_are_valid_within_stack,
    user_stack_top as supervisor_stack_top, x86_64_scheduler_enter, x86_64_scheduler_resume_kernel,
};
use super::{descriptors, exceptions, interrupts, paging};

const page_size: u64 = frame_size;
const user_image_address: u64 = 0x0000_6000_0000_1000;
const user_image_page_limit: usize = 64;
const user_mailbox_address: u64 = 0x0000_6000_0008_0000;
const user_stack_address: u64 = 0x0000_6000_0009_0000;
const user_stack_pages: usize = 4;
const user_stack_top: u64 = user_stack_address + user_stack_pages as u64 * page_size;
const rite_gate_vector: u64 = 0xf3;

const scheduler_capacity: usize = 2;
const source_principal: u64 = 1;
const renderer_principal: u64 = 2;
const supervisor_principal: u64 = 3;
const endpoint_slot: usize = 0;
const endpoint_generation: u64 = 1;
const stage_frame_count: usize = 576;
const expected_rows: usize = 768;
const display_payload_size: u32 = 3112;
const exit_payload_size: u32 = 8;
const expected_exit_code: i32 = 0;
const source_completion_status: u64 = 0x71;
const presentation_completion_status: u64 = 0x72;

const surface_slot: u64 = 51;
const surface_generation: u64 = 3;
const scene_slot: u64 = 41;
const scene_generation: u64 = 7;
const base_revision: u64 = 1;
const revision: u64 = 2;
const renderer_slot: u64 = 61;
const renderer_generation: u64 = 1;
const delivery_attempt: u64 = 1;

const source_grant: u64 = rite_rights::semantic_surface_submit | rite_rights::task_exit;
const renderer_grant: u64 =
    rite_rights::semantic_surface_read | rite_rights::display_update | rite_rights::task_exit;

const source_read_trace: u8 = 0x11;
const source_malformed_trace: u8 = 0x12;
const source_freshness_trace: u8 = 0x13;
const source_submit_trace: u8 = 0x14;
const source_backpressure_trace: u8 = 0x15;
const source_exit_trace: u8 = 0x16;
const supervisor_value_a_offset: u64 = 0;
const supervisor_value_b_offset: u64 = 8;
const supervisor_state_offset: u64 = 16;
const supervisor_value_a: u64 = 0x8d6b_19e2_4af5_30c7;
const supervisor_value_b: u64 = 0x31c7_e894_5db2_0fa6;
const supervisor_continuity: u64 = 0x6ab4_3d09_f172_c85e;
const operation_supervisor_await_completion: u64 = 2;
const operation_supervisor_complete: u64 = 5;
const wake_completion: u64 = 0x42;

const _: () = {
    assert!(page_size == 4096);
    assert!(call_size == 40);
    assert!(reply_size == 40);
    assert!(semantic_surface_request_size == 16);
    assert!(semantic_surface_snapshot_size == 528);
    assert!(surface_snapshot_v1_byte_count == 528);
    assert!(rite_semantic_surface_snapshot_v1_size == 528);
    assert!(privilege_stack_pages == 4);
    assert!(descriptors::privilege_stack_size == 16 * 1024);
    assert!(source_grant == 0x44);
    assert!(renderer_grant == 0x94);
    assert!(rgb8_frame_width == 1024);
    assert!(rgb8_frame_height == expected_rows);
    assert!(rgb8_frame_row_bytes == 3072);
    assert!(rgb8_frame_byte_length == stage_frame_count * page_size as usize);
    assert!(display_payload_size as usize == 40 + rgb8_frame_row_bytes);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum fixture_role {
    source,
    renderer,
    supervisor,
}

impl fixture_role {
    const fn grant(self) -> u64 {
        match self {
            Self::source => source_grant,
            Self::renderer => renderer_grant,
            Self::supervisor => 0,
        }
    }

    const fn principal(self) -> u64 {
        match self {
            Self::source => source_principal,
            Self::renderer => renderer_principal,
            Self::supervisor => supervisor_principal,
        }
    }

    const fn origin(self) -> synthetic_origin_metadata {
        match self {
            Self::source => synthetic_origin_metadata::human_role_fixture,
            Self::renderer | Self::supervisor => synthetic_origin_metadata::machine_role_fixture,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum presentation_phase {
    source_read_denied,
    source_malformed_submit,
    source_freshness_denied,
    source_submit,
    source_backpressured,
    source_exit,
    source_retired,
    renderer_submit_denied,
    renderer_read,
    renderer_row(u32),
    renderer_exit,
    idle,
}

impl presentation_phase {
    const fn role(self) -> Option<fixture_role> {
        match self {
            Self::source_read_denied
            | Self::source_malformed_submit
            | Self::source_freshness_denied
            | Self::source_submit
            | Self::source_backpressured
            | Self::source_exit => Some(fixture_role::source),
            Self::renderer_submit_denied
            | Self::renderer_read
            | Self::renderer_row(_)
            | Self::renderer_exit => Some(fixture_role::renderer),
            Self::source_retired | Self::idle => None,
        }
    }

    const fn sequence(self) -> Option<u64> {
        match self {
            Self::source_read_denied => Some(1),
            Self::source_malformed_submit => Some(2),
            Self::source_freshness_denied => Some(3),
            Self::source_submit => Some(4),
            Self::source_backpressured => Some(5),
            Self::source_exit => Some(6),
            Self::renderer_submit_denied => Some(1),
            Self::renderer_read => Some(2),
            Self::renderer_row(row) => Some(3 + row as u64),
            Self::renderer_exit => Some(3 + expected_rows as u64),
            Self::source_retired | Self::idle => None,
        }
    }

    const fn trace(self) -> Option<id_128> {
        match self {
            Self::renderer_submit_denied => return Some(renderer_submit_denied_trace()),
            Self::renderer_read => return Some(renderer_read_trace()),
            Self::renderer_row(row) => return Some(display_trace(row)),
            Self::renderer_exit => return Some(renderer_exit_trace()),
            _ => {}
        }
        let value = match self {
            Self::source_read_denied => source_read_trace,
            Self::source_malformed_submit => source_malformed_trace,
            Self::source_freshness_denied => source_freshness_trace,
            Self::source_submit => source_submit_trace,
            Self::source_backpressured => source_backpressure_trace,
            Self::source_exit => source_exit_trace,
            Self::renderer_submit_denied
            | Self::renderer_read
            | Self::renderer_row(_)
            | Self::renderer_exit => return None,
            Self::source_retired | Self::idle => return None,
        };
        Some(id_128::from_bytes([value; 16]))
    }
}

#[derive(Clone, Copy)]
struct payload_contract {
    start: *const u8,
    payload_end: *const u8,
    end: *const u8,
    entry: *const u8,
}

impl payload_contract {
    fn length(self) -> Option<usize> {
        (self.end as usize).checked_sub(self.start as usize)
    }

    fn payload_length(self) -> Option<usize> {
        (self.payload_end as usize).checked_sub(self.start as usize)
    }

    fn page_count(self) -> Option<usize> {
        let length = self.length()?;
        (length != 0).then_some(length.div_ceil(page_size as usize))
    }

    fn linked_user_address(self, symbol: *const u8) -> Option<u64> {
        let offset = (symbol as usize).checked_sub(self.start as usize)?;
        if offset >= self.payload_length()? {
            return None;
        }
        user_image_address.checked_add(offset as u64)
    }
}

struct task_space {
    root: address_space,
    image_physical: physical_range,
    mailbox_physical: physical_range,
    stack_physical: physical_range,
    privilege_stack: frame_lease,
    privilege_stack_range: physical_range,
    frame: *mut scheduler_frame,
    payload: payload_contract,
}

struct presentation_task {
    circle: circle_id,
    identity: task_identity,
    role: fixture_role,
    space: task_space,
}

struct supervisor_task {
    circle: circle_id,
    identity: task_identity,
    space: super::scheduler_test::task_space,
    wait_installed: bool,
}

struct presentation_stage {
    lease: frame_lease,
    stage: Option<rgb8_frame_stage<'static>>,
    task: task_identity,
    channel_generation: u64,
    generation: u64,
    acknowledged: bool,
}

struct render_delivery_owner {
    receipt: semantic_surface_delivery_receipt,
    task: task_identity,
    channel_generation: u64,
    stage_generation: u64,
}

struct render_acknowledgement {
    delivery: render_delivery_owner,
    observation: supervision_observation,
}

#[derive(Clone, Copy)]
struct presentation_trace {
    rite_calls: usize,
    body_copies: usize,
    body_bytes: usize,
    submissions: usize,
    deliveries: usize,
    backpressure: usize,
    denials: usize,
    malformed: usize,
    freshness_denials: usize,
    rows: usize,
    exits: usize,
    observations: usize,
    frame_commits: usize,
    stage_clears: usize,
    source_retired_before_delivery: bool,
    destination_header_published_last: bool,
    framebuffer_unchanged: bool,
    framebuffer_verified: bool,
    kernel_continuations: usize,
}

impl presentation_trace {
    const fn empty() -> Self {
        Self {
            rite_calls: 0,
            body_copies: 0,
            body_bytes: 0,
            submissions: 0,
            deliveries: 0,
            backpressure: 0,
            denials: 0,
            malformed: 0,
            freshness_denials: 0,
            rows: 0,
            exits: 0,
            observations: 0,
            frame_commits: 0,
            stage_clears: 0,
            source_retired_before_delivery: false,
            destination_header_published_last: false,
            framebuffer_unchanged: false,
            framebuffer_verified: false,
            kernel_continuations: 0,
        }
    }
}

struct presentation_runtime {
    bootstrap_presentation: bool,
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    supervisions: supervision_table,
    relationship: Option<supervision_id>,
    source: Option<presentation_task>,
    renderer: Option<presentation_task>,
    supervisor: supervisor_task,
    source_channel: Option<rite_channel>,
    renderer_channel: Option<rite_channel>,
    endpoint: Option<semantic_surface_endpoint>,
    binding: Option<semantic_surface_binding>,
    stage: Option<presentation_stage>,
    delivery: Option<render_delivery_owner>,
    acknowledgement: Option<render_acknowledgement>,
    active_role: Option<fixture_role>,
    phase: presentation_phase,
    scratch: rite_scratch,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    trace: presentation_trace,
}

#[derive(Clone, Copy)]
struct presentation_evidence {
    trace: presentation_trace,
    source_image_pages: usize,
    renderer_image_pages: usize,
    initial_frames: frame_accounting,
}

static active_runtime: AtomicPtr<presentation_runtime> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);
static rite_gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_semantic_surface_source_payload_start: u8;
    static x86_64_semantic_surface_source_payload_end: u8;
    static x86_64_semantic_surface_source_payload_size: u8;
    static x86_64_semantic_surface_source_entry: u8;
    static x86_64_semantic_surface_source_read_denied_return: u8;
    static x86_64_semantic_surface_source_malformed_submit_return: u8;
    static x86_64_semantic_surface_source_freshness_denied_return: u8;
    static x86_64_semantic_surface_source_submit_return: u8;
    static x86_64_semantic_surface_source_backpressured_return: u8;
    static x86_64_semantic_surface_source_exit_return: u8;

    static x86_64_semantic_surface_renderer_payload_start: u8;
    static x86_64_semantic_surface_renderer_payload_end: u8;
    static x86_64_semantic_surface_renderer_payload_size: u8;
    static x86_64_semantic_surface_renderer_entry: u8;
    static x86_64_semantic_surface_renderer_submit_denied_return: u8;
    static x86_64_semantic_surface_renderer_delivery_return: u8;
    static x86_64_semantic_surface_renderer_display_return: u8;
    static x86_64_semantic_surface_renderer_exit_return: u8;
    static x86_64_semantic_surface_renderer_failed: u8;

    static x86_64_semantic_surface_presentation_supervisor_start: u8;
    static x86_64_semantic_surface_presentation_supervisor_completion_return: u8;
    static x86_64_semantic_surface_presentation_supervisor_complete_return: u8;
    static x86_64_semantic_surface_presentation_supervisor_end: u8;
}

fn source_payload() -> payload_contract {
    let start = linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_payload_start));
    payload_contract {
        start,
        payload_end: linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_payload_end)),
        end: linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_payload_end)),
        entry: start,
    }
}

fn renderer_payload() -> payload_contract {
    let start = linked_symbol(ptr::addr_of!(
        x86_64_semantic_surface_renderer_payload_start
    ));
    payload_contract {
        start,
        payload_end: linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_payload_end)),
        end: linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_payload_end)),
        entry: start,
    }
}

#[inline(never)]
fn linked_symbol(symbol: *const u8) -> *const u8 {
    // Wrapper aliases are resolved only by the final kernel link. Keeping the
    // numeric address opaque prevents release optimization from treating two
    // distinct extern declarations as necessarily distinct allocations when
    // the linker intentionally resolves entry and payload-start to one byte.
    core::hint::black_box(symbol)
}

pub(super) fn run(framebuffer: framebuffer_info) -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("semantic_presentation_initial_state_invalid");
    }
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("semantic_presentation_interrupt_mask_invalid");
    }

    run_presentation(framebuffer, false);
    super::qemu_exit_success();
    super::halt_forever()
}

/// Render once before the external-interrupt backend has ever been prepared.
/// Every native task and temporary owner is destroyed before returning.
#[cfg(feature = "desktop_dev")]
pub(super) fn run_bootstrap_presentation(framebuffer: framebuffer_info) {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::prepare_bootstrap_presentation()
    {
        fail("desktop_bootstrap_presentation_state_invalid");
    }
    run_presentation(framebuffer, true);
    if !interrupts::bootstrap_presentation_backend_is_virgin() {
        fail("desktop_bootstrap_interrupt_state_changed");
    }
    // SAFETY: Presentation has returned, so its exclusive presenter is gone.
    // IF is still clear on the sole BSP. This starts a new bootstrap console
    // lifetime after the proof's consuming diagnostic-registration withdrawal.
    if !unsafe { crate::console::restore_bootstrap_interrupt_failure_surface(framebuffer) } {
        fail("desktop_bootstrap_failure_surface_restore_failed");
    }
}

fn run_presentation(framebuffer: framebuffer_info, bootstrap_presentation: bool) {
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Threshold keeps NMI masked and this proof is serialized on the
    // BSP until the complete feature runtime has been withdrawn.
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        run_with_allocator(
            allocator,
            kernel_cr3,
            bootstrap_privilege_stack_top,
            framebuffer,
            bootstrap_presentation,
        )
    });
    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact live root captured before feature entry.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    let evidence = match result {
        Ok(Ok(evidence)) => evidence,
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    };
    emit_passed(evidence);
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    framebuffer: framebuffer_info,
    bootstrap_presentation: bool,
) -> Result<presentation_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !interrupts::scheduler_interrupts_are_masked()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("semantic_presentation_setup_state_invalid");
    }
    let source_payload = validate_payload_contract(fixture_role::source)?;
    let renderer_payload = validate_payload_contract(fixture_role::renderer)?;
    let source_image_pages = source_payload
        .page_count()
        .ok_or("semantic_presentation_source_page_count_invalid")?;
    let renderer_image_pages = renderer_payload
        .page_count()
        .ok_or("semantic_presentation_renderer_page_count_invalid")?;
    let required_frames = source_image_pages
        .checked_add(renderer_image_pages)
        .and_then(|count| count.checked_add(stage_frame_count + 40))
        .ok_or("semantic_presentation_frame_requirement_overflow")?;
    let initial = allocator.accounting();
    if initial.free < required_frames
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("semantic_presentation_initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let source_circle = circles
        .create()
        .map_err(|_| "semantic_presentation_source_circle_create_failed")?;
    let renderer_circle = circles
        .create()
        .map_err(|_| "semantic_presentation_renderer_circle_create_failed")?;
    let supervisor_circle = circles
        .create()
        .map_err(|_| "semantic_presentation_supervisor_circle_create_failed")?;
    let source_space =
        prepare_task_space(allocator, kernel_cr3, fixture_role::source, source_payload)?;
    let renderer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        fixture_role::renderer,
        renderer_payload,
    )?;
    let supervisor_space = prepare_supervisor_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_start),
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_end),
    )?;
    let framebuffer_length = framebuffer_aperture_length(framebuffer)
        .ok_or("semantic_presentation_framebuffer_extent_invalid")?;
    validate_no_framebuffer_mapping(&source_space.root, framebuffer, framebuffer_length)?;
    validate_no_framebuffer_mapping(&renderer_space.root, framebuffer, framebuffer_length)?;
    validate_no_framebuffer_mapping(&supervisor_space.root, framebuffer, framebuffer_length)?;
    attach_task_space(&mut circles, source_circle)?;
    attach_task_space(&mut circles, renderer_circle)?;
    super::scheduler_test::attach_task_space(&mut circles, supervisor_circle)?;
    let source_identity = create_task(
        &mut circles,
        &mut tasks,
        source_circle,
        fixture_role::source,
        &source_space,
    )?;
    let renderer_identity = create_task(
        &mut circles,
        &mut tasks,
        renderer_circle,
        fixture_role::renderer,
        &renderer_space,
    )?;
    let supervisor_identity = super::scheduler_test::create_task(
        &mut circles,
        &mut tasks,
        supervisor_circle,
        supervisor_principal,
        supervisor_space.payload_length,
    )?;
    let snapshot_binding = expected_snapshot_binding()?;
    let endpoint_identity =
        semantic_surface_endpoint_identity::try_new(endpoint_slot, endpoint_generation)
            .map_err(|_| "semantic_presentation_endpoint_identity_invalid")?;
    let binding = semantic_surface_binding::try_new(
        endpoint_identity,
        source_identity,
        renderer_identity,
        snapshot_binding,
    )
    .map_err(|_| "semantic_presentation_binding_invalid")?;
    let endpoint = semantic_surface_endpoint::new(binding);
    let source_channel = rite_channel::try_new(source_identity, source_grant)
        .map_err(|_| "semantic_presentation_source_channel_create_failed")?;
    let renderer_channel = rite_channel::try_new(renderer_identity, renderer_grant)
        .map_err(|_| "semantic_presentation_renderer_channel_create_failed")?;
    let stage = allocate_stage(allocator, renderer_identity, 1, 1)?;
    validate_stage_is_supervisor_only(&source_space.root, &stage)?;
    validate_stage_is_supervisor_only(&renderer_space.root, &stage)?;
    validate_stage_is_supervisor_only(&supervisor_space.root, &stage)?;
    let mut scheduler = scheduler_table::new(scheduler_capacity)
        .map_err(|_| "semantic_presentation_scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, source_identity)
        .map_err(|_| "semantic_presentation_source_admission_failed")?;
    let supervisions = supervision_table::new();

    seed_framebuffer_baseline(framebuffer)?;
    let mut runtime = presentation_runtime {
        bootstrap_presentation,
        circles,
        tasks,
        scheduler,
        supervisions,
        relationship: None,
        source: Some(presentation_task {
            circle: source_circle,
            identity: source_identity,
            role: fixture_role::source,
            space: source_space,
        }),
        renderer: Some(presentation_task {
            circle: renderer_circle,
            identity: renderer_identity,
            role: fixture_role::renderer,
            space: renderer_space,
        }),
        supervisor: supervisor_task {
            circle: supervisor_circle,
            identity: supervisor_identity,
            space: supervisor_space,
            wait_installed: false,
        },
        source_channel: Some(source_channel),
        renderer_channel: Some(renderer_channel),
        endpoint: Some(endpoint),
        binding: Some(binding),
        stage: Some(stage),
        delivery: None,
        acknowledgement: None,
        active_role: None,
        phase: presentation_phase::source_read_denied,
        scratch: [0; page_size as usize],
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        trace: presentation_trace::empty(),
    };

    validate_ready_state(
        &runtime,
        initial,
        allocator.accounting(),
        source_image_pages,
        renderer_image_pages,
    )?;
    emit_ready(source_image_pages, renderer_image_pages);
    enter_source(&mut runtime)?;
    runtime.trace.kernel_continuations += 1;
    if runtime.phase != presentation_phase::source_retired
        || runtime.trace.submissions != 1
        || runtime.trace.backpressure != 1
        || runtime.trace.exits != 1
    {
        return Err("semantic_presentation_source_completion_invalid");
    }
    retire_source(allocator, &mut runtime)?;
    runtime.trace.source_retired_before_delivery = true;
    emit_source_released();

    runtime
        .scheduler
        .admit(
            &mut runtime.circles,
            &mut runtime.tasks,
            supervisor_identity,
        )
        .map_err(|_| "semantic_presentation_supervisor_admission_failed")?;
    runtime
        .scheduler
        .admit(&mut runtime.circles, &mut runtime.tasks, renderer_identity)
        .map_err(|_| "semantic_presentation_renderer_admission_failed")?;
    let relationship = runtime
        .supervisions
        .create(
            &mut runtime.circles,
            &runtime.tasks,
            supervisor_identity,
            renderer_identity,
        )
        .map_err(|_| "semantic_presentation_supervision_create_failed")?;
    runtime.relationship = Some(relationship);
    validate_supervised_renderer_ready(&runtime, relationship)?;
    runtime.phase = presentation_phase::renderer_submit_denied;
    enter_supervised_renderer(&mut runtime)?;
    runtime.trace.kernel_continuations += 1;
    if runtime.phase != presentation_phase::idle
        || runtime.trace.deliveries != 1
        || runtime.trace.rows != expected_rows
        || runtime.trace.exits != 2
        || runtime.trace.observations != 1
        || runtime.acknowledgement.is_none()
        || !runtime.trace.source_retired_before_delivery
        || !runtime.trace.destination_header_published_last
    {
        return Err("semantic_presentation_renderer_completion_invalid");
    }
    emit_render_acknowledged();

    if !framebuffer_baseline_is_intact(framebuffer)? {
        return Err("semantic_presentation_framebuffer_changed_before_commit");
    }
    runtime.trace.framebuffer_unchanged = true;
    teardown_prepared_runtime(allocator, &mut runtime)?;
    validate_prepublication_teardown(allocator, &runtime, initial)?;
    emit_prepared();
    publish_complete_frame(allocator, &mut runtime, framebuffer)?;
    emit_presented();

    let final_frames = allocator.accounting();
    if final_frames != initial
        || runtime.stage.is_some()
        || runtime.trace.frame_commits != 1
        || runtime.trace.stage_clears != 1
        || !runtime.trace.framebuffer_verified
    {
        return Err("semantic_presentation_final_restoration_failed");
    }
    emit_released(final_frames);
    let trace = runtime.trace;
    retire_runtime_metadata(&mut runtime);
    Ok(presentation_evidence {
        trace,
        source_image_pages,
        renderer_image_pages,
        initial_frames: initial,
    })
}

fn expected_snapshot_binding() -> Result<surface_snapshot_binding, &'static str> {
    let surface = transport_identity::try_new(surface_slot, surface_generation)
        .ok_or("semantic_presentation_surface_identity_invalid")?;
    let scene = transport_identity::try_new(scene_slot, scene_generation)
        .ok_or("semantic_presentation_scene_identity_invalid")?;
    let renderer = transport_identity::try_new(renderer_slot, renderer_generation)
        .ok_or("semantic_presentation_renderer_identity_invalid")?;
    surface_snapshot_binding::try_new(
        surface,
        scene,
        base_revision,
        revision,
        renderer,
        delivery_attempt,
    )
    .map_err(|_| "semantic_presentation_snapshot_binding_invalid")
}

fn validate_payload_contract(role: fixture_role) -> Result<payload_contract, &'static str> {
    let payload = match role {
        fixture_role::source => source_payload(),
        fixture_role::renderer => renderer_payload(),
        fixture_role::supervisor => return Err("semantic_presentation_payload_role_invalid"),
    };
    let length = payload
        .length()
        .ok_or("semantic_presentation_payload_extent_invalid")?;
    let payload_length = payload
        .payload_length()
        .ok_or("semantic_presentation_payload_extent_invalid")?;
    let pages = payload
        .page_count()
        .ok_or("semantic_presentation_payload_page_count_invalid")?;
    if pages > user_image_page_limit
        || payload_length == 0
        || payload_length > length
        || payload.linked_user_address(payload.entry) != Some(user_image_address)
    {
        return Err("semantic_presentation_payload_contract_invalid");
    }
    let linked_size = match role {
        fixture_role::source => {
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_payload_size))
        }
        fixture_role::renderer => {
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_payload_size))
        }
        fixture_role::supervisor => return Err("semantic_presentation_payload_role_invalid"),
    };
    if linked_size as usize != length {
        return Err("semantic_presentation_source_size_contract_invalid");
    }
    let symbols: &[*const u8] = match role {
        fixture_role::source => &[
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_read_denied_return
            )),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_malformed_submit_return
            )),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_freshness_denied_return
            )),
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_submit_return)),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_backpressured_return
            )),
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_exit_return)),
        ],
        fixture_role::renderer => &[
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_failed)),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_submit_denied_return
            )),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_delivery_return
            )),
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_display_return
            )),
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_exit_return)),
        ],
        fixture_role::supervisor => return Err("semantic_presentation_payload_role_invalid"),
    };
    if symbols
        .iter()
        .any(|symbol| payload.linked_user_address(*symbol).is_none())
        || symbols
            .windows(2)
            .any(|pair| pair[0] as usize >= pair[1] as usize)
    {
        return Err("semantic_presentation_payload_symbol_contract_invalid");
    }
    Ok(payload)
}

fn allocate_data(
    allocator: &mut frame_allocator<'_>,
    count: usize,
    alignment: usize,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator grants exclusive access to this exact whole-frame
    // extent for the duration of the synchronous clearing callback.
    unsafe {
        allocator.allocate_with(count, alignment, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    role: fixture_role,
    payload: payload_contract,
) -> Result<task_space, &'static str> {
    let image_pages = payload
        .page_count()
        .ok_or("semantic_presentation_payload_page_count_invalid")?;
    let image = allocate_data(
        allocator,
        image_pages,
        1,
        "semantic_presentation_image_allocation_failed",
    )?;
    let mailbox = allocate_data(
        allocator,
        1,
        1,
        "semantic_presentation_mailbox_allocation_failed",
    )?;
    let stack = allocate_data(
        allocator,
        user_stack_pages,
        user_stack_pages,
        "semantic_presentation_stack_allocation_failed",
    )?;
    let privilege_stack = allocate_data(
        allocator,
        privilege_stack_pages,
        privilege_stack_pages,
        "semantic_presentation_privilege_stack_allocation_failed",
    )?;
    let image_physical = image
        .range()
        .ok_or("semantic_presentation_image_extent_missing")?;
    let mailbox_physical = mailbox
        .range()
        .ok_or("semantic_presentation_mailbox_extent_missing")?;
    let stack_physical = stack
        .range()
        .ok_or("semantic_presentation_stack_extent_missing")?;
    let privilege_stack_range = privilege_stack
        .range()
        .ok_or("semantic_presentation_privilege_stack_extent_missing")?;
    if image_physical.length != image_pages as u64 * page_size
        || mailbox_physical.length != page_size
        || stack_physical.length != user_stack_pages as u64 * page_size
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
        return Err("semantic_presentation_task_frames_invalid");
    }
    copy_payload(payload, image_physical)?;

    // SAFETY: The allocator is exclusively borrowed and clears every newly
    // allocated page-table extent before publication.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "semantic_presentation_address_space_create_failed")?;
    root.map_owned_extent(
        allocator,
        user_image_address,
        image,
        paging::page_permissions {
            user: true,
            writable: false,
            executable: true,
        },
    )
    .map_err(|_| "semantic_presentation_image_mapping_failed")?;
    root.map_owned(
        allocator,
        user_mailbox_address,
        mailbox,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "semantic_presentation_mailbox_mapping_failed")?;
    root.map_owned_extent(
        allocator,
        user_stack_address,
        stack,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "semantic_presentation_stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "semantic_presentation_mapping_validation_failed")?;
    for page in 0..image_pages {
        validate_mapping(
            root.validate_mapping(user_image_address + page as u64 * page_size)
                .map_err(|_| "semantic_presentation_image_mapping_walk_failed")?,
            image_physical.base + page as u64 * page_size,
            false,
            true,
        )?;
    }
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "semantic_presentation_mailbox_mapping_walk_failed")?,
        mailbox_physical.base,
        true,
        false,
    )?;
    for page in 0..user_stack_pages {
        validate_mapping(
            root.validate_mapping(user_stack_address + page as u64 * page_size)
                .map_err(|_| "semantic_presentation_stack_mapping_walk_failed")?,
            stack_physical.base + page as u64 * page_size,
            true,
            false,
        )?;
    }
    for guard in [
        user_image_address - page_size,
        user_image_address + image_pages as u64 * page_size,
        user_mailbox_address - page_size,
        user_mailbox_address + page_size,
        user_stack_address - page_size,
        user_stack_top,
    ] {
        root.validate_absent(guard)
            .map_err(|_| "semantic_presentation_guard_mapping_present")?;
    }
    if !payload_matches(payload, image_physical)
        || (role == fixture_role::renderer
            && payload_contains_snapshot(payload, image_physical, None))
    {
        return Err("semantic_presentation_payload_copy_invalid");
    }

    allocator
        .bind(&privilege_stack)
        .map_err(|_| "semantic_presentation_privilege_stack_binding_failed")?;
    let privilege_stack_top = privilege_stack_range
        .end()
        .ok_or("semantic_presentation_privilege_stack_extent_invalid")?;
    let frame_address = privilege_stack_top
        .checked_sub(context_size)
        .ok_or("semantic_presentation_initial_frame_address_invalid")?;
    if frame_address < privilege_stack_range.base
        || frame_address & (mem::align_of::<u64>() as u64 - 1) != 0
    {
        return Err("semantic_presentation_initial_frame_address_invalid");
    }
    let entry = payload
        .linked_user_address(payload.entry)
        .ok_or("semantic_presentation_entry_invalid")?;
    let frame = frame_address as usize as *mut scheduler_frame;
    let mut initial = scheduler_frame::initial(entry);
    initial.stack_pointer = user_stack_top;
    // SAFETY: The bound supervisor-only privilege stack owns this complete,
    // aligned scheduler-frame slot.
    unsafe { ptr::write_volatile(frame, initial) };
    compiler_fence(Ordering::SeqCst);
    Ok(task_space {
        root,
        image_physical,
        mailbox_physical,
        stack_physical,
        privilege_stack,
        privilege_stack_range,
        frame,
        payload,
    })
}

fn attach_task_space(circles: &mut circle_table, circle: circle_id) -> Result<(), &'static str> {
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "semantic_presentation_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "semantic_presentation_root_attachment_rollback_failed")?;
        return Err("semantic_presentation_mapping_attachment_failed");
    }
    Ok(())
}

fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    role: fixture_role,
    space: &task_space,
) -> Result<task_identity, &'static str> {
    let payload_length = space
        .payload
        .payload_length()
        .ok_or("semantic_presentation_task_payload_length_invalid")?;
    let code_end = user_image_address
        .checked_add(payload_length as u64)
        .ok_or("semantic_presentation_task_code_extent_invalid")?;
    // SAFETY: Preparation initialized this complete frame before task creation.
    let entry = unsafe { ptr::read_volatile(space.frame) }.instruction_pointer;
    let bounds = task_bounds::new(
        user_image_address,
        code_end,
        entry,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("semantic_presentation_task_bounds_invalid")?;
    let principal = fixture_principal_id::new(role.principal())
        .ok_or("semantic_presentation_task_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(circle, principal, role.origin(), bounds),
        )
        .map_err(|_| "semantic_presentation_task_create_failed")
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
        return Err("semantic_presentation_mapping_permissions_invalid");
    }
    Ok(())
}

fn copy_payload(
    payload: payload_contract,
    destination: physical_range,
) -> Result<(), &'static str> {
    let length = payload
        .payload_length()
        .ok_or("semantic_presentation_payload_extent_invalid")?;
    if length as u64 > destination.length {
        return Err("semantic_presentation_payload_destination_invalid");
    }
    let base = usize::try_from(destination.base)
        .map_err(|_| "semantic_presentation_payload_address_invalid")?;
    // SAFETY: The destination is a cleared exclusive allocation and the
    // linker contract bounds the source image extent.
    unsafe { ptr::copy_nonoverlapping(payload.start, base as *mut u8, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

fn payload_matches(payload: payload_contract, destination: physical_range) -> bool {
    let Some(length) = payload.payload_length() else {
        return false;
    };
    let Ok(base) = usize::try_from(destination.base) else {
        return false;
    };
    if length as u64 > destination.length {
        return false;
    }
    for offset in 0..length {
        let expected = unsafe { ptr::read(payload.start.add(offset)) };
        let actual = unsafe { ptr::read_volatile((base as *const u8).add(offset)) };
        if expected != actual {
            return false;
        }
    }
    (length..destination.length as usize)
        .all(|offset| unsafe { ptr::read_volatile((base as *const u8).add(offset)) == 0 })
}

fn payload_contains_snapshot(
    payload: payload_contract,
    range: physical_range,
    expected: Option<&[u8; semantic_surface_snapshot_size]>,
) -> bool {
    let Some(length) = payload.payload_length() else {
        return true;
    };
    let Ok(base) = usize::try_from(range.base) else {
        return true;
    };
    if length < semantic_surface_snapshot_size || length as u64 > range.length {
        return false;
    }
    for offset in 0..=length - semantic_surface_snapshot_size {
        let candidate = unsafe {
            core::slice::from_raw_parts(
                (base as *const u8).add(offset),
                semantic_surface_snapshot_size,
            )
        };
        if expected.is_some_and(|body| candidate == body)
            || surface_snapshot_body_v1::try_from_slice(candidate).is_ok()
        {
            return true;
        }
    }
    false
}

fn allocate_stage(
    allocator: &mut frame_allocator<'_>,
    task: task_identity,
    channel_generation: u64,
    generation: u64,
) -> Result<presentation_stage, &'static str> {
    if channel_generation == 0 || generation == 0 {
        return Err("semantic_presentation_stage_identity_invalid");
    }
    let lease = allocate_data(
        allocator,
        stage_frame_count,
        1,
        "semantic_presentation_stage_allocation_failed",
    )?;
    let range = lease
        .range()
        .ok_or("semantic_presentation_stage_extent_missing")?;
    if range.length != rgb8_frame_byte_length as u64 || !range_is_zero(range) {
        return Err("semantic_presentation_stage_extent_invalid");
    }
    allocator
        .bind(&lease)
        .map_err(|_| "semantic_presentation_stage_binding_failed")?;
    let base =
        usize::try_from(range.base).map_err(|_| "semantic_presentation_stage_address_invalid")?;
    // SAFETY: This bound exclusive data lease owns the complete fixed frame.
    let storage =
        unsafe { core::slice::from_raw_parts_mut(base as *mut u8, rgb8_frame_byte_length) };
    let stage = rgb8_frame_stage::try_new(storage)
        .map_err(|_| "semantic_presentation_stage_construction_failed")?;
    Ok(presentation_stage {
        lease,
        stage: Some(stage),
        task,
        channel_generation,
        generation,
        acknowledged: false,
    })
}

fn validate_stage_is_supervisor_only(
    root: &address_space,
    stage: &presentation_stage,
) -> Result<(), &'static str> {
    let range = stage
        .lease
        .range()
        .ok_or("semantic_presentation_stage_extent_missing")?;
    root.validate_borrowed_supervisor_range(range.base, range.length)
        .map_err(|_| "semantic_presentation_stage_user_accessible")
}

fn validate_no_framebuffer_mapping(
    root: &address_space,
    framebuffer: framebuffer_info,
    length: usize,
) -> Result<(), &'static str> {
    root.validate_borrowed_supervisor_range(framebuffer.address, length as u64)
        .map_err(|_| "semantic_presentation_framebuffer_user_accessible")
}

fn validate_ready_state(
    runtime: &presentation_runtime,
    initial: frame_accounting,
    frames: frame_accounting,
    source_pages: usize,
    renderer_pages: usize,
) -> Result<(), &'static str> {
    let source = runtime
        .source
        .as_ref()
        .ok_or("semantic_presentation_source_missing")?;
    let renderer = runtime
        .renderer
        .as_ref()
        .ok_or("semantic_presentation_renderer_missing")?;
    let stage = runtime
        .stage
        .as_ref()
        .ok_or("semantic_presentation_stage_missing")?;
    let accounting = runtime.circles.accounting();
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "semantic_presentation_ready_scheduler_invalid")?;
    if source_pages == 0
        || renderer_pages == 0
        || source.role != fixture_role::source
        || renderer.role != fixture_role::renderer
        || fixture_role::source.grant() != source_grant
        || fixture_role::renderer.grant() != renderer_grant
        || source.space.image_physical == renderer.space.image_physical
        || source.space.mailbox_physical == renderer.space.mailbox_physical
        || source.space.stack_physical == renderer.space.stack_physical
        || runtime.source_channel.is_none()
        || runtime.renderer_channel.is_none()
        || runtime
            .endpoint
            .as_ref()
            .is_none_or(|endpoint| !endpoint.storage_is_clear())
        || runtime
            .endpoint
            .as_ref()
            .zip(runtime.binding)
            .is_none_or(|(endpoint, binding)| endpoint.binding() != binding)
        || runtime.delivery.is_some()
        || runtime.acknowledgement.is_some()
        || !scratch_is_zero(&runtime.scratch)
        || stage.task != renderer.identity
        || stage.channel_generation != 1
        || stage.generation != 1
        || stage.acknowledged
        || stage
            .stage
            .as_ref()
            .is_none_or(|stage| stage.rows_staged() != 0)
        || accounting.circles != 3
        || accounting.tasks != 3
        || accounting.task_contexts != 1
        || accounting.supervisions != 0
        || accounting.address_spaces != 3
        || accounting.mappings != 9
        || runtime.tasks.active_count() != 3
        || runtime.tasks.attached_context_count() != 1
        || runtime.supervisions.active_count() != 0
        || runtime.relationship.is_some()
        || scheduler.admitted() != 1
        || scheduler.runnable() != 1
        || scheduler.current().is_some()
        || initial.free <= frames.free
        || frames.data_owned < stage_frame_count
        || frames.active_leases == 0
        || frames.bound_leases == 0
    {
        return Err("semantic_presentation_ready_state_invalid");
    }
    Ok(())
}

fn validate_supervised_renderer_ready(
    runtime: &presentation_runtime,
    relationship: supervision_id,
) -> Result<(), &'static str> {
    let renderer = runtime
        .renderer
        .as_ref()
        .ok_or("semantic_presentation_renderer_missing")?;
    let supervision = runtime
        .supervisions
        .snapshot(relationship)
        .map_err(|_| "semantic_presentation_supervision_ready_snapshot_invalid")?;
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "semantic_presentation_supervision_ready_scheduler_invalid")?;
    let accounting = runtime.circles.accounting();
    if runtime.relationship != Some(relationship)
        || supervision.phase() != supervision_phase::active_incarnation
        || supervision.supervisor() != runtime.supervisor.identity
        || supervision.active_incarnation() != Some(renderer.identity)
        || supervision.pending_observation().is_some()
        || runtime.supervisions.active_count() != 1
        || runtime.tasks.active_count() != 2
        || runtime.tasks.attached_context_count() != 2
        || accounting.circles != 2
        || accounting.tasks != 2
        || accounting.task_contexts != 2
        || accounting.supervisions != 1
        || accounting.address_spaces != 2
        || accounting.mappings != 6
        || scheduler.admitted() != 2
        || scheduler.runnable() != 2
        || scheduler.current().is_some()
    {
        return Err("semantic_presentation_supervision_ready_state_invalid");
    }
    Ok(())
}

impl presentation_runtime {
    fn relationship_value(&self) -> Result<supervision_id, &'static str> {
        self.relationship
            .ok_or("semantic_presentation_relationship_owner_missing")
    }

    fn binding_value(&self) -> Result<semantic_surface_binding, &'static str> {
        self.binding
            .ok_or("semantic_presentation_binding_owner_missing")
    }

    fn endpoint_ref(&self) -> Result<&semantic_surface_endpoint, &'static str> {
        self.endpoint
            .as_ref()
            .ok_or("semantic_presentation_endpoint_owner_missing")
    }

    fn endpoint_mut(&mut self) -> Result<&mut semantic_surface_endpoint, &'static str> {
        self.endpoint
            .as_mut()
            .ok_or("semantic_presentation_endpoint_owner_missing")
    }

    fn task(&self, role: fixture_role) -> Result<&presentation_task, &'static str> {
        match role {
            fixture_role::source => self
                .source
                .as_ref()
                .ok_or("semantic_presentation_source_missing"),
            fixture_role::renderer => self
                .renderer
                .as_ref()
                .ok_or("semantic_presentation_renderer_missing"),
            fixture_role::supervisor => Err("semantic_presentation_payload_task_role_invalid"),
        }
    }

    fn identity_for_role(&self, role: fixture_role) -> Result<task_identity, &'static str> {
        match role {
            fixture_role::source | fixture_role::renderer => Ok(self.task(role)?.identity),
            fixture_role::supervisor => Ok(self.supervisor.identity),
        }
    }

    fn role_for_identity(&self, identity: task_identity) -> Option<fixture_role> {
        if self
            .source
            .as_ref()
            .is_some_and(|task| task.identity == identity)
        {
            Some(fixture_role::source)
        } else if self
            .renderer
            .as_ref()
            .is_some_and(|task| task.identity == identity)
        {
            Some(fixture_role::renderer)
        } else if self.supervisor.identity == identity {
            Some(fixture_role::supervisor)
        } else {
            None
        }
    }

    fn current_role(&self) -> Option<fixture_role> {
        let current = self.scheduler.snapshot(&self.tasks).ok()?.current()?;
        let role = self.role_for_identity(current)?;
        (self.active_role == Some(role)).then_some(role)
    }

    fn select_role(
        &mut self,
        role: fixture_role,
        require_task_switched: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let (root, privilege_stack, frame) = match role {
            fixture_role::source | fixture_role::renderer => {
                let task = self.task(role)?;
                (
                    &task.space.root,
                    task.space.privilege_stack_range,
                    task.space.frame,
                )
            }
            fixture_role::supervisor => (
                &self.supervisor.space.root,
                self.supervisor.space.privilege_stack_range,
                self.supervisor.space.frame,
            ),
        };
        let root_address = root
            .root_address()
            .map_err(|_| "semantic_presentation_selected_root_missing")?;
        let stack_top = privilege_stack
            .end()
            .ok_or("semantic_presentation_selected_stack_extent_invalid")?;
        // SAFETY: The selected task owns this complete root and bound stack.
        unsafe { root.activate() }
            .map_err(|_| "semantic_presentation_selected_root_activation_failed")?;
        // SAFETY: The exact privilege stack remains bound until retirement.
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("semantic_presentation_selected_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root_address
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || !self.frame_is_valid_for_role(frame, role, require_task_switched)
        {
            return Err("semantic_presentation_selected_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: fixture_role,
        require_task_switched: bool,
    ) -> bool {
        let (expected_frame, privilege_stack, root, identity, expected_stack_top) = match role {
            fixture_role::source | fixture_role::renderer => {
                let Ok(task) = self.task(role) else {
                    return false;
                };
                (
                    task.space.frame,
                    task.space.privilege_stack_range,
                    &task.space.root,
                    task.identity,
                    user_stack_top,
                )
            }
            fixture_role::supervisor => (
                self.supervisor.space.frame,
                self.supervisor.space.privilege_stack_range,
                &self.supervisor.space.root,
                self.supervisor.identity,
                supervisor_stack_top,
            ),
        };
        if frame != expected_frame
            || !frame_address_matches_stack(frame, privilege_stack)
            || root.validate_all_mappings().is_err()
        {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(identity) else {
            return false;
        };
        if snapshot.state() != task_state::running || !snapshot.context_attached() {
            return false;
        }
        if root.root_address().ok() != Some(paging::entry_address(paging::read_cr3()))
            || privilege_stack.end() != Some(unsafe { descriptors::selected_privilege_stack_top() })
            || (require_task_switched && paging::read_cr0() & cr0_task_switched == 0)
        {
            return false;
        }
        let saved = unsafe { ptr::read(frame) };
        if require_task_switched {
            user_return_fields_are_valid_within_stack(saved, snapshot.bounds())
        } else {
            user_return_fields_are_valid_with_stack_top(
                saved,
                snapshot.bounds(),
                expected_stack_top,
            )
        }
    }

    fn take_channel(&mut self, role: fixture_role) -> Result<rite_channel, &'static str> {
        match role {
            fixture_role::source => self
                .source_channel
                .take()
                .ok_or("semantic_presentation_source_channel_missing"),
            fixture_role::renderer => self
                .renderer_channel
                .take()
                .ok_or("semantic_presentation_renderer_channel_missing"),
            fixture_role::supervisor => Err("semantic_presentation_supervisor_has_no_channel"),
        }
    }

    fn restore_channel(&mut self, role: fixture_role, channel: rite_channel) {
        let destination = match role {
            fixture_role::source => &mut self.source_channel,
            fixture_role::renderer => &mut self.renderer_channel,
            fixture_role::supervisor => fail("semantic_presentation_supervisor_has_no_channel"),
        };
        if destination.is_some() {
            fail("semantic_presentation_channel_replacement_invalid");
        }
        *destination = Some(channel);
    }
}

fn publish_runtime(runtime: *mut presentation_runtime) -> Result<(), &'static str> {
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("semantic_presentation_runtime_publication_failed");
    }
    Ok(())
}

fn enter_source(runtime: &mut presentation_runtime) -> Result<(), &'static str> {
    let dispatch = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "semantic_presentation_source_dispatch_failed")?;
    let identity = runtime.task(fixture_role::source)?.identity;
    if dispatch.task() != identity
        || dispatch.sequence() != 0
        || dispatch.context() != scheduler_context::initial
    {
        return Err("semantic_presentation_source_dispatch_invalid");
    }
    let frame = runtime.select_role(fixture_role::source, false)?;
    publish_runtime(ptr::from_mut(runtime))?;
    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The published runtime, selected root, bound privilege stack, and
    // initial frame stay live until the source terminal call resumes here.
    let status = unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    if status != source_completion_status
        || runtime.active_role.is_some()
        || paging::read_cr3() != runtime.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || runtime.original_task_switched != original_task_switched
        || runtime.saved_kernel_stack == 0
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
    {
        return Err("semantic_presentation_source_continuation_invalid");
    }
    runtime.saved_kernel_stack = 0;
    Ok(())
}

fn enter_supervised_renderer(runtime: &mut presentation_runtime) -> Result<(), &'static str> {
    let dispatch = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "semantic_presentation_supervisor_dispatch_failed")?;
    if dispatch.task() != runtime.supervisor.identity
        || dispatch.sequence() != 1
        || dispatch.context() != scheduler_context::initial
    {
        return Err("semantic_presentation_supervisor_dispatch_invalid");
    }
    let frame = runtime.select_role(fixture_role::supervisor, false)?;
    publish_runtime(ptr::from_mut(runtime))?;
    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The supervisor and renderer roots, frames, and runtime remain
    // live until the supervisor consumes the one success observation.
    let status = unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    if status != presentation_completion_status
        || runtime.active_role.is_some()
        || paging::read_cr3() != runtime.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || runtime.original_task_switched != original_task_switched
        || runtime.saved_kernel_stack == 0
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !runtime.supervisor.wait_installed
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("semantic_presentation_supervisor_continuation_invalid");
    }
    runtime.saved_kernel_stack = 0;
    Ok(())
}

pub(super) fn semantic_surface_presentation_is_active() -> bool {
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
    _frame: *mut interrupts::interrupt_frame,
    _observed_at: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    None
}

pub(super) fn contain_expected_user_exception(
    _frame: *mut exceptions::exception_frame,
    _snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    None
}

pub(super) fn reject_unexpected_exception() -> ! {
    fail("semantic_presentation_unexpected_user_exception")
}

pub(super) unsafe fn gate_dispatch(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("semantic_presentation_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("semantic_presentation_gate_without_runtime");
    }
    let runtime = unsafe { &mut *runtime_pointer };
    if runtime.current_role() != Some(fixture_role::supervisor)
        || !runtime.frame_is_valid_for_role(frame, fixture_role::supervisor, true)
    {
        fail("semantic_presentation_gate_runtime_state_invalid");
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != scheduler_gate_vector || incoming.error_code != 0 {
        fail("semantic_presentation_gate_vector_invalid");
    }
    let result = match incoming.accumulator {
        operation_supervisor_await_completion => {
            handle_supervisor_await_completion(runtime, incoming)
        }
        operation_supervisor_complete => finish_supervisor_completion(runtime, incoming),
        _ => Err("semantic_presentation_gate_operation_invalid"),
    };
    match result {
        Ok(next) => {
            gate_active.store(false, Ordering::Release);
            next
        }
        Err(reason) => fail(reason),
    }
}

fn handle_supervisor_await_completion(
    runtime: &mut presentation_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_start),
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_completion_return),
        runtime.supervisor.space.payload_length,
    )
    .ok_or("semantic_presentation_supervisor_completion_return_invalid")?;
    if runtime.supervisor.wait_installed
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != supervisor_stack_top
        || !gate_registers_are_exact(
            incoming,
            operation_supervisor_await_completion,
            supervisor_mailbox_address,
        )
        || !supervisor_mailbox_matches(&runtime.supervisor, 0)
    {
        return Err("semantic_presentation_supervisor_wait_gate_invalid");
    }
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let relationship = runtime.relationship_value()?;
    let wait = runtime
        .supervisions
        .prepare_completion_wait(
            &runtime.tasks,
            relationship,
            runtime.supervisor.identity,
            renderer,
        )
        .map_err(|_| "semantic_presentation_completion_wait_preflight_failed")?;
    let transition = runtime
        .scheduler
        .block_current_and_dispatch(&mut runtime.tasks, runtime.supervisor.identity, renderer)
        .map_err(|_| "semantic_presentation_completion_wait_block_failed")?;
    if transition.task() != runtime.supervisor.identity
        || transition.dispatch().task() != renderer
        || transition.dispatch().sequence() != 2
        || transition.dispatch().context() != scheduler_context::initial
        || wait.supervisor() != runtime.supervisor.identity
        || wait.incarnation() != renderer
        || wait.expected_outcome() != supervision_outcome::successful_exit
    {
        return Err("semantic_presentation_completion_wait_transition_invalid");
    }
    wait.commit();
    runtime.supervisor.wait_installed = true;
    runtime.select_role(fixture_role::renderer, true)
}

fn finish_supervisor_completion(
    runtime: &mut presentation_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_start),
        ptr::addr_of!(x86_64_semantic_surface_presentation_supervisor_complete_return),
        runtime.supervisor.space.payload_length,
    )
    .ok_or("semantic_presentation_supervisor_complete_return_invalid")?;
    if !runtime.supervisor.wait_installed
        || runtime.trace.observations != 1
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
        return Err("semantic_presentation_supervisor_completion_gate_invalid");
    }
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let relationship = runtime.relationship_value()?;
    let completion = runtime
        .supervisions
        .prepare_supervisor_completion(
            &runtime.tasks,
            relationship,
            runtime.supervisor.identity,
            renderer,
        )
        .map_err(|_| "semantic_presentation_supervisor_completion_preflight_failed")?;
    runtime
        .scheduler
        .complete_current(&mut runtime.tasks, runtime.supervisor.identity)
        .map_err(|_| "semantic_presentation_supervisor_completion_transition_failed")?;
    if completion.supervisor() != runtime.supervisor.identity {
        return Err("semantic_presentation_supervisor_completion_identity_invalid");
    }
    completion.commit();
    runtime.phase = presentation_phase::idle;
    resume_kernel(runtime, presentation_completion_status)
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

fn supervisor_mailbox_matches(task: &supervisor_task, state: u64) -> bool {
    read_mailbox_word(task.space.physical_pages[1], supervisor_value_a_offset)
        == Some(supervisor_value_a)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_value_b_offset)
            == Some(supervisor_value_b)
        && read_mailbox_word(task.space.physical_pages[1], supervisor_state_offset) == Some(state)
}

fn resume_kernel(
    runtime: &mut presentation_runtime,
    status: u64,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.bootstrap_presentation {
        #[cfg(feature = "desktop_dev")]
        if !interrupts::bootstrap_presentation_backend_is_virgin() {
            return Err("desktop_bootstrap_resume_interrupt_state_invalid");
        }
        #[cfg(not(feature = "desktop_dev"))]
        return Err("desktop_bootstrap_presentation_unavailable");
    } else {
        interrupts::disable_external_interrupts();
    }
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("semantic_presentation_resume_interrupt_mask_failed");
    }
    // SAFETY: No user continuation can run after the terminal scheduler path.
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3
        || !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || runtime.saved_kernel_stack == 0
        || !(runtime.saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(runtime.original_task_switched, 0 | cr0_task_switched)
    {
        return Err("semantic_presentation_kernel_restore_invalid");
    }
    runtime.active_role = None;
    let stack = runtime.saved_kernel_stack;
    let task_switched = runtime.original_task_switched;
    gate_active.store(false, Ordering::Release);
    rite_gate_active.store(false, Ordering::Release);
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: This consumes the exact continuation captured by scheduler entry.
    unsafe { x86_64_scheduler_resume_kernel(status, stack, task_switched) }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum rite_gate_outcome {
    return_to_user(*mut scheduler_frame),
    source_terminal,
}

#[derive(Clone, Copy)]
struct rite_gate_call {
    frame: *mut scheduler_frame,
    role: fixture_role,
    identity: task_identity,
    mailbox: u64,
    phase: presentation_phase,
    call: rite_call,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_semantic_surface_presentation_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || rite_gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("semantic_presentation_rite_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("semantic_presentation_rite_gate_without_runtime");
    }
    let runtime = unsafe { &mut *runtime_pointer };
    match dispatch_rite(runtime, frame) {
        Ok(rite_gate_outcome::return_to_user(next)) => {
            rite_gate_active.store(false, Ordering::Release);
            next
        }
        Ok(rite_gate_outcome::source_terminal) => {
            match resume_kernel(runtime, source_completion_status) {
                Ok(_) => fail("semantic_presentation_source_resume_returned"),
                Err(reason) => fail(reason),
            }
        }
        Err(reason) => fail(reason),
    }
}

fn dispatch_rite(
    runtime: &mut presentation_runtime,
    frame: *mut scheduler_frame,
) -> Result<rite_gate_outcome, &'static str> {
    let phase = runtime.phase;
    let role = phase
        .role()
        .ok_or("semantic_presentation_rite_idle_phase")?;
    let identity = runtime.task(role)?.identity;
    if runtime.current_role() != Some(role) || !runtime.frame_is_valid_for_role(frame, role, true) {
        return Err("semantic_presentation_rite_runtime_state_invalid");
    }
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || incoming.instruction_pointer != expected_rite_return(runtime, phase)?
        || !gate_registers_are_canonical(incoming, user_mailbox_address)
    {
        return Err("semantic_presentation_rite_frame_invalid");
    }
    let mailbox = runtime.task(role)?.space.mailbox_physical.base;
    // SAFETY: This is the exclusive supervisor alias of the selected task's
    // fixed one-page mailbox while the synchronous gate owns its execution.
    let call = unsafe { read_call(mailbox) }?;
    validate_rite_call(phase, call)?;
    let mut channel = runtime.take_channel(role)?;
    let gate = rite_gate_call {
        frame,
        role,
        identity,
        mailbox,
        phase,
        call,
    };
    let outcome = dispatch_channel(runtime, gate, &mut channel);
    runtime.restore_channel(role, channel);
    outcome
}

fn dispatch_channel(
    runtime: &mut presentation_runtime,
    gate: rite_gate_call,
    channel: &mut rite_channel,
) -> Result<rite_gate_outcome, &'static str> {
    let decision = channel
        .assess(&runtime.tasks, gate.identity, gate.call)
        .map_err(|_| "semantic_presentation_rite_assessment_failed")?;
    runtime.trace.rite_calls = runtime
        .trace
        .rite_calls
        .checked_add(1)
        .ok_or("semantic_presentation_rite_call_count_overflow")?;
    let admission = match decision {
        rite_decision::rejected(reply) => {
            if !matches!(
                gate.phase,
                presentation_phase::source_read_denied | presentation_phase::renderer_submit_denied
            ) || reply.status != rite_status::denied
            {
                return Err("semantic_presentation_unexpected_rite_denial");
            }
            let staged = stage_empty_reply(gate.mailbox, reply)?;
            runtime.trace.denials = runtime
                .trace
                .denials
                .checked_add(1)
                .ok_or("semantic_presentation_denial_count_overflow")?;
            publish_empty_reply(staged);
            runtime.phase = next_phase(gate.phase)?;
            return Ok(rite_gate_outcome::return_to_user(gate.frame));
        }
        rite_decision::admitted(admission) => admission,
    };
    if admission.vessel() != gate.identity
        || admission.sequence()
            != gate
                .phase
                .sequence()
                .ok_or("semantic_presentation_phase_sequence_missing")?
        || admission.trace_id()
            != gate
                .phase
                .trace()
                .ok_or("semantic_presentation_phase_trace_missing")?
    {
        return Err("semantic_presentation_rite_admission_invalid");
    }
    match gate.phase {
        presentation_phase::source_malformed_submit
        | presentation_phase::source_freshness_denied
        | presentation_phase::source_submit
        | presentation_phase::source_backpressured => {
            handle_source_submit(runtime, gate.frame, gate.mailbox, gate.phase, admission)
        }
        presentation_phase::source_exit => {
            handle_source_exit(runtime, gate.role, gate.mailbox, admission)
        }
        presentation_phase::renderer_read => {
            handle_renderer_read(runtime, gate.frame, gate.mailbox, admission)
        }
        presentation_phase::renderer_row(row) => {
            handle_renderer_row(runtime, gate.frame, gate.mailbox, row, admission)
        }
        presentation_phase::renderer_exit => {
            handle_renderer_exit(runtime, gate.frame, gate.mailbox, admission)
        }
        presentation_phase::source_read_denied | presentation_phase::renderer_submit_denied => {
            Err("semantic_presentation_denied_rite_admitted")
        }
        presentation_phase::source_retired | presentation_phase::idle => {
            Err("semantic_presentation_rite_idle_phase")
        }
    }
}

fn handle_source_submit(
    runtime: &mut presentation_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    phase: presentation_phase,
    admission: rite_admission<'_>,
) -> Result<rite_gate_outcome, &'static str> {
    if admission.operation() != rite_operation::semantic_surface_submit
        || admission.payload_kind() != rite_payload_kind::semantic_surface_snapshot
        || admission.payload_size() as usize != semantic_surface_snapshot_size
    {
        return Err("semantic_presentation_submit_admission_shape_invalid");
    }
    // SAFETY: Admission fixes the complete source body span and this gate owns
    // the exact mailbox mapping for the copy.
    unsafe {
        copy_admitted_body(
            mailbox,
            user_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copies,
            &mut runtime.trace.body_bytes,
        )
    }?;
    let body = surface_snapshot_body_v1::try_from_slice(
        &runtime.scratch[..semantic_surface_snapshot_size],
    );
    let next = next_phase(phase)?;
    match phase {
        presentation_phase::source_malformed_submit => {
            if body.is_ok() {
                return Err("semantic_presentation_malformed_submit_accepted");
            }
            let reply = admission
                .semantic_surface_empty_reply(rite_status::bad_payload)
                .map_err(|_| "semantic_presentation_malformed_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            clear_scratch(runtime)?;
            let settled = admission
                .reject_after_revalidation(&runtime.tasks, rite_status::bad_payload)
                .map_err(|_| "semantic_presentation_malformed_rejection_failed")?;
            if settled != reply {
                fail("semantic_presentation_malformed_reply_changed");
            }
            runtime.trace.malformed += 1;
            publish_empty_reply(staged);
        }
        presentation_phase::source_freshness_denied => {
            let body = body.map_err(|_| "semantic_presentation_freshness_body_invalid")?;
            let binding = runtime.binding_value()?;
            let rejection = match runtime.endpoint_ref()?.prepare_submit(binding, body) {
                Ok(_) => return Err("semantic_presentation_freshness_body_queued"),
                Err(rejection) => rejection,
            };
            if rejection.error() != semantic_surface_error::wrong_surface {
                return Err("semantic_presentation_freshness_rejection_invalid");
            }
            drop(rejection);
            let reply = admission
                .semantic_surface_empty_reply(rite_status::denied)
                .map_err(|_| "semantic_presentation_freshness_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            clear_scratch(runtime)?;
            let settled = admission
                .reject_after_revalidation(&runtime.tasks, rite_status::denied)
                .map_err(|_| "semantic_presentation_freshness_rejection_failed")?;
            if settled != reply {
                fail("semantic_presentation_freshness_reply_changed");
            }
            runtime.trace.freshness_denials += 1;
            publish_empty_reply(staged);
        }
        presentation_phase::source_submit => {
            let body = body.map_err(|_| "semantic_presentation_submit_body_invalid")?;
            let completion = admission
                .prepare_semantic_surface_submit_ok(&runtime.tasks)
                .map_err(|_| "semantic_presentation_submit_revalidation_failed")?;
            let binding = runtime.binding_value()?;
            let prepared = runtime
                .endpoint_ref()?
                .prepare_submit(binding, body)
                .map_err(|_| "semantic_presentation_submit_preparation_failed")?;
            let reply = completion.reply();
            let staged = stage_empty_reply(mailbox, reply)?;
            clear_scratch(runtime)?;
            let endpoint = runtime.endpoint_mut()?;
            let settled = completion.commit(|| {
                let receipt = endpoint
                    .commit_submit(prepared)
                    .unwrap_or_else(|_| fail("semantic_presentation_submit_commit_failed"));
                if receipt.binding() != binding {
                    fail("semantic_presentation_submit_receipt_invalid");
                }
            });
            if settled != reply {
                fail("semantic_presentation_submit_reply_changed");
            }
            runtime.trace.submissions += 1;
            publish_empty_reply(staged);
        }
        presentation_phase::source_backpressured => {
            let body = body.map_err(|_| "semantic_presentation_backpressure_body_invalid")?;
            let binding = runtime.binding_value()?;
            let rejection = match runtime.endpoint_ref()?.prepare_submit(binding, body) {
                Ok(_) => return Err("semantic_presentation_duplicate_submit_accepted"),
                Err(rejection) => rejection,
            };
            if rejection.error() != semantic_surface_error::endpoint_full {
                return Err("semantic_presentation_backpressure_rejection_invalid");
            }
            drop(rejection);
            let reply = admission
                .semantic_surface_empty_reply(rite_status::would_block)
                .map_err(|_| "semantic_presentation_backpressure_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            clear_scratch(runtime)?;
            let settled = admission
                .finish_semantic_surface_submit_would_block(&runtime.tasks)
                .map_err(|_| "semantic_presentation_backpressure_completion_failed")?;
            if settled != reply {
                fail("semantic_presentation_backpressure_reply_changed");
            }
            runtime.trace.backpressure += 1;
            publish_empty_reply(staged);
        }
        _ => return Err("semantic_presentation_submit_phase_invalid"),
    }
    runtime.phase = next;
    Ok(rite_gate_outcome::return_to_user(frame))
}

fn handle_source_exit(
    runtime: &mut presentation_runtime,
    role: fixture_role,
    mailbox: u64,
    admission: rite_admission<'_>,
) -> Result<rite_gate_outcome, &'static str> {
    validate_and_copy_exit(runtime, mailbox, &admission)?;
    let identity = runtime.task(role)?.identity;
    clear_scratch(runtime)?;
    let scheduler = &mut runtime.scheduler;
    let reply = admission
        .finish_ok(&mut runtime.tasks, |tasks, committed| {
            if committed != identity {
                fail("semantic_presentation_source_exit_identity_changed");
            }
            scheduler
                .complete_current(tasks, committed)
                .unwrap_or_else(|_| fail("semantic_presentation_source_exit_transition_failed"));
        })
        .map_err(|_| "semantic_presentation_source_exit_completion_failed")?;
    validate_empty_success_reply(presentation_phase::source_exit, reply)?;
    runtime.trace.exits += 1;
    runtime.phase = presentation_phase::source_retired;
    runtime.active_role = None;
    Ok(rite_gate_outcome::source_terminal)
}

fn handle_renderer_read(
    runtime: &mut presentation_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    admission: rite_admission<'_>,
) -> Result<rite_gate_outcome, &'static str> {
    if admission.operation() != rite_operation::semantic_surface_read
        || admission.payload_kind() != rite_payload_kind::semantic_surface_request
        || admission.payload_size() as usize != semantic_surface_request_size
    {
        return Err("semantic_presentation_read_admission_shape_invalid");
    }
    // SAFETY: Admission fixes the complete request and the renderer mailbox is
    // exclusively owned by this synchronous gate.
    unsafe {
        copy_admitted_body(
            mailbox,
            user_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copies,
            &mut runtime.trace.body_bytes,
        )
    }?;
    let request = decode_semantic_surface_request(&runtime.scratch);
    if !request.has_supported_body()
        || request.reply_capacity() as usize != semantic_surface_snapshot_size
    {
        return Err("semantic_presentation_read_request_invalid");
    }
    let binding = runtime.binding_value()?;
    let delivery = runtime
        .endpoint_ref()?
        .prepare_delivery(binding)
        .map_err(|_| "semantic_presentation_delivery_preparation_failed")?;
    let reply = admission
        .semantic_surface_delivery_reply()
        .map_err(|_| "semantic_presentation_delivery_reply_preparation_failed")?;
    let prepared_reply = {
        let body = delivery
            .body(runtime.endpoint_ref()?)
            .map_err(|_| "semantic_presentation_delivery_body_missing")?;
        prepare_semantic_surface_delivery_reply(reply, body)?
    };
    // SAFETY: All destination spans and the complete source body were
    // validated. The reply header remains invalid until both commits settle.
    let staged = unsafe { prepared_reply.stage(mailbox) }?;
    clear_scratch(runtime)?;
    let mut receipt = None;
    let (tasks, endpoint) = (&runtime.tasks, &mut runtime.endpoint);
    let endpoint = endpoint
        .as_mut()
        .ok_or("semantic_presentation_endpoint_owner_missing")?;
    let settled = admission
        .finish_semantic_surface_delivery(tasks, || {
            let committed = endpoint.commit_prepared_delivery(delivery);
            if committed.binding() != binding {
                fail("semantic_presentation_delivery_receipt_invalid");
            }
            receipt = Some(committed);
        })
        .map_err(|_| "semantic_presentation_delivery_completion_failed")?;
    if settled != reply {
        fail("semantic_presentation_delivery_reply_changed");
    }
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let owner = render_delivery_owner {
        receipt: receipt.ok_or("semantic_presentation_delivery_receipt_missing")?,
        task: renderer,
        channel_generation: 1,
        stage_generation: 1,
    };
    if runtime.delivery.replace(owner).is_some() {
        return Err("semantic_presentation_delivery_owner_replaced");
    }
    // SAFETY: The endpoint and Rite commits are now the exact source-consume
    // linearization point for this still-live destination mailbox.
    unsafe { staged.publish() };
    runtime.trace.deliveries += 1;
    runtime.trace.destination_header_published_last = true;
    runtime.phase = presentation_phase::renderer_row(0);
    Ok(rite_gate_outcome::return_to_user(frame))
}

fn handle_renderer_row(
    runtime: &mut presentation_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    row: u32,
    admission: rite_admission<'_>,
) -> Result<rite_gate_outcome, &'static str> {
    if admission.operation() != rite_operation::display_update
        || admission.payload_kind() != rite_payload_kind::display_patch
        || admission.payload_size() != display_payload_size
    {
        return Err("semantic_presentation_display_admission_shape_invalid");
    }
    // SAFETY: Admission fixes the complete patch body and the selected
    // renderer exclusively owns this mailbox for the synchronous copy.
    unsafe {
        copy_admitted_body(
            mailbox,
            user_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copies,
            &mut runtime.trace.body_bytes,
        )
    }?;
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
        || usize::try_from(row).ok() != Some(runtime.trace.rows)
    {
        return Err("semantic_presentation_display_patch_invalid");
    }
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let stage_owner = runtime
        .stage
        .as_mut()
        .ok_or("semantic_presentation_stage_missing")?;
    if stage_owner.task != renderer
        || stage_owner.channel_generation != 1
        || stage_owner.generation != 1
        || stage_owner
            .stage
            .as_ref()
            .is_none_or(|stage| stage.rows_staged() != row as usize)
    {
        return Err("semantic_presentation_stage_correlation_invalid");
    }
    let stage = stage_owner
        .stage
        .as_mut()
        .ok_or("semantic_presentation_stage_owner_missing")?;
    let reply = admission
        .finish_ok(&mut runtime.tasks, |_, committed| {
            if committed != renderer {
                fail("semantic_presentation_display_identity_changed");
            }
            stage
                .stage_row(row as usize, pixels)
                .unwrap_or_else(|_| fail("semantic_presentation_stage_commit_failed"));
        })
        .map_err(|_| "semantic_presentation_display_completion_failed")?;
    validate_empty_success_reply(presentation_phase::renderer_row(row), reply)?;
    if !clear_and_verify_bytes(&mut runtime.scratch) {
        return Err("semantic_presentation_display_scratch_clear_failed");
    }
    runtime.trace.rows = row as usize + 1;
    runtime.phase = if runtime.trace.rows == expected_rows {
        presentation_phase::renderer_exit
    } else {
        presentation_phase::renderer_row(row + 1)
    };
    // SAFETY: The payload-free result is fully settled and the selected
    // mailbox remains exclusively owned until the gate returns.
    unsafe { write_reply(mailbox, reply) }?;
    compiler_fence(Ordering::SeqCst);
    Ok(rite_gate_outcome::return_to_user(frame))
}

fn handle_renderer_exit(
    runtime: &mut presentation_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    admission: rite_admission<'_>,
) -> Result<rite_gate_outcome, &'static str> {
    validate_and_copy_exit(runtime, mailbox, &admission)?;
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let stage = runtime
        .stage
        .as_ref()
        .ok_or("semantic_presentation_stage_missing")?;
    let delivery = runtime
        .delivery
        .as_ref()
        .ok_or("semantic_presentation_delivery_owner_missing")?;
    if stage.task != renderer
        || stage.channel_generation != 1
        || stage.generation != 1
        || stage
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != expected_rows)
        || delivery.receipt.binding() != runtime.binding_value()?
        || delivery.task != renderer
        || delivery.channel_generation != 1
        || delivery.stage_generation != 1
    {
        return Err("semantic_presentation_exit_ownership_invalid");
    }
    let supervisor = runtime.supervisor.identity;
    let relationship = runtime.relationship_value()?;
    clear_scratch(runtime)?;
    let publication = runtime
        .supervisions
        .prepare_publication(
            &runtime.tasks,
            relationship,
            renderer,
            supervision_outcome::successful_exit,
        )
        .map_err(|_| "semantic_presentation_completion_publication_preflight_failed")?;
    let expected_observation = publication.observation();
    if publication.supervisor() != supervisor
        || expected_observation.event_sequence() != 1
        || expected_observation.incarnation() != renderer
        || expected_observation.outcome() != supervision_outcome::successful_exit
    {
        return Err("semantic_presentation_completion_publication_invalid");
    }
    let scheduler = &mut runtime.scheduler;
    let mut dispatch_sequence = None;
    let reply = admission
        .finish_ok(&mut runtime.tasks, |tasks, committed| {
            if committed != renderer {
                fail("semantic_presentation_renderer_exit_identity_changed");
            }
            let transition = scheduler
                .complete_current_and_wake(tasks, committed, supervisor)
                .unwrap_or_else(|_| fail("semantic_presentation_completion_wake_failed"));
            if transition.task() != renderer
                || transition.dispatch().task() != supervisor
                || transition.dispatch().sequence() != 3
                || transition.dispatch().context() != scheduler_context::saved
            {
                fail("semantic_presentation_completion_wake_invalid");
            }
            dispatch_sequence = Some(transition.dispatch().sequence());
            publication.commit();
        })
        .map_err(|_| "semantic_presentation_renderer_exit_completion_failed")?;
    validate_empty_success_reply(presentation_phase::renderer_exit, reply)?;
    if dispatch_sequence != Some(3) {
        return Err("semantic_presentation_completion_dispatch_invalid");
    }
    let observation = runtime
        .supervisions
        .take_observation(&runtime.tasks, relationship, supervisor)
        .map_err(|_| "semantic_presentation_completion_observation_consume_failed")?;
    if observation != expected_observation {
        return Err("semantic_presentation_completion_observation_invalid");
    }
    let delivery = runtime
        .delivery
        .take()
        .ok_or("semantic_presentation_delivery_owner_missing")?;
    runtime.acknowledgement = Some(render_acknowledgement {
        delivery,
        observation,
    });
    runtime.trace.exits += 1;
    runtime.trace.observations += 1;
    let next = runtime.select_role(fixture_role::supervisor, true)?;
    if next == frame {
        return Err("semantic_presentation_terminal_frame_reused");
    }
    // SAFETY: This is the saved supervisor frame returned by the exact wake.
    unsafe { ptr::addr_of_mut!((*next).accumulator).write(wake_completion) };
    compiler_fence(Ordering::SeqCst);
    Ok(rite_gate_outcome::return_to_user(next))
}

fn validate_and_copy_exit(
    runtime: &mut presentation_runtime,
    mailbox: u64,
    admission: &rite_admission<'_>,
) -> Result<(), &'static str> {
    if admission.operation() != rite_operation::task_exit
        || admission.payload_kind() != rite_payload_kind::exit_status
        || admission.payload_size() != exit_payload_size
    {
        return Err("semantic_presentation_exit_admission_shape_invalid");
    }
    // SAFETY: Admission fixes the complete exit body and this gate owns the
    // selected task's mailbox through the copy.
    unsafe {
        copy_admitted_body(
            mailbox,
            user_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copies,
            &mut runtime.trace.body_bytes,
        )
    }?;
    let exit = decode_exit_payload(&runtime.scratch)?;
    if !exit.has_supported_body() || exit.code != expected_exit_code || exit.reserved != 0 {
        return Err("semantic_presentation_exit_body_invalid");
    }
    Ok(())
}

fn clear_scratch(runtime: &mut presentation_runtime) -> Result<(), &'static str> {
    if clear_and_verify_bytes(&mut runtime.scratch) {
        Ok(())
    } else {
        Err("semantic_presentation_scratch_clear_failed")
    }
}

fn stage_empty_reply(
    mailbox: u64,
    reply: rite_reply,
) -> Result<staged_semantic_surface_reply, &'static str> {
    let prepared = prepare_semantic_surface_empty_reply(reply)?;
    // SAFETY: The current fixed mailbox was validated before admission and is
    // exclusively held by the synchronous gate.
    unsafe { prepared.stage(mailbox) }
}

fn publish_empty_reply(staged: staged_semantic_surface_reply) {
    // SAFETY: No operation-specific state remains to commit for this empty
    // settled outcome, so the validity header may now be published.
    unsafe { staged.publish() };
}

fn next_phase(phase: presentation_phase) -> Result<presentation_phase, &'static str> {
    Ok(match phase {
        presentation_phase::source_read_denied => presentation_phase::source_malformed_submit,
        presentation_phase::source_malformed_submit => presentation_phase::source_freshness_denied,
        presentation_phase::source_freshness_denied => presentation_phase::source_submit,
        presentation_phase::source_submit => presentation_phase::source_backpressured,
        presentation_phase::source_backpressured => presentation_phase::source_exit,
        presentation_phase::renderer_submit_denied => presentation_phase::renderer_read,
        presentation_phase::renderer_read => presentation_phase::renderer_row(0),
        presentation_phase::source_exit
        | presentation_phase::source_retired
        | presentation_phase::renderer_row(_)
        | presentation_phase::renderer_exit
        | presentation_phase::idle => {
            return Err("semantic_presentation_phase_advance_invalid");
        }
    })
}

fn expected_rite_return(
    runtime: &presentation_runtime,
    phase: presentation_phase,
) -> Result<u64, &'static str> {
    let (role, symbol) = match phase {
        presentation_phase::source_read_denied => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_read_denied_return
            )),
        ),
        presentation_phase::source_malformed_submit => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_malformed_submit_return
            )),
        ),
        presentation_phase::source_freshness_denied => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_freshness_denied_return
            )),
        ),
        presentation_phase::source_submit => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_submit_return)),
        ),
        presentation_phase::source_backpressured => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_source_backpressured_return
            )),
        ),
        presentation_phase::source_exit => (
            fixture_role::source,
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_source_exit_return)),
        ),
        presentation_phase::renderer_submit_denied => (
            fixture_role::renderer,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_submit_denied_return
            )),
        ),
        presentation_phase::renderer_read => (
            fixture_role::renderer,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_delivery_return
            )),
        ),
        presentation_phase::renderer_row(_) => (
            fixture_role::renderer,
            linked_symbol(ptr::addr_of!(
                x86_64_semantic_surface_renderer_display_return
            )),
        ),
        presentation_phase::renderer_exit => (
            fixture_role::renderer,
            linked_symbol(ptr::addr_of!(x86_64_semantic_surface_renderer_exit_return)),
        ),
        presentation_phase::source_retired | presentation_phase::idle => {
            return Err("semantic_presentation_return_phase_invalid");
        }
    };
    runtime
        .task(role)?
        .space
        .payload
        .linked_user_address(symbol)
        .ok_or("semantic_presentation_return_symbol_invalid")
}

fn validate_rite_call(phase: presentation_phase, call: rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.reserved != 0
        || !payload_span_fits(user_mailbox_address, call.payload_size)
        || call.trace_id
            != phase
                .trace()
                .ok_or("semantic_presentation_call_trace_missing")?
    {
        return Err("semantic_presentation_rite_call_header_invalid");
    }
    let (operation, kind, rights, size) = match phase {
        presentation_phase::source_read_denied => (
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            semantic_surface_request_size as u32,
        ),
        presentation_phase::source_malformed_submit
        | presentation_phase::source_freshness_denied
        | presentation_phase::source_submit
        | presentation_phase::source_backpressured
        | presentation_phase::renderer_submit_denied => (
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            semantic_surface_snapshot_size as u32,
        ),
        presentation_phase::renderer_read => (
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            semantic_surface_request_size as u32,
        ),
        presentation_phase::renderer_row(_) => (
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            display_payload_size,
        ),
        presentation_phase::source_exit | presentation_phase::renderer_exit => (
            rite_operation::task_exit,
            rite_payload_kind::exit_status,
            rite_rights::task_exit,
            exit_payload_size,
        ),
        presentation_phase::source_retired | presentation_phase::idle => {
            return Err("semantic_presentation_call_phase_invalid");
        }
    };
    if call.operation != operation
        || call.payload_kind != kind
        || call.rights_present != rights
        || call.payload_size != size
        || call.payload_limit != size
    {
        return Err("semantic_presentation_rite_call_shape_invalid");
    }
    Ok(())
}

fn validate_empty_success_reply(
    phase: presentation_phase,
    reply: rite_reply,
) -> Result<(), &'static str> {
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence
            != phase
                .sequence()
                .ok_or("semantic_presentation_reply_sequence_missing")?
        || reply.trace_id
            != phase
                .trace()
                .ok_or("semantic_presentation_reply_trace_missing")?
    {
        return Err("semantic_presentation_rite_reply_invalid");
    }
    Ok(())
}

const fn renderer_submit_denied_trace() -> id_128 {
    id_128::from_bytes(*b"surface-deny-v1!")
}

const fn renderer_read_trace() -> id_128 {
    id_128::from_bytes(*b"surface-read-v1!")
}

const fn display_trace(row: u32) -> id_128 {
    let encoded_row = row.to_le_bytes();
    id_128::from_bytes([
        b's',
        b'u',
        b'r',
        b'f',
        1,
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

const fn renderer_exit_trace() -> id_128 {
    id_128::from_bytes(*b"surface-exit-v1!")
}

fn retire_source(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut presentation_runtime,
) -> Result<(), &'static str> {
    let identity = runtime.task(fixture_role::source)?.identity;
    let circle = runtime.task(fixture_role::source)?.circle;
    let mut channel = runtime.take_channel(fixture_role::source)?;
    channel
        .close_idle()
        .map_err(|_| "semantic_presentation_source_channel_retirement_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, identity)
        .map_err(|_| "semantic_presentation_source_context_withdrawal_failed")?;
    let mut task = runtime
        .source
        .take()
        .ok_or("semantic_presentation_source_missing")?;
    release_task_space(allocator, &mut runtime.circles, circle, &mut task.space)?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, identity)
        .map_err(|_| "semantic_presentation_source_task_destroy_failed")?;
    runtime
        .circles
        .destroy(circle)
        .map_err(|_| "semantic_presentation_source_circle_destroy_failed")?;
    Ok(())
}

fn release_task_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: circle_id,
    space: &mut task_space,
) -> Result<(), &'static str> {
    let mut image = space
        .root
        .unmap_owned(allocator, user_image_address)
        .map_err(|_| "semantic_presentation_image_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "semantic_presentation_mailbox_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "semantic_presentation_stack_withdrawal_failed")?;
    // SAFETY: All owned mappings have been withdrawn and the allocator remains
    // exclusively borrowed for recursive page-table clearing.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "semantic_presentation_root_destroy_failed")?;
    // SAFETY: Each lease is detached and uniquely owned for verified clearing.
    unsafe { allocator.release_with(&mut image, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_presentation_image_release_failed")?;
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_presentation_mailbox_release_failed")?;
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_presentation_stack_release_failed")?;
    allocator
        .unbind(&space.privilege_stack)
        .map_err(|_| "semantic_presentation_privilege_stack_unbind_failed")?;
    unsafe {
        allocator.release_with(&mut space.privilege_stack, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "semantic_presentation_privilege_stack_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "semantic_presentation_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "semantic_presentation_root_detachment_failed")?;
    retire_task_space_metadata(space);
    Ok(())
}

fn retire_task_space_metadata(space: &mut task_space) {
    debug_assert!(space.root.root_address().is_err());
    debug_assert!(space.privilege_stack.range().is_none());
    // SAFETY: Every address-bearing owner was withdrawn; only dead bookkeeping
    // remains before the outer task owner is dropped.
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
        ptr::write_volatile(
            ptr::addr_of_mut!(space.payload),
            payload_contract {
                start: ptr::null(),
                payload_end: ptr::null(),
                end: ptr::null(),
                entry: ptr::null(),
            },
        );
    }
}

fn teardown_prepared_runtime(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut presentation_runtime,
) -> Result<(), &'static str> {
    let renderer = runtime.task(fixture_role::renderer)?.identity;
    let supervisor = runtime.supervisor.identity;
    let relationship = runtime.relationship_value()?;
    let mut channel = runtime.take_channel(fixture_role::renderer)?;
    channel
        .close_idle()
        .map_err(|_| "semantic_presentation_renderer_channel_retirement_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, renderer)
        .map_err(|_| "semantic_presentation_renderer_context_withdrawal_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, supervisor)
        .map_err(|_| "semantic_presentation_supervisor_context_withdrawal_failed")?;
    runtime
        .supervisions
        .withdraw_incarnation(&runtime.tasks, relationship, supervisor, renderer)
        .map_err(|_| "semantic_presentation_incarnation_withdrawal_failed")?;
    let mut renderer_task = runtime
        .renderer
        .take()
        .ok_or("semantic_presentation_renderer_missing")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        renderer_task.circle,
        &mut renderer_task.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, renderer)
        .map_err(|_| "semantic_presentation_renderer_task_destroy_failed")?;
    runtime
        .circles
        .destroy(renderer_task.circle)
        .map_err(|_| "semantic_presentation_renderer_circle_destroy_failed")?;

    release_supervisor_space(
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
            relationship,
            supervisor,
        )
        .map_err(|_| "semantic_presentation_supervisor_withdrawal_failed")?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, supervisor)
        .map_err(|_| "semantic_presentation_supervisor_task_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.supervisor.circle)
        .map_err(|_| "semantic_presentation_supervisor_circle_destroy_failed")?;
    runtime
        .supervisions
        .release(&runtime.circles, &runtime.tasks, relationship)
        .map_err(|_| "semantic_presentation_relationship_release_failed")?;
    runtime.relationship = None;

    let binding = runtime
        .binding
        .take()
        .ok_or("semantic_presentation_binding_owner_missing")?;
    let mut endpoint = runtime
        .endpoint
        .take()
        .ok_or("semantic_presentation_endpoint_owner_missing")?;
    let close = endpoint
        .close(binding)
        .map_err(|_| "semantic_presentation_endpoint_close_failed")?;
    if !close.first_close() || close.discarded() || !endpoint.storage_is_clear() {
        return Err("semantic_presentation_endpoint_close_invalid");
    }
    let acknowledgement = runtime
        .acknowledgement
        .take()
        .ok_or("semantic_presentation_acknowledgement_missing")?;
    let stage = runtime
        .stage
        .as_mut()
        .ok_or("semantic_presentation_stage_missing")?;
    if acknowledgement.delivery.receipt.binding() != binding
        || acknowledgement.delivery.channel_generation != 1
        || acknowledgement.delivery.stage_generation != 1
        || acknowledgement.observation.event_sequence() != 1
        || acknowledgement.observation.incarnation() != acknowledgement.delivery.task
        || acknowledgement.observation.outcome() != supervision_outcome::successful_exit
        || stage.task != acknowledgement.delivery.task
        || stage.channel_generation != acknowledgement.delivery.channel_generation
        || stage.generation != acknowledgement.delivery.stage_generation
        || stage.acknowledged
    {
        return Err("semantic_presentation_acknowledgement_correlation_invalid");
    }
    stage.acknowledged = true;
    allocator
        .unbind(&stage.lease)
        .map_err(|_| "semantic_presentation_stage_unbind_failed")?;
    if !scratch_is_zero(&runtime.scratch) {
        return Err("semantic_presentation_teardown_scratch_not_clear");
    }
    Ok(())
}

fn validate_prepublication_teardown(
    allocator: &frame_allocator<'_>,
    runtime: &presentation_runtime,
    initial: frame_accounting,
) -> Result<(), &'static str> {
    let retained = allocator.accounting();
    let circles = runtime.circles.accounting();
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "semantic_presentation_final_scheduler_invalid")?;
    let stage = runtime
        .stage
        .as_ref()
        .ok_or("semantic_presentation_stage_missing")?;
    if initial.free.checked_sub(stage_frame_count) != Some(retained.free)
        || retained.data_owned != stage_frame_count
        || retained.table_owned != 0
        || retained.active_leases != 1
        || retained.bound_leases != 0
        || circles != empty_circle_accounting()
        || runtime.tasks.active_count() != 0
        || runtime.tasks.attached_context_count() != 0
        || runtime.supervisions.active_count() != 0
        || runtime.relationship.is_some()
        || scheduler.admitted() != 0
        || scheduler.runnable() != 0
        || scheduler.current().is_some()
        || runtime.source.is_some()
        || runtime.renderer.is_some()
        || runtime.source_channel.is_some()
        || runtime.renderer_channel.is_some()
        || runtime.endpoint.is_some()
        || runtime.binding.is_some()
        || runtime.delivery.is_some()
        || runtime.acknowledgement.is_some()
        || runtime.active_role.is_some()
        || runtime.phase != presentation_phase::idle
        || !stage.acknowledged
        || stage
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != expected_rows)
        || allocator
            .is_bound(&stage.lease)
            .map_err(|_| "semantic_presentation_stage_binding_state_invalid")?
        || !runtime.trace.framebuffer_unchanged
        || runtime.trace.frame_commits != 0
        || runtime.trace.stage_clears != 0
        || !interrupts::scheduler_interrupts_are_masked()
        || paging::read_cr3() != runtime.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
    {
        return Err("semantic_presentation_prepublication_teardown_invalid");
    }
    Ok(())
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

fn publish_complete_frame(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut presentation_runtime,
    framebuffer: framebuffer_info,
) -> Result<(), &'static str> {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || runtime.active_role.is_some()
        || !interrupts::scheduler_interrupts_are_masked()
        || !framebuffer_baseline_is_intact(framebuffer)?
        || runtime.trace.rows != expected_rows
        || runtime.trace.frame_commits != 0
        || runtime.trace.stage_clears != 0
    {
        return Err("semantic_presentation_publication_state_invalid");
    }
    let stage_owner = runtime
        .stage
        .as_ref()
        .ok_or("semantic_presentation_stage_missing")?;
    if !stage_owner.acknowledged
        || stage_owner
            .stage
            .as_ref()
            .is_none_or(|stage| !stage.is_complete() || stage.rows_staged() != expected_rows)
    {
        return Err("semantic_presentation_publication_acknowledgement_invalid");
    }
    let mut stage_owner = runtime
        .stage
        .take()
        .ok_or("semantic_presentation_stage_missing")?;
    let stage_range = stage_owner
        .lease
        .range()
        .ok_or("semantic_presentation_stage_extent_missing")?;
    let prepared_release = allocator
        .prepare_release(&mut stage_owner.lease)
        .map_err(|_| "semantic_presentation_stage_release_preflight_failed")?;
    if prepared_release.range() != stage_range {
        return Err("semantic_presentation_stage_release_extent_changed");
    }
    let mut stage = stage_owner
        .stage
        .take()
        .ok_or("semantic_presentation_stage_owner_missing")?;
    let completion = stage
        .completion()
        .map_err(|_| "semantic_presentation_stage_completion_failed")?;

    // SAFETY: Console initialization owns the exact validated framebuffer;
    // this transfer occurs only after every non-framebuffer preflight above.
    let withdrawn = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .ok_or("semantic_presentation_failure_surface_missing")?;
    if !framebuffer_info_matches(withdrawn, framebuffer) {
        return Err("semantic_presentation_failure_surface_mismatch");
    }
    // SAFETY: Withdrawal transfers unique ownership of the complete aperture.
    let mut presenter = unsafe { framebuffer_presenter::try_new(withdrawn) }
        .map_err(|_| "semantic_presentation_presenter_construction_failed")?;
    if presenter.width() != rgb8_frame_width || presenter.height() != rgb8_frame_height {
        return Err("semantic_presentation_presenter_geometry_invalid");
    }
    let prepared = presenter
        .prepare_complete_rgb8_frame(completion)
        .map_err(|_| "semantic_presentation_frame_preflight_failed")?;
    prepared.commit();
    runtime.trace.frame_commits = 1;
    if !framebuffer_matches_stage(framebuffer, stage_range)? {
        fail("semantic_presentation_framebuffer_readback_mismatch");
    }
    runtime.trace.framebuffer_verified = true;
    let mut stage = Some(stage);
    // SAFETY: The prepared release owns this exact unbound lease. The callback
    // consumes and verifies the only stage owner before releasing its frames.
    let release = unsafe {
        prepared_release.clear_and_release_with(|range| {
            range == stage_range && stage.take().is_some_and(rgb8_frame_stage::clear_and_verify)
        })
    };
    if release.is_err() {
        fail("semantic_presentation_published_stage_quarantined");
    }
    runtime.trace.stage_clears = 1;
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
    // SAFETY: The serialized proof owns the complete validated aperture before
    // any ring-3 task can execute.
    unsafe { crate::display::initialize_supervised_renderer_baseline(info) }
        .map_err(|_| "semantic_presentation_framebuffer_baseline_seed_failed")
}

fn framebuffer_baseline_is_intact(info: framebuffer_info) -> Result<bool, &'static str> {
    // SAFETY: No presenter has been constructed and the serialized proof is the
    // sole software owner of this aperture.
    unsafe { crate::display::supervised_renderer_baseline_is_intact(info) }
        .map_err(|_| "semantic_presentation_framebuffer_baseline_check_failed")
}

fn framebuffer_matches_stage(
    info: framebuffer_info,
    stage: physical_range,
) -> Result<bool, &'static str> {
    let aperture_length = framebuffer_aperture_length(info)
        .ok_or("semantic_presentation_framebuffer_extent_invalid")?;
    if stage.length != rgb8_frame_byte_length as u64 {
        return Err("semantic_presentation_stage_extent_invalid");
    }
    let stage_base =
        usize::try_from(stage.base).map_err(|_| "semantic_presentation_stage_address_invalid")?;
    let aperture_base = usize::try_from(info.address)
        .map_err(|_| "semantic_presentation_framebuffer_address_invalid")?;
    let bytes_per_pixel = usize::from(info.bits_per_pixel).div_ceil(8);
    let visible_row_bytes = info
        .width
        .checked_mul(bytes_per_pixel)
        .ok_or("semantic_presentation_framebuffer_extent_invalid")?;
    let mut matches = true;
    for y in 0..info.height {
        for x in 0..info.width {
            let source = y * rgb8_frame_row_bytes + x * 3;
            let red = unsafe { ptr::read_volatile((stage_base as *const u8).add(source)) };
            let green = unsafe { ptr::read_volatile((stage_base as *const u8).add(source + 1)) };
            let blue = unsafe { ptr::read_volatile((stage_base as *const u8).add(source + 2)) };
            let packed = encode_framebuffer_channel(red, info.red)
                | encode_framebuffer_channel(green, info.green)
                | encode_framebuffer_channel(blue, info.blue);
            let destination = y * info.pitch + x * bytes_per_pixel;
            for byte in 0..bytes_per_pixel {
                let observed = unsafe {
                    ptr::read_volatile((aperture_base as *const u8).add(destination + byte))
                };
                matches &= observed == (packed >> (byte * 8)) as u8;
            }
        }
        for offset in visible_row_bytes..info.pitch {
            let index = y * info.pitch + offset;
            if index >= aperture_length {
                return Err("semantic_presentation_framebuffer_extent_invalid");
            }
            let observed = unsafe { ptr::read_volatile((aperture_base as *const u8).add(index)) };
            matches &= observed == framebuffer_baseline_byte(index);
        }
    }
    Ok(matches)
}

fn encode_framebuffer_channel(value: u8, channel: color_channel) -> u32 {
    let maximum = (1u32 << channel.size) - 1;
    let scaled = (u32::from(value) * maximum + 127) / 255;
    scaled << channel.position
}

fn framebuffer_baseline_byte(index: usize) -> u8 {
    (index as u8).rotate_left(3) ^ 0xa5
}

fn retire_runtime_metadata(runtime: &mut presentation_runtime) {
    debug_assert!(runtime.source.is_none());
    debug_assert!(runtime.renderer.is_none());
    debug_assert!(runtime.stage.is_none());
    debug_assert!(runtime.endpoint.is_none());
    debug_assert!(runtime.binding.is_none());
    debug_assert!(runtime.acknowledgement.is_none());
    debug_assert!(runtime.supervisor.space.frame.is_null());
    // SAFETY: All external owners are withdrawn and these fields are dead
    // metadata immediately before the runtime drops.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(runtime.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.bootstrap_privilege_stack_top), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.original_task_switched), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.scratch), [0; page_size as usize]);
        ptr::write_volatile(
            ptr::addr_of_mut!(runtime.trace),
            presentation_trace::empty(),
        );
    }
}

fn emit_ready(source_pages: usize, renderer_pages: usize) {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.ready","level":"info","source_image_pages":{},"renderer_image_pages":{},"supervisor_image_pages":1,"stage_frames":576,"source_mappings":3,"renderer_mappings":3,"supervisor_mappings":3}}"#,
        source_pages, renderer_pages
    );
}

fn emit_source_released() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.source_released","level":"info","queued":true,"task_released":true}}"#
    );
}

fn emit_render_acknowledged() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.render_acknowledged","level":"info","deliveries":1,"rows":768,"frame_bytes":2359296,"observations":1}}"#
    );
}

fn emit_prepared() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.prepared","level":"info","tasks":0,"circles":0,"channels":0,"endpoints":0,"supervisions":0,"retained_stage_frames":576,"framebuffer_unchanged":true}}"#
    );
}

fn emit_presented() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.presented","level":"info","frame_commits":1,"framebuffer_verified":true,"stage_clears":1}}"#
    );
}

fn emit_released(frames: frame_accounting) {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.released","level":"info","free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_leases":0}}"#,
        frames.free
    );
}

fn emit_passed(evidence: presentation_evidence) {
    let trace = evidence.trace;
    if evidence.source_image_pages == 0
        || evidence.renderer_image_pages == 0
        || evidence.initial_frames.free == 0
        || trace.rite_calls != 777
        || trace.body_copies != 775
        || trace.body_bytes != 2_392_160
        || trace.submissions != 1
        || trace.deliveries != 1
        || trace.backpressure != 1
        || trace.denials != 2
        || trace.malformed != 1
        || trace.freshness_denials != 1
        || trace.rows != expected_rows
        || trace.exits != 2
        || trace.observations != 1
        || trace.frame_commits != 1
        || trace.stage_clears != 1
        || !trace.source_retired_before_delivery
        || !trace.destination_header_published_last
        || !trace.framebuffer_unchanged
        || !trace.framebuffer_verified
        || trace.kernel_continuations != 2
    {
        fail("semantic_presentation_evidence_invalid");
    }
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.passed","level":"info","rite_calls":777,"body_copies":775,"body_bytes":2392160,"submissions":1,"deliveries":1,"backpressure":1,"denials":2,"malformed":1,"freshness_denials":1,"rows":768,"exits":2,"observations":1,"frame_commits":1}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_presentation_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
