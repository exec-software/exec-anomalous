//! Removable x86-64 proof for one copied semantic-surface delivery.
//!
//! The feature owns two unrelated ring-3 address spaces. The source can only
//! submit one fixed body and exit; the consumer can only poll that body and
//! exit. The source is terminally retired, including its image and mailbox,
//! before the consumer is admitted. Delivery empties only the transport slot:
//! it does not complete a render lease, attest presentation, publish semantics,
//! or touch a framebuffer.

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

use crate::circle::{circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::serial_port;
use crate::identity::{circle_id, fixture_principal_id, task_identity};
use crate::memory;
use crate::memory::physical::{
    frame_accounting, frame_allocator, frame_lease, frame_purpose, frame_size, physical_range,
};
use crate::rite::{rite_admission, rite_channel, rite_decision};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::semantic_surface::{
    semantic_surface_binding, semantic_surface_endpoint, semantic_surface_endpoint_identity,
    semantic_surface_error,
};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_exit_payload,
    decode_semantic_surface_request, payload_span_fits, prepare_semantic_surface_delivery_reply,
    prepare_semantic_surface_empty_reply, read_call, reply_size, rite_scratch, scratch_is_zero,
    semantic_surface_request_size, semantic_surface_snapshot_size, staged_semantic_surface_reply,
};
use super::scheduler_test::{
    context_size, cr0_task_switched, frame_address_matches_stack, gate_registers_are_canonical,
    initial_user_flags, privilege_stack_pages, scheduler_frame,
    user_return_fields_are_valid_with_stack_top, user_return_fields_are_valid_within_stack,
    x86_64_scheduler_enter, x86_64_scheduler_resume_kernel,
};
use super::{descriptors, interrupts, paging};

const page_size: u64 = frame_size;
const user_image_address: u64 = 0x0000_6000_0000_1000;
const user_image_page_limit: usize = 64;
const user_mailbox_address: u64 = 0x0000_6000_0008_0000;
const user_stack_address: u64 = 0x0000_6000_0009_0000;
const user_stack_pages: usize = 4;
const user_stack_top: u64 = user_stack_address + user_stack_pages as u64 * page_size;
const rite_gate_vector: u64 = 0xf3;
const scheduler_capacity: usize = 1;
const source_principal: u64 = 1;
const consumer_principal: u64 = 2;
const endpoint_slot: usize = 0;
const endpoint_generation: u64 = 1;
const source_completion_status: u64 = 0x51;
const consumer_completion_status: u64 = 0x52;
const expected_exit_code: i32 = 0;

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
const consumer_grant: u64 = rite_rights::semantic_surface_read | rite_rights::task_exit;

const source_read_trace: u8 = 0x11;
const source_malformed_trace: u8 = 0x12;
const source_freshness_trace: u8 = 0x13;
const source_submit_trace: u8 = 0x14;
const source_backpressure_trace: u8 = 0x15;
const source_exit_trace: u8 = 0x16;
const consumer_submit_trace: u8 = 0x21;
const consumer_malformed_trace: u8 = 0x22;
const consumer_read_trace: u8 = 0x23;
const consumer_empty_trace: u8 = 0x24;
const consumer_exit_trace: u8 = 0x25;

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
    assert!(consumer_grant == 0x84);
    assert!(user_stack_pages == 4);
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum fixture_role {
    source,
    consumer,
}

impl fixture_role {
    const fn grant(self) -> u64 {
        match self {
            Self::source => source_grant,
            Self::consumer => consumer_grant,
        }
    }

    const fn principal(self) -> u64 {
        match self {
            Self::source => source_principal,
            Self::consumer => consumer_principal,
        }
    }

    const fn origin(self) -> synthetic_origin_metadata {
        match self {
            Self::source => synthetic_origin_metadata::human_role_fixture,
            Self::consumer => synthetic_origin_metadata::machine_role_fixture,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum transport_phase {
    source_read_denied,
    source_malformed_submit,
    source_freshness_denied,
    source_submit,
    source_backpressured,
    source_exit,
    source_retired,
    consumer_submit_denied,
    consumer_malformed_read,
    consumer_read,
    consumer_empty,
    consumer_exit,
    idle,
}

impl transport_phase {
    const fn role(self) -> Option<fixture_role> {
        match self {
            Self::source_read_denied
            | Self::source_malformed_submit
            | Self::source_freshness_denied
            | Self::source_submit
            | Self::source_backpressured
            | Self::source_exit => Some(fixture_role::source),
            Self::consumer_submit_denied
            | Self::consumer_malformed_read
            | Self::consumer_read
            | Self::consumer_empty
            | Self::consumer_exit => Some(fixture_role::consumer),
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
            Self::consumer_submit_denied => Some(1),
            Self::consumer_malformed_read => Some(2),
            Self::consumer_read => Some(3),
            Self::consumer_empty => Some(4),
            Self::consumer_exit => Some(5),
            Self::source_retired | Self::idle => None,
        }
    }

    const fn trace(self) -> Option<id_128> {
        let value = match self {
            Self::source_read_denied => source_read_trace,
            Self::source_malformed_submit => source_malformed_trace,
            Self::source_freshness_denied => source_freshness_trace,
            Self::source_submit => source_submit_trace,
            Self::source_backpressured => source_backpressure_trace,
            Self::source_exit => source_exit_trace,
            Self::consumer_submit_denied => consumer_submit_trace,
            Self::consumer_malformed_read => consumer_malformed_trace,
            Self::consumer_read => consumer_read_trace,
            Self::consumer_empty => consumer_empty_trace,
            Self::consumer_exit => consumer_exit_trace,
            Self::source_retired | Self::idle => return None,
        };
        Some(id_128::from_bytes([value; 16]))
    }
}

#[derive(Clone, Copy)]
struct payload_contract {
    start: *const u8,
    end: *const u8,
    size: *const u8,
    entry: *const u8,
}

impl payload_contract {
    fn length(self) -> Option<usize> {
        (self.end as usize).checked_sub(self.start as usize)
    }

    fn page_count(self) -> Option<usize> {
        let length = self.length()?;
        (length != 0).then_some(length.div_ceil(page_size as usize))
    }

    fn linked_user_address(self, symbol: *const u8) -> Option<u64> {
        let offset = (symbol as usize).checked_sub(self.start as usize)?;
        if offset >= self.length()? {
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

struct transport_task {
    circle: circle_id,
    identity: task_identity,
    role: fixture_role,
    space: task_space,
}

#[derive(Clone, Copy)]
struct transport_trace {
    rite_calls: usize,
    body_copies: usize,
    body_bytes: usize,
    submissions: usize,
    deliveries: usize,
    backpressure: usize,
    denials: usize,
    malformed: usize,
    exits: usize,
    source_retired_before_delivery: bool,
    destination_header_published_last: bool,
    stale_reply_cleared: bool,
    kernel_continuations: usize,
}

impl transport_trace {
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
            exits: 0,
            source_retired_before_delivery: false,
            destination_header_published_last: false,
            stale_reply_cleared: false,
            kernel_continuations: 0,
        }
    }
}

struct transport_runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    source: Option<transport_task>,
    consumer: Option<transport_task>,
    source_channel: Option<rite_channel>,
    consumer_channel: Option<rite_channel>,
    endpoint: semantic_surface_endpoint,
    binding: semantic_surface_binding,
    active_role: Option<fixture_role>,
    phase: transport_phase,
    scratch: rite_scratch,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    trace: transport_trace,
}

#[derive(Clone, Copy)]
struct transport_evidence {
    trace: transport_trace,
    source_image_pages: usize,
    consumer_image_pages: usize,
    initial_frames: frame_accounting,
}

static active_runtime: AtomicPtr<transport_runtime> = AtomicPtr::new(ptr::null_mut());
static rite_gate_active: AtomicBool = AtomicBool::new(false);

// The build packager must add every symbol below to the two binary wrapper
// objects. Each entry/return symbol is the corresponding target-ELF symbol at
// its exact offset within the wrapper's readonly payload section.
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

    static x86_64_semantic_surface_consumer_payload_start: u8;
    static x86_64_semantic_surface_consumer_payload_end: u8;
    static x86_64_semantic_surface_consumer_payload_size: u8;
    static x86_64_semantic_surface_consumer_entry: u8;
    static x86_64_semantic_surface_consumer_submit_denied_return: u8;
    static x86_64_semantic_surface_consumer_malformed_read_return: u8;
    static x86_64_semantic_surface_consumer_delivery_return: u8;
    static x86_64_semantic_surface_consumer_empty_return: u8;
    static x86_64_semantic_surface_consumer_exit_return: u8;
}

fn payload_for(role: fixture_role) -> payload_contract {
    match role {
        fixture_role::source => payload_contract {
            start: ptr::addr_of!(x86_64_semantic_surface_source_payload_start),
            end: ptr::addr_of!(x86_64_semantic_surface_source_payload_end),
            size: ptr::addr_of!(x86_64_semantic_surface_source_payload_size),
            entry: ptr::addr_of!(x86_64_semantic_surface_source_entry),
        },
        fixture_role::consumer => payload_contract {
            start: ptr::addr_of!(x86_64_semantic_surface_consumer_payload_start),
            end: ptr::addr_of!(x86_64_semantic_surface_consumer_payload_end),
            size: ptr::addr_of!(x86_64_semantic_surface_consumer_payload_size),
            entry: ptr::addr_of!(x86_64_semantic_surface_consumer_entry),
        },
    }
}

pub(super) fn run() -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("semantic_surface_initial_state_invalid");
    }
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("semantic_surface_interrupt_mask_invalid");
    }

    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Threshold keeps NMI masked and this proof runs on the BSP with IF
    // clear for the complete pair of task entries.
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        run_with_allocator(allocator, kernel_cr3, bootstrap_privilege_stack_top)
    });

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact live root captured before either task root.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    let evidence = match result {
        Ok(Ok(evidence)) => evidence,
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    };
    emit_passed(evidence);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
) -> Result<transport_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !interrupts::scheduler_interrupts_are_masked()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("semantic_surface_setup_state_invalid");
    }
    let source_payload = validate_payload_contract(fixture_role::source)?;
    let consumer_payload = validate_payload_contract(fixture_role::consumer)?;
    let source_image_pages = source_payload
        .page_count()
        .ok_or("semantic_surface_source_page_count_invalid")?;
    let consumer_image_pages = consumer_payload
        .page_count()
        .ok_or("semantic_surface_consumer_page_count_invalid")?;
    let required_frames = source_image_pages
        .checked_add(consumer_image_pages)
        .and_then(|count| count.checked_add(26))
        .ok_or("semantic_surface_frame_requirement_overflow")?;
    let initial = allocator.accounting();
    if initial.free < required_frames
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("semantic_surface_initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let source_circle = circles
        .create()
        .map_err(|_| "semantic_surface_source_circle_create_failed")?;
    let consumer_circle = circles
        .create()
        .map_err(|_| "semantic_surface_consumer_circle_create_failed")?;
    let source_space =
        prepare_task_space(allocator, kernel_cr3, fixture_role::source, source_payload)?;
    let consumer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        fixture_role::consumer,
        consumer_payload,
    )?;
    attach_task_space(&mut circles, source_circle)?;
    attach_task_space(&mut circles, consumer_circle)?;
    let source_identity = create_task(
        &mut circles,
        &mut tasks,
        source_circle,
        fixture_role::source,
        &source_space,
    )?;
    let consumer_identity = create_task(
        &mut circles,
        &mut tasks,
        consumer_circle,
        fixture_role::consumer,
        &consumer_space,
    )?;
    let snapshot_binding = expected_snapshot_binding()?;
    let endpoint_identity =
        semantic_surface_endpoint_identity::try_new(endpoint_slot, endpoint_generation)
            .map_err(|_| "semantic_surface_endpoint_identity_invalid")?;
    if endpoint_identity
        .next_generation()
        .map_err(|_| "semantic_surface_endpoint_generation_invalid")?
        .generation()
        != endpoint_generation + 1
    {
        return Err("semantic_surface_endpoint_generation_invalid");
    }
    let binding = semantic_surface_binding::try_new(
        endpoint_identity,
        source_identity,
        consumer_identity,
        snapshot_binding,
    )
    .map_err(|_| "semantic_surface_binding_invalid")?;
    let endpoint = semantic_surface_endpoint::new(binding);
    let source_channel = rite_channel::try_new(source_identity, fixture_role::source.grant())
        .map_err(|_| "semantic_surface_source_channel_create_failed")?;
    let consumer_channel = rite_channel::try_new(consumer_identity, fixture_role::consumer.grant())
        .map_err(|_| "semantic_surface_consumer_channel_create_failed")?;
    let mut scheduler = scheduler_table::new(scheduler_capacity)
        .map_err(|_| "semantic_surface_scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, source_identity)
        .map_err(|_| "semantic_surface_source_admission_failed")?;

    let mut runtime = transport_runtime {
        circles,
        tasks,
        scheduler,
        source: Some(transport_task {
            circle: source_circle,
            identity: source_identity,
            role: fixture_role::source,
            space: source_space,
        }),
        consumer: Some(transport_task {
            circle: consumer_circle,
            identity: consumer_identity,
            role: fixture_role::consumer,
            space: consumer_space,
        }),
        source_channel: Some(source_channel),
        consumer_channel: Some(consumer_channel),
        endpoint,
        binding,
        active_role: None,
        phase: transport_phase::source_read_denied,
        scratch: [0; page_size as usize],
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        trace: transport_trace::empty(),
    };

    validate_ready_state(&runtime, initial, source_image_pages, consumer_image_pages)?;
    emit_ready(source_image_pages, consumer_image_pages);

    enter_role(
        &mut runtime,
        fixture_role::source,
        0,
        source_completion_status,
    )?;
    runtime.trace.kernel_continuations += 1;
    if runtime.phase != transport_phase::source_retired
        || runtime.trace.submissions != 1
        || runtime.trace.backpressure != 1
        || runtime.trace.exits != 1
        || runtime.endpoint.binding() != runtime.binding
    {
        return Err("semantic_surface_source_completion_invalid");
    }
    retire_role(allocator, &mut runtime, fixture_role::source)?;
    runtime.trace.source_retired_before_delivery = true;
    emit_source_released();

    runtime
        .scheduler
        .admit(&mut runtime.circles, &mut runtime.tasks, consumer_identity)
        .map_err(|_| "semantic_surface_consumer_admission_failed")?;
    runtime.phase = transport_phase::consumer_submit_denied;
    enter_role(
        &mut runtime,
        fixture_role::consumer,
        1,
        consumer_completion_status,
    )?;
    runtime.trace.kernel_continuations += 1;
    if runtime.phase != transport_phase::idle
        || runtime.trace.deliveries != 1
        || runtime.trace.exits != 2
        || !runtime.trace.source_retired_before_delivery
        || !runtime.trace.destination_header_published_last
        || !runtime.trace.stale_reply_cleared
    {
        return Err("semantic_surface_consumer_completion_invalid");
    }
    retire_role(allocator, &mut runtime, fixture_role::consumer)?;
    emit_consumer_released();

    let close = runtime
        .endpoint
        .close(runtime.binding)
        .map_err(|_| "semantic_surface_endpoint_close_failed")?;
    if !close.first_close() || close.discarded() || !runtime.endpoint.storage_is_clear() {
        return Err("semantic_surface_endpoint_close_invalid");
    }
    if !clear_and_verify_bytes(&mut runtime.scratch) {
        return Err("semantic_surface_final_scratch_clear_failed");
    }

    let final_scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "semantic_surface_final_scheduler_invalid")?;
    let final_circles = runtime.circles.accounting();
    let final_frames = allocator.accounting();
    if final_frames != initial
        || final_circles.circles != 0
        || final_circles.tasks != 0
        || final_circles.task_contexts != 0
        || final_circles.endpoints != 0
        || final_circles.sigils != 0
        || final_circles.supervisions != 0
        || final_circles.address_spaces != 0
        || final_circles.mappings != 0
        || runtime.tasks.active_count() != 0
        || runtime.tasks.attached_context_count() != 0
        || final_scheduler.admitted() != 0
        || final_scheduler.runnable() != 0
        || final_scheduler.current().is_some()
        || final_scheduler.reservation().is_some()
        || runtime.source.is_some()
        || runtime.consumer.is_some()
        || runtime.source_channel.is_some()
        || runtime.consumer_channel.is_some()
        || runtime.active_role.is_some()
        || !active_runtime.load(Ordering::Acquire).is_null()
        || rite_gate_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_privilege_stack_top
        || paging::read_cr0() & cr0_task_switched != runtime.original_task_switched
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("semantic_surface_final_state_invalid");
    }
    if !descriptors::wipe_privilege_stack() {
        return Err("semantic_surface_transition_stack_clear_failed");
    }

    emit_released(final_frames);
    Ok(transport_evidence {
        trace: runtime.trace,
        source_image_pages,
        consumer_image_pages,
        initial_frames: initial,
    })
}

fn expected_snapshot_binding() -> Result<surface_snapshot_binding, &'static str> {
    let surface = transport_identity::try_new(surface_slot, surface_generation)
        .ok_or("semantic_surface_surface_identity_invalid")?;
    let scene = transport_identity::try_new(scene_slot, scene_generation)
        .ok_or("semantic_surface_scene_identity_invalid")?;
    let renderer = transport_identity::try_new(renderer_slot, renderer_generation)
        .ok_or("semantic_surface_renderer_identity_invalid")?;
    surface_snapshot_binding::try_new(
        surface,
        scene,
        base_revision,
        revision,
        renderer,
        delivery_attempt,
    )
    .map_err(|_| "semantic_surface_snapshot_binding_invalid")
}

fn enter_role(
    runtime: &mut transport_runtime,
    role: fixture_role,
    expected_dispatch_sequence: u64,
    expected_status: u64,
) -> Result<(), &'static str> {
    let dispatch = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "semantic_surface_dispatch_failed")?;
    let identity = runtime.task(role)?.identity;
    if dispatch.task() != identity
        || dispatch.sequence() != expected_dispatch_sequence
        || dispatch.context() != scheduler_context::initial
    {
        return Err("semantic_surface_dispatch_invalid");
    }
    let frame = runtime.select_role(role, false)?;
    let runtime_pointer = ptr::from_mut(runtime);
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("semantic_surface_runtime_publication_failed");
    }
    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The selected root, TSS stack, initial frame, and published
    // runtime remain live until terminal Rite consumes this continuation.
    let status = unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    if status != expected_status
        || runtime.active_role.is_some()
        || paging::read_cr3() != runtime.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || runtime.original_task_switched != original_task_switched
        || runtime.saved_kernel_stack == 0
    {
        return Err("semantic_surface_kernel_continuation_invalid");
    }
    Ok(())
}

fn validate_payload_contract(role: fixture_role) -> Result<payload_contract, &'static str> {
    let payload = payload_for(role);
    let length = payload
        .length()
        .ok_or("semantic_surface_payload_extent_invalid")?;
    let pages = payload
        .page_count()
        .ok_or("semantic_surface_payload_page_count_invalid")?;
    if pages > user_image_page_limit
        || payload.size as usize != length
        || payload.linked_user_address(payload.entry) != Some(user_image_address)
    {
        return Err("semantic_surface_payload_contract_invalid");
    }
    let symbols: &[*const u8] = match role {
        fixture_role::source => &[
            ptr::addr_of!(x86_64_semantic_surface_source_read_denied_return),
            ptr::addr_of!(x86_64_semantic_surface_source_malformed_submit_return),
            ptr::addr_of!(x86_64_semantic_surface_source_freshness_denied_return),
            ptr::addr_of!(x86_64_semantic_surface_source_submit_return),
            ptr::addr_of!(x86_64_semantic_surface_source_backpressured_return),
            ptr::addr_of!(x86_64_semantic_surface_source_exit_return),
        ],
        fixture_role::consumer => &[
            ptr::addr_of!(x86_64_semantic_surface_consumer_submit_denied_return),
            ptr::addr_of!(x86_64_semantic_surface_consumer_malformed_read_return),
            ptr::addr_of!(x86_64_semantic_surface_consumer_delivery_return),
            ptr::addr_of!(x86_64_semantic_surface_consumer_empty_return),
            ptr::addr_of!(x86_64_semantic_surface_consumer_exit_return),
        ],
    };
    if symbols
        .iter()
        .any(|symbol| payload.linked_user_address(*symbol).is_none())
        || symbols.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("semantic_surface_payload_symbol_contract_invalid");
    }
    Ok(payload)
}

fn allocate_data(
    allocator: &mut frame_allocator<'_>,
    count: usize,
    alignment: usize,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator grants the callback exclusive access to the exact
    // whole-frame extent for synchronous clearing.
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
        .ok_or("semantic_surface_payload_page_count_invalid")?;
    let image = allocate_data(
        allocator,
        image_pages,
        1,
        "semantic_surface_image_allocation_failed",
    )?;
    let mailbox = allocate_data(
        allocator,
        1,
        1,
        "semantic_surface_mailbox_allocation_failed",
    )?;
    let stack = allocate_data(
        allocator,
        user_stack_pages,
        user_stack_pages,
        "semantic_surface_stack_allocation_failed",
    )?;
    let privilege_stack = allocate_data(
        allocator,
        privilege_stack_pages,
        privilege_stack_pages,
        "semantic_surface_privilege_stack_allocation_failed",
    )?;
    let image_physical = image
        .range()
        .ok_or("semantic_surface_image_extent_missing")?;
    let mailbox_physical = mailbox
        .range()
        .ok_or("semantic_surface_mailbox_extent_missing")?;
    let stack_physical = stack
        .range()
        .ok_or("semantic_surface_stack_extent_missing")?;
    let privilege_stack_range = privilege_stack
        .range()
        .ok_or("semantic_surface_privilege_stack_extent_missing")?;
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
        return Err("semantic_surface_task_frames_invalid");
    }
    copy_payload(payload, image_physical)?;

    // SAFETY: The allocator is exclusively borrowed and the callback clears
    // every page-table extent before the new root is published.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "semantic_surface_address_space_create_failed")?;
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
    .map_err(|_| "semantic_surface_image_mapping_failed")?;
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
    .map_err(|_| "semantic_surface_mailbox_mapping_failed")?;
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
    .map_err(|_| "semantic_surface_stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "semantic_surface_mapping_validation_failed")?;
    for page in 0..image_pages {
        validate_mapping(
            root.validate_mapping(user_image_address + page as u64 * page_size)
                .map_err(|_| "semantic_surface_image_mapping_walk_failed")?,
            image_physical.base + page as u64 * page_size,
            false,
            true,
        )?;
    }
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "semantic_surface_mailbox_mapping_walk_failed")?,
        mailbox_physical.base,
        true,
        false,
    )?;
    for page in 0..user_stack_pages {
        validate_mapping(
            root.validate_mapping(user_stack_address + page as u64 * page_size)
                .map_err(|_| "semantic_surface_stack_mapping_walk_failed")?,
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
            .map_err(|_| "semantic_surface_guard_mapping_present")?;
    }
    if !payload_matches(payload, image_physical) {
        return Err("semantic_surface_payload_copy_mismatch");
    }

    allocator
        .bind(&privilege_stack)
        .map_err(|_| "semantic_surface_privilege_stack_binding_failed")?;
    let privilege_stack_top = privilege_stack_range
        .end()
        .ok_or("semantic_surface_privilege_stack_extent_invalid")?;
    let frame_address = privilege_stack_top
        .checked_sub(context_size)
        .ok_or("semantic_surface_initial_frame_address_invalid")?;
    if frame_address < privilege_stack_range.base
        || frame_address & (mem::align_of::<u64>() as u64 - 1) != 0
    {
        return Err("semantic_surface_initial_frame_address_invalid");
    }
    let entry = payload
        .linked_user_address(payload.entry)
        .ok_or("semantic_surface_entry_invalid")?;
    let frame = frame_address as usize as *mut scheduler_frame;
    let mut initial = scheduler_frame::initial(entry);
    initial.stack_pointer = user_stack_top;
    // SAFETY: The bound supervisor-only privilege-stack lease exclusively owns
    // this aligned complete frame slot.
    unsafe { ptr::write_volatile(frame, initial) };
    compiler_fence(Ordering::SeqCst);

    if role == fixture_role::consumer && payload_contains_snapshot(payload, image_physical, None) {
        return Err("semantic_surface_consumer_contains_source_body");
    }
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
        .map_err(|_| "semantic_surface_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "semantic_surface_root_attachment_rollback_failed")?;
        return Err("semantic_surface_mapping_attachment_failed");
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
        .length()
        .ok_or("semantic_surface_task_payload_length_invalid")?;
    let code_end = user_image_address
        .checked_add(payload_length as u64)
        .ok_or("semantic_surface_task_code_extent_invalid")?;
    // SAFETY: Preparation initialized the complete frame before task creation.
    let entry = unsafe { ptr::read_volatile(space.frame) }.instruction_pointer;
    let bounds = task_bounds::new(
        user_image_address,
        code_end,
        entry,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("semantic_surface_task_bounds_invalid")?;
    let principal = fixture_principal_id::new(role.principal())
        .ok_or("semantic_surface_task_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(circle, principal, role.origin(), bounds),
        )
        .map_err(|_| "semantic_surface_task_create_failed")
}

impl transport_runtime {
    fn task(&self, role: fixture_role) -> Result<&transport_task, &'static str> {
        match role {
            fixture_role::source => self
                .source
                .as_ref()
                .ok_or("semantic_surface_source_missing"),
            fixture_role::consumer => self
                .consumer
                .as_ref()
                .ok_or("semantic_surface_consumer_missing"),
        }
    }

    fn current_role(&self) -> Option<fixture_role> {
        let current = self.scheduler.snapshot(&self.tasks).ok()?.current()?;
        let role = if self
            .source
            .as_ref()
            .is_some_and(|task| task.identity == current)
        {
            fixture_role::source
        } else if self
            .consumer
            .as_ref()
            .is_some_and(|task| task.identity == current)
        {
            fixture_role::consumer
        } else {
            return None;
        };
        (self.active_role == Some(role)).then_some(role)
    }

    fn select_role(
        &mut self,
        role: fixture_role,
        require_task_switched: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let task = self.task(role)?;
        let root = task
            .space
            .root
            .root_address()
            .map_err(|_| "semantic_surface_selected_root_missing")?;
        let stack_top = task
            .space
            .privilege_stack_range
            .end()
            .ok_or("semantic_surface_selected_stack_extent_invalid")?;
        let frame = task.space.frame;
        // SAFETY: The selected task owns the complete root and bound privilege stack.
        unsafe { task.space.root.activate() }
            .map_err(|_| "semantic_surface_selected_root_activation_failed")?;
        // SAFETY: The exact stack remains bound until terminal task retirement.
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("semantic_surface_selected_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || !self.frame_is_valid_for_role(frame, role, require_task_switched)
        {
            return Err("semantic_surface_selected_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: fixture_role,
        require_task_switched: bool,
    ) -> bool {
        let Ok(task) = self.task(role) else {
            return false;
        };
        if frame != task.space.frame
            || !frame_address_matches_stack(frame, task.space.privilege_stack_range)
            || task.space.privilege_stack.frame_count() != privilege_stack_pages
            || task.space.privilege_stack.range() != Some(task.space.privilege_stack_range)
            || task.space.root.validate_all_mappings().is_err()
        {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(task.identity) else {
            return false;
        };
        if snapshot.state() != task_state::running || !snapshot.context_attached() {
            return false;
        }
        if task.space.root.root_address().ok() != Some(paging::entry_address(paging::read_cr3()))
            || task.space.privilege_stack_range.end()
                != Some(unsafe { descriptors::selected_privilege_stack_top() })
            || (require_task_switched && paging::read_cr0() & cr0_task_switched == 0)
        {
            return false;
        }
        // SAFETY: The preceding checks prove the complete frame remains live.
        let frame = unsafe { ptr::read(frame) };
        if require_task_switched {
            user_return_fields_are_valid_within_stack(frame, snapshot.bounds())
        } else {
            user_return_fields_are_valid_with_stack_top(frame, snapshot.bounds(), user_stack_top)
        }
    }

    fn take_channel(&mut self, role: fixture_role) -> Result<rite_channel, &'static str> {
        match role {
            fixture_role::source => self
                .source_channel
                .take()
                .ok_or("semantic_surface_source_channel_missing"),
            fixture_role::consumer => self
                .consumer_channel
                .take()
                .ok_or("semantic_surface_consumer_channel_missing"),
        }
    }

    fn restore_channel(&mut self, role: fixture_role, channel: rite_channel) {
        let destination = match role {
            fixture_role::source => &mut self.source_channel,
            fixture_role::consumer => &mut self.consumer_channel,
        };
        if destination.is_some() {
            fail("semantic_surface_channel_replacement_invalid");
        }
        *destination = Some(channel);
    }
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
        return Err("semantic_surface_mapping_permissions_invalid");
    }
    Ok(())
}

fn copy_payload(
    payload: payload_contract,
    destination: physical_range,
) -> Result<(), &'static str> {
    let length = payload
        .length()
        .ok_or("semantic_surface_payload_extent_invalid")?;
    if length as u64 > destination.length {
        return Err("semantic_surface_payload_destination_invalid");
    }
    let base = usize::try_from(destination.base)
        .map_err(|_| "semantic_surface_payload_address_invalid")?;
    // SAFETY: The destination is a cleared exclusive frame allocation and the
    // linker contract bounds the source extent.
    unsafe { ptr::copy_nonoverlapping(payload.start, base as *mut u8, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

fn payload_matches(payload: payload_contract, destination: physical_range) -> bool {
    let Some(length) = payload.length() else {
        return false;
    };
    let Ok(base) = usize::try_from(destination.base) else {
        return false;
    };
    if length as u64 > destination.length {
        return false;
    }
    for offset in 0..length {
        // SAFETY: Both extents were validated by the fixed wrapper contract.
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
    let Some(length) = payload.length() else {
        return true;
    };
    let Ok(base) = usize::try_from(range.base) else {
        return true;
    };
    if length < semantic_surface_snapshot_size || length as u64 > range.length {
        return false;
    }
    for offset in 0..=length - semantic_surface_snapshot_size {
        // SAFETY: The complete copied image allocation remains live and bounded.
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

fn validate_ready_state(
    runtime: &transport_runtime,
    initial: frame_accounting,
    source_pages: usize,
    consumer_pages: usize,
) -> Result<(), &'static str> {
    let frames = runtime
        .source
        .as_ref()
        .zip(runtime.consumer.as_ref())
        .ok_or("semantic_surface_ready_task_missing")?;
    let accounting = runtime.circles.accounting();
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "semantic_surface_ready_scheduler_invalid")?;
    if source_pages == 0
        || consumer_pages == 0
        || frames.0.space.image_physical == frames.1.space.image_physical
        || frames.0.space.mailbox_physical == frames.1.space.mailbox_physical
        || frames.0.space.stack_physical == frames.1.space.stack_physical
        || runtime.source_channel.is_none()
        || runtime.consumer_channel.is_none()
        || !runtime.endpoint.storage_is_clear()
        || !scratch_is_zero(&runtime.scratch)
        || accounting.circles != 2
        || accounting.tasks != 2
        || accounting.task_contexts != 1
        || accounting.address_spaces != 2
        || accounting.mappings != 6
        || runtime.tasks.active_count() != 2
        || runtime.tasks.attached_context_count() != 1
        || scheduler.admitted() != 1
        || scheduler.runnable() != 1
        || scheduler.current().is_some()
        || initial.free <= source_pages + consumer_pages
    {
        return Err("semantic_surface_ready_state_invalid");
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum gate_outcome {
    return_to_user(*mut scheduler_frame),
    terminal,
}

#[derive(Clone, Copy)]
struct gate_call {
    frame: *mut scheduler_frame,
    role: fixture_role,
    identity: task_identity,
    mailbox: u64,
    phase: transport_phase,
    call: rite_call,
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_semantic_surface_transport_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || rite_gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("semantic_surface_rite_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("semantic_surface_rite_gate_without_runtime");
    }
    // SAFETY: The published runtime is stable on the original kernel stack;
    // the interrupt gate cleared IF and the fixture runs only on the BSP.
    let runtime = unsafe { &mut *runtime_pointer };
    match dispatch_rite(runtime, frame) {
        Ok(gate_outcome::return_to_user(next)) => {
            rite_gate_active.store(false, Ordering::Release);
            next
        }
        Ok(gate_outcome::terminal) => resume_terminal(runtime),
        Err(reason) => fail(reason),
    }
}

fn dispatch_rite(
    runtime: &mut transport_runtime,
    frame: *mut scheduler_frame,
) -> Result<gate_outcome, &'static str> {
    let phase = runtime.phase;
    let role = phase.role().ok_or("semantic_surface_rite_idle_phase")?;
    let identity = runtime.task(role)?.identity;
    if runtime.current_role() != Some(role) || !runtime.frame_is_valid_for_role(frame, role, true) {
        return Err("semantic_surface_rite_runtime_state_invalid");
    }
    // SAFETY: The validated complete frame remains live until it is returned
    // to the gate or terminally abandoned by the scheduler continuation.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || incoming.instruction_pointer != expected_rite_return(runtime, phase)?
        || !gate_registers_are_canonical(incoming, user_mailbox_address)
    {
        return Err("semantic_surface_rite_frame_invalid");
    }
    let mailbox = runtime.task(role)?.space.mailbox_physical.base;
    // SAFETY: This is the exclusive supervisor alias of the selected task's
    // mapped one-page mailbox for the duration of the interrupt gate.
    let call = unsafe { read_call(mailbox) }?;
    validate_rite_call(phase, call)?;

    let mut channel = runtime.take_channel(role)?;
    let gate_call = gate_call {
        frame,
        role,
        identity,
        mailbox,
        phase,
        call,
    };
    let outcome = dispatch_channel(runtime, gate_call, &mut channel);
    runtime.restore_channel(role, channel);
    outcome
}

fn dispatch_channel(
    runtime: &mut transport_runtime,
    gate: gate_call,
    channel: &mut rite_channel,
) -> Result<gate_outcome, &'static str> {
    let gate_call {
        frame,
        role,
        identity,
        mailbox,
        phase,
        call,
    } = gate;
    let decision = channel
        .assess(&runtime.tasks, identity, call)
        .map_err(|_| "semantic_surface_rite_assessment_failed")?;
    runtime.trace.rite_calls = runtime
        .trace
        .rite_calls
        .checked_add(1)
        .ok_or("semantic_surface_rite_call_count_overflow")?;
    let admission = match decision {
        rite_decision::rejected(reply) => {
            if !matches!(
                phase,
                transport_phase::source_read_denied | transport_phase::consumer_submit_denied
            ) || reply.status != rite_status::denied
            {
                return Err("semantic_surface_unexpected_rite_denial");
            }
            let staged = stage_empty_reply(mailbox, reply)?;
            runtime.trace.denials += 1;
            publish_empty_reply(staged);
            advance_phase(runtime, phase)?;
            return Ok(gate_outcome::return_to_user(frame));
        }
        rite_decision::admitted(admission) => admission,
    };
    if admission.vessel() != identity
        || admission.sequence()
            != phase
                .sequence()
                .ok_or("semantic_surface_phase_sequence_missing")?
        || admission.trace_id()
            != phase
                .trace()
                .ok_or("semantic_surface_phase_trace_missing")?
    {
        return Err("semantic_surface_rite_admission_invalid");
    }

    match phase {
        transport_phase::source_malformed_submit
        | transport_phase::source_freshness_denied
        | transport_phase::source_submit
        | transport_phase::source_backpressured => {
            handle_submit(runtime, frame, mailbox, phase, admission)
        }
        transport_phase::consumer_malformed_read
        | transport_phase::consumer_read
        | transport_phase::consumer_empty => handle_read(runtime, frame, mailbox, phase, admission),
        transport_phase::source_exit | transport_phase::consumer_exit => {
            handle_exit(runtime, role, admission)
        }
        transport_phase::source_read_denied | transport_phase::consumer_submit_denied => {
            Err("semantic_surface_denied_rite_admitted")
        }
        transport_phase::source_retired | transport_phase::idle => {
            Err("semantic_surface_rite_idle_phase")
        }
    }
}

fn handle_submit(
    runtime: &mut transport_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    phase: transport_phase,
    admission: rite_admission<'_>,
) -> Result<gate_outcome, &'static str> {
    if admission.operation() != rite_operation::semantic_surface_submit
        || admission.payload_kind() != rite_payload_kind::semantic_surface_snapshot
        || admission.payload_size() as usize != semantic_surface_snapshot_size
    {
        return Err("semantic_surface_submit_admission_shape_invalid");
    }
    // SAFETY: Admission fixed the complete body span and the gate retains the
    // selected source mailbox's mapping and exclusion for the exact copy.
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
        transport_phase::source_malformed_submit => {
            if body.is_ok() {
                return Err("semantic_surface_malformed_submit_accepted");
            }
            let reply = admission
                .semantic_surface_empty_reply(rite_status::bad_payload)
                .map_err(|_| "semantic_surface_malformed_submit_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            let malformed = runtime
                .trace
                .malformed
                .checked_add(1)
                .ok_or("semantic_surface_malformed_count_overflow")?;
            clear_scratch(runtime)?;
            let settled = admission
                .reject_after_revalidation(&runtime.tasks, rite_status::bad_payload)
                .map_err(|_| "semantic_surface_malformed_submit_rejection_failed")?;
            if settled != reply {
                fail("semantic_surface_malformed_submit_reply_changed");
            }
            runtime.trace.malformed = malformed;
            publish_empty_reply(staged);
        }
        transport_phase::source_freshness_denied => {
            let body = body.map_err(|_| "semantic_surface_freshness_body_invalid")?;
            let rejection = match runtime.endpoint.prepare_submit(runtime.binding, body) {
                Ok(_) => return Err("fixture freshness body unexpectedly queued"),
                Err(rejection) => rejection,
            };
            if rejection.error() != semantic_surface_error::wrong_surface {
                return Err("semantic_surface_freshness_rejection_invalid");
            }
            drop(rejection);
            let reply = admission
                .semantic_surface_empty_reply(rite_status::denied)
                .map_err(|_| "semantic_surface_freshness_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            // RITE admitted this call; the endpoint's exact wrong-surface
            // rejection proves freshness correlation and is not an authority
            // denial. Only RITE channel rejections contribute to `denials`.
            clear_scratch(runtime)?;
            let settled = admission
                .reject_after_revalidation(&runtime.tasks, rite_status::denied)
                .map_err(|_| "semantic_surface_freshness_rejection_failed")?;
            if settled != reply {
                fail("semantic_surface_freshness_reply_changed");
            }
            publish_empty_reply(staged);
        }
        transport_phase::source_submit => {
            let body = body.map_err(|_| "semantic_surface_submit_body_invalid")?;
            let completion = admission
                .prepare_semantic_surface_submit_ok(&runtime.tasks)
                .map_err(|_| "semantic_surface_submit_revalidation_failed")?;
            let prepared = runtime
                .endpoint
                .prepare_submit(runtime.binding, body)
                .map_err(|_| "semantic_surface_submit_preparation_failed")?;
            let reply = completion.reply();
            let staged = stage_empty_reply(mailbox, reply)?;
            let submissions = runtime
                .trace
                .submissions
                .checked_add(1)
                .ok_or("semantic_surface_submission_count_overflow")?;
            clear_scratch(runtime)?;
            let binding = runtime.binding;
            let endpoint = &mut runtime.endpoint;
            let settled = completion.commit(|| {
                let receipt = endpoint
                    .commit_submit(prepared)
                    .unwrap_or_else(|_| fail("semantic_surface_submit_commit_failed"));
                if receipt.binding() != binding {
                    fail("semantic_surface_submit_receipt_invalid");
                }
            });
            if settled != reply {
                fail("semantic_surface_submit_reply_changed");
            }
            runtime.trace.submissions = submissions;
            publish_empty_reply(staged);
        }
        transport_phase::source_backpressured => {
            let body = body.map_err(|_| "semantic_surface_backpressure_body_invalid")?;
            let rejection = match runtime.endpoint.prepare_submit(runtime.binding, body) {
                Ok(_) => return Err("semantic_surface_duplicate_submit_accepted"),
                Err(rejection) => rejection,
            };
            if rejection.error() != semantic_surface_error::endpoint_full {
                return Err("semantic_surface_backpressure_rejection_invalid");
            }
            drop(rejection);
            let reply = admission
                .semantic_surface_empty_reply(rite_status::would_block)
                .map_err(|_| "semantic_surface_backpressure_reply_preparation_failed")?;
            let staged = stage_empty_reply(mailbox, reply)?;
            let backpressure = runtime
                .trace
                .backpressure
                .checked_add(1)
                .ok_or("semantic_surface_backpressure_count_overflow")?;
            clear_scratch(runtime)?;
            let settled = admission
                .finish_semantic_surface_submit_would_block(&runtime.tasks)
                .map_err(|_| "semantic_surface_backpressure_completion_failed")?;
            if settled != reply {
                fail("semantic_surface_backpressure_reply_changed");
            }
            runtime.trace.backpressure = backpressure;
            publish_empty_reply(staged);
        }
        _ => return Err("semantic_surface_submit_phase_invalid"),
    }
    runtime.phase = next;
    Ok(gate_outcome::return_to_user(frame))
}

fn handle_read(
    runtime: &mut transport_runtime,
    frame: *mut scheduler_frame,
    mailbox: u64,
    phase: transport_phase,
    admission: rite_admission<'_>,
) -> Result<gate_outcome, &'static str> {
    if admission.operation() != rite_operation::semantic_surface_read
        || admission.payload_kind() != rite_payload_kind::semantic_surface_request
        || admission.payload_size() as usize != semantic_surface_request_size
    {
        return Err("semantic_surface_read_admission_shape_invalid");
    }
    let next = next_phase(phase)?;
    // SAFETY: Admission fixed the one complete request span and the gate owns
    // the consumer mailbox until the reply header has been published.
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
        if phase != transport_phase::consumer_malformed_read {
            return Err("semantic_surface_read_request_invalid");
        }
        let reply = admission
            .semantic_surface_empty_reply(rite_status::bad_payload)
            .map_err(|_| "semantic_surface_malformed_read_reply_preparation_failed")?;
        let staged = stage_empty_reply(mailbox, reply)?;
        let malformed = runtime
            .trace
            .malformed
            .checked_add(1)
            .ok_or("semantic_surface_malformed_count_overflow")?;
        clear_scratch(runtime)?;
        let settled = admission
            .reject_after_revalidation(&runtime.tasks, rite_status::bad_payload)
            .map_err(|_| "semantic_surface_malformed_read_rejection_failed")?;
        if settled != reply {
            fail("semantic_surface_malformed_read_reply_changed");
        }
        runtime.trace.malformed = malformed;
        publish_empty_reply(staged);
        runtime.phase = next;
        return Ok(gate_outcome::return_to_user(frame));
    }
    if phase == transport_phase::consumer_malformed_read {
        return Err("semantic_surface_malformed_read_accepted");
    }

    if phase == transport_phase::consumer_empty {
        match runtime.endpoint.prepare_delivery(runtime.binding) {
            Err(semantic_surface_error::endpoint_empty) => {}
            Ok(_) => return Err("semantic_surface_empty_read_has_body"),
            Err(_) => return Err("semantic_surface_empty_read_authority_invalid"),
        }
        let reply = admission
            .semantic_surface_empty_reply(rite_status::would_block)
            .map_err(|_| "semantic_surface_empty_read_reply_preparation_failed")?;
        let staged = stage_empty_reply(mailbox, reply)?;
        clear_scratch(runtime)?;
        let settled = admission
            .finish_semantic_surface_would_block(&runtime.tasks)
            .map_err(|_| "semantic_surface_empty_read_completion_failed")?;
        if settled != reply {
            fail("semantic_surface_empty_read_reply_changed");
        }
        publish_empty_reply(staged);
        runtime.trace.stale_reply_cleared = true;
        runtime.phase = next;
        return Ok(gate_outcome::return_to_user(frame));
    }
    if phase != transport_phase::consumer_read {
        return Err("semantic_surface_read_phase_invalid");
    }

    let delivery = runtime
        .endpoint
        .prepare_delivery(runtime.binding)
        .map_err(|_| "semantic_surface_delivery_preparation_failed")?;
    let reply = admission
        .semantic_surface_delivery_reply()
        .map_err(|_| "semantic_surface_delivery_reply_preparation_failed")?;
    let prepared_reply = {
        let body = delivery
            .body(&runtime.endpoint)
            .map_err(|_| "semantic_surface_delivery_body_missing")?;
        // The kernel has already performed structural validation. Only the
        // exact ring-3 consumer validates the complete fixed scene profile.
        prepare_semantic_surface_delivery_reply(reply, body)?
    };
    // SAFETY: Every destination span and body validation completed above. The
    // staged reply deliberately leaves its validity header zero until commit.
    let staged = unsafe { prepared_reply.stage(mailbox) }?;
    let deliveries = runtime
        .trace
        .deliveries
        .checked_add(1)
        .ok_or("semantic_surface_delivery_count_overflow")?;
    clear_scratch(runtime)?;
    let binding = runtime.binding;
    let (tasks, endpoint) = (&runtime.tasks, &mut runtime.endpoint);
    let settled = admission
        .finish_semantic_surface_delivery(tasks, || {
            let receipt = endpoint.commit_prepared_delivery(delivery);
            if receipt.binding() != binding {
                fail("semantic_surface_delivery_receipt_invalid");
            }
        })
        .map_err(|_| "semantic_surface_delivery_completion_failed")?;
    if settled != reply {
        fail("semantic_surface_delivery_reply_changed");
    }
    // SAFETY: The preceding endpoint and Rite commits are the required source
    // consumption linearization point for this exact still-live mailbox.
    unsafe { staged.publish() };
    runtime.trace.deliveries = deliveries;
    runtime.trace.destination_header_published_last = true;
    runtime.phase = next;
    Ok(gate_outcome::return_to_user(frame))
}

fn handle_exit(
    runtime: &mut transport_runtime,
    role: fixture_role,
    admission: rite_admission<'_>,
) -> Result<gate_outcome, &'static str> {
    if admission.operation() != rite_operation::task_exit
        || admission.payload_kind() != rite_payload_kind::exit_status
        || admission.payload_size() != mem::size_of::<rite_exit_payload>() as u32
    {
        return Err("semantic_surface_exit_admission_shape_invalid");
    }
    let mailbox = runtime.task(role)?.space.mailbox_physical.base;
    // SAFETY: The terminal call's admitted body is copied before task state
    // can change; the selected task still owns the mailbox at this point.
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
    if exit.code != expected_exit_code || exit.reserved != 0 {
        return Err("semantic_surface_exit_body_invalid");
    }
    let identity = runtime.task(role)?.identity;
    let exits = runtime
        .trace
        .exits
        .checked_add(1)
        .ok_or("semantic_surface_exit_count_overflow")?;
    clear_scratch(runtime)?;
    let scheduler = &mut runtime.scheduler;
    let reply = admission
        .finish_ok(&mut runtime.tasks, |tasks, committed| {
            if committed != identity {
                fail("semantic_surface_exit_identity_changed");
            }
            scheduler
                .complete_current(tasks, committed)
                .unwrap_or_else(|_| fail("semantic_surface_exit_completion_transition_failed"));
        })
        .map_err(|_| "semantic_surface_exit_completion_failed")?;
    if reply.status != rite_status::ok || reply.payload_kind != rite_payload_kind::none {
        fail("semantic_surface_exit_reply_invalid");
    }
    runtime.trace.exits = exits;
    runtime.phase = match role {
        fixture_role::source => transport_phase::source_retired,
        fixture_role::consumer => transport_phase::idle,
    };
    runtime.active_role = None;
    Ok(gate_outcome::terminal)
}

fn resume_terminal(runtime: &mut transport_runtime) -> ! {
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("semantic_surface_terminal_interrupt_mask_invalid");
    }
    // SAFETY: Both terminal paths remove the current task before returning to
    // the original kernel continuation; no task root remains selected after this.
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3
        || !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || runtime.saved_kernel_stack == 0
        || !(runtime.saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
    {
        fail("semantic_surface_terminal_restore_invalid");
    }
    let stack = runtime.saved_kernel_stack;
    let task_switched = runtime.original_task_switched;
    rite_gate_active.store(false, Ordering::Release);
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: This consumes the one exact continuation captured before the
    // terminal user entry and restores CR0.TS to its original value.
    unsafe {
        x86_64_scheduler_resume_kernel(
            match runtime.phase {
                transport_phase::source_retired => source_completion_status,
                transport_phase::idle => consumer_completion_status,
                _ => fail("semantic_surface_terminal_phase_invalid"),
            },
            stack,
            task_switched,
        )
    }
}

fn clear_scratch(runtime: &mut transport_runtime) -> Result<(), &'static str> {
    if clear_and_verify_bytes(&mut runtime.scratch) {
        Ok(())
    } else {
        Err("semantic_surface_scratch_clear_failed")
    }
}

fn stage_empty_reply(
    mailbox: u64,
    reply: rite_reply,
) -> Result<staged_semantic_surface_reply, &'static str> {
    let prepared = prepare_semantic_surface_empty_reply(reply)?;
    // SAFETY: The current task's fixed mailbox mapping was validated before
    // admission and remains exclusively held by the synchronous gate.
    unsafe { prepared.stage(mailbox) }
}

fn publish_empty_reply(staged: staged_semantic_surface_reply) {
    // SAFETY: An empty outcome has settled its channel and leaves no endpoint
    // state to commit; publishing this final header is therefore safe.
    unsafe { staged.publish() };
}

fn next_phase(phase: transport_phase) -> Result<transport_phase, &'static str> {
    Ok(match phase {
        transport_phase::source_read_denied => transport_phase::source_malformed_submit,
        transport_phase::source_malformed_submit => transport_phase::source_freshness_denied,
        transport_phase::source_freshness_denied => transport_phase::source_submit,
        transport_phase::source_submit => transport_phase::source_backpressured,
        transport_phase::source_backpressured => transport_phase::source_exit,
        transport_phase::consumer_submit_denied => transport_phase::consumer_malformed_read,
        transport_phase::consumer_malformed_read => transport_phase::consumer_read,
        transport_phase::consumer_read => transport_phase::consumer_empty,
        transport_phase::consumer_empty => transport_phase::consumer_exit,
        transport_phase::source_exit
        | transport_phase::consumer_exit
        | transport_phase::source_retired
        | transport_phase::idle => return Err("semantic_surface_phase_advance_invalid"),
    })
}

fn advance_phase(
    runtime: &mut transport_runtime,
    phase: transport_phase,
) -> Result<(), &'static str> {
    runtime.phase = next_phase(phase)?;
    Ok(())
}

fn expected_rite_return(
    runtime: &transport_runtime,
    phase: transport_phase,
) -> Result<u64, &'static str> {
    let (role, symbol) = match phase {
        transport_phase::source_read_denied => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_read_denied_return),
        ),
        transport_phase::source_malformed_submit => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_malformed_submit_return),
        ),
        transport_phase::source_freshness_denied => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_freshness_denied_return),
        ),
        transport_phase::source_submit => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_submit_return),
        ),
        transport_phase::source_backpressured => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_backpressured_return),
        ),
        transport_phase::source_exit => (
            fixture_role::source,
            ptr::addr_of!(x86_64_semantic_surface_source_exit_return),
        ),
        transport_phase::consumer_submit_denied => (
            fixture_role::consumer,
            ptr::addr_of!(x86_64_semantic_surface_consumer_submit_denied_return),
        ),
        transport_phase::consumer_malformed_read => (
            fixture_role::consumer,
            ptr::addr_of!(x86_64_semantic_surface_consumer_malformed_read_return),
        ),
        transport_phase::consumer_read => (
            fixture_role::consumer,
            ptr::addr_of!(x86_64_semantic_surface_consumer_delivery_return),
        ),
        transport_phase::consumer_empty => (
            fixture_role::consumer,
            ptr::addr_of!(x86_64_semantic_surface_consumer_empty_return),
        ),
        transport_phase::consumer_exit => (
            fixture_role::consumer,
            ptr::addr_of!(x86_64_semantic_surface_consumer_exit_return),
        ),
        transport_phase::source_retired | transport_phase::idle => {
            return Err("semantic_surface_return_phase_invalid");
        }
    };
    runtime
        .task(role)?
        .space
        .payload
        .linked_user_address(symbol)
        .ok_or("semantic_surface_return_symbol_invalid")
}

fn validate_rite_call(phase: transport_phase, call: rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.reserved != 0
        || !payload_span_fits(user_mailbox_address, call.payload_size)
        || call.trace_id != phase.trace().ok_or("semantic_surface_call_trace_missing")?
    {
        return Err("semantic_surface_rite_call_header_invalid");
    }
    let (operation, kind, rights, size) = match phase {
        transport_phase::source_read_denied => (
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            semantic_surface_request_size as u32,
        ),
        transport_phase::source_malformed_submit
        | transport_phase::source_freshness_denied
        | transport_phase::source_submit
        | transport_phase::source_backpressured => (
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            semantic_surface_snapshot_size as u32,
        ),
        transport_phase::consumer_submit_denied => (
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            semantic_surface_snapshot_size as u32,
        ),
        transport_phase::consumer_malformed_read
        | transport_phase::consumer_read
        | transport_phase::consumer_empty => (
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            semantic_surface_request_size as u32,
        ),
        transport_phase::source_exit | transport_phase::consumer_exit => (
            rite_operation::task_exit,
            rite_payload_kind::exit_status,
            rite_rights::task_exit,
            mem::size_of::<rite_exit_payload>() as u32,
        ),
        transport_phase::source_retired | transport_phase::idle => {
            return Err("semantic_surface_call_phase_invalid");
        }
    };
    if call.operation != operation
        || call.payload_kind != kind
        || call.rights_present != rights
        || call.payload_size != size
        || call.payload_limit != size
    {
        return Err("semantic_surface_rite_call_shape_invalid");
    }
    Ok(())
}

fn retire_role(
    allocator: &mut frame_allocator<'_>,
    runtime: &mut transport_runtime,
    role: fixture_role,
) -> Result<(), &'static str> {
    let identity = runtime.task(role)?.identity;
    let circle = runtime.task(role)?.circle;
    let mut channel = runtime.take_channel(role)?;
    channel
        .close_idle()
        .map_err(|_| "semantic_surface_channel_retirement_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(&mut runtime.circles, &mut runtime.tasks, identity)
        .map_err(|_| "semantic_surface_context_withdrawal_failed")?;
    let mut task = match role {
        fixture_role::source => runtime
            .source
            .take()
            .ok_or("semantic_surface_source_missing")?,
        fixture_role::consumer => runtime
            .consumer
            .take()
            .ok_or("semantic_surface_consumer_missing")?,
    };
    release_task_space(allocator, &mut runtime.circles, circle, &mut task.space)?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, identity)
        .map_err(|_| "semantic_surface_task_destroy_failed")?;
    runtime
        .circles
        .destroy(circle)
        .map_err(|_| "semantic_surface_circle_destroy_failed")?;
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
        .map_err(|_| "semantic_surface_image_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "semantic_surface_mailbox_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "semantic_surface_stack_withdrawal_failed")?;
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "semantic_surface_root_destroy_failed")?;
    unsafe { allocator.release_with(&mut image, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_surface_image_release_failed")?;
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_surface_mailbox_release_failed")?;
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "semantic_surface_stack_release_failed")?;
    allocator
        .unbind(&space.privilege_stack)
        .map_err(|_| "semantic_surface_privilege_stack_unbind_failed")?;
    unsafe {
        allocator.release_with(&mut space.privilege_stack, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "semantic_surface_privilege_stack_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "semantic_surface_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "semantic_surface_root_detachment_failed")?;
    retire_task_space_metadata(space);
    Ok(())
}

fn retire_task_space_metadata(space: &mut task_space) {
    debug_assert!(space.root.root_address().is_err());
    debug_assert!(space.privilege_stack.range().is_none());
    // SAFETY: All address-bearing kernel owners have been withdrawn and the
    // remaining values are dead bookkeeping before their outer owner drops.
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
                end: ptr::null(),
                size: ptr::null(),
                entry: ptr::null(),
            },
        );
    }
}

fn emit_ready(source_pages: usize, consumer_pages: usize) {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.ready","level":"info","arch":"x86_64","source_image_pages":{},"consumer_image_pages":{},"mailbox_pages":1,"stack_pages":4,"grants_split":true,"content_redacted":true}}"#,
        source_pages, consumer_pages
    );
}

fn emit_source_released() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.source.released","level":"info","queued_body_retained":true,"source_retired_before_delivery":true,"content_redacted":true}}"#
    );
}

fn emit_consumer_released() {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.consumer.released","level":"info","delivery_consumed_once":true,"content_redacted":true}}"#
    );
}

fn emit_released(frames: frame_accounting) {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.released","level":"info","free_frames":{},"resources_restored":true,"private_storage_cleared":true,"content_redacted":true}}"#,
        frames.free
    );
}

fn emit_passed(evidence: transport_evidence) {
    let trace = evidence.trace;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.passed","level":"info","lifecycle_complete":true,"rite_calls":{},"body_copies":{},"body_bytes":{},"submissions":{},"deliveries":{},"backpressure":{},"denials":{},"malformed":{},"exits":{},"source_retired_before_delivery":{},"header_published_last":{},"stale_reply_cleared":{},"kernel_continuations":{},"source_image_pages":{},"consumer_image_pages":{},"free_frames":{},"content_redacted":true}}"#,
        trace.rite_calls,
        trace.body_copies,
        trace.body_bytes,
        trace.submissions,
        trace.deliveries,
        trace.backpressure,
        trace.denials,
        trace.malformed,
        trace.exits,
        trace.source_retired_before_delivery,
        trace.destination_header_published_last,
        trace.stale_reply_cleared,
        trace.kernel_continuations,
        evidence.source_image_pages,
        evidence.consumer_image_pages,
        evidence.initial_frames.free,
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.semantic_surface_transport.failed","level":"fatal","reason":"{reason}","content_redacted":true}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
