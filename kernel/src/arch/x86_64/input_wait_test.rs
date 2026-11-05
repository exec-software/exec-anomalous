//! Removable x86-64 semantic-session input and exact-wake proof.
//!
//! The fixture owns one private endpoint, one exact consumer, and one peer. It
//! proves only the bounded mechanism selected by ADR 0022: fixed native reply
//! bodies, pointer-free pending Rite state, endpoint-authorized FIFO wake,
//! reserved selection, route-close interruption, and complete teardown. It is
//! not an input router, focus authority, or source authenticator.

use anomalous_abi::{
    id_128, rite_call, rite_operation, rite_payload_kind, rite_reply, rite_rights,
    rite_session_input_action, rite_session_input_delivery_v1, rite_session_input_origin,
    rite_session_input_request_v1, rite_status,
};
use core::cell::Cell;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::{circle_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator, physical_range};
use crate::rite::{
    rite_admission, rite_channel, rite_decision, rite_mailbox_mapping_identity,
    rite_session_input_completion_outcome, rite_session_input_pending_view,
};
use crate::scheduler::{
    compose_endpoint_wake_authorization, scheduler_context, scheduler_endpoint_wake_authorization,
    scheduler_selection_authorization, scheduler_table, scheduler_wake_authorization,
};
use crate::session_input::{
    session_input_action, session_input_action_kind, session_input_binding,
    session_input_close_preparation, session_input_close_reason, session_input_consumer_identity,
    session_input_delivery, session_input_endpoint, session_input_endpoint_identity,
    session_input_origin, session_input_receive_preparation, session_input_route_epoch,
    session_input_session_identity, session_input_wait_acknowledgement, session_input_wait_commit,
    session_input_wake_correlation, session_input_wake_outcome,
};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_session_input_request,
    prepare_session_input_delivery_reply, prepare_session_input_empty_reply, read_call, reply_size,
    rite_scratch, scratch_is_zero, session_input_delivery_size, session_input_reply_span_fits,
};
use super::scheduler_test::{
    all_general_registers_zero, attach_task_space, cr0_task_switched, create_task,
    frame_address_matches_stack, initial_user_flags, invalid_opcode_vector, mailbox_is_zero,
    prepare_task_space, privilege_stack_pages, release_task_space, relocated_user_address,
    resume_flag, scheduler_frame, scheduler_gate_vector, user_mailbox_address,
    user_return_fields_are_valid, user_stack_top,
};
use super::{descriptors, exceptions, interrupts, paging};

const input_task_count: usize = 2;
const page_size: usize = 4096;
const completion_status: u64 = 1;
const rite_gate_vector: u64 = 0xf3;
const operation_publish: u64 = 1;
const operation_preempt: u64 = 2;
const operation_close: u64 = 3;
const operation_complete: u64 = 4;
const expected_delivery_size: u32 = 280;
const expected_request_size: u32 = 16;
const private_text: [u8; 5] = [0x41, 0x0a, 0xe2, 0x98, 0x83];

const trace_1: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
const trace_2: [u8; 16] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
];
const trace_3: [u8; 16] = [
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
];

const _: () = {
    assert!(call_size == 40);
    assert!(reply_size == 40);
    assert!(expected_request_size as usize == mem::size_of::<rite_session_input_request_v1>());
    assert!(expected_delivery_size as usize == session_input_delivery_size);
    assert!(privilege_stack_pages == 4);
    assert!(descriptors::privilege_stack_size == 16 * 1024);
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum fixture_role {
    consumer,
    peer,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum input_phase {
    immediate,
    first_wait,
    published,
    second_wait,
    closing,
    completing,
    idle,
}

impl input_phase {
    const fn trace(self) -> Option<id_128> {
        let bytes = match self {
            Self::immediate => trace_1,
            Self::first_wait => trace_2,
            Self::second_wait => trace_3,
            Self::published | Self::closing | Self::completing | Self::idle => return None,
        };
        Some(id_128::from_bytes(bytes))
    }

    const fn sequence(self) -> Option<u64> {
        match self {
            Self::immediate => Some(1),
            Self::first_wait => Some(2),
            Self::second_wait => Some(3),
            Self::published | Self::closing | Self::completing | Self::idle => None,
        }
    }
}

struct input_task {
    circle: circle_id,
    identity: task_identity,
    space: super::scheduler_test::task_space,
}

#[derive(Clone, Copy)]
struct input_trace {
    dispatches: [u64; 5],
    preemptions: [u64; 1],
    rite_calls: usize,
    body_copies: usize,
    body_bytes: usize,
    deliveries: usize,
    blocks: usize,
    publications: usize,
    wakes: usize,
    closures: usize,
    faults: usize,
    completions: usize,
    immediate_copy_before_consume: bool,
    pending_is_pointer_free: bool,
    publisher_remained_current: bool,
    fifo_tail_wake: bool,
    reserved_preemption: bool,
    selected_copy_before_consume: bool,
    route_queue_cleared: bool,
    reserved_fault: bool,
    payload_free_interruption: bool,
    kernel_continuation_restored: bool,
}

impl input_trace {
    const fn empty() -> Self {
        Self {
            dispatches: [u64::MAX; 5],
            preemptions: [u64::MAX; 1],
            rite_calls: 0,
            body_copies: 0,
            body_bytes: 0,
            deliveries: 0,
            blocks: 0,
            publications: 0,
            wakes: 0,
            closures: 0,
            faults: 0,
            completions: 0,
            immediate_copy_before_consume: false,
            pending_is_pointer_free: false,
            publisher_remained_current: false,
            fifo_tail_wake: false,
            reserved_preemption: false,
            selected_copy_before_consume: false,
            route_queue_cleared: false,
            reserved_fault: false,
            payload_free_interruption: false,
            kernel_continuation_restored: false,
        }
    }
}

struct input_runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    consumer: input_task,
    peer: input_task,
    active_role: Option<fixture_role>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    endpoint: Option<session_input_endpoint>,
    channel: Option<rite_channel>,
    binding: session_input_binding,
    mapping: rite_mailbox_mapping_identity,
    scheduler_wake: Option<scheduler_wake_authorization>,
    endpoint_wake: Option<scheduler_endpoint_wake_authorization>,
    selection: Option<scheduler_selection_authorization>,
    phase: input_phase,
    scratch: rite_scratch,
    expected_delivery_sequence: u64,
    trace: input_trace,
}

#[derive(Clone, Copy)]
struct input_evidence {
    trace: input_trace,
}

static active_runtime: AtomicPtr<input_runtime> = AtomicPtr::new(ptr::null_mut());
static switch_active: AtomicBool = AtomicBool::new(false);
static gate_active: AtomicBool = AtomicBool::new(false);
static rite_gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_input_wait_consumer_start: u8;
    static x86_64_input_wait_consumer_immediate_return: u8;
    static x86_64_input_wait_consumer_delivery_return: u8;
    static x86_64_input_wait_consumer_interrupted_return: u8;
    static x86_64_input_wait_consumer_completion_return: u8;
    static x86_64_input_wait_consumer_end: u8;
    static x86_64_input_wait_peer_start: u8;
    static x86_64_input_wait_peer_publish_return: u8;
    static x86_64_input_wait_peer_preempt_return: u8;
    static x86_64_input_wait_peer_close_return: u8;
    static x86_64_input_wait_peer_fault_instruction: u8;
    static x86_64_input_wait_peer_end: u8;
}

pub(super) fn run() -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_state_invalid");
    }

    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("interrupt_mask_invalid");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Threshold keeps NMI masked, IF is clear, and the feature uses the
    // boot processor exclusively.
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        run_with_allocator(allocator, kernel_cr3, bootstrap_privilege_stack_top)
    });

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This was the exact live root before private task roots.
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
) -> Result<input_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("setup_state_invalid");
    }
    let initial = allocator.accounting();
    if initial.free < 22
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let consumer_circle = circles
        .create()
        .map_err(|_| "consumer_circle_create_failed")?;
    let peer_circle = circles.create().map_err(|_| "peer_circle_create_failed")?;
    let consumer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_input_wait_consumer_start),
        ptr::addr_of!(x86_64_input_wait_consumer_end),
    )?;
    let peer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_input_wait_peer_start),
        ptr::addr_of!(x86_64_input_wait_peer_end),
    )?;
    attach_task_space(&mut circles, consumer_circle)?;
    attach_task_space(&mut circles, peer_circle)?;
    let consumer_identity = create_task(
        &mut circles,
        &mut tasks,
        consumer_circle,
        1,
        consumer_space.payload_length,
    )?;
    let peer_identity = create_task(
        &mut circles,
        &mut tasks,
        peer_circle,
        2,
        peer_space.payload_length,
    )?;
    let binding = session_input_binding::new(
        session_input_endpoint_identity::try_new(0, 1).map_err(|_| "endpoint_identity_invalid")?,
        session_input_session_identity::try_new(1, 1).map_err(|_| "session_identity_invalid")?,
        session_input_consumer_identity::try_new(1, 1).map_err(|_| "consumer_identity_invalid")?,
        session_input_route_epoch::try_new(1).map_err(|_| "route_identity_invalid")?,
        consumer_identity,
    );
    let mapping = rite_mailbox_mapping_identity::try_new(consumer_identity, 1)
        .map_err(|_| "mapping_identity_invalid")?;
    let mut endpoint = session_input_endpoint::new(binding);
    let human_action = session_input_action::try_insert_text(&private_text)
        .map_err(|_| "prequeued_action_invalid")?;
    let prepared = endpoint
        .prepare_publication(binding, session_input_origin::human_text, human_action)
        .map_err(|_| "prequeued_publication_prepare_failed")?;
    if prepared.wake_correlation().is_some() {
        return Err("prequeued_publication_wake_invalid");
    }
    let prequeued = endpoint
        .commit_publication(prepared)
        .map_err(|_| "prequeued_publication_commit_failed")?;
    if prequeued.sequence().get() != 1 || prequeued.wake().is_some() {
        return Err("prequeued_publication_result_invalid");
    }
    drop(prequeued.into_wake());

    let channel = rite_channel::try_new(consumer_identity, rite_rights::session_input_read)
        .map_err(|_| "rite_channel_create_failed")?;
    let mut scheduler =
        scheduler_table::new(input_task_count).map_err(|_| "scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, consumer_identity)
        .map_err(|_| "consumer_admission_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, peer_identity)
        .map_err(|_| "peer_admission_failed")?;

    let ready = allocator.accounting();
    validate_ready_ownership(
        initial,
        ready,
        &circles,
        &tasks,
        &scheduler,
        &consumer_space,
        &peer_space,
    )?;

    let mut runtime = input_runtime {
        circles,
        tasks,
        scheduler,
        consumer: input_task {
            circle: consumer_circle,
            identity: consumer_identity,
            space: consumer_space,
        },
        peer: input_task {
            circle: peer_circle,
            identity: peer_identity,
            space: peer_space,
        },
        active_role: None,
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        endpoint: Some(endpoint),
        channel: Some(channel),
        binding,
        mapping,
        scheduler_wake: None,
        endpoint_wake: None,
        selection: None,
        phase: input_phase::immediate,
        scratch: [0; page_size],
        expected_delivery_sequence: 1,
        trace: input_trace::empty(),
    };
    runtime.trace.publications = 1;

    let first = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "initial_dispatch_failed")?;
    if first.task() != runtime.consumer.identity
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        return Err("initial_dispatch_invalid");
    }
    runtime.trace.dispatches[0] = first.sequence();
    let first_frame = runtime.select_role(fixture_role::consumer, false)?;
    let runtime_pointer = ptr::from_mut(&mut runtime);
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("runtime_publication_failed");
    }

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The exact root, TSS stack, frame, and stable runtime are live.
    let status = unsafe {
        super::scheduler_test::x86_64_scheduler_enter(
            first_frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };

    active_runtime.store(ptr::null_mut(), Ordering::Release);
    runtime.trace.kernel_continuation_restored = true;
    validate_restored_continuation(&runtime, status, original_task_switched, kernel_cr3)?;

    let mut channel = runtime.channel.take().ok_or("restored_channel_missing")?;
    channel
        .close_idle()
        .map_err(|_| "restored_channel_retire_failed")?;
    drop(channel);
    let endpoint = runtime.endpoint.take().ok_or("restored_endpoint_missing")?;
    drop(endpoint);
    if !clear_and_verify_bytes(&mut runtime.scratch) {
        return Err("restored_scratch_clear_failed");
    }
    validate_completion(&runtime, status, original_task_switched, kernel_cr3)?;
    runtime.saved_kernel_stack = 0;
    runtime.original_task_switched = 0;

    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            runtime.consumer.identity,
        )
        .map_err(|_| "consumer_context_withdrawal_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            runtime.peer.identity,
        )
        .map_err(|_| "peer_context_withdrawal_failed")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        runtime.consumer.circle,
        &mut runtime.consumer.space,
    )?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        runtime.peer.circle,
        &mut runtime.peer.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, runtime.consumer.identity)
        .map_err(|_| "consumer_destroy_failed")?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, runtime.peer.identity)
        .map_err(|_| "peer_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.consumer.circle)
        .map_err(|_| "consumer_circle_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.peer.circle)
        .map_err(|_| "peer_circle_destroy_failed")?;

    let final_frames = allocator.accounting();
    let final_circles = runtime.circles.accounting();
    let final_scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "final_scheduler_invalid")?;
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
    {
        return Err("final_accounting_invalid");
    }

    let evidence = input_evidence {
        trace: runtime.trace,
    };
    retire_runtime_metadata(&mut runtime);
    Ok(evidence)
}

fn validate_ready_ownership(
    initial: frame_accounting,
    ready: frame_accounting,
    circles: &circle_table,
    tasks: &task_table,
    scheduler: &scheduler_table,
    consumer: &super::scheduler_test::task_space,
    peer: &super::scheduler_test::task_space,
) -> Result<(), &'static str> {
    if initial.free.checked_sub(22) != Some(ready.free)
        || ready.data_owned != 14
        || ready.table_owned != 8
        || ready.active_leases != 10
        || ready.bound_leases != 10
    {
        return Err("ready_frame_accounting_invalid");
    }
    let circle_accounting = circles.accounting();
    let scheduler_snapshot = scheduler
        .snapshot(tasks)
        .map_err(|_| "ready_scheduler_invalid")?;
    if circle_accounting.circles != 2
        || circle_accounting.tasks != 2
        || circle_accounting.task_contexts != 2
        || circle_accounting.endpoints != 0
        || circle_accounting.sigils != 0
        || circle_accounting.supervisions != 0
        || circle_accounting.address_spaces != 2
        || circle_accounting.mappings != 6
        || tasks.active_count() != 2
        || tasks.attached_context_count() != 2
        || scheduler_snapshot.admitted() != 2
        || scheduler_snapshot.runnable() != 2
        || scheduler_snapshot.current().is_some()
        || scheduler_snapshot.next_dispatch_sequence() != Some(0)
        || scheduler_snapshot.next_preemption_sequence() != Some(0)
        || scheduler_snapshot.reservation().is_some()
        || !mailbox_is_zero(consumer)
        || !mailbox_is_zero(peer)
    {
        return Err("ready_object_accounting_invalid");
    }
    validate_mailbox_mapping(consumer)?;
    validate_mailbox_mapping(peer)?;

    let ranges = [
        physical_range::new(consumer.physical_pages[0], page_size as u64),
        physical_range::new(consumer.physical_pages[1], page_size as u64),
        physical_range::new(consumer.physical_pages[2], page_size as u64),
        physical_range::new(
            consumer
                .root
                .root_address()
                .map_err(|_| "consumer_root_missing")?,
            4 * page_size as u64,
        ),
        consumer.privilege_stack_range,
        physical_range::new(peer.physical_pages[0], page_size as u64),
        physical_range::new(peer.physical_pages[1], page_size as u64),
        physical_range::new(peer.physical_pages[2], page_size as u64),
        physical_range::new(
            peer.root.root_address().map_err(|_| "peer_root_missing")?,
            4 * page_size as u64,
        ),
        peer.privilege_stack_range,
    ];
    for (index, range) in ranges.iter().enumerate() {
        if range.length == 0
            || range
                .end()
                .is_none_or(|end| end > paging::identity_map_limit)
            || ranges[index + 1..]
                .iter()
                .any(|other| super::scheduler_test::ranges_overlap(*range, *other))
        {
            return Err("owned_extents_overlap");
        }
    }
    Ok(())
}

fn validate_mailbox_mapping(
    space: &super::scheduler_test::task_space,
) -> Result<u64, &'static str> {
    let evidence = space
        .root
        .validate_mapping(user_mailbox_address)
        .map_err(|_| "mailbox_mapping_missing")?;
    if evidence.virtual_address != user_mailbox_address
        || evidence.physical_address != space.physical_pages[1]
        || !evidence.permissions.present
        || !evidence.permissions.user
        || !evidence.permissions.writable
        || evidence.permissions.executable
        || space.root.validate_all_mappings().is_err()
        || !session_input_reply_span_fits(user_mailbox_address)
        || evidence.physical_address & (page_size as u64 - 1) != 0
    {
        return Err("mailbox_mapping_invalid");
    }
    Ok(evidence.physical_address)
}

fn validate_restored_continuation(
    runtime: &input_runtime,
    status: u64,
    original_task_switched: u64,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "restored_scheduler_invalid")?;
    let endpoint = runtime
        .endpoint
        .as_ref()
        .ok_or("restored_endpoint_missing")?;
    let channel = runtime.channel.as_ref().ok_or("restored_channel_missing")?;
    if status != completion_status
        || runtime.original_task_switched != original_task_switched
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || paging::read_cr3() != kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || interrupts::interrupts_enabled()
        || !interrupts::scheduler_interrupts_are_masked()
        || !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || runtime.active_role.is_some()
        || runtime.phase != input_phase::idle
        || runtime.expected_delivery_sequence != 0
        || !scratch_is_zero(&runtime.scratch)
        || endpoint.binding() != runtime.binding
        || endpoint.prepare_receive(runtime.binding).is_ok()
        || channel.session_input_pending().is_ok()
        || scheduler.current().is_some()
        || scheduler.runnable() != 0
        || scheduler.reservation().is_some()
    {
        return Err("restored_continuation_invalid");
    }
    Ok(())
}

fn validate_completion(
    runtime: &input_runtime,
    status: u64,
    original_task_switched: u64,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    let trace = runtime.trace;
    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "completion_scheduler_invalid")?;
    if status != completion_status
        || runtime.original_task_switched != original_task_switched
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || paging::read_cr3() != kernel_cr3
        || interrupts::interrupts_enabled()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || rite_gate_active.load(Ordering::Acquire)
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
        || runtime.endpoint.is_some()
        || runtime.channel.is_some()
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.phase != input_phase::idle
        || !scratch_is_zero(&runtime.scratch)
        || runtime.expected_delivery_sequence != 0
        || scheduler.current().is_some()
        || scheduler.runnable() != 0
        || scheduler.reservation().is_some()
        || runtime
            .tasks
            .snapshot(runtime.consumer.identity)
            .map_err(|_| "completion_consumer_missing")?
            .state()
            != task_state::exited
        || runtime
            .tasks
            .snapshot(runtime.peer.identity)
            .map_err(|_| "completion_peer_missing")?
            .state()
            != task_state::faulted
        || trace.dispatches != [0, 1, 2, 3, 4]
        || trace.preemptions != [0]
        || trace.rite_calls != 3
        || trace.body_copies != 3
        || trace.body_bytes != 3 * expected_request_size as usize
        || trace.deliveries != 2
        || trace.blocks != 2
        || trace.publications != 2
        || trace.wakes != 2
        || trace.closures != 1
        || trace.faults != 1
        || trace.completions != 1
        || !trace.immediate_copy_before_consume
        || !trace.pending_is_pointer_free
        || !trace.publisher_remained_current
        || !trace.fifo_tail_wake
        || !trace.reserved_preemption
        || !trace.selected_copy_before_consume
        || !trace.route_queue_cleared
        || !trace.reserved_fault
        || !trace.payload_free_interruption
        || !trace.kernel_continuation_restored
        || !interrupts::scheduler_interrupts_are_masked()
    {
        return Err("completion_state_invalid");
    }
    Ok(())
}

fn retire_runtime_metadata(runtime: &mut input_runtime) {
    debug_assert!(runtime.consumer.space.frame.is_null());
    debug_assert!(runtime.peer.space.frame.is_null());
    debug_assert!(runtime.endpoint.is_none());
    debug_assert!(runtime.channel.is_none());
    // SAFETY: Private owners are retired and every address-bearing scalar is
    // now dead bookkeeping on the restored kernel stack.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(runtime.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.bootstrap_privilege_stack_top), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.original_task_switched), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.phase), input_phase::idle);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.scratch), [0; page_size]);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.expected_delivery_sequence), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.trace), input_trace::empty());
    }
}

impl input_runtime {
    fn task(&self, role: fixture_role) -> &input_task {
        match role {
            fixture_role::consumer => &self.consumer,
            fixture_role::peer => &self.peer,
        }
    }

    fn role_for_identity(&self, identity: task_identity) -> Option<fixture_role> {
        if identity == self.consumer.identity {
            Some(fixture_role::consumer)
        } else if identity == self.peer.identity {
            Some(fixture_role::peer)
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
        let task = self.task(role);
        let root = task
            .space
            .root
            .root_address()
            .map_err(|_| "selected_root_missing")?;
        let stack_top = task
            .space
            .privilege_stack_range
            .end()
            .ok_or("selected_stack_extent_invalid")?;
        let frame = task.space.frame;
        // SAFETY: The fixed root and bound stack remain exclusively live.
        unsafe { task.space.root.activate() }.map_err(|_| "selected_root_activation_failed")?;
        // SAFETY: The task owns this exact bound privilege-stack lease.
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("selected_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || !self.frame_is_valid_for_role(frame, role, require_task_switched)
        {
            return Err("selected_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: fixture_role,
        require_task_switched: bool,
    ) -> bool {
        let task = self.task(role);
        if frame != task.space.frame
            || !frame_address_matches_stack(frame, task.space.privilege_stack_range)
            || task.space.privilege_stack.frame_count() != privilege_stack_pages
            || task.space.privilege_stack.range() != Some(task.space.privilege_stack_range)
            || task.space.root.validate_all_mappings().is_err()
            || validate_mailbox_mapping(&task.space).is_err()
        {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(task.identity) else {
            return false;
        };
        if snapshot.state() != task_state::running || !snapshot.context_attached() {
            return false;
        }
        let root = task.space.root.root_address().ok();
        let stack_top = task.space.privilege_stack_range.end();
        if root != Some(paging::entry_address(paging::read_cr3()))
            || stack_top != Some(unsafe { descriptors::selected_privilege_stack_top() })
            || (require_task_switched && paging::read_cr0() & cr0_task_switched == 0)
        {
            return false;
        }
        // SAFETY: The complete frame lies in the validated live stack extent.
        let frame = unsafe { ptr::read(frame) };
        user_return_fields_are_valid(frame, snapshot.bounds())
    }
}

pub(super) fn input_wait_is_active() -> bool {
    !active_runtime.load(Ordering::Acquire).is_null()
}

pub(super) fn user_interrupt_frame_is_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return false;
    }
    // SAFETY: Publication is stable until the terminal nonlocal return and the
    // caller holds IF clear on the only processor.
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

pub(super) unsafe fn gate_dispatch(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("gate_without_runtime");
    }
    // SAFETY: The published stable runtime is unique while IF and NMI are
    // excluded on the boot processor.
    let runtime = unsafe { &mut *runtime_pointer };
    let role = runtime
        .current_role()
        .unwrap_or_else(|| fail("gate_without_current_task"));
    if !runtime.frame_is_valid_for_role(frame, role, true) {
        fail("gate_frame_invalid");
    }
    // SAFETY: Validation covers the complete live 22-qword frame.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != scheduler_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || !control_registers_are_exact(incoming)
    {
        fail("gate_frame_shape_invalid");
    }

    let result = match (role, runtime.phase, incoming.accumulator) {
        (fixture_role::peer, input_phase::published, operation_publish) => {
            handle_publish(runtime, frame, incoming)
        }
        (fixture_role::peer, input_phase::published, operation_preempt) => {
            handle_reserved_preemption(runtime, incoming)
        }
        (fixture_role::peer, input_phase::closing, operation_close) => {
            handle_close(runtime, frame, incoming)
        }
        (fixture_role::consumer, input_phase::completing, operation_complete) => {
            finish_consumer(runtime, incoming)
        }
        _ => Err("gate_operation_invalid"),
    };
    match result {
        Ok(next) => {
            gate_active.store(false, Ordering::Release);
            next
        }
        Err(reason) => fail(reason),
    }
}

fn handle_publish(
    runtime: &mut input_runtime,
    frame: *mut scheduler_frame,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if incoming.instruction_pointer
        != relocated_user_address(
            ptr::addr_of!(x86_64_input_wait_peer_start),
            ptr::addr_of!(x86_64_input_wait_peer_publish_return),
            runtime.peer.space.payload_length,
        )
        .ok_or("publish_return_invalid")?
        || runtime.scheduler_wake.is_none()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.trace.blocks != 1
        || runtime.trace.wakes != 0
        || runtime.expected_delivery_sequence != 1
    {
        return Err("publish_state_invalid");
    }
    let pending = exact_pending(runtime)?;
    let endpoint = runtime.endpoint.as_mut().ok_or("endpoint_missing")?;
    let action = session_input_action::try_insert_text(&private_text)
        .map_err(|_| "machine_action_invalid")?;
    let prepared = endpoint
        .prepare_publication(runtime.binding, session_input_origin::machine_text, action)
        .map_err(|_| "publication_prepare_failed")?;
    let correlation = prepared
        .wake_correlation()
        .ok_or("publication_correlation_missing")?;
    validate_correlation(
        pending,
        correlation,
        session_input_wake_outcome::input_ready,
    )?;
    let preflight = runtime
        .scheduler
        .preflight_endpoint_wake(
            &runtime.tasks,
            &runtime.scheduler_wake,
            correlation,
            &runtime.endpoint_wake,
            &runtime.selection,
        )
        .map_err(|_| "publication_scheduler_preflight_failed")?;
    let receipt = endpoint
        .commit_publication(prepared)
        .map_err(|_| "publication_commit_failed")?;
    if receipt.sequence().get() != 2
        || receipt
            .wake()
            .is_none_or(|wake| wake.correlation() != correlation)
    {
        return Err("publication_receipt_invalid");
    }
    let wake = receipt.into_wake().ok_or("publication_wake_missing")?;
    compose_endpoint_wake_authorization(
        &mut runtime.scheduler_wake,
        wake,
        preflight,
        &mut runtime.endpoint_wake,
    )
    .map_err(|_| "publication_wake_composition_failed")?;
    let prepared_wake = runtime
        .scheduler
        .prepare_endpoint_wake(
            &mut runtime.tasks,
            &mut runtime.endpoint_wake,
            &mut runtime.selection,
        )
        .map_err(|_| "publication_wake_prepare_failed")?;
    if prepared_wake.task() != runtime.consumer.identity {
        return Err("publication_wake_task_invalid");
    }
    let wake = prepared_wake.commit();
    let snapshot = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "publication_scheduler_invalid")?;
    let reservation = snapshot
        .reservation()
        .ok_or("publication_reservation_missing")?;
    if wake.task() != runtime.consumer.identity
        || snapshot.current() != Some(runtime.peer.identity)
        || snapshot.runnable() != 1
        || !reservation.runnable()
        || reservation.task() != runtime.consumer.identity
        || reservation.current() != runtime.peer.identity
        || reservation.sequence() != 2
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_none()
        || runtime
            .tasks
            .snapshot(runtime.consumer.identity)
            .map_err(|_| "published_consumer_missing")?
            .state()
            != task_state::suspended
        || exact_pending(runtime)? != pending
    {
        return Err("publication_wake_state_invalid");
    }

    runtime.expected_delivery_sequence = 2;
    runtime.trace.publications += 1;
    runtime.trace.wakes += 1;
    runtime.trace.publisher_remained_current = true;
    runtime.trace.fifo_tail_wake = true;
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn handle_reserved_preemption(
    runtime: &mut input_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if incoming.instruction_pointer
        != relocated_user_address(
            ptr::addr_of!(x86_64_input_wait_peer_start),
            ptr::addr_of!(x86_64_input_wait_peer_preempt_return),
            runtime.peer.space.payload_length,
        )
        .ok_or("preempt_return_invalid")?
        || runtime.selection.is_none()
        || runtime.expected_delivery_sequence != 2
        || runtime.trace.publications != 2
        || runtime.trace.deliveries != 1
    {
        return Err("preempt_state_invalid");
    }

    complete_selected_delivery(runtime)?;
    runtime.phase = input_phase::second_wait;
    runtime.trace.reserved_preemption = true;
    runtime.trace.selected_copy_before_consume = true;
    let next = runtime.select_role(fixture_role::consumer, true)?;
    validate_saved_consumer_return(
        runtime,
        next,
        ptr::addr_of!(x86_64_input_wait_consumer_delivery_return),
    )?;
    Ok(next)
}

fn handle_close(
    runtime: &mut input_runtime,
    frame: *mut scheduler_frame,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if incoming.instruction_pointer
        != relocated_user_address(
            ptr::addr_of!(x86_64_input_wait_peer_start),
            ptr::addr_of!(x86_64_input_wait_peer_close_return),
            runtime.peer.space.payload_length,
        )
        .ok_or("close_return_invalid")?
        || runtime.scheduler_wake.is_none()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.trace.blocks != 2
        || runtime.trace.closures != 0
        || runtime.trace.deliveries != 2
        || runtime.trace.wakes != 1
        || runtime.expected_delivery_sequence != 2
    {
        return Err("close_state_invalid");
    }
    let pending = exact_pending(runtime)?;
    let mailbox_alias = consumer_mailbox_alias(runtime)?;
    if mailbox_alias != runtime.consumer.space.physical_pages[1] {
        return Err("close_mailbox_alias_invalid");
    }
    let prepared = match runtime
        .endpoint
        .as_ref()
        .ok_or("endpoint_missing")?
        .prepare_close(runtime.binding, session_input_close_reason::focus_lost)
        .map_err(|_| "close_prepare_failed")?
    {
        session_input_close_preparation::prepared(prepared) => prepared,
        session_input_close_preparation::already_closed => {
            return Err("close_was_already_committed");
        }
    };
    let correlation = prepared
        .wake_correlation()
        .ok_or("close_correlation_missing")?;
    validate_correlation(
        pending,
        correlation,
        session_input_wake_outcome::terminal(session_input_close_reason::focus_lost),
    )?;
    let preflight = runtime
        .scheduler
        .preflight_endpoint_wake(
            &runtime.tasks,
            &runtime.scheduler_wake,
            correlation,
            &runtime.endpoint_wake,
            &runtime.selection,
        )
        .map_err(|_| "close_scheduler_preflight_failed")?;

    {
        let completion = runtime
            .channel
            .as_mut()
            .ok_or("close_channel_missing")?
            .prepare_session_input_completion(
                pending,
                rite_session_input_completion_outcome::interrupted,
            )
            .map_err(|_| "close_rite_preflight_failed")?;
        let reply = completion.reply();
        if completion.outcome() != rite_session_input_completion_outcome::interrupted
            || !reply_is_exact(
                reply,
                rite_status::interrupted,
                rite_payload_kind::none,
                0,
                3,
                id_128::from_bytes(trace_3),
            )
        {
            return Err("close_reply_preflight_invalid");
        }
        let transport = prepare_session_input_empty_reply(reply)?;
        drop(transport);
        drop(completion);
    }
    if exact_pending(runtime)? != pending {
        return Err("close_pending_changed_during_preflight");
    }

    let receipt = runtime
        .endpoint
        .as_mut()
        .ok_or("endpoint_missing")?
        .commit_close(prepared)
        .map_err(|_| "close_commit_failed")?;
    if !receipt.was_first_close()
        || receipt.discarded_count() != 0
        || receipt.terminal_correlation() != Some(correlation)
        || receipt
            .wake()
            .is_none_or(|wake| wake.correlation() != correlation)
    {
        return Err("close_receipt_invalid");
    }
    let wake = receipt.into_wake().ok_or("close_wake_missing")?;
    compose_endpoint_wake_authorization(
        &mut runtime.scheduler_wake,
        wake,
        preflight,
        &mut runtime.endpoint_wake,
    )
    .map_err(|_| "close_wake_composition_failed")?;
    let prepared_wake = runtime
        .scheduler
        .prepare_endpoint_wake(
            &mut runtime.tasks,
            &mut runtime.endpoint_wake,
            &mut runtime.selection,
        )
        .map_err(|_| "close_wake_prepare_failed")?;
    let wake = prepared_wake.commit();
    let snapshot = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "close_scheduler_invalid")?;
    let reservation = snapshot.reservation().ok_or("close_reservation_missing")?;
    if wake.task() != runtime.consumer.identity
        || snapshot.current() != Some(runtime.peer.identity)
        || snapshot.runnable() != 1
        || !reservation.runnable()
        || reservation.task() != runtime.consumer.identity
        || reservation.current() != runtime.peer.identity
        || reservation.sequence() != 4
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_none()
        || exact_pending(runtime)? != pending
    {
        return Err("close_wake_state_invalid");
    }

    runtime.trace.closures = 1;
    runtime.trace.wakes += 1;
    runtime.trace.route_queue_cleared = true;
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn finish_consumer(
    runtime: &mut input_runtime,
    incoming: scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    if incoming.instruction_pointer
        != relocated_user_address(
            ptr::addr_of!(x86_64_input_wait_consumer_start),
            ptr::addr_of!(x86_64_input_wait_consumer_completion_return),
            runtime.consumer.space.payload_length,
        )
        .ok_or("completion_return_invalid")?
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.trace.faults != 1
        || runtime.trace.deliveries != 2
        || !runtime.trace.payload_free_interruption
    {
        return Err("consumer_completion_state_invalid");
    }
    runtime
        .scheduler
        .complete_current(&mut runtime.tasks, runtime.consumer.identity)
        .map_err(|_| "consumer_completion_transition_failed")?;
    runtime.trace.completions = 1;
    if runtime
        .channel
        .as_ref()
        .ok_or("completion_channel_missing")?
        .session_input_pending()
        .is_ok()
        || runtime
            .endpoint
            .as_ref()
            .ok_or("completion_endpoint_missing")?
            .binding()
            != runtime.binding
        || !scratch_is_zero(&runtime.scratch)
    {
        return Err("completion_private_owner_state_invalid");
    }
    runtime.expected_delivery_sequence = 0;
    runtime.phase = input_phase::idle;

    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        return Err("completion_interrupt_mask_failed");
    }
    // SAFETY: Both tasks are terminal and every move-only authority is gone.
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3 {
        return Err("completion_kernel_root_restore_failed");
    }
    // SAFETY: No later privilege transition can target a task stack.
    if !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
    {
        return Err("completion_tss_restore_failed");
    }
    let saved_kernel_stack = runtime.saved_kernel_stack;
    let original_task_switched = runtime.original_task_switched;
    if saved_kernel_stack == 0
        || !(saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(original_task_switched, 0 | cr0_task_switched)
        || paging::read_cr0() & cr0_task_switched == 0
    {
        return Err("completion_continuation_invalid");
    }
    runtime.active_role = None;
    gate_active.store(false, Ordering::Release);
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: This consumes the exact saved kernel continuation and abandons
    // only the now-terminal consumer transition frame.
    unsafe {
        super::scheduler_test::x86_64_scheduler_resume_kernel(
            completion_status,
            saved_kernel_stack,
            original_task_switched,
        )
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_input_wait_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<scheduler_frame>())
        || rite_gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("rite_gate_reentry_or_frame_invalid");
    }
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        fail("rite_gate_without_runtime");
    }
    // SAFETY: The published runtime is stable on the original kernel stack;
    // the interrupt gate cleared IF and this feature runs with NMI masked.
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
    runtime: &mut input_runtime,
    frame: *mut scheduler_frame,
) -> Result<*mut scheduler_frame, &'static str> {
    let phase = runtime.phase;
    if !matches!(
        phase,
        input_phase::immediate | input_phase::first_wait | input_phase::second_wait
    ) || runtime.current_role() != Some(fixture_role::consumer)
        || !runtime.frame_is_valid_for_role(frame, fixture_role::consumer, true)
        || runtime.endpoint.is_none()
        || runtime.channel.is_none()
    {
        return Err("rite_runtime_state_invalid");
    }
    // SAFETY: The complete frame belongs to the validated running consumer and
    // remains live until this gate returns the same or another owned frame.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || incoming.accumulator != 0
        || !control_registers_are_exact(incoming)
        || expected_rite_return(runtime, phase) != Some(incoming.instruction_pointer)
    {
        return Err("rite_frame_shape_invalid");
    }

    let mailbox_alias = consumer_mailbox_alias(runtime)?;
    // SAFETY: This is the supervisor-only alias of the validated current
    // consumer's one live RW+NX mailbox, exclusively held by this gate.
    let call = unsafe { read_call(mailbox_alias) }?;
    validate_rite_call(phase, call)?;

    let mut channel = runtime.channel.take().ok_or("rite_channel_missing")?;
    let result = dispatch_admitted_rite(runtime, frame, mailbox_alias, call, phase, &mut channel);
    if runtime.channel.is_some() {
        return Err("rite_channel_slot_changed");
    }
    runtime.channel = Some(channel);
    result
}

fn dispatch_admitted_rite(
    runtime: &mut input_runtime,
    frame: *mut scheduler_frame,
    mailbox_alias: u64,
    call: rite_call,
    phase: input_phase,
    channel: &mut rite_channel,
) -> Result<*mut scheduler_frame, &'static str> {
    let admission = match channel
        .assess(&runtime.tasks, runtime.consumer.identity, call)
        .map_err(|_| "rite_assessment_failed")?
    {
        rite_decision::admitted(admission) => admission,
        rite_decision::rejected(_) => return Err("fixed_rite_rejected"),
    };
    if admission.vessel() != runtime.consumer.identity
        || admission.operation() != rite_operation::session_input_read
        || admission.payload_kind() != rite_payload_kind::session_input_request
        || admission.payload_size() != expected_request_size
        || admission.sequence() != phase.sequence().ok_or("rite_phase_sequence_missing")?
        || admission.trace_id() != phase.trace().ok_or("rite_phase_trace_missing")?
    {
        return Err("rite_admission_invalid");
    }

    // SAFETY: Admission fixed the exact body span. The alias and gate retain
    // ownership and exclusion for the complete copy.
    unsafe {
        copy_admitted_body(
            mailbox_alias,
            user_mailbox_address,
            admission.payload_size(),
            &mut runtime.scratch,
            &mut runtime.trace.body_copies,
            &mut runtime.trace.body_bytes,
        )
    }?;
    let request = decode_session_input_request(&runtime.scratch);
    if request != rite_session_input_request_v1::new(true)
        || !request.has_supported_body()
        || !request.wait_if_empty()
        || request.reply_capacity() != expected_delivery_size
    {
        return Err("rite_request_invalid");
    }
    runtime.trace.rite_calls += 1;

    match phase {
        input_phase::immediate => {
            complete_immediate_delivery(runtime, frame, mailbox_alias, admission, &request)
        }
        input_phase::first_wait | input_phase::second_wait => {
            let next = install_pending_wait(runtime, mailbox_alias, admission, &request, phase)?;
            let pending = channel
                .session_input_pending()
                .map_err(|_| "pending_rite_missing")?;
            let wait_sequence = if phase == input_phase::first_wait {
                1
            } else {
                2
            };
            validate_pending_view(
                runtime,
                pending,
                phase.sequence().ok_or("pending_sequence_missing")?,
                phase.trace().ok_or("pending_trace_missing")?,
                wait_sequence,
            )?;
            Ok(next)
        }
        _ => Err("rite_phase_invalid"),
    }
}

fn complete_immediate_delivery(
    runtime: &mut input_runtime,
    frame: *mut scheduler_frame,
    mailbox_alias: u64,
    admission: rite_admission<'_>,
    _request: &rite_session_input_request_v1,
) -> Result<*mut scheduler_frame, &'static str> {
    if runtime.phase != input_phase::immediate
        || runtime.expected_delivery_sequence != 1
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.trace.deliveries != 0
    {
        return Err("immediate_delivery_state_invalid");
    }
    let endpoint = runtime.endpoint.as_mut().ok_or("endpoint_missing")?;
    if !matches!(
        endpoint
            .prepare_receive(runtime.binding)
            .map_err(|_| "immediate_receive_prepare_failed")?,
        session_input_receive_preparation::ready
    ) {
        return Err("immediate_receive_not_ready");
    }
    let prepared_delivery = endpoint
        .prepare_delivery(runtime.binding, None)
        .map_err(|_| "immediate_delivery_prepare_failed")?;
    if prepared_delivery.correlation().is_some() {
        return Err("immediate_delivery_correlation_invalid");
    }
    let mut wire = {
        let delivery = prepared_delivery
            .delivery(endpoint)
            .map_err(|_| "immediate_delivery_revalidation_failed")?;
        wire_delivery(
            &delivery,
            runtime.binding,
            session_input_origin::human_text,
            1,
        )?
    };
    let reply = admission
        .session_input_delivery_reply()
        .map_err(|_| "immediate_reply_prepare_failed")?;
    if !reply_is_exact(
        reply,
        rite_status::ok,
        rite_payload_kind::session_input_delivery,
        expected_delivery_size,
        1,
        id_128::from_bytes(trace_1),
    ) {
        return Err("immediate_reply_invalid");
    }
    let prepared_reply = prepare_session_input_delivery_reply(reply, &wire)?;
    wire.clear();
    if !wire.is_clear() {
        return Err("immediate_wire_clear_failed");
    }
    if !clear_and_verify_bytes(&mut runtime.scratch) {
        return Err("immediate_scratch_clear_failed");
    }
    // SAFETY: All fallible source and encoding checks finished above. The
    // exact mailbox alias remains live and exclusively mutation-held.
    let staged = unsafe { prepared_reply.stage(mailbox_alias) }?;
    let receipt = Cell::new(None);
    let settled_reply = admission
        .finish_session_input_delivery(&runtime.tasks, || {
            let committed = endpoint.commit_prepared_delivery(prepared_delivery, None);
            receipt.set(Some((
                committed.sequence().get(),
                committed.acknowledged_wait(),
            )));
        })
        .map_err(|_| "immediate_delivery_settlement_failed")?;
    if settled_reply != reply || receipt.get() != Some((1, false)) {
        return Err("immediate_delivery_receipt_invalid");
    }
    // SAFETY: Source consumption and Rite settlement are complete; this is the
    // final validity-marker store for the already complete fixed reply.
    unsafe { staged.publish() };

    runtime.phase = input_phase::first_wait;
    runtime.trace.deliveries += 1;
    runtime.trace.immediate_copy_before_consume = true;
    compiler_fence(Ordering::SeqCst);
    Ok(frame)
}

fn install_pending_wait(
    runtime: &mut input_runtime,
    mailbox_alias: u64,
    admission: rite_admission<'_>,
    request: &rite_session_input_request_v1,
    phase: input_phase,
) -> Result<*mut scheduler_frame, &'static str> {
    let (expected_rite_sequence, expected_wait_sequence, expected_dispatch, next_phase) =
        match phase {
            input_phase::first_wait => (2, 1, 1, input_phase::published),
            input_phase::second_wait => (3, 2, 3, input_phase::closing),
            _ => return Err("wait_phase_invalid"),
        };
    if runtime.expected_delivery_sequence != expected_rite_sequence - 1
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.selection.is_some()
        || runtime.trace.blocks != (expected_wait_sequence - 1) as usize
        || mailbox_alias != runtime.consumer.space.physical_pages[1]
    {
        return Err("wait_state_invalid");
    }

    let endpoint = runtime.endpoint.as_mut().ok_or("endpoint_missing")?;
    let prepared_wait = match endpoint
        .prepare_receive(runtime.binding)
        .map_err(|_| "wait_prepare_failed")?
    {
        session_input_receive_preparation::ready => return Err("empty_wait_reported_ready"),
        session_input_receive_preparation::wait(prepared) => prepared,
    };
    let wait = prepared_wait.proposed_wait();
    let readiness = prepared_wait.proposed_readiness();
    if prepared_wait.binding() != runtime.binding
        || wait.binding() != runtime.binding
        || readiness.binding() != runtime.binding
        || wait.sequence() != expected_wait_sequence
        || readiness.generation() != expected_wait_sequence
    {
        return Err("wait_correlation_invalid");
    }
    let pending_preparation = admission
        .prepare_session_input_pending(request, runtime.binding, &prepared_wait, runtime.mapping)
        .map_err(|_| "pending_rite_prepare_failed")?;
    if pending_preparation.binding() != runtime.binding
        || pending_preparation.wait() != wait
        || pending_preparation.readiness() != readiness
        || pending_preparation.mapping() != runtime.mapping
    {
        return Err("pending_rite_identity_invalid");
    }
    let scheduler_preparation = runtime
        .scheduler
        .prepare_block_current_and_reserve(
            &mut runtime.tasks,
            runtime.consumer.identity,
            runtime.peer.identity,
            wait,
            readiness,
            &mut runtime.scheduler_wake,
        )
        .map_err(|_| "reserved_block_prepare_failed")?;
    let predicted = scheduler_preparation.transition();
    let expected_context = if phase == input_phase::first_wait {
        scheduler_context::initial
    } else {
        scheduler_context::saved
    };
    if predicted.task() != runtime.consumer.identity
        || predicted.dispatch().task() != runtime.peer.identity
        || predicted.dispatch().sequence() != expected_dispatch
        || predicted.dispatch().context() != expected_context
    {
        return Err("reserved_block_prediction_invalid");
    }
    if !clear_and_verify_bytes(&mut runtime.scratch) {
        return Err("wait_scratch_clear_failed");
    }
    let blocked = match endpoint
        .commit_wait(prepared_wait)
        .map_err(|_| "wait_commit_failed")?
    {
        session_input_wait_commit::ready => return Err("wait_commit_became_ready"),
        session_input_wait_commit::blocked(blocked) => blocked,
    };
    assert!(
        blocked.binding() == runtime.binding
            && blocked.identity() == wait
            && blocked.readiness() == readiness,
        "committed session input wait changed after preflight"
    );
    pending_preparation.commit(blocked);
    let transition = scheduler_preparation.commit();
    assert!(
        transition == predicted && runtime.scheduler_wake.is_some(),
        "reserved session input block changed after preflight"
    );

    let snapshot = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "blocked_scheduler_invalid")?;
    let reservation = snapshot.reservation().ok_or("block_reservation_missing")?;
    if snapshot.current() != Some(runtime.peer.identity)
        || snapshot.runnable() != 0
        || reservation.runnable()
        || reservation.task() != runtime.consumer.identity
        || reservation.current() != runtime.peer.identity
        || reservation.sequence() != expected_dispatch + 1
        || runtime
            .tasks
            .snapshot(runtime.consumer.identity)
            .map_err(|_| "blocked_consumer_missing")?
            .state()
            != task_state::blocked
        || runtime
            .tasks
            .snapshot(runtime.peer.identity)
            .map_err(|_| "dispatched_peer_missing")?
            .state()
            != task_state::running
    {
        return Err("blocked_runtime_state_invalid");
    }

    runtime.phase = next_phase;
    runtime.trace.blocks += 1;
    runtime.trace.pending_is_pointer_free = true;
    runtime.trace.dispatches[expected_dispatch as usize] = expected_dispatch;
    let next = runtime.select_role(fixture_role::peer, true)?;
    validate_peer_dispatch(runtime, next, phase)?;
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

fn complete_selected_delivery(runtime: &mut input_runtime) -> Result<(), &'static str> {
    let pending = exact_pending(runtime)?;
    let correlation = session_input_wake_correlation::new(
        pending.wait(),
        pending.readiness(),
        session_input_wake_outcome::input_ready,
    );
    validate_correlation(
        pending,
        correlation,
        session_input_wake_outcome::input_ready,
    )?;
    let mailbox_alias = consumer_mailbox_alias(runtime)?;
    let endpoint = runtime.endpoint.as_mut().ok_or("endpoint_missing")?;
    let prepared_delivery = endpoint
        .prepare_delivery(runtime.binding, Some(correlation))
        .map_err(|_| "selected_delivery_prepare_failed")?;
    if prepared_delivery.correlation() != Some(correlation) {
        return Err("selected_delivery_correlation_invalid");
    }
    let mut wire = {
        let delivery = prepared_delivery
            .delivery(endpoint)
            .map_err(|_| "selected_delivery_revalidation_failed")?;
        wire_delivery(
            &delivery,
            runtime.binding,
            session_input_origin::machine_text,
            2,
        )?
    };
    let completion = runtime
        .channel
        .as_mut()
        .ok_or("selected_delivery_channel_missing")?
        .prepare_session_input_completion(pending, rite_session_input_completion_outcome::delivery)
        .map_err(|_| "selected_delivery_rite_prepare_failed")?;
    let reply = completion.reply();
    if !reply_is_exact(
        reply,
        rite_status::ok,
        rite_payload_kind::session_input_delivery,
        expected_delivery_size,
        2,
        id_128::from_bytes(trace_2),
    ) {
        return Err("selected_delivery_reply_invalid");
    }
    let prepared_reply = prepare_session_input_delivery_reply(reply, &wire)?;
    wire.clear();
    if !wire.is_clear() {
        return Err("selected_wire_clear_failed");
    }
    let selection = runtime
        .scheduler
        .prepare_reserved_preemption(
            &mut runtime.tasks,
            runtime.peer.identity,
            &mut runtime.selection,
        )
        .map_err(|_| "reserved_preemption_prepare_failed")?;
    let predicted = selection.transition();
    if predicted.task() != runtime.peer.identity
        || predicted.sequence() != 0
        || predicted.dispatch().task() != runtime.consumer.identity
        || predicted.dispatch().sequence() != 2
        || predicted.dispatch().context() != scheduler_context::saved
    {
        return Err("reserved_preemption_prediction_invalid");
    }
    // SAFETY: Every source, scheduler, mapping, and encoding check completed;
    // this fixed alias stays exclusive through final header publication.
    let staged = unsafe { prepared_reply.stage(mailbox_alias) }?;
    let transition = selection.commit();
    if transition != predicted {
        return Err("reserved_preemption_commit_invalid");
    }
    let acknowledgement =
        session_input_wait_acknowledgement::new(pending.wait(), pending.readiness());
    let receipt = endpoint.commit_prepared_delivery(prepared_delivery, Some(acknowledgement));
    if receipt.sequence().get() != 2 || !receipt.acknowledged_wait() {
        return Err("selected_delivery_receipt_invalid");
    }
    let settled_reply = completion.commit();
    if settled_reply != reply {
        return Err("selected_delivery_settlement_invalid");
    }
    // SAFETY: Selection, wait acknowledgement, source consumption, and Rite
    // settlement are complete; publish only the saved validity marker.
    unsafe { staged.publish() };

    let snapshot = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "selected_delivery_scheduler_invalid")?;
    if snapshot.current() != Some(runtime.consumer.identity)
        || snapshot.runnable() != 1
        || snapshot.reservation().is_some()
        || runtime.selection.is_some()
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime
            .channel
            .as_ref()
            .ok_or("selected_delivery_channel_missing")?
            .session_input_pending()
            .is_ok()
        || runtime
            .tasks
            .snapshot(runtime.consumer.identity)
            .map_err(|_| "selected_consumer_missing")?
            .state()
            != task_state::running
        || runtime
            .tasks
            .snapshot(runtime.peer.identity)
            .map_err(|_| "preempted_peer_missing")?
            .state()
            != task_state::suspended
    {
        return Err("selected_delivery_state_invalid");
    }
    runtime.trace.preemptions[0] = transition.sequence();
    runtime.trace.dispatches[2] = transition.dispatch().sequence();
    runtime.trace.deliveries += 1;
    Ok(())
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
    result.map(|selected| selected.cast())
}

fn contain_user_exception(
    frame: *mut scheduler_frame,
    snapshot: exceptions::exception_snapshot,
) -> Result<*mut scheduler_frame, &'static str> {
    let runtime_pointer = active_runtime.load(Ordering::Acquire);
    if runtime_pointer.is_null() {
        return Err("fault_without_runtime");
    }
    // SAFETY: The exception gate cleared IF and the switch latch exclusively
    // owns the stable published runtime on this BSP.
    let runtime = unsafe { &mut *runtime_pointer };
    if runtime.current_role() != Some(fixture_role::peer)
        || runtime.phase != input_phase::closing
        || runtime.selection.is_none()
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime.trace.closures != 1
        || runtime.trace.wakes != 2
        || runtime.trace.faults != 0
        || !runtime.frame_is_valid_for_role(frame, fixture_role::peer, true)
    {
        return Err("fault_state_invalid");
    }
    let expected_instruction = relocated_user_address(
        ptr::addr_of!(x86_64_input_wait_peer_start),
        ptr::addr_of!(x86_64_input_wait_peer_fault_instruction),
        runtime.peer.space.payload_length,
    )
    .ok_or("fault_instruction_invalid")?;
    // SAFETY: The complete exception frame belongs to the validated peer.
    let incoming = unsafe { ptr::read(frame) };
    if !expected_invalid_opcode_matches(snapshot, incoming, expected_instruction) {
        return Err("unexpected_user_exception");
    }

    complete_selected_interruption(runtime)?;
    let next = runtime.select_role(fixture_role::consumer, true)?;
    validate_saved_consumer_return(
        runtime,
        next,
        ptr::addr_of!(x86_64_input_wait_consumer_interrupted_return),
    )?;
    compiler_fence(Ordering::SeqCst);
    Ok(next)
}

fn complete_selected_interruption(runtime: &mut input_runtime) -> Result<(), &'static str> {
    let pending = exact_pending(runtime)?;
    let outcome = session_input_wake_outcome::terminal(session_input_close_reason::focus_lost);
    let correlation =
        session_input_wake_correlation::new(pending.wait(), pending.readiness(), outcome);
    validate_correlation(pending, correlation, outcome)?;
    let mailbox_alias = consumer_mailbox_alias(runtime)?;
    let endpoint = runtime.endpoint.as_mut().ok_or("endpoint_missing")?;
    let prepared_terminal = endpoint
        .prepare_terminal(runtime.binding, correlation)
        .map_err(|_| "terminal_prepare_failed")?;
    if prepared_terminal.correlation() != correlation {
        return Err("terminal_correlation_invalid");
    }
    let completion = runtime
        .channel
        .as_mut()
        .ok_or("terminal_channel_missing")?
        .prepare_session_input_completion(
            pending,
            rite_session_input_completion_outcome::interrupted,
        )
        .map_err(|_| "terminal_rite_prepare_failed")?;
    let reply = completion.reply();
    if !reply_is_exact(
        reply,
        rite_status::interrupted,
        rite_payload_kind::none,
        0,
        3,
        id_128::from_bytes(trace_3),
    ) {
        return Err("terminal_reply_invalid");
    }
    let prepared_reply = prepare_session_input_empty_reply(reply)?;
    let selection = runtime
        .scheduler
        .prepare_reserved_fault(
            &mut runtime.tasks,
            runtime.peer.identity,
            &mut runtime.selection,
        )
        .map_err(|_| "reserved_fault_prepare_failed")?;
    let predicted = selection.transition();
    if predicted.task() != runtime.peer.identity
        || predicted.dispatch().task() != runtime.consumer.identity
        || predicted.dispatch().sequence() != 4
        || predicted.dispatch().context() != scheduler_context::saved
    {
        return Err("reserved_fault_prediction_invalid");
    }
    // SAFETY: Every terminal, scheduler, mapping, and encoding preflight is
    // complete and this exact alias remains exclusive through publication.
    let staged = unsafe { prepared_reply.stage(mailbox_alias) }?;
    let transition = selection.commit();
    if transition != predicted {
        return Err("reserved_fault_commit_invalid");
    }
    let acknowledgement =
        session_input_wait_acknowledgement::new(pending.wait(), pending.readiness());
    let terminal = endpoint.commit_prepared_terminal(prepared_terminal, acknowledgement);
    if terminal.wait() != pending.wait()
        || terminal.readiness() != pending.readiness()
        || terminal.reason() != session_input_close_reason::focus_lost
    {
        return Err("terminal_acknowledgement_invalid");
    }
    let settled_reply = completion.commit();
    if settled_reply != reply {
        return Err("terminal_settlement_invalid");
    }
    // SAFETY: Reserved selection, terminal acknowledgement, and channel
    // settlement are complete. The cleared body is now made visible last.
    unsafe { staged.publish() };

    let scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "terminal_scheduler_invalid")?;
    if scheduler.current() != Some(runtime.consumer.identity)
        || scheduler.runnable() != 0
        || scheduler.reservation().is_some()
        || runtime.selection.is_some()
        || runtime.scheduler_wake.is_some()
        || runtime.endpoint_wake.is_some()
        || runtime
            .channel
            .as_ref()
            .ok_or("terminal_channel_missing")?
            .session_input_pending()
            .is_ok()
        || runtime
            .tasks
            .snapshot(runtime.consumer.identity)
            .map_err(|_| "terminal_consumer_missing")?
            .state()
            != task_state::running
        || runtime
            .tasks
            .snapshot(runtime.peer.identity)
            .map_err(|_| "terminal_peer_missing")?
            .state()
            != task_state::faulted
    {
        return Err("terminal_selected_state_invalid");
    }
    runtime.phase = input_phase::completing;
    runtime.trace.dispatches[4] = transition.dispatch().sequence();
    runtime.trace.faults = 1;
    runtime.trace.reserved_fault = true;
    runtime.trace.payload_free_interruption = true;
    Ok(())
}

pub(super) fn reject_unexpected_exception() -> ! {
    fail("unexpected_exception")
}

fn exact_pending(runtime: &input_runtime) -> Result<rite_session_input_pending_view, &'static str> {
    let (rite_sequence, wait_sequence, trace) = match runtime.phase {
        input_phase::first_wait | input_phase::published => (2, 1, id_128::from_bytes(trace_2)),
        input_phase::second_wait | input_phase::closing => (3, 2, id_128::from_bytes(trace_3)),
        _ => return Err("pending_phase_invalid"),
    };
    let pending = runtime
        .channel
        .as_ref()
        .ok_or("pending_channel_missing")?
        .session_input_pending()
        .map_err(|_| "pending_rite_missing")?;
    validate_pending_view(runtime, pending, rite_sequence, trace, wait_sequence)?;
    Ok(pending)
}

fn validate_pending_view(
    runtime: &input_runtime,
    pending: rite_session_input_pending_view,
    rite_sequence: u64,
    trace: id_128,
    wait_sequence: u64,
) -> Result<(), &'static str> {
    if pending.vessel() != runtime.consumer.identity
        || pending.binding() != runtime.binding
        || pending.wait().binding() != runtime.binding
        || pending.readiness().binding() != runtime.binding
        || pending.wait().sequence() != wait_sequence
        || pending.readiness().generation() != wait_sequence
        || pending.mapping() != runtime.mapping
        || pending.mapping().vessel() != runtime.consumer.identity
        || pending.mapping().generation() != 1
        || pending.sequence() != rite_sequence
        || pending.trace_id() != trace
        || pending.reply_capacity() != expected_delivery_size
    {
        return Err("pending_correlation_invalid");
    }
    Ok(())
}

fn validate_correlation(
    pending: rite_session_input_pending_view,
    correlation: session_input_wake_correlation,
    outcome: session_input_wake_outcome,
) -> Result<(), &'static str> {
    if correlation.binding() != pending.binding()
        || correlation.wait() != pending.wait()
        || correlation.readiness() != pending.readiness()
        || correlation.outcome() != outcome
    {
        return Err("wake_correlation_invalid");
    }
    Ok(())
}

fn wire_delivery(
    delivery: &session_input_delivery<'_>,
    expected_binding: session_input_binding,
    expected_origin: session_input_origin,
    expected_sequence: u64,
) -> Result<rite_session_input_delivery_v1, &'static str> {
    let fixed_text = delivery.expose_fixed_text();
    if delivery.binding() != expected_binding
        || delivery.expose_origin() != expected_origin
        || delivery.expose_action_kind() != session_input_action_kind::insert_text
        || delivery.expose_sequence().get() != expected_sequence
        || usize::from(delivery.expose_byte_len()) != private_text.len()
        || delivery.expose_text().map(str::as_bytes) != Some(private_text.as_slice())
        || delivery.expose_erase_count().is_some()
        || fixed_text[..private_text.len()] != private_text
        || fixed_text[private_text.len()..]
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err("private_delivery_invalid");
    }
    let origin = match expected_origin {
        session_input_origin::human_text => rite_session_input_origin::human_text,
        session_input_origin::machine_text => rite_session_input_origin::machine_text,
        _ => return Err("private_delivery_origin_invalid"),
    };
    let wire = rite_session_input_delivery_v1::try_insert_text(
        origin,
        expected_sequence,
        &fixed_text[..private_text.len()],
    )
    .ok_or("wire_delivery_invalid")?;
    if !wire.has_supported_body()
        || wire.expose_action() != rite_session_input_action::insert_text
        || wire.expose_origin() != origin
        || wire.delivery_sequence() != expected_sequence
    {
        return Err("wire_delivery_shape_invalid");
    }
    Ok(wire)
}

fn consumer_mailbox_alias(runtime: &input_runtime) -> Result<u64, &'static str> {
    if runtime.mapping.vessel() != runtime.consumer.identity || runtime.mapping.generation() != 1 {
        return Err("mailbox_identity_invalid");
    }
    let alias = validate_mailbox_mapping(&runtime.consumer.space)?;
    if alias != runtime.consumer.space.physical_pages[1] {
        return Err("mailbox_alias_invalid");
    }
    Ok(alias)
}

fn validate_rite_call(phase: input_phase, call: rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.operation != rite_operation::session_input_read
        || call.payload_kind != rite_payload_kind::session_input_request
        || call.reserved != 0
        || call.rights_present != rite_rights::session_input_read
        || call.payload_size != expected_request_size
        || call.payload_limit != expected_request_size
        || call.trace_id != phase.trace().ok_or("rite_call_trace_missing")?
    {
        return Err("rite_call_invalid");
    }
    Ok(())
}

fn reply_is_exact(
    reply: rite_reply,
    status: rite_status,
    payload_kind: rite_payload_kind,
    payload_size: u32,
    sequence: u64,
    trace: id_128,
) -> bool {
    reply.has_supported_header()
        && reply.status == status
        && reply.payload_kind == payload_kind
        && reply.payload_size == payload_size
        && reply.sequence == sequence
        && reply.trace_id == trace
}

fn expected_rite_return(runtime: &input_runtime, phase: input_phase) -> Option<u64> {
    let label = match phase {
        input_phase::immediate => ptr::addr_of!(x86_64_input_wait_consumer_immediate_return),
        input_phase::first_wait => ptr::addr_of!(x86_64_input_wait_consumer_delivery_return),
        input_phase::second_wait => ptr::addr_of!(x86_64_input_wait_consumer_interrupted_return),
        _ => return None,
    };
    relocated_user_address(
        ptr::addr_of!(x86_64_input_wait_consumer_start),
        label,
        runtime.consumer.space.payload_length,
    )
}

fn validate_saved_consumer_return(
    runtime: &input_runtime,
    frame: *const scheduler_frame,
    label: *const u8,
) -> Result<(), &'static str> {
    if !runtime.frame_is_valid_for_role(frame, fixture_role::consumer, true) {
        return Err("saved_consumer_frame_invalid");
    }
    let expected = relocated_user_address(
        ptr::addr_of!(x86_64_input_wait_consumer_start),
        label,
        runtime.consumer.space.payload_length,
    )
    .ok_or("saved_consumer_return_invalid")?;
    // SAFETY: Frame ownership and full stack extent were validated above.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != rite_gate_vector
        || incoming.error_code != 0
        || incoming.instruction_pointer != expected
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || incoming.accumulator != 0
        || !control_registers_are_exact(incoming)
    {
        return Err("saved_consumer_return_shape_invalid");
    }
    Ok(())
}

fn validate_peer_dispatch(
    runtime: &input_runtime,
    frame: *const scheduler_frame,
    blocked_phase: input_phase,
) -> Result<(), &'static str> {
    if !runtime.frame_is_valid_for_role(frame, fixture_role::peer, true) {
        return Err("peer_dispatch_frame_invalid");
    }
    // SAFETY: Frame ownership and complete stack extent were validated above.
    let incoming = unsafe { ptr::read(frame) };
    let (label, vector, accumulator, mailbox) = match blocked_phase {
        input_phase::first_wait => (ptr::addr_of!(x86_64_input_wait_peer_start), 0, 0, 0),
        input_phase::second_wait => (
            ptr::addr_of!(x86_64_input_wait_peer_preempt_return),
            scheduler_gate_vector,
            operation_preempt,
            user_mailbox_address,
        ),
        _ => return Err("peer_dispatch_phase_invalid"),
    };
    let expected = relocated_user_address(
        ptr::addr_of!(x86_64_input_wait_peer_start),
        label,
        runtime.peer.space.payload_length,
    )
    .ok_or("peer_dispatch_return_invalid")?;
    if incoming.vector != vector
        || incoming.error_code != 0
        || incoming.instruction_pointer != expected
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || incoming.accumulator != accumulator
        || incoming.register_15 != mailbox
        || incoming.base != 0
        || incoming.counter != 0
        || incoming.data != 0
        || incoming.frame_pointer != 0
        || incoming.source_index != 0
        || incoming.destination_index != 0
        || incoming.register_8 != 0
        || incoming.register_9 != 0
        || incoming.register_10 != 0
        || incoming.register_11 != 0
        || incoming.register_12 != 0
        || incoming.register_13 != 0
        || incoming.register_14 != 0
    {
        return Err("peer_dispatch_shape_invalid");
    }
    Ok(())
}

fn control_registers_are_exact(frame: scheduler_frame) -> bool {
    frame.base == 0
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
        // Fault-class exceptions may set RF in the saved return flags.
        && (snapshot.flags == initial_user_flags
            || snapshot.flags == initial_user_flags | resume_flag)
        && snapshot.stack_pointer == user_stack_top
        && snapshot.stack_segment == Some(u64::from(descriptors::user_data_selector))
        && snapshot.fault_address.is_none()
        && snapshot.accumulator == 0
        && all_general_registers_zero(registers)
}

fn emit_evidence(evidence: input_evidence) {
    let trace = evidence.trace;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.ready","level":"info","arch":"x86_64","policy":"fixed_session_input","content_redacted":true,"nmi_masked":true,"bsp_only":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.immediate.completed","level":"info","body_copied":{},"source_consumed":true,"header_published_last":true,"content_redacted":true}}"#,
        trace.immediate_copy_before_consume
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.wait.blocked","level":"info","pending_owned":{},"user_pointer_retained":false,"exact_reservation":true,"content_redacted":true}}"#,
        trace.pending_is_pointer_free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.publication.woke","level":"info","same_semantic_shape":true,"publisher_remained_current":{},"fifo_tail":{},"source_consumed":false,"exact_wake":true,"content_redacted":true}}"#,
        trace.publisher_remained_current, trace.fifo_tail_wake
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.delivery.selected","level":"info","reserved_selection":{},"body_copied":{},"source_consumed":true,"channel_settled":true,"header_published_last":true,"content_redacted":true}}"#,
        trace.reserved_preemption, trace.selected_copy_before_consume
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.wait.blocked_again","level":"info","pending_owned":{},"user_pointer_retained":false,"exact_reservation":true,"content_redacted":true}}"#,
        trace.pending_is_pointer_free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.route.closed","level":"info","route_queue_cleared":{},"exact_wake":true,"content_redacted":true}}"#,
        trace.route_queue_cleared
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.fault.selected","level":"info","kind":"invalid_opcode","peer_terminal":true,"reserved_selection":{},"content_redacted":true}}"#,
        trace.reserved_fault
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.interruption.completed","level":"info","payload_free":{},"channel_settled":true,"header_published_last":true,"content_redacted":true}}"#,
        trace.payload_free_interruption
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.released","level":"info","frame_accounting_restored":true,"tasks_retired":true,"contexts_withdrawn":true,"roots_withdrawn":true,"private_storage_cleared":true,"channel_retired":true,"endpoint_retired":true,"scratch_wiped":true,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true,"content_redacted":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.passed","level":"info","lifecycle_complete":true,"exact_wake":true,"copy_before_consume":true,"payload_free_interruption":true,"kernel_continuation_restored":{},"content_redacted":true}}"#,
        trace.kernel_continuation_restored
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.input_wait_test.failed","level":"fatal","reason":"{reason}","content_redacted":true}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
