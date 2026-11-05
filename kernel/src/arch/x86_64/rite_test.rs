use anomalous_abi::{
    id_128, rite_call, rite_exit_payload, rite_operation, rite_payload_kind, rite_reply,
    rite_rights, rite_status,
};
use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_exit_payload, payload_span_fits,
    read_call, reply_size, rite_scratch, scratch_is_zero, transport_payload_limit, write_reply,
};
use super::{descriptors, paging};
use crate::circle::{circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::{screen_console, serial_port};
use crate::identity::{circle_id, fixture_principal_id, task_identity};
use crate::input;
use crate::memory;
use crate::memory::physical::{frame_allocator, frame_lease, frame_purpose, physical_range};
use crate::rite::{rite_admission, rite_channel, rite_decision};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};

const page_size: u64 = 4096;
const user_window_address: u64 = 0x0000_6000_0000_0000;
const user_code_address: u64 = user_window_address + 0x1000;
const user_mailbox_address: u64 = user_window_address + 0x3000;
const user_stack_address: u64 = user_window_address + 0x5000;
const user_stack_top: u64 = user_window_address + 0x6000;

const rite_gate_vector: u64 = 0xf3;
const sanitized_flags: u64 = 0x2;
const cr0_task_switched: u64 = 1 << 3;
const status_completed: u64 = 0x40;
const call_count: usize = 9;
const returning_call_count: usize = call_count - 1;
const private_console_text: [u8; 16] = *b"NOX7Q9M4R2X6P8ZK";
const invalid_text: [u8; 2] = [0xc3, 0x28];
const expected_exit_code: i32 = 42;
const expected_body_copies: usize = 4;
const expected_body_bytes: usize =
    invalid_text.len() + private_console_text.len() + 2 * mem::size_of::<rite_exit_payload>();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum gate_phase {
    unknown,
    missing_console,
    display_overclaim,
    transport_limit,
    invalid_text,
    console_write,
    input_empty,
    invalid_exit,
    task_exit,
    idle,
}

impl gate_phase {
    const fn index(self) -> Option<usize> {
        match self {
            Self::unknown => Some(0),
            Self::missing_console => Some(1),
            Self::display_overclaim => Some(2),
            Self::transport_limit => Some(3),
            Self::invalid_text => Some(4),
            Self::console_write => Some(5),
            Self::input_empty => Some(6),
            Self::invalid_exit => Some(7),
            Self::task_exit => Some(8),
            Self::idle => None,
        }
    }

    const fn sequence(self) -> Option<u64> {
        match self.index() {
            Some(index) => Some(index as u64 + 1),
            None => None,
        }
    }

    const fn trace(self) -> Option<id_128> {
        let byte = match self {
            Self::unknown => 0x11,
            Self::missing_console => 0x22,
            Self::display_overclaim => 0x33,
            Self::transport_limit => 0x44,
            Self::invalid_text => 0x55,
            Self::console_write => 0x66,
            Self::input_empty => 0x77,
            Self::invalid_exit => 0x88,
            Self::task_exit => 0x99,
            Self::idle => return None,
        };
        Some(id_128::from_bytes([byte; 16]))
    }

    const fn expected_status(self) -> Option<rite_status> {
        match self {
            Self::unknown => Some(rite_status::bad_operation),
            Self::missing_console | Self::display_overclaim => Some(rite_status::denied),
            Self::transport_limit => Some(rite_status::limit_exceeded),
            Self::invalid_text | Self::invalid_exit => Some(rite_status::bad_payload),
            Self::console_write | Self::task_exit => Some(rite_status::ok),
            Self::input_empty => Some(rite_status::would_block),
            Self::idle => None,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::unknown => Some(Self::missing_console),
            Self::missing_console => Some(Self::display_overclaim),
            Self::display_overclaim => Some(Self::transport_limit),
            Self::transport_limit => Some(Self::invalid_text),
            Self::invalid_text => Some(Self::console_write),
            Self::console_write => Some(Self::input_empty),
            Self::input_empty => Some(Self::invalid_exit),
            Self::invalid_exit => Some(Self::task_exit),
            Self::task_exit | Self::idle => None,
        }
    }
}

#[derive(Clone, Copy)]
struct rite_observation {
    sequence: u64,
    status: rite_status,
}

impl rite_observation {
    const fn empty() -> Self {
        Self {
            sequence: 0,
            status: rite_status::faulted,
        }
    }
}

struct rite_context {
    circles: circle_table,
    tasks: task_table,
    channel: Option<rite_channel>,
    screen: screen_console,
    current_task: Option<task_identity>,
    phase: gate_phase,
    mailbox_alias: u64,
    active_root: u64,
    transition_stack_top: u64,
    scratch: rite_scratch,
    console_copy: [u8; private_console_text.len()],
    observations: [rite_observation; call_count],
    observation_count: usize,
    body_copy_count: usize,
    body_bytes_copied: usize,
    reply_copy_count: usize,
    console_bytes_committed: usize,
    exit_code: Option<i32>,
    terminal_reply_copied: bool,
}

struct task_space {
    root: address_space,
    physical_pages: [u64; 3],
    payload_length: usize,
}

impl task_space {
    const fn mailbox_alias(&self) -> u64 {
        self.physical_pages[1]
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct rite_gate_frame {
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

const _: () = {
    assert!(call_size == 40);
    assert!(reply_size == 40);
    assert!(transport_payload_limit == 4056);
    assert!(mem::size_of::<rite_gate_frame>() == 22 * mem::size_of::<u64>());
};

static active_context: AtomicPtr<rite_context> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);
static mut saved_kernel_stack: u64 = 0;
static mut saved_task_switched: u64 = 0;

unsafe extern "C" {
    fn x86_64_enter_rite_task(
        instruction_pointer: u64,
        stack_pointer: u64,
        data_selector: u64,
        code_selector: u64,
        saved_stack: *mut u64,
    ) -> u64;
    fn x86_64_resume_rite_task(status: u64, saved_stack: u64, original_task_switched: u64) -> !;

    static x86_64_rite_test_user_start: u8;
    static x86_64_rite_test_unknown_return: u8;
    static x86_64_rite_test_missing_console_return: u8;
    static x86_64_rite_test_display_overclaim_return: u8;
    static x86_64_rite_test_transport_limit_return: u8;
    static x86_64_rite_test_invalid_utf8_return: u8;
    static x86_64_rite_test_console_return: u8;
    static x86_64_rite_test_input_return: u8;
    static x86_64_rite_test_malformed_exit_return: u8;
    static x86_64_rite_test_exit_return: u8;
    static x86_64_rite_test_user_end: u8;
}

pub(super) fn run(mut screen: screen_console) -> ! {
    screen.initialize();
    let kernel_cr3 = paging::read_cr3();
    let result = memory::with_frame_allocator(move |allocator| {
        run_with_allocator(allocator, kernel_cr3, screen)
    });

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact root captured before the bounded proof, and
        // every feature-owned root borrows its still-live supervisor branch.
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
        r#"{{"schema":1,"event":"kernel.rite_test.passed","level":"info","calls":9,"rejections":6,"commits":2,"would_block":1,"terminal":true}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    screen: screen_console,
) -> Result<(), &'static str> {
    if !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || !paging::protections_are_enabled()
        || !maskable_interrupts_are_disabled()
    {
        return Err("initial_rite_state_invalid");
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
        r#"{{"schema":1,"event":"kernel.rite_test.ready","level":"info","arch":"x86_64","vector":243,"grant":{},"transport_limit":{}}}"#,
        rite_rights::shell_minimum,
        transport_payload_limit
    );

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let circle = circles.create().map_err(|_| "rite_circle_create_failed")?;
    let mut space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_rite_test_user_start),
        ptr::addr_of!(x86_64_rite_test_user_end),
    )?;
    attach_task_space(&mut circles, circle)?;
    let task = create_task(&mut circles, &mut tasks, circle, space.payload_length)?;
    let channel = rite_channel::try_new(task, rite_rights::shell_minimum)
        .map_err(|_| "rite_channel_create_failed")?;

    let task_snapshot = tasks
        .snapshot(task)
        .map_err(|_| "rite_task_snapshot_failed")?;
    let circle_snapshot = circles
        .snapshot(circle)
        .map_err(|_| "rite_circle_snapshot_failed")?;
    let children = circle_snapshot.children();
    if task_snapshot.identity() != task
        || task_snapshot.state() != task_state::ready
        || task_snapshot.circle() != circle
        || task_snapshot.bounds().entry() != user_code_address
        || circle_snapshot.identity() != circle
        || children.tasks() != 1
        || children.address_spaces() != 1
        || children.mappings() != circle_mapping_capacity
        || children.total() != u32::from(2 + circle_mapping_capacity)
    {
        return Err("rite_task_ready_state_invalid");
    }

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.task.entered","level":"info","origin":"user","mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}}"#
    );

    let mut context = rite_context {
        circles,
        tasks,
        channel: Some(channel),
        screen,
        current_task: None,
        phase: gate_phase::unknown,
        mailbox_alias: space.mailbox_alias(),
        active_root: 0,
        transition_stack_top: 0,
        scratch: [0; page_size as usize],
        console_copy: [0; private_console_text.len()],
        observations: [rite_observation::empty(); call_count],
        observation_count: 0,
        body_copy_count: 0,
        body_bytes_copied: 0,
        reply_copy_count: 0,
        console_bytes_committed: 0,
        exit_code: None,
        terminal_reply_copied: false,
    };

    execute_task(&mut context, &space, task, kernel_cr3)?;

    let exited = context
        .tasks
        .snapshot(task)
        .map_err(|_| "rite_task_final_snapshot_failed")?;
    if exited.state() != task_state::exited
        || context.phase != gate_phase::idle
        || context.observation_count != call_count
        || context.body_copy_count != expected_body_copies
        || context.body_bytes_copied != expected_body_bytes
        || context.reply_copy_count != returning_call_count
        || context.console_bytes_committed != private_console_text.len()
        || context.console_copy != private_console_text
        || context.exit_code != Some(expected_exit_code)
        || context.terminal_reply_copied
        || !scratch_is_zero(&context.scratch)
    {
        return Err("rite_completion_evidence_invalid");
    }
    validate_observations(&context.observations)?;

    context
        .channel
        .take()
        .ok_or("rite_channel_missing_at_retirement")?;
    if !clear_and_verify_bytes(&mut context.console_copy)
        || !clear_and_verify_bytes(&mut context.scratch)
    {
        return Err("rite_private_storage_clear_failed");
    }

    release_task_space(allocator, &mut context.circles, circle, &mut space)?;
    context
        .tasks
        .destroy(&mut context.circles, task)
        .map_err(|_| "rite_task_destroy_failed")?;
    context
        .circles
        .destroy(circle)
        .map_err(|_| "rite_circle_destroy_failed")?;

    let final_accounting = allocator.accounting();
    let circle_accounting = context.circles.accounting();
    if final_accounting != initial
        || circle_accounting.circles != 0
        || circle_accounting.tasks != 0
        || circle_accounting.task_contexts != 0
        || circle_accounting.endpoints != 0
        || circle_accounting.sigils != 0
        || circle_accounting.supervisions != 0
        || circle_accounting.address_spaces != 0
        || circle_accounting.mappings != 0
        || context.tasks.active_count() != 0
        || context.channel.is_some()
        || context.current_task.is_some()
        || context.mailbox_alias != 0
        || context.active_root != 0
        || context.transition_stack_top != 0
        || !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || !scratch_is_zero(&context.scratch)
        || context.console_copy != [0; private_console_text.len()]
    {
        return Err("final_rite_state_mismatch");
    }

    emit_outcomes(&mut serial, &context)?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":{},"table_owned_frames":{},"active_circles":{},"active_tasks":{},"active_mappings":{},"channel_retired":true,"mailbox_wiped":true,"transition_stack_wiped":true}}"#,
        final_accounting.free,
        initial.free,
        final_accounting.data_owned,
        final_accounting.table_owned,
        circle_accounting.circles,
        context.tasks.active_count(),
        circle_accounting.mappings
    );
    Ok(())
}

fn emit_outcomes(serial: &mut serial_port, context: &rite_context) -> Result<(), &'static str> {
    let observations = &context.observations;
    for observation in &observations[..5] {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":{},"status":"{}","state_unchanged":true}}"#,
            observation.sequence,
            status_label(observation.status)?
        );
    }
    let console = observations[5];
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.console.committed","level":"info","sequence":{},"status":"{}","bytes":{}}}"#,
        console.sequence,
        status_label(console.status)?,
        private_console_text.len()
    );
    let input = observations[6];
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.input.empty","level":"info","sequence":{},"status":"{}","state_unchanged":true}}"#,
        input.sequence,
        status_label(input.status)?
    );
    let invalid_exit = observations[7];
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.call.rejected","level":"info","sequence":{},"status":"{}","state_unchanged":true}}"#,
        invalid_exit.sequence,
        status_label(invalid_exit.status)?
    );
    let exit = observations[8];
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.rite_test.task.exited","level":"info","sequence":{},"status":"{}","state":"exited","returned_from_user":true,"reply_copied":{},"iret":false}}"#,
        exit.sequence,
        status_label(exit.status)?,
        context.terminal_reply_copied
    );
    Ok(())
}

fn status_label(status: rite_status) -> Result<&'static str, &'static str> {
    match status.raw() {
        value if value == rite_status::ok.raw() => Ok("ok"),
        value if value == rite_status::would_block.raw() => Ok("would_block"),
        value if value == rite_status::denied.raw() => Ok("denied"),
        value if value == rite_status::bad_operation.raw() => Ok("bad_operation"),
        value if value == rite_status::bad_payload.raw() => Ok("bad_payload"),
        value if value == rite_status::limit_exceeded.raw() => Ok("limit_exceeded"),
        _ => Err("unexpected_rite_status"),
    }
}

fn validate_observations(
    observations: &[rite_observation; call_count],
) -> Result<(), &'static str> {
    for (index, observation) in observations.iter().enumerate() {
        let phase = phase_from_index(index).ok_or("rite_observation_index_invalid")?;
        if observation.sequence != phase.sequence().ok_or("rite_sequence_missing")?
            || observation.status
                != phase
                    .expected_status()
                    .ok_or("rite_expected_status_missing")?
        {
            return Err("rite_observation_mismatch");
        }
    }
    Ok(())
}

const fn phase_from_index(index: usize) -> Option<gate_phase> {
    match index {
        0 => Some(gate_phase::unknown),
        1 => Some(gate_phase::missing_console),
        2 => Some(gate_phase::display_overclaim),
        3 => Some(gate_phase::transport_limit),
        4 => Some(gate_phase::invalid_text),
        5 => Some(gate_phase::console_write),
        6 => Some(gate_phase::input_empty),
        7 => Some(gate_phase::invalid_exit),
        8 => Some(gate_phase::task_exit),
        _ => None,
    }
}

fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    code_length: usize,
) -> Result<task_identity, &'static str> {
    let code_length = u64::try_from(code_length).map_err(|_| "rite_code_length_invalid")?;
    let code_end = user_code_address
        .checked_add(code_length)
        .filter(|end| *end <= user_code_address + page_size)
        .ok_or("rite_code_length_invalid")?;
    let bounds = task_bounds::new(
        user_code_address,
        code_end,
        user_code_address,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("rite_task_bounds_invalid")?;
    let principal = fixture_principal_id::new(1).ok_or("rite_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(
                circle,
                principal,
                synthetic_origin_metadata::human_role_fixture,
                bounds,
            ),
        )
        .map_err(|_| "rite_task_create_failed")
}

fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    payload_start: *const u8,
    payload_end: *const u8,
) -> Result<task_space, &'static str> {
    let code = allocate_data(allocator, "rite_code_frame_allocation_failed")?;
    let mailbox = allocate_data(allocator, "rite_mailbox_frame_allocation_failed")?;
    let stack = allocate_data(allocator, "rite_stack_frame_allocation_failed")?;
    let code_range = code.range().ok_or("rite_code_frame_extent_missing")?;
    let mailbox_range = mailbox.range().ok_or("rite_mailbox_frame_extent_missing")?;
    let stack_range = stack.range().ok_or("rite_stack_frame_extent_missing")?;
    if code_range == mailbox_range
        || code_range == stack_range
        || mailbox_range == stack_range
        || !range_is_zero(code_range)
        || !range_is_zero(mailbox_range)
        || !range_is_zero(stack_range)
    {
        return Err("rite_private_frames_not_distinct_and_clear");
    }
    // SAFETY: The allocator grants the exclusive one-page code lease and the
    // immutable feature template must fit wholly in it.
    let payload_length = unsafe { copy_payload(payload_start, payload_end, code_range)? };

    // SAFETY: The published allocator serializes the proof, and the callback
    // clears every exclusive table extent before publication.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "rite_address_space_create_failed")?;
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
    .map_err(|_| "rite_code_mapping_failed")?;
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
    .map_err(|_| "rite_mailbox_mapping_failed")?;
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
    .map_err(|_| "rite_stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "rite_address_space_validation_failed")?;
    validate_mapping(
        root.validate_mapping(user_code_address)
            .map_err(|_| "rite_code_mapping_walk_failed")?,
        code_range.base,
        false,
        true,
    )?;
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "rite_mailbox_mapping_walk_failed")?,
        mailbox_range.base,
        true,
        false,
    )?;
    validate_mapping(
        root.validate_mapping(user_stack_address)
            .map_err(|_| "rite_stack_mapping_walk_failed")?,
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
            return Err("rite_guard_mapping_present");
        }
    }
    if !payload_matches(payload_start, payload_length, code_range.base) {
        return Err("rite_payload_copy_mismatch");
    }
    Ok(task_space {
        root,
        physical_pages: [code_range.base, mailbox_range.base, stack_range.base],
        payload_length,
    })
}

fn attach_task_space(circles: &mut circle_table, circle: circle_id) -> Result<(), &'static str> {
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "rite_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "rite_root_attachment_rollback_failed")?;
        return Err("rite_mapping_attachment_failed");
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
        .ok_or("rite_payload_extent_invalid")?;
    if length == 0 || length > page_size as usize || destination.length != page_size {
        return Err("rite_payload_extent_invalid");
    }
    let destination = usize::try_from(destination.base)
        .map_err(|_| "rite_payload_destination_invalid")? as *mut u8;
    // SAFETY: The caller supplies an exclusive page and the checked immutable
    // linker range fits without overlap.
    unsafe { ptr::copy_nonoverlapping(start, destination, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(length)
}

fn payload_matches(start: *const u8, length: usize, destination: u64) -> bool {
    let Ok(destination) = usize::try_from(destination) else {
        return false;
    };
    for offset in 0..length {
        // SAFETY: Both ranges cover the checked feature payload length.
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
        return Err("rite_mapping_permissions_invalid");
    }
    Ok(())
}

fn execute_task(
    context: &mut rite_context,
    space: &task_space,
    task: task_identity,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    // SAFETY: This single-BSP feature exclusively owns both continuation cells.
    let saved_stack_clear = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) == 0 };
    // SAFETY: The same exclusive interval protects the saved TS cell.
    let saved_task_switched_clear =
        unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) == 0 };
    // SAFETY: IF is clear, NMI is masked, and no privilege transition can race
    // this read of the packed TSS RSP0 field.
    let transition_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    if context.current_task.is_some()
        || context.mailbox_alias == 0
        || context.channel.is_none()
        || context.phase != gate_phase::unknown
        || !active_context.load(Ordering::Acquire).is_null()
        || space.payload_length == 0
        || !saved_stack_clear
        || !saved_task_switched_clear
        || transition_stack_top == 0
        || transition_stack_top & 0x0f != 0
    {
        return Err("rite_task_entry_state_invalid");
    }

    context
        .tasks
        .start(task)
        .map_err(|_| "rite_task_start_failed")?;
    context.current_task = Some(task);
    context.active_root = space
        .root
        .root_address()
        .map_err(|_| "rite_task_root_extent_missing")?;
    context.transition_stack_top = transition_stack_top;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The bootstrap processor is the only writer and the gate cannot
    // run until after context publication and IRETQ.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(saved_task_switched),
            original_task_switched,
        )
    };
    // SAFETY: The root has exactly three validated mappings and borrows the
    // live kernel branch; maskable interrupts and NMI remain excluded.
    unsafe { space.root.activate() }.map_err(|_| "rite_root_activation_failed")?;
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
        // SAFETY: User entry did not occur, so restoring the original root is
        // immediate and cannot race another execution context.
        unsafe { paging::write_cr3(kernel_cr3) };
        return Err("rite_context_publication_failed");
    }

    // SAFETY: Task identity, root, selectors, fixed mailbox, TSS RSP0, and the
    // feature-local gate were validated before this one BSP enters CPL3.
    let status = unsafe {
        x86_64_enter_rite_task(
            user_code_address,
            user_stack_top,
            u64::from(descriptors::user_data_selector),
            u64::from(descriptors::user_code_selector),
            ptr::addr_of_mut!(saved_kernel_stack),
        )
    };

    active_context.store(ptr::null_mut(), Ordering::Release);
    let task_switched_restored = paging::read_cr0() & cr0_task_switched == original_task_switched;
    // SAFETY: The nonlocal completion restored the original Rust stack. The
    // exact kernel root must be active before withdrawing any owned mapping.
    unsafe { paging::write_cr3(kernel_cr3) };
    let kernel_root_restored = paging::read_cr3() == kernel_cr3;
    context.current_task = None;
    context.mailbox_alias = 0;
    context.active_root = 0;
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    context.transition_stack_top = 0;
    // SAFETY: Completion consumed both single-use continuation values.
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
        return Err("rite_task_completion_restore_failed");
    }
    Ok(())
}

fn release_task_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: circle_id,
    space: &mut task_space,
) -> Result<(), &'static str> {
    let mut code = space
        .root
        .unmap_owned(allocator, user_code_address)
        .map_err(|_| "rite_code_mapping_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "rite_mailbox_mapping_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "rite_stack_mapping_withdrawal_failed")?;
    // SAFETY: The root is inactive, owns no mappings, and its table frames are
    // exclusive to this allocator-authorized address-space object.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "rite_address_space_destroy_failed")?;
    // SAFETY: Unmapping returned three exclusive, unbound data leases.
    unsafe { allocator.release_with(&mut code, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "rite_code_frame_release_failed")?;
    // SAFETY: The mailbox lease is inactive and exclusively held.
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "rite_mailbox_frame_release_failed")?;
    // SAFETY: The stack lease is inactive and exclusively held.
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "rite_stack_frame_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "rite_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "rite_root_detachment_failed")?;
    Ok(())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_rite_gate_dispatch(frame: *mut rite_gate_frame) {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<u64>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("rite_gate_reentry_or_frame_invalid");
    }
    let context_pointer = active_context.load(Ordering::Acquire);
    if context_pointer.is_null() {
        fail("rite_gate_without_active_context");
    }
    // SAFETY: Publication names the unique live kernel-stack context. IF is
    // clear, NMI is masked, and the latch provides exclusive mutable access.
    let context = unsafe { &mut *context_pointer };
    // SAFETY: Assembly supplies exactly 22 naturally aligned qwords whose
    // lifetime covers this dispatch.
    let frame = unsafe { &mut *frame };
    if !valid_gate_frame(context, frame) {
        fail("rite_gate_frame_validation_failed");
    }

    let reply_copies_before = context.reply_copy_count;
    match dispatch_gate(context) {
        Ok(gate_action::return_to_user(reply)) => {
            // SAFETY: The active task space retains exclusive ownership of its
            // live identity-mapped mailbox alias through this gate dispatch.
            if unsafe { write_reply(context.mailbox_alias, reply) }.is_err() {
                fail("rite_reply_copy_failed");
            }
            context.reply_copy_count += 1;
            compiler_fence(Ordering::SeqCst);
            gate_active.store(false, Ordering::Release);
        }
        Ok(gate_action::complete) => {
            context.terminal_reply_copied = context.reply_copy_count != reply_copies_before;
            if context.terminal_reply_copied {
                fail("rite_terminal_reply_was_copied");
            }
            // SAFETY: Both values were written once immediately before IRETQ
            // and remain private until this exact terminal completion.
            let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
            // SAFETY: The same single-BSP publication contract protects TS.
            let original_task_switched =
                unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
            if saved_stack == 0
                || saved_stack & (mem::align_of::<u64>() as u64 - 1) != 0
                || !matches!(original_task_switched, 0 | cr0_task_switched)
                || paging::read_cr0() & cr0_task_switched == 0
            {
                fail("rite_completion_continuation_invalid");
            }
            gate_active.store(false, Ordering::Release);
            active_context.store(ptr::null_mut(), Ordering::Release);
            // SAFETY: The terminal task is exited and its success reply was not
            // copied. This single-use trampoline abandons the user/TSS frames,
            // restores CR0.TS, and resumes the saved kernel stack.
            unsafe {
                x86_64_resume_rite_task(status_completed, saved_stack, original_task_switched)
            }
        }
        Err(reason) => fail(reason),
    }
}

enum gate_action {
    return_to_user(rite_reply),
    complete,
}

fn valid_gate_frame(context: &rite_context, frame: &rite_gate_frame) -> bool {
    let Some(task) = context.current_task else {
        return false;
    };
    let Ok(snapshot) = context.tasks.snapshot(task) else {
        return false;
    };
    let Some(frame_end) = (frame as *const rite_gate_frame as usize as u64)
        .checked_add(mem::size_of::<rite_gate_frame>() as u64)
    else {
        return false;
    };
    // SAFETY: IF is clear and NMI remains masked for this single-BSP proof.
    let selected_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let bounds = snapshot.bounds();
    snapshot.identity() == task
        && snapshot.state() == task_state::running
        && frame.vector == rite_gate_vector
        && frame.error_code == 0
        && frame.code_segment == u64::from(descriptors::user_code_selector)
        && frame.stack_segment == u64::from(descriptors::user_data_selector)
        && frame.flags == sanitized_flags
        && bounds.contains_code(frame.instruction_pointer)
        && expected_gate_instruction_pointer(context.phase) == Some(frame.instruction_pointer)
        && bounds.contains_stack_pointer(frame.stack_pointer)
        && frame.stack_pointer == bounds.initial_stack_pointer()
        && frame.accumulator == 0
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
        && context.mailbox_alias != 0
        && context.mailbox_alias & (page_size - 1) == 0
        && context
            .mailbox_alias
            .checked_add(page_size)
            .is_some_and(|end| end <= memory::address_limit)
        && context.active_root != 0
        && paging::entry_address(paging::read_cr3()) == context.active_root
        && context.transition_stack_top == selected_stack_top
        && frame_end == selected_stack_top
        && maskable_interrupts_are_disabled()
}

fn expected_gate_instruction_pointer(phase: gate_phase) -> Option<u64> {
    let symbol = match phase {
        gate_phase::unknown => ptr::addr_of!(x86_64_rite_test_unknown_return),
        gate_phase::missing_console => {
            ptr::addr_of!(x86_64_rite_test_missing_console_return)
        }
        gate_phase::display_overclaim => {
            ptr::addr_of!(x86_64_rite_test_display_overclaim_return)
        }
        gate_phase::transport_limit => {
            ptr::addr_of!(x86_64_rite_test_transport_limit_return)
        }
        gate_phase::invalid_text => ptr::addr_of!(x86_64_rite_test_invalid_utf8_return),
        gate_phase::console_write => ptr::addr_of!(x86_64_rite_test_console_return),
        gate_phase::input_empty => ptr::addr_of!(x86_64_rite_test_input_return),
        gate_phase::invalid_exit => ptr::addr_of!(x86_64_rite_test_malformed_exit_return),
        gate_phase::task_exit => ptr::addr_of!(x86_64_rite_test_exit_return),
        gate_phase::idle => return None,
    };
    let start = ptr::addr_of!(x86_64_rite_test_user_start);
    let offset = (symbol as usize).checked_sub(start as usize)?;
    (offset < page_size as usize).then_some(user_code_address + offset as u64)
}

fn dispatch_gate(context: &mut rite_context) -> Result<gate_action, &'static str> {
    let phase = context.phase;
    let index = phase.index().ok_or("rite_gate_idle_phase")?;
    if context.observation_count != index {
        return Err("rite_observation_order_invalid");
    }
    // SAFETY: The active task space retains exclusive ownership of its live,
    // identity-mapped mailbox alias through this gate dispatch.
    let call = unsafe { read_call(context.mailbox_alias) }?;
    validate_fixture_call(phase, &call)?;

    let before_copies = context.body_copy_count;
    let completion = {
        let channel = context.channel.as_mut().ok_or("rite_channel_not_live")?;
        let decision = channel
            .assess(
                &context.tasks,
                context.current_task.ok_or("rite_task_missing")?,
                call,
            )
            .map_err(|_| "rite_assessment_failed")?;
        match decision {
            rite_decision::rejected(reply) => (reply, false),
            rite_decision::admitted(admission) => complete_admitted(
                phase,
                admission,
                &mut context.tasks,
                &mut context.screen,
                context.mailbox_alias,
                &mut context.scratch,
                &mut context.console_copy,
                &mut context.body_copy_count,
                &mut context.body_bytes_copied,
                &mut context.console_bytes_committed,
                &mut context.exit_code,
            )?,
        }
    };
    let (reply, terminal) = completion;
    let expected_copies = expected_copy_count_after(phase).ok_or("rite_copy_count_missing")?;
    if context.body_copy_count != expected_copies
        || context.body_copy_count < before_copies
        || !scratch_is_zero(&context.scratch)
    {
        return Err("rite_body_copy_order_invalid");
    }
    validate_reply(phase, reply)?;

    context.observations[index] = rite_observation {
        sequence: reply.sequence,
        status: reply.status,
    };
    context.observation_count += 1;

    if terminal {
        if phase != gate_phase::task_exit
            || context
                .tasks
                .snapshot(context.current_task.ok_or("rite_task_missing")?)
                .map_err(|_| "rite_terminal_snapshot_failed")?
                .state()
                != task_state::exited
        {
            return Err("rite_terminal_state_invalid");
        }
        context.phase = gate_phase::idle;
        Ok(gate_action::complete)
    } else {
        context.phase = phase.next().ok_or("rite_phase_advanced_past_terminal")?;
        Ok(gate_action::return_to_user(reply))
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_admitted(
    phase: gate_phase,
    admission: rite_admission<'_>,
    tasks: &mut task_table,
    screen: &mut screen_console,
    mailbox: u64,
    scratch: &mut rite_scratch,
    console_copy: &mut [u8; private_console_text.len()],
    body_copy_count: &mut usize,
    body_bytes_copied: &mut usize,
    console_bytes_committed: &mut usize,
    exit_code: &mut Option<i32>,
) -> Result<(rite_reply, bool), &'static str> {
    let vessel = admission.vessel();
    match phase {
        gate_phase::unknown
        | gate_phase::missing_console
        | gate_phase::display_overclaim
        | gate_phase::idle => Err("rite_unexpected_admission"),
        gate_phase::transport_limit => {
            if payload_span_fits(user_mailbox_address, admission.payload_size()) {
                return Err("rite_transport_limit_not_exceeded");
            }
            let reply = admission
                .reject_after_revalidation(tasks, rite_status::limit_exceeded)
                .map_err(|_| "rite_transport_rejection_failed")?;
            Ok((reply, false))
        }
        gate_phase::invalid_text => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for `user_mailbox_address`.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    user_mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let invalid = core::str::from_utf8(
                &scratch[..usize::try_from(admission.payload_size())
                    .map_err(|_| "rite_text_length_invalid")?],
            )
            .is_err();
            if !invalid {
                return Err("rite_invalid_text_was_valid");
            }
            let reply = admission
                .reject_after_revalidation(tasks, rite_status::bad_payload)
                .map_err(|_| "rite_text_rejection_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("rite_text_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::console_write => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for `user_mailbox_address`.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    user_mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let length = usize::try_from(admission.payload_size())
                .map_err(|_| "rite_console_length_invalid")?;
            let text = core::str::from_utf8(&scratch[..length])
                .map_err(|_| "rite_console_utf8_invalid")?;
            if text.as_bytes() != private_console_text
                || !text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
                || length != console_copy.len()
            {
                return Err("rite_console_body_invalid");
            }
            let reply = admission
                .finish_ok(tasks, |_, exact_vessel| {
                    if exact_vessel != vessel {
                        fail("rite_console_identity_changed");
                    }
                    console_copy.copy_from_slice(text.as_bytes());
                    if screen.write_str(text).is_err() {
                        fail("rite_console_commit_failed");
                    }
                    *console_bytes_committed = length;
                })
                .map_err(|_| "rite_console_completion_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("rite_console_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::input_empty => {
            // SAFETY: IF is clear, NMI is masked, this is the BSP, and external
            // interrupt initialization has not run. The readiness result stays
            // current for the complete would-block decision.
            if unsafe { input::ps2_delivery_ready_if_disabled() } {
                return Err("rite_input_not_empty");
            }
            let reply = admission
                .finish_would_block(tasks)
                .map_err(|_| "rite_input_completion_failed")?;
            Ok((reply, false))
        }
        gate_phase::invalid_exit => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for `user_mailbox_address`.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    user_mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let payload = decode_exit_payload(scratch)?;
            if payload.has_supported_body() {
                return Err("rite_invalid_exit_was_supported");
            }
            let reply = admission
                .reject_after_revalidation(tasks, rite_status::bad_payload)
                .map_err(|_| "rite_exit_rejection_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("rite_exit_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::task_exit => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for `user_mailbox_address`.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    user_mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let payload = decode_exit_payload(scratch)?;
            if !payload.has_supported_body() || payload.code != expected_exit_code {
                return Err("rite_exit_body_invalid");
            }
            let reply = admission
                .finish_ok(tasks, |tasks, exact_vessel| {
                    if exact_vessel != vessel || tasks.exit(exact_vessel).is_err() {
                        fail("rite_task_exit_commit_failed");
                    }
                })
                .map_err(|_| "rite_task_exit_completion_failed")?;
            *exit_code = Some(payload.code);
            if !clear_and_verify_bytes(scratch) {
                return Err("rite_terminal_scratch_clear_failed");
            }
            Ok((reply, true))
        }
    }
}

const fn expected_copy_count_after(phase: gate_phase) -> Option<usize> {
    match phase {
        gate_phase::unknown
        | gate_phase::missing_console
        | gate_phase::display_overclaim
        | gate_phase::transport_limit => Some(0),
        gate_phase::invalid_text => Some(1),
        gate_phase::console_write | gate_phase::input_empty => Some(2),
        gate_phase::invalid_exit => Some(3),
        gate_phase::task_exit => Some(4),
        gate_phase::idle => None,
    }
}

fn validate_fixture_call(phase: gate_phase, call: &rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size || call.reserved != 0 {
        return Err("rite_fixture_header_invalid");
    }
    if call.trace_id != phase.trace().ok_or("rite_fixture_trace_phase_invalid")? {
        return Err("rite_fixture_trace_invalid");
    }
    let exact = match phase {
        gate_phase::unknown => {
            call.operation.raw() == u16::MAX
                && call.payload_kind.raw() == u16::MAX
                && call.rights_present == u64::MAX
                && call.payload_size == u32::MAX
                && call.payload_limit == 0
        }
        gate_phase::missing_console => {
            call.operation == rite_operation::console_write
                && call.payload_kind == rite_payload_kind::utf8_text
                && call.rights_present == rite_rights::input_read
                && call.payload_size == 4097
                && call.payload_limit == 0
        }
        gate_phase::display_overclaim => {
            call.operation == rite_operation::display_update
                && call.payload_kind == rite_payload_kind::display_patch
                && call.rights_present == rite_rights::display_update
                && call.payload_size == 1
                && call.payload_limit == 1
        }
        gate_phase::transport_limit => {
            call.operation == rite_operation::console_write
                && call.payload_kind == rite_payload_kind::utf8_text
                && call.rights_present == rite_rights::console_write
                && call.payload_size == transport_payload_limit + 1
                && call.payload_limit == 4096
        }
        gate_phase::invalid_text => {
            call.operation == rite_operation::console_write
                && call.payload_kind == rite_payload_kind::utf8_text
                && call.rights_present == rite_rights::console_write
                && call.payload_size == invalid_text.len() as u32
                && call.payload_limit == invalid_text.len() as u32
        }
        gate_phase::console_write => {
            call.operation == rite_operation::console_write
                && call.payload_kind == rite_payload_kind::utf8_text
                && call.rights_present == rite_rights::console_write
                && call.payload_size == private_console_text.len() as u32
                && call.payload_limit == private_console_text.len() as u32
        }
        gate_phase::input_empty => {
            call.operation == rite_operation::input_read
                && call.payload_kind == rite_payload_kind::input_request
                && call.rights_present == rite_rights::input_read
                && call.payload_size == 0
                && call.payload_limit == 0
        }
        gate_phase::invalid_exit | gate_phase::task_exit => {
            call.operation == rite_operation::task_exit
                && call.payload_kind == rite_payload_kind::exit_status
                && call.rights_present == rite_rights::task_exit
                && call.payload_size == mem::size_of::<rite_exit_payload>() as u32
                && call.payload_limit == mem::size_of::<rite_exit_payload>() as u32
        }
        gate_phase::idle => return Err("rite_fixture_idle_phase"),
    };
    if !exact {
        return Err(match phase {
            gate_phase::unknown => "rite_fixture_unknown_shape_invalid",
            gate_phase::missing_console => "rite_fixture_missing_right_shape_invalid",
            gate_phase::display_overclaim => "rite_fixture_overclaim_shape_invalid",
            gate_phase::transport_limit => "rite_fixture_transport_shape_invalid",
            gate_phase::invalid_text => "rite_fixture_invalid_text_shape_invalid",
            gate_phase::console_write => "rite_fixture_console_shape_invalid",
            gate_phase::input_empty => "rite_fixture_input_shape_invalid",
            gate_phase::invalid_exit => "rite_fixture_invalid_exit_shape_invalid",
            gate_phase::task_exit => "rite_fixture_exit_shape_invalid",
            gate_phase::idle => "rite_fixture_idle_phase",
        });
    }
    Ok(())
}

fn validate_reply(phase: gate_phase, reply: rite_reply) -> Result<(), &'static str> {
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status != phase.expected_status().ok_or("rite_status_missing")?
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence != phase.sequence().ok_or("rite_sequence_missing")?
        || reply.trace_id != phase.trace().ok_or("rite_trace_missing")?
    {
        return Err("rite_reply_invalid");
    }
    Ok(())
}

fn maskable_interrupts_are_disabled() -> bool {
    let flags: u64;
    // SAFETY: Reading RFLAGS is non-mutating and this proof runs at CPL0.
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
        // SAFETY: The one-page allocator extent is identity mapped and remains
        // exclusively borrowed throughout this validation.
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
        r#"{{"schema":1,"event":"kernel.rite_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
