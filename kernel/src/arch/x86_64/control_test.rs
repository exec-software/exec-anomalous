use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::address_space::{address_space, mapping_evidence};
use super::{descriptors, paging};
use crate::circle::{circle_capacity, circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::serial_port;
use crate::identity::{endpoint_id, fixture_principal_id, task_identity};
use crate::ipc::{
    control_ipc_table, ipc_authorized_error, ipc_endpoint_capacity, ipc_error, ipc_message,
    ipc_payload, ipc_queue_capacity,
};
use crate::memory;
use crate::memory::physical::{frame_allocator, frame_lease, frame_purpose, physical_range};
use crate::sigil::{
    control_sigil_table, sigil_binding_capacity, sigil_error, sigil_lineage_capacity,
    sigil_record_capacity, sigil_reference, sigil_revocation, sigil_rights,
};
use crate::task::{
    synthetic_origin_metadata, task_binding, task_bounds, task_capacity, task_state, task_table,
};

const page_size: u64 = 4096;
const user_window_address: u64 = 0x0000_6000_0000_0000;
const user_code_address: u64 = user_window_address + 0x1000;
const user_mailbox_address: u64 = user_window_address + 0x3000;
const user_stack_address: u64 = user_window_address + 0x5000;
const user_stack_top: u64 = user_window_address + 0x6000;

const control_gate_vector: u64 = 0xf1;
const sanitized_flags: u64 = 0x2;
const cr0_task_switched: u64 = 1 << 3;

const operation_probe: u64 = 0x10;
const operation_send: u64 = 0x11;
const operation_receive: u64 = 0x12;
const operation_complete: u64 = 0x13;

const status_accepted: u64 = 0;
const status_denied: u64 = 1;
const status_would_block: u64 = 2;
const status_completed: u64 = 0x40;

const mailbox_valid_reference_offset: usize = 0;
const mailbox_copied_reference_offset: usize = 24;
const mailbox_expected_first_offset: usize = 64;
const mailbox_expected_second_offset: usize = 136;
const mailbox_transfer_offset: usize = 256;
const encoded_message_word_count: usize = 9;

const private_payload_first: u64 = 0x9ad4_c1e7_6b20_8f35;
const private_payload_second: u64 = 0x53b8_f02d_a691_4ce7;
const private_payload_bytes: [u8; 16] = [
    0x35, 0x8f, 0x20, 0x6b, 0xe7, 0xc1, 0xd4, 0x9a, 0xe7, 0x4c, 0x91, 0xa6, 0x2d, 0xf0, 0xb8, 0x53,
];

const receiver_principal: u64 = 1;
const human_principal: u64 = 2;
const machine_principal: u64 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum fixture_role {
    human,
    machine,
    receiver,
}

impl fixture_role {
    const fn label(self) -> &'static str {
        match self {
            Self::human => "human_fixture",
            Self::machine => "machine_fixture",
            Self::receiver => "receiver_fixture",
        }
    }

    const fn origin(self) -> synthetic_origin_metadata {
        match self {
            Self::human => synthetic_origin_metadata::human_role_fixture,
            Self::machine => synthetic_origin_metadata::machine_role_fixture,
            Self::receiver => synthetic_origin_metadata::receiver_role_fixture,
        }
    }

    const fn principal(self) -> u64 {
        match self {
            Self::receiver => receiver_principal,
            Self::human => human_principal,
            Self::machine => machine_principal,
        }
    }

    const fn producer_index(self) -> Option<usize> {
        match self {
            Self::human => Some(0),
            Self::machine => Some(1),
            Self::receiver => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum gate_phase {
    idle,
    producer_probe,
    producer_send,
    producer_complete,
    receiver_probe,
    receiver_first,
    receiver_second,
    receiver_empty,
    receiver_complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct sigil_reference_words {
    slot: u64,
    generation: u64,
    tag: u64,
}

impl sigil_reference_words {
    const fn from_reference(reference: sigil_reference) -> Self {
        Self {
            slot: reference.local_slot() as u64,
            generation: reference.binding_generation(),
            tag: reference.tag() as u64,
        }
    }

    fn decode(self) -> Result<sigil_reference, &'static str> {
        let tag = u32::try_from(self.tag).map_err(|_| "sigil_tag_out_of_range")?;
        let local_slot = u32::try_from(self.slot).map_err(|_| "sigil_slot_out_of_range")?;
        Ok(sigil_reference::from_raw(tag, local_slot, self.generation))
    }
}

#[derive(Clone, Copy)]
struct producer_provenance {
    task: task_identity,
    principal: fixture_principal_id,
    origin: synthetic_origin_metadata,
}

struct control_context {
    circles: circle_table,
    tasks: task_table,
    sigils: control_sigil_table,
    ipc: control_ipc_table,
    endpoint: endpoint_id,
    receiver_sigil: sigil_reference,
    active_sigil: Option<sigil_reference>,
    current_task: Option<task_identity>,
    current_role: fixture_role,
    phase: gate_phase,
    mailbox_alias: u64,
    active_root: u64,
    accepted_sequence: Option<u64>,
    delivered_message: Option<ipc_message>,
    producers: [Option<producer_provenance>; 2],
    copied_producer_reference: Option<sigil_reference>,
}

struct task_space {
    root: address_space,
    physical_pages: [u64; 3],
    payload_length: usize,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct control_gate_frame {
    accumulator: u64,
    base: u64,
    counter: u64,
    data: u64,
    frame_pointer: u64,
    source_index: u64,
    destination_index: u64,
    register_8: u64,
    register_9: u64,
    register_10: u64,
    register_11: u64,
    register_12: u64,
    register_13: u64,
    register_14: u64,
    register_15: u64,
    vector: u64,
    error_code: u64,
    instruction_pointer: u64,
    code_segment: u64,
    flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

const _: () = assert!(mem::size_of::<control_gate_frame>() == 22 * mem::size_of::<u64>());

static active_context: AtomicPtr<control_context> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);
static mut saved_kernel_stack: u64 = 0;
static mut saved_task_switched: u64 = 0;

unsafe extern "C" {
    fn x86_64_enter_control_task(
        instruction_pointer: u64,
        stack_pointer: u64,
        data_selector: u64,
        code_selector: u64,
        saved_stack: *mut u64,
    ) -> u64;
    fn x86_64_resume_control_task(status: u64, saved_stack: u64, original_task_switched: u64) -> !;

    static x86_64_control_producer_start: u8;
    static x86_64_control_producer_probe_return: u8;
    static x86_64_control_producer_send_return: u8;
    static x86_64_control_producer_complete_return: u8;
    static x86_64_control_producer_end: u8;
    static x86_64_control_receiver_start: u8;
    static x86_64_control_receiver_probe_return: u8;
    static x86_64_control_receiver_first_receive_return: u8;
    static x86_64_control_receiver_second_receive_return: u8;
    static x86_64_control_receiver_empty_return: u8;
    static x86_64_control_receiver_complete_return: u8;
    static x86_64_control_receiver_end: u8;
}

pub(super) fn run() -> ! {
    let kernel_cr3 = paging::read_cr3();
    let result =
        memory::with_frame_allocator(|allocator| run_with_allocator(allocator, kernel_cr3));

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact root captured before entering the bounded
        // proof, and every feature-owned root borrows its live kernel branch.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    if paging::read_cr3() != kernel_cr3 {
        fail("kernel_root_restore_failed");
    }
    match result {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.passed","level":"info","producer_runs":2,"messages":2,"denials":3,"coequal":true}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    if !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || !paging::protections_are_enabled()
        || !maskable_interrupts_are_disabled()
    {
        return Err("initial_control_state_invalid");
    }
    let initial = allocator.accounting();
    if initial.free == 0
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("initial_accounting_invalid");
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.ready","level":"info","circle_capacity":{},"task_capacity":{},"endpoint_capacity":{},"sigil_capacity":{},"binding_capacity":{},"lineage_capacity":{},"queue_capacity":{},"mappings_per_circle":{},"scheduler":false,"user_interrupts":false,"nmi_masked":true}}"#,
        circle_capacity,
        task_capacity,
        ipc_endpoint_capacity,
        sigil_record_capacity,
        sigil_binding_capacity,
        sigil_lineage_capacity,
        ipc_queue_capacity,
        circle_mapping_capacity
    );

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let mut sigils = control_sigil_table::new();
    let mut ipc = control_ipc_table::new();
    let receiver_circle = circles
        .create()
        .map_err(|_| "receiver_circle_create_failed")?;
    let mut receiver_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_control_receiver_start),
        ptr::addr_of!(x86_64_control_receiver_end),
    )?;
    attach_task_space(&mut circles, receiver_circle)?;
    let receiver_task = create_task(
        &mut circles,
        &mut tasks,
        receiver_circle,
        fixture_role::receiver,
        receiver_space.payload_length,
    )?;
    let endpoint = ipc
        .create_endpoint(&mut circles, &tasks, receiver_task)
        .map_err(|_| "endpoint_create_failed")?;
    let receiver_sigil = sigils
        .issue_root(
            &mut circles,
            &tasks,
            receiver_task,
            endpoint,
            sigil_rights::receive.union(sigil_rights::revoke),
        )
        .map_err(|_| "receiver_authority_create_failed")?;
    let receiver_reference = sigil_reference_words::from_reference(receiver_sigil);
    write_reference(
        receiver_space.mailbox_alias(),
        mailbox_valid_reference_offset,
        receiver_reference,
    )?;

    let mut context = control_context {
        circles,
        tasks,
        sigils,
        ipc,
        endpoint,
        receiver_sigil,
        active_sigil: None,
        current_task: None,
        current_role: fixture_role::receiver,
        phase: gate_phase::idle,
        mailbox_alias: 0,
        active_root: 0,
        accepted_sequence: None,
        delivered_message: None,
        producers: [None, None],
        copied_producer_reference: None,
    };
    let receiver_circle_snapshot = context
        .circles
        .snapshot(receiver_circle)
        .map_err(|_| "receiver_circle_snapshot_failed")?;
    let receiver_task_snapshot = context
        .tasks
        .snapshot(receiver_task)
        .map_err(|_| "receiver_task_snapshot_failed")?;
    let receiver_endpoint_snapshot = context
        .ipc
        .snapshot(endpoint)
        .map_err(|_| "receiver_endpoint_snapshot_failed")?;
    let receiver_children = receiver_circle_snapshot.children();
    let receiver_authority = context.sigils.accounting();
    if receiver_circle.generation_value() != 1
        || receiver_task.task().generation_value() != 1
        || receiver_task.circle() != receiver_circle
        || receiver_task_snapshot.identity() != receiver_task
        || receiver_endpoint_snapshot.endpoint() != endpoint
        || endpoint.generation_value() != 1
        || receiver_endpoint_snapshot.receiver() != receiver_task
        || receiver_endpoint_snapshot.depth() != 0
        || receiver_endpoint_snapshot.capacity() != ipc_queue_capacity
        || receiver_endpoint_snapshot.next_sequence() != Some(0)
        || receiver_children.tasks() != 1
        || receiver_children.endpoints() != 1
        || receiver_children.sigils() != 1
        || receiver_children.address_spaces() != 1
        || receiver_children.mappings() != circle_mapping_capacity
        || receiver_authority.bindings != 1
        || receiver_authority.records != 1
        || receiver_authority.lineages != 1
    {
        return Err("receiver_ready_state_invalid");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.receiver.ready","level":"info","circle_generation":{},"task_generation":{},"endpoint_generation":{},"mappings":{},"rights":"receive_revoke","code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}}"#,
        receiver_circle.generation_value(),
        receiver_task.task().generation_value(),
        endpoint.generation_value(),
        receiver_children.mappings()
    );

    let receiver_accounting = allocator.accounting();
    let first_pages = run_producer(
        allocator,
        kernel_cr3,
        &mut context,
        fixture_role::human,
        None,
    )?;
    if allocator.accounting() != receiver_accounting {
        return Err("human_teardown_accounting_mismatch");
    }
    if context.producers[0].is_none() || context.accepted_sequence != Some(0) {
        return Err("human_sequence_missing");
    }

    let second_pages = run_producer(
        allocator,
        kernel_cr3,
        &mut context,
        fixture_role::machine,
        Some(first_pages),
    )?;
    if second_pages != first_pages || allocator.accounting() != receiver_accounting {
        return Err("replacement_storage_mismatch");
    }
    let producer_reference = context
        .copied_producer_reference
        .ok_or("machine_reference_missing")?;

    write_reference(
        receiver_space.mailbox_alias(),
        mailbox_copied_reference_offset,
        sigil_reference_words::from_reference(producer_reference),
    )?;
    prepare_receiver_expectations(&context, receiver_space.mailbox_alias())?;
    context.current_role = fixture_role::receiver;
    context.phase = gate_phase::receiver_probe;
    context.mailbox_alias = receiver_space.mailbox_alias();
    context.active_sigil = Some(receiver_sigil);
    context.accepted_sequence = None;
    context.delivered_message = None;
    execute_task(&mut context, &receiver_space, receiver_task, kernel_cr3)?;
    let receiver_after = context
        .tasks
        .snapshot(receiver_task)
        .map_err(|_| "receiver_final_snapshot_failed")?;
    if receiver_after.state() != task_state::exited || context.phase != gate_phase::idle {
        return Err("receiver_did_not_exit");
    }

    let withdrawn = context
        .sigils
        .withdraw_task(&mut context.circles, &context.tasks, receiver_task)
        .map_err(|_| "receiver_authority_withdrawal_failed")?;
    if withdrawn.bindings() != 1 || withdrawn.revoked_lineages() != 0 {
        return Err("receiver_authority_withdrawal_mismatch");
    }
    let discarded = context
        .ipc
        .withdraw_endpoint(
            &mut context.circles,
            &context.tasks,
            receiver_task,
            endpoint,
        )
        .map_err(|_| "endpoint_withdrawal_failed")?;
    if discarded != 0 {
        return Err("endpoint_retained_messages");
    }
    release_task_space(
        allocator,
        &mut context.circles,
        receiver_circle,
        &mut receiver_space,
    )?;
    context
        .tasks
        .destroy(&mut context.circles, receiver_task)
        .map_err(|_| "receiver_task_destroy_failed")?;
    context
        .circles
        .destroy(receiver_circle)
        .map_err(|_| "receiver_circle_destroy_failed")?;

    let final_accounting = allocator.accounting();
    let circle_accounting = context.circles.accounting();
    let active_tasks = context.tasks.active_count();
    let authority_accounting = context.sigils.accounting();
    let ipc_accounting = context.ipc.accounting();
    if final_accounting != initial
        || circle_accounting.circles != 0
        || circle_accounting.tasks != 0
        || circle_accounting.endpoints != 0
        || circle_accounting.sigils != 0
        || circle_accounting.address_spaces != 0
        || circle_accounting.mappings != 0
        || active_tasks != 0
        || authority_accounting.bindings != 0
        || authority_accounting.records != 0
        || authority_accounting.lineages != 0
        || ipc_accounting.endpoints != 0
        || ipc_accounting.messages != 0
        || context.current_task.is_some()
        || context.active_sigil.is_some()
        || context.mailbox_alias != 0
        || context.active_root != 0
        || !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
    {
        return Err("final_control_state_mismatch");
    }
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    if !transition_stack_wiped {
        return Err("final_transition_stack_clear_failed");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":{},"table_owned_frames":{},"active_circles":{},"active_tasks":{},"active_mappings":{},"active_bindings":{},"active_sigils":{},"active_lineages":{},"active_endpoints":{},"queue_cleared":true,"transition_stack_wiped":true}}"#,
        final_accounting.free,
        initial.free,
        final_accounting.data_owned,
        final_accounting.table_owned,
        circle_accounting.circles,
        active_tasks,
        circle_accounting.mappings,
        authority_accounting.bindings,
        authority_accounting.records,
        authority_accounting.lineages,
        ipc_accounting.endpoints
    );
    Ok(())
}

fn run_producer(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    context: &mut control_context,
    role: fixture_role,
    replacement_pages: Option<[u64; 4]>,
) -> Result<[u64; 4], &'static str> {
    let producer_index = role.producer_index().ok_or("receiver_used_as_producer")?;
    let circle = context
        .circles
        .create()
        .map_err(|_| "producer_circle_create_failed")?;
    let mut space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_control_producer_start),
        ptr::addr_of!(x86_64_control_producer_end),
    )?;
    attach_task_space(&mut context.circles, circle)?;
    let storage_extents = space.storage_extents()?;
    if replacement_pages.is_some_and(|expected| expected != storage_extents) {
        return Err("producer_frames_not_reused");
    }
    let task = create_task(
        &mut context.circles,
        &mut context.tasks,
        circle,
        role,
        space.payload_length,
    )?;
    let reference = context
        .sigils
        .issue_root(
            &mut context.circles,
            &context.tasks,
            task,
            context.endpoint,
            sigil_rights::send,
        )
        .map_err(|_| "producer_authority_create_failed")?;
    write_reference(
        space.mailbox_alias(),
        mailbox_valid_reference_offset,
        sigil_reference_words::from_reference(reference),
    )?;
    let task_snapshot = context
        .tasks
        .snapshot(task)
        .map_err(|_| "producer_snapshot_failed")?;
    context.producers[producer_index] = Some(producer_provenance {
        task,
        principal: task_snapshot.principal(),
        origin: task_snapshot.origin(),
    });
    context.current_role = role;
    context.phase = gate_phase::producer_probe;
    context.mailbox_alias = space.mailbox_alias();
    context.active_sigil = Some(reference);
    context.accepted_sequence = None;
    context.delivered_message = None;
    context.copied_producer_reference = Some(reference);

    execute_task(context, &space, task, kernel_cr3)?;
    if context.phase != gate_phase::idle
        || context.accepted_sequence != Some(producer_index as u64)
        || context
            .tasks
            .snapshot(task)
            .map_err(|_| "producer_final_snapshot_failed")?
            .state()
            != task_state::exited
    {
        return Err("producer_did_not_exit");
    }

    let withdrawn = context
        .sigils
        .withdraw_task(&mut context.circles, &context.tasks, task)
        .map_err(|_| "producer_authority_withdrawal_failed")?;
    if withdrawn.bindings() != 1 || withdrawn.revoked_lineages() != 1 {
        return Err("producer_authority_withdrawal_failed");
    }
    let pages = storage_extents;
    release_task_space(allocator, &mut context.circles, circle, &mut space)?;
    context
        .tasks
        .destroy(&mut context.circles, task)
        .map_err(|_| "producer_task_destroy_failed")?;
    context
        .circles
        .destroy(circle)
        .map_err(|_| "producer_circle_destroy_failed")?;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.producer.withdrawn","level":"info","role":"{}","authority_invalidated":true,"root_withdrawn":true,"storage_wiped":true,"transition_stack_wiped":true}}"#,
        role.label()
    );
    Ok(pages)
}

fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: crate::identity::circle_id,
    role: fixture_role,
    code_length: usize,
) -> Result<task_identity, &'static str> {
    let code_length = u64::try_from(code_length).map_err(|_| "task_code_length_invalid")?;
    let code_end = user_code_address
        .checked_add(code_length)
        .filter(|end| *end <= user_code_address + page_size)
        .ok_or("task_code_length_invalid")?;
    let bounds = task_bounds::new(
        user_code_address,
        code_end,
        user_code_address,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("task_bounds_invalid")?;
    let principal = fixture_principal_id::new(role.principal()).ok_or("principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(circle, principal, role.origin(), bounds),
        )
        .map_err(|_| "task_create_failed")
}

fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    payload_start: *const u8,
    payload_end: *const u8,
) -> Result<task_space, &'static str> {
    let code = allocate_data(allocator, "code_frame_allocation_failed")?;
    let mailbox = allocate_data(allocator, "mailbox_frame_allocation_failed")?;
    let stack = allocate_data(allocator, "stack_frame_allocation_failed")?;
    let code_range = code.range().ok_or("code_frame_extent_missing")?;
    let mailbox_range = mailbox.range().ok_or("mailbox_frame_extent_missing")?;
    let stack_range = stack.range().ok_or("stack_frame_extent_missing")?;
    if code_range == mailbox_range
        || code_range == stack_range
        || mailbox_range == stack_range
        || !range_is_zero(code_range)
        || !range_is_zero(mailbox_range)
        || !range_is_zero(stack_range)
    {
        return Err("private_frames_not_distinct_and_clear");
    }
    // SAFETY: The allocator grants the exclusive, one-page code lease and the
    // checked immutable template is required to fit without overlap.
    let payload_length = unsafe { copy_payload(payload_start, payload_end, code_range)? };

    // SAFETY: The published allocator serializes this proof, and the callback
    // clears every exclusive page-table extent before publication.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "address_space_create_failed")?;
    root.map_owned(
        allocator,
        user_code_address,
        code,
        paging::page_permissions {
            user: true,
            writable: false,
            executable: true,
        },
    )
    .map_err(|_| "code_mapping_failed")?;
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
    .map_err(|_| "mailbox_mapping_failed")?;
    root.map_owned(
        allocator,
        user_stack_address,
        stack,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "address_space_validation_failed")?;
    validate_mapping(
        root.validate_mapping(user_code_address)
            .map_err(|_| "code_mapping_walk_failed")?,
        code_range.base,
        false,
        true,
    )?;
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "mailbox_mapping_walk_failed")?,
        mailbox_range.base,
        true,
        false,
    )?;
    validate_mapping(
        root.validate_mapping(user_stack_address)
            .map_err(|_| "stack_mapping_walk_failed")?,
        stack_range.base,
        true,
        false,
    )?;
    for guard in [
        user_window_address,
        user_window_address + 0x2000,
        user_window_address + 0x4000,
        user_window_address + 0x6000,
    ] {
        if root.validate_absent(guard).is_err() {
            return Err("guard_mapping_present");
        }
    }
    if !payload_matches(payload_start, payload_length, code_range.base) {
        return Err("payload_copy_mismatch");
    }
    Ok(task_space {
        root,
        physical_pages: [code_range.base, mailbox_range.base, stack_range.base],
        payload_length,
    })
}

fn attach_task_space(
    circles: &mut circle_table,
    circle: crate::identity::circle_id,
) -> Result<(), &'static str> {
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "circle_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "circle_root_attachment_rollback_failed")?;
        return Err("circle_mapping_attachment_failed");
    }
    Ok(())
}

fn allocate_data<'allocator>(
    allocator: &mut frame_allocator<'allocator>,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator is the sole authority for the callback range and
    // the fixed clearer consumes and verifies the complete extent now.
    unsafe {
        allocator.allocate_with(1, 1, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

unsafe fn copy_payload(
    start: *const u8,
    end: *const u8,
    destination: physical_range,
) -> Result<usize, &'static str> {
    let length = (end as usize)
        .checked_sub(start as usize)
        .ok_or("payload_extent_invalid")?;
    if length == 0 || length > page_size as usize || destination.length != page_size {
        return Err("payload_extent_invalid");
    }
    let destination =
        usize::try_from(destination.base).map_err(|_| "payload_destination_invalid")? as *mut u8;
    // SAFETY: The caller supplies an exclusive one-page allocator lease; the
    // checked immutable linker range fits wholly and cannot overlap it.
    unsafe { ptr::copy_nonoverlapping(start, destination, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(length)
}

fn payload_matches(start: *const u8, length: usize, destination: u64) -> bool {
    let Ok(destination) = usize::try_from(destination) else {
        return false;
    };
    for offset in 0..length {
        // SAFETY: The checked template and allocator-owned destination both
        // cover `length` initialized bytes and remain live for this comparison.
        let expected = unsafe { ptr::read(start.add(offset)) };
        let actual = unsafe { ptr::read_volatile((destination as *const u8).add(offset)) };
        if actual != expected {
            return false;
        }
    }
    true
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
        return Err("mapping_permissions_invalid");
    }
    Ok(())
}

fn execute_task(
    context: &mut control_context,
    space: &task_space,
    task: task_identity,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    // SAFETY: Only this single-BSP feature path writes these continuation
    // cells, and no gate can run before the context is published below.
    let saved_stack_clear = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) == 0 };
    // SAFETY: The same exclusive bootstrap interval protects the TS cell.
    let saved_task_switched_clear =
        unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) == 0 };
    if context.current_task.is_some()
        || context.mailbox_alias == 0
        || context.active_sigil.is_none()
        || !active_context.load(Ordering::Acquire).is_null()
        || space.payload_length == 0
        || !saved_stack_clear
        || !saved_task_switched_clear
    {
        return Err("task_entry_state_invalid");
    }
    context.tasks.start(task).map_err(|_| "task_start_failed")?;
    context.current_task = Some(task);
    context.active_root = space
        .root
        .root_address()
        .map_err(|_| "task_root_extent_missing")?;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The bootstrap processor is the only writer and the gate cannot
    // run until after the following context publication and IRETQ.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(saved_task_switched),
            original_task_switched,
        )
    };
    // SAFETY: The root has exactly three validated mappings and borrows the
    // still-live kernel branch; interrupts and NMI remain disabled.
    unsafe { space.root.activate() }.map_err(|_| "task_root_activation_failed")?;
    let context_pointer = ptr::from_mut(context);
    if active_context
        .compare_exchange(
            ptr::null_mut(),
            context_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        // SAFETY: Activation succeeded but user entry did not occur, so the
        // original kernel root can be selected immediately.
        unsafe { paging::write_cr3(kernel_cr3) };
        return Err("context_publication_failed");
    }

    // SAFETY: The current task, root, selectors, fixed mailbox, TSS RSP0, and
    // feature gate were all validated before this single-BSP IRETQ.
    let status = unsafe {
        x86_64_enter_control_task(
            user_code_address,
            user_stack_top,
            u64::from(descriptors::user_data_selector),
            u64::from(descriptors::user_code_selector),
            ptr::addr_of_mut!(saved_kernel_stack),
        )
    };

    active_context.store(ptr::null_mut(), Ordering::Release);
    let task_switched_restored = paging::read_cr0() & cr0_task_switched == original_task_switched;
    // SAFETY: The completion trampoline restored the original Rust stack; this
    // exact kernel root must be active before any owned mapping is withdrawn.
    unsafe { paging::write_cr3(kernel_cr3) };
    let kernel_root_restored = paging::read_cr3() == kernel_cr3;
    context.current_task = None;
    context.mailbox_alias = 0;
    context.active_root = 0;
    context.active_sigil = None;
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    // SAFETY: The nonlocal completion consumed both feature-local values and
    // no later path may reuse the abandoned continuation.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(saved_task_switched), 0);
    }
    compiler_fence(Ordering::SeqCst);
    if status != status_completed
        || !task_switched_restored
        || !kernel_root_restored
        || !transition_stack_wiped
        || gate_active.load(Ordering::Acquire)
    {
        return Err("task_completion_restore_failed");
    }
    Ok(())
}

fn release_task_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: crate::identity::circle_id,
    space: &mut task_space,
) -> Result<(), &'static str> {
    let mut code = space
        .root
        .unmap_owned(allocator, user_code_address)
        .map_err(|_| "code_mapping_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "mailbox_mapping_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "stack_mapping_withdrawal_failed")?;
    // SAFETY: The root is inactive, owns no mappings, and its table frames are
    // exclusive to this allocator-authorized address-space object.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "address_space_destroy_failed")?;
    // SAFETY: Unmapping returned the three exclusive, unbound data leases.
    unsafe { allocator.release_with(&mut code, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "code_frame_release_failed")?;
    // SAFETY: The mailbox lease is likewise inactive and unbound.
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "mailbox_frame_release_failed")?;
    // SAFETY: The stack lease is likewise inactive and unbound.
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "stack_frame_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "circle_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "circle_root_detachment_failed")?;
    Ok(())
}

impl task_space {
    const fn mailbox_alias(&self) -> u64 {
        self.physical_pages[1]
    }

    fn storage_extents(&self) -> Result<[u64; 4], &'static str> {
        Ok([
            self.physical_pages[0],
            self.physical_pages[1],
            self.physical_pages[2],
            self.root
                .root_address()
                .map_err(|_| "task_root_extent_missing")?,
        ])
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_control_gate_dispatch(frame: *mut control_gate_frame) {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<u64>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("gate_reentry_or_frame_invalid");
    }
    let context_pointer = active_context.load(Ordering::Acquire);
    if context_pointer.is_null() {
        fail("gate_without_active_context");
    }
    // SAFETY: Publication occurs only for the unique live stack context before
    // IRETQ. IF is clear, NMI is masked, and the reentry latch grants the only
    // mutable access until this dispatch completes or abandons the frame.
    let context = unsafe { &mut *context_pointer };
    // SAFETY: The assembly entry supplies exactly 22 naturally aligned qwords
    // whose lifetime covers this call; the latch makes the borrow exclusive.
    let frame = unsafe { &mut *frame };
    if !valid_gate_frame(context, frame) {
        fail("gate_frame_validation_failed");
    }

    let action = dispatch_gate(context, frame);
    match action {
        Ok(gate_action::return_to_user(status)) => {
            frame.accumulator = status;
            compiler_fence(Ordering::SeqCst);
            gate_active.store(false, Ordering::Release);
        }
        Ok(gate_action::complete) => {
            // SAFETY: These values are written once immediately before IRETQ
            // and remain feature-private until exact completion consumes them.
            let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
            // SAFETY: The same single-BSP publication rule applies to TS.
            let original_task_switched =
                unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
            if saved_stack == 0
                || saved_stack & (mem::align_of::<u64>() as u64 - 1) != 0
                || !matches!(original_task_switched, 0 | cr0_task_switched)
                || paging::read_cr0() & cr0_task_switched == 0
            {
                fail("completion_continuation_invalid");
            }
            gate_active.store(false, Ordering::Release);
            active_context.store(ptr::null_mut(), Ordering::Release);
            // SAFETY: Exact completion is the only path that consumes this
            // single-use continuation. The trampoline restores CR0.TS and the
            // saved Rust stack while abandoning every user/TSS frame.
            unsafe {
                x86_64_resume_control_task(status_completed, saved_stack, original_task_switched)
            }
        }
        Err(reason) => fail(reason),
    }
}

enum gate_action {
    return_to_user(u64),
    complete,
}

fn valid_gate_frame(context: &control_context, frame: &control_gate_frame) -> bool {
    let Some(task) = context.current_task else {
        return false;
    };
    let Ok(snapshot) = context.tasks.snapshot(task) else {
        return false;
    };
    let bounds = snapshot.bounds();
    snapshot.state() == task_state::running
        && frame.vector == control_gate_vector
        && frame.error_code == 0
        && frame.code_segment == u64::from(descriptors::user_code_selector)
        && frame.stack_segment == u64::from(descriptors::user_data_selector)
        && frame.flags == sanitized_flags
        && bounds.contains_code(frame.instruction_pointer)
        && expected_gate_instruction_pointer(context) == Some(frame.instruction_pointer)
        && bounds.contains_stack_pointer(frame.stack_pointer)
        && frame.stack_pointer == bounds.initial_stack_pointer()
        && frame.frame_pointer == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
        && frame.register_15 == user_mailbox_address
        && context.mailbox_alias != 0
        && context.active_root != 0
        && paging::entry_address(paging::read_cr3()) == context.active_root
        && frame.base <= u64::from(u32::MAX)
        && frame.data <= u64::from(u32::MAX)
        && phase_indexes_are_valid(context.phase, frame.source_index, frame.destination_index)
}

fn expected_gate_instruction_pointer(context: &control_context) -> Option<u64> {
    let (template, symbol) = match context.phase {
        gate_phase::producer_probe => (
            ptr::addr_of!(x86_64_control_producer_start),
            ptr::addr_of!(x86_64_control_producer_probe_return),
        ),
        gate_phase::producer_send => (
            ptr::addr_of!(x86_64_control_producer_start),
            ptr::addr_of!(x86_64_control_producer_send_return),
        ),
        gate_phase::producer_complete => (
            ptr::addr_of!(x86_64_control_producer_start),
            ptr::addr_of!(x86_64_control_producer_complete_return),
        ),
        gate_phase::receiver_probe => (
            ptr::addr_of!(x86_64_control_receiver_start),
            ptr::addr_of!(x86_64_control_receiver_probe_return),
        ),
        gate_phase::receiver_first => (
            ptr::addr_of!(x86_64_control_receiver_start),
            ptr::addr_of!(x86_64_control_receiver_first_receive_return),
        ),
        gate_phase::receiver_second => (
            ptr::addr_of!(x86_64_control_receiver_start),
            ptr::addr_of!(x86_64_control_receiver_second_receive_return),
        ),
        gate_phase::receiver_empty => (
            ptr::addr_of!(x86_64_control_receiver_start),
            ptr::addr_of!(x86_64_control_receiver_empty_return),
        ),
        gate_phase::receiver_complete => (
            ptr::addr_of!(x86_64_control_receiver_start),
            ptr::addr_of!(x86_64_control_receiver_complete_return),
        ),
        gate_phase::idle => return None,
    };
    let offset = (symbol as usize).checked_sub(template as usize)?;
    (offset < page_size as usize).then_some(user_code_address + offset as u64)
}

const fn phase_indexes_are_valid(phase: gate_phase, source: u64, destination: u64) -> bool {
    match phase {
        gate_phase::producer_probe
        | gate_phase::producer_send
        | gate_phase::producer_complete
        | gate_phase::receiver_probe
        | gate_phase::receiver_first => source == 0 && destination == 0,
        gate_phase::receiver_second => {
            source == user_mailbox_address + mailbox_expected_second_offset as u64
                && destination
                    == user_mailbox_address
                        + mailbox_transfer_offset as u64
                        + (encoded_message_word_count * mem::size_of::<u64>()) as u64
        }
        gate_phase::receiver_empty | gate_phase::receiver_complete => {
            source
                == user_mailbox_address
                    + mailbox_expected_second_offset as u64
                    + (encoded_message_word_count * mem::size_of::<u64>()) as u64
                && destination
                    == user_mailbox_address
                        + mailbox_transfer_offset as u64
                        + (encoded_message_word_count * mem::size_of::<u64>()) as u64
        }
        gate_phase::idle => false,
    }
}

fn maskable_interrupts_are_disabled() -> bool {
    let flags: u64;
    // SAFETY: Reading RFLAGS is non-mutating and this proof runs at CPL 0.
    unsafe {
        asm!(
            "pushfq",
            "pop {}",
            out(reg) flags,
            options(preserves_flags)
        );
    }
    flags & (1 << 9) == 0
}

fn dispatch_gate(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    match (context.phase, frame.accumulator) {
        (gate_phase::producer_probe, operation_probe) => producer_probe(context, frame),
        (gate_phase::producer_send, operation_send) => producer_send(context, frame),
        (gate_phase::producer_complete, operation_complete) => producer_complete(context, frame),
        (gate_phase::receiver_probe, operation_probe) => receiver_probe(context, frame),
        (gate_phase::receiver_first, operation_receive) => receiver_receive(context, frame, 0),
        (gate_phase::receiver_second, operation_receive) => receiver_receive(context, frame, 1),
        (gate_phase::receiver_empty, operation_receive) => receiver_empty(context, frame),
        (gate_phase::receiver_complete, operation_complete) => receiver_complete(context, frame),
        _ => Err("unexpected_gate_operation"),
    }
}

fn producer_probe(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    let task = context.current_task.ok_or("probe_without_current_task")?;
    if frame_reference(frame)
        != (sigil_reference_words {
            slot: 0,
            generation: 0,
            tag: 0,
        })
    {
        return Err("invalid_reference_not_zero");
    }
    let before = context
        .ipc
        .snapshot(context.endpoint)
        .map_err(|_| "producer_probe_snapshot_failed")?;
    if context.sigils.authorize_endpoint(
        &context.tasks,
        task,
        frame_reference(frame).decode()?,
        context.endpoint,
        sigil_rights::send,
    ) != Err(sigil_error::invalid_reference)
        || context.ipc.snapshot(context.endpoint) != Ok(before)
    {
        return Err("invalid_reference_changed_state");
    }
    emit_producer_entered(context)?;
    context.phase = gate_phase::producer_send;
    Ok(gate_action::return_to_user(status_denied))
}

fn emit_producer_entered(context: &control_context) -> Result<(), &'static str> {
    let task = context.current_task.ok_or("entry_without_current_task")?;
    let snapshot = context
        .tasks
        .snapshot(task)
        .map_err(|_| "producer_entry_snapshot_failed")?;
    let producer_index = context
        .current_role
        .producer_index()
        .ok_or("receiver_entered_as_producer")?;
    let expected_generation = producer_index as u64 + 1;
    if snapshot.task().generation_value() != expected_generation
        || snapshot.circle().generation_value() != expected_generation
        || snapshot.origin() != context.current_role.origin()
    {
        return Err("producer_replacement_generation_invalid");
    }
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.producer.entered","level":"info","role":"{}","circle_generation":{},"task_generation":{},"origin":"user","mappings":3,"code_identical":true,"storage_zero":true,"replacement":{},"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}}"#,
        context.current_role.label(),
        snapshot.circle().generation_value(),
        snapshot.task().generation_value(),
        context.current_role == fixture_role::machine
    );
    Ok(())
}

fn producer_send(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    let task = context.current_task.ok_or("send_without_current_task")?;
    let sigil = require_active_reference(context, frame)?;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.authority.denied","level":"info","role":"{}","reason":"invalid_reference","state_unchanged":true}}"#,
        context.current_role.label()
    );
    let payload = payload_from_mailbox(context.mailbox_alias)?;
    let sequence = context
        .ipc
        .send_authorized(
            &context.tasks,
            &context.sigils,
            task,
            sigil,
            context.endpoint,
            payload,
        )
        .map_err(|_| "message_send_failed")?;
    let producer_index = context
        .current_role
        .producer_index()
        .ok_or("receiver_attempted_send")?;
    if sequence != producer_index as u64 {
        return Err("message_sequence_invalid");
    }
    context.accepted_sequence = Some(sequence);
    context.phase = gate_phase::producer_complete;
    Ok(gate_action::return_to_user(status_accepted))
}

fn producer_complete(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    require_active_reference(context, frame)?;
    let task = context
        .current_task
        .ok_or("completion_without_current_task")?;
    let sequence = context
        .accepted_sequence
        .ok_or("send_return_not_observed")?;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.message.sent","level":"info","role":"{}","sequence":{},"attribution":"kernel","returned_to_user":true}}"#,
        context.current_role.label(),
        sequence
    );
    context
        .tasks
        .exit(task)
        .map_err(|_| "producer_exit_failed")?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.producer.exited","level":"info","role":"{}","state":"exited"}}"#,
        context.current_role.label()
    );
    context.phase = gate_phase::idle;
    Ok(gate_action::complete)
}

fn receiver_probe(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    let task = context.current_task.ok_or("probe_without_current_task")?;
    let copied = frame_reference(frame).decode()?;
    if Some(copied) != context.copied_producer_reference {
        return Err("copied_reference_mismatch");
    }
    let before = context
        .ipc
        .snapshot(context.endpoint)
        .map_err(|_| "receiver_probe_snapshot_failed")?;
    if context.sigils.authorize_endpoint(
        &context.tasks,
        task,
        copied,
        context.endpoint,
        sigil_rights::send,
    ) != Err(sigil_error::stale_reference)
        || context.ipc.snapshot(context.endpoint) != Ok(before)
    {
        return Err("cross_namespace_reference_accepted");
    }
    let snapshot = context
        .tasks
        .snapshot(task)
        .map_err(|_| "receiver_entry_snapshot_failed")?;
    if snapshot.task().generation_value() != 1
        || snapshot.circle().generation_value() != 1
        || snapshot.origin() != synthetic_origin_metadata::receiver_role_fixture
    {
        return Err("receiver_identity_invalid");
    }
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.receiver.entered","level":"info","circle_generation":{},"task_generation":{},"origin":"user","mappings":3}}"#,
        snapshot.circle().generation_value(),
        snapshot.task().generation_value()
    );
    context.phase = gate_phase::receiver_first;
    Ok(gate_action::return_to_user(status_denied))
}

fn receiver_receive(
    context: &mut control_context,
    frame: &control_gate_frame,
    expected_index: usize,
) -> Result<gate_action, &'static str> {
    if let Some(previous) = context.delivered_message.take() {
        emit_received(
            previous,
            expected_index
                .checked_sub(1)
                .ok_or("receive_phase_invalid")?,
        )?;
    } else if expected_index != 0 {
        return Err("previous_receive_return_not_observed");
    }
    if expected_index == 0 {
        let mut serial = serial_port::com1();
        serial.initialize();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.control_test.authority.denied","level":"info","role":"receiver_fixture","reason":"cross_namespace","state_unchanged":true}}"#
        );
    }
    let task = context.current_task.ok_or("receive_without_current_task")?;
    let sigil = require_active_reference(context, frame)?;
    let expected = context.producers[expected_index].ok_or("expected_producer_missing")?;
    let endpoint = context.endpoint;
    let mailbox = context.mailbox_alias;
    let message = context
        .ipc
        .receive_authorized_with(
            &context.tasks,
            &context.sigils,
            task,
            sigil,
            endpoint,
            |message| {
                validate_received_message(endpoint, expected, message, expected_index).is_ok()
                    && mailbox_words_fit(
                        mailbox,
                        mailbox_transfer_offset,
                        encoded_message_word_count,
                    )
            },
            |message| {
                if write_received_message(mailbox, *message).is_err() {
                    fail("received_message_commit_failed");
                }
            },
        )
        .map_err(|_| "message_receive_failed")?;
    validate_received_message(endpoint, expected, &message, expected_index)?;
    context.delivered_message = Some(message);
    context.phase = if expected_index == 0 {
        gate_phase::receiver_second
    } else {
        gate_phase::receiver_empty
    };
    Ok(gate_action::return_to_user(status_accepted))
}

fn receiver_empty(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    let previous = context
        .delivered_message
        .take()
        .ok_or("second_receive_return_not_observed")?;
    emit_received(previous, 1)?;
    let task = context.current_task.ok_or("empty_without_current_task")?;
    let sigil = require_active_reference(context, frame)?;
    let before = context
        .ipc
        .snapshot(context.endpoint)
        .map_err(|_| "empty_snapshot_failed")?;
    if context.ipc.receive_authorized_with(
        &context.tasks,
        &context.sigils,
        task,
        sigil,
        context.endpoint,
        |_| true,
        |_| fail("empty_receive_committed_message"),
    ) != Err(ipc_authorized_error::ipc(ipc_error::would_block))
        || context.ipc.snapshot(context.endpoint) != Ok(before)
        || before.depth() != 0
    {
        return Err("empty_receive_not_atomic");
    }
    context.phase = gate_phase::receiver_complete;
    Ok(gate_action::return_to_user(status_would_block))
}

fn receiver_complete(
    context: &mut control_context,
    frame: &control_gate_frame,
) -> Result<gate_action, &'static str> {
    let task = context
        .current_task
        .ok_or("receiver_completion_without_task")?;
    let sigil = require_active_reference(context, frame)?;
    if sigil != context.receiver_sigil {
        return Err("revocation_reference_mismatch");
    }
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.queue.empty","level":"info","status":"would_block","depth":0,"state_unchanged":true}}"#
    );
    let revoked = context
        .sigils
        .revoke_lineage(&context.tasks, task, sigil, context.endpoint)
        .map_err(|_| "lineage_revocation_failed")?;
    if revoked != (sigil_revocation::revoked { bindings: 1 })
        || context.sigils.authorize_endpoint(
            &context.tasks,
            task,
            sigil,
            context.endpoint,
            sigil_rights::receive,
        ) != Err(sigil_error::revoked)
        || context
            .sigils
            .revoke_lineage(&context.tasks, task, sigil, context.endpoint)
            != Ok(sigil_revocation::already_revoked { bindings: 1 })
    {
        return Err("lineage_revocation_boundary_failed");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.lineage.revoked","level":"info","revoked":true,"next_lookup_denied":true,"repeated_result":"already_revoked"}}"#
    );
    context
        .tasks
        .exit(task)
        .map_err(|_| "receiver_exit_failed")?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.receiver.exited","level":"info","state":"exited","returned_from_user":true}}"#
    );
    context.phase = gate_phase::idle;
    Ok(gate_action::complete)
}

fn frame_reference(frame: &control_gate_frame) -> sigil_reference_words {
    sigil_reference_words {
        slot: frame.base,
        generation: frame.counter,
        tag: frame.data,
    }
}

fn require_active_reference(
    context: &control_context,
    frame: &control_gate_frame,
) -> Result<sigil_reference, &'static str> {
    let reference = frame_reference(frame).decode()?;
    if Some(reference) != context.active_sigil {
        return Err("active_reference_mismatch");
    }
    Ok(reference)
}

fn payload_from_mailbox(mailbox: u64) -> Result<ipc_payload, &'static str> {
    let first = read_mailbox_word(mailbox, mailbox_transfer_offset)?;
    let second = read_mailbox_word(mailbox, mailbox_transfer_offset + 8)?;
    if first != private_payload_first || second != private_payload_second {
        return Err("fixed_message_body_invalid");
    }
    ipc_payload::copy_from(&private_payload_bytes).map_err(|_| "fixed_message_copy_failed")
}

fn validate_received_message(
    endpoint: endpoint_id,
    expected: producer_provenance,
    message: &ipc_message,
    expected_index: usize,
) -> Result<(), &'static str> {
    let provenance = message.provenance();
    if message.endpoint() != endpoint
        || message.sequence() != expected_index as u64
        || provenance.task() != expected.task
        || provenance.principal() != expected.principal
        || provenance.origin() != expected.origin
        || message.payload().as_bytes() != private_payload_bytes
    {
        return Err("received_message_invalid");
    }
    Ok(())
}

fn emit_received(message: ipc_message, expected_index: usize) -> Result<(), &'static str> {
    if message.sequence() != expected_index as u64 {
        return Err("returned_message_sequence_invalid");
    }
    let source_role = if expected_index == 0 {
        fixture_role::human.label()
    } else {
        fixture_role::machine.label()
    };
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.message.received","level":"info","source_role":"{}","sequence":{},"fifo":true,"body_equal":true,"attribution_verified":true}}"#,
        source_role, expected_index
    );
    Ok(())
}

fn prepare_receiver_expectations(
    context: &control_context,
    mailbox: u64,
) -> Result<(), &'static str> {
    for (index, offset) in [
        mailbox_expected_first_offset,
        mailbox_expected_second_offset,
    ]
    .into_iter()
    .enumerate()
    {
        let producer = context.producers[index].ok_or("producer_provenance_missing")?;
        let encoded = encode_message(
            index as u64,
            producer.task,
            producer.principal,
            producer.origin,
        );
        write_mailbox_words(mailbox, offset, &encoded)?;
    }
    Ok(())
}

fn write_received_message(mailbox: u64, message: ipc_message) -> Result<(), &'static str> {
    let provenance = message.provenance();
    let encoded = encode_message(
        message.sequence(),
        provenance.task(),
        provenance.principal(),
        provenance.origin(),
    );
    write_mailbox_words(mailbox, mailbox_transfer_offset, &encoded)
}

fn encode_message(
    sequence: u64,
    task: task_identity,
    principal: fixture_principal_id,
    origin: synthetic_origin_metadata,
) -> [u64; encoded_message_word_count] {
    [
        sequence,
        task.task().slot() as u64,
        task.task().generation_value(),
        task.circle().slot() as u64,
        task.circle().generation_value(),
        principal.value(),
        origin_code(origin),
        private_payload_first,
        private_payload_second,
    ]
}

const fn origin_code(origin: synthetic_origin_metadata) -> u64 {
    match origin {
        synthetic_origin_metadata::human_role_fixture => 1,
        synthetic_origin_metadata::machine_role_fixture => 2,
        synthetic_origin_metadata::receiver_role_fixture => 3,
    }
}

fn write_reference(
    mailbox: u64,
    offset: usize,
    reference: sigil_reference_words,
) -> Result<(), &'static str> {
    write_mailbox_words(
        mailbox,
        offset,
        &[reference.slot, reference.generation, reference.tag],
    )
}

fn write_mailbox_words(mailbox: u64, offset: usize, words: &[u64]) -> Result<(), &'static str> {
    if !mailbox_words_fit(mailbox, offset, words.len()) {
        return Err("mailbox_write_out_of_bounds");
    }
    let base = usize::try_from(mailbox).map_err(|_| "mailbox_alias_invalid")?;
    for (index, word) in words.iter().enumerate() {
        // SAFETY: The fixed offset/length preflight bounds this naturally
        // aligned write to the allocator-owned mailbox's physical alias.
        unsafe {
            ptr::write_volatile(
                (base as *mut u64).add(offset / mem::size_of::<u64>() + index),
                *word,
            )
        };
    }
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

fn mailbox_words_fit(mailbox: u64, offset: usize, word_count: usize) -> bool {
    let Some(end) = word_count
        .checked_mul(mem::size_of::<u64>())
        .and_then(|length| offset.checked_add(length))
    else {
        return false;
    };
    usize::try_from(mailbox).is_ok()
        && mailbox & (mem::align_of::<u64>() as u64 - 1) == 0
        && offset.is_multiple_of(mem::align_of::<u64>())
        && end <= page_size as usize
}

fn read_mailbox_word(mailbox: u64, offset: usize) -> Result<u64, &'static str> {
    let end = offset
        .checked_add(mem::size_of::<u64>())
        .ok_or("mailbox_read_overflow")?;
    if !offset.is_multiple_of(mem::align_of::<u64>()) || end > page_size as usize {
        return Err("mailbox_read_out_of_bounds");
    }
    let base = usize::try_from(mailbox).map_err(|_| "mailbox_alias_invalid")?;
    // SAFETY: The fixed word lies in the live allocator-owned mailbox alias.
    Ok(unsafe { ptr::read_volatile((base as *const u64).add(offset / 8)) })
}

fn range_is_zero(range: physical_range) -> bool {
    let Some(end) = range.end() else {
        return false;
    };
    if range.length != page_size || range.base & (page_size - 1) != 0 || end > memory::address_limit
    {
        return false;
    }
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    for offset in 0..page_size as usize {
        // SAFETY: The allocator-owned one-page extent is identity mapped and
        // remains exclusively borrowed throughout this validation.
        if unsafe { ptr::read_volatile((base as *const u8).add(offset)) } != 0 {
            return false;
        }
    }
    true
}

fn fail(reason: &'static str) -> ! {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.control_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
