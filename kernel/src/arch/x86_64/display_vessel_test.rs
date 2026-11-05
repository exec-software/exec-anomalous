use anomalous_abi::{
    id_128, rite_call, rite_display_patch_v1, rite_exit_payload, rite_operation, rite_payload_kind,
    rite_reply, rite_rights, rite_status,
};
use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_display_patch,
    decode_exit_payload, display_patch_pixels, read_call, reply_size, rite_scratch,
    scratch_is_zero, write_reply,
};
use super::{descriptors, paging};
use crate::boot_omen::framebuffer_info;
use crate::circle::{circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::serial_port;
use crate::display::framebuffer_presenter;
use crate::identity::{circle_id, fixture_principal_id, task_identity};
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
const expected_exit_code: i32 = 42;
const display_width: u32 = 1024;
const display_height: u32 = 768;
const display_patch_count: usize = display_height as usize;
const call_count: usize = display_patch_count + 4;
const returning_call_count: usize = call_count - 1;
const display_small_payload_size: u32 = 43;
const display_row_payload_size: u32 = 3112;
const display_row_pixel_bytes: u32 = display_width * 3;
const display_state_offset: usize = 3200;
const expected_body_copies: usize = display_patch_count + 3;
const expected_body_bytes: usize = 2 * display_small_payload_size as usize
    + display_patch_count * display_row_payload_size as usize
    + mem::size_of::<rite_exit_payload>();

const _: () = {
    assert!(call_size == 40);
    assert!(reply_size == 40);
    assert!(mem::size_of::<rite_display_patch_v1>() == 40);
    assert!(display_row_payload_size as usize + call_size <= display_state_offset);
    assert!(display_state_offset + mem::size_of::<u32>() <= page_size as usize);
    assert!(call_count == 772);
    assert!(expected_body_copies == 771);
    assert!(expected_body_bytes == 2_390_110);
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum gate_phase {
    denied,
    malformed,
    bounds,
    row(u32),
    task_exit,
    idle,
}

impl gate_phase {
    const fn sequence(self) -> Option<u64> {
        match self {
            Self::denied => Some(1),
            Self::malformed => Some(2),
            Self::bounds => Some(3),
            Self::row(y) if y < display_height => Some(y as u64 + 4),
            Self::task_exit => Some(call_count as u64),
            Self::row(_) | Self::idle => None,
        }
    }

    const fn trace(self) -> Option<id_128> {
        let byte = match self {
            Self::denied => 0x11,
            Self::malformed => 0x22,
            Self::bounds => 0x33,
            Self::row(_) => 0x44,
            Self::task_exit => 0x55,
            Self::idle => return None,
        };
        Some(id_128::from_bytes([byte; 16]))
    }

    const fn expected_status(self) -> Option<rite_status> {
        match self {
            Self::denied => Some(rite_status::denied),
            Self::malformed | Self::bounds => Some(rite_status::bad_payload),
            Self::row(y) if y < display_height => Some(rite_status::ok),
            Self::task_exit => Some(rite_status::ok),
            Self::row(_) | Self::idle => None,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::denied => Some(Self::malformed),
            Self::malformed => Some(Self::bounds),
            Self::bounds => Some(Self::row(0)),
            Self::row(y) if y + 1 < display_height => Some(Self::row(y + 1)),
            Self::row(_) => Some(Self::task_exit),
            Self::task_exit | Self::idle => None,
        }
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

const _: () = assert!(mem::size_of::<rite_gate_frame>() == 22 * mem::size_of::<u64>());

struct display_context {
    circles: circle_table,
    tasks: task_table,
    channel: Option<rite_channel>,
    presenter: framebuffer_presenter,
    current_task: Option<task_identity>,
    phase: gate_phase,
    mailbox_alias: u64,
    active_root: u64,
    transition_stack_top: u64,
    scratch: rite_scratch,
    calls: usize,
    body_copy_count: usize,
    body_bytes_copied: usize,
    reply_copy_count: usize,
    rejection_statuses: [rite_status; 3],
    rejection_surface_checks: usize,
    patches_committed: usize,
    exit_code: Option<i32>,
    exit_status: rite_status,
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

static active_context: AtomicPtr<display_context> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);
static mut saved_kernel_stack: u64 = 0;
static mut saved_task_switched: u64 = 0;

unsafe extern "C" {
    fn x86_64_display_vessel_enter_task(
        instruction_pointer: u64,
        stack_pointer: u64,
        data_selector: u64,
        code_selector: u64,
        saved_stack: *mut u64,
    ) -> u64;
    fn x86_64_display_vessel_resume_task(
        status: u64,
        saved_stack: u64,
        original_task_switched: u64,
    ) -> !;

    static x86_64_display_vessel_user_start: u8;
    static x86_64_display_vessel_denied_return: u8;
    static x86_64_display_vessel_malformed_return: u8;
    static x86_64_display_vessel_bounds_return: u8;
    static x86_64_display_vessel_row_return: u8;
    static x86_64_display_vessel_exit_return: u8;
    static x86_64_display_vessel_user_end: u8;
}

pub(super) fn run(framebuffer: framebuffer_info) -> ! {
    // SAFETY: `kernel_main` transfers the validated boot aperture exclusively
    // to this terminal proof after its temporary boot drawing has completed.
    let presenter = match unsafe { framebuffer_presenter::try_new(framebuffer) } {
        Ok(presenter) => presenter,
        Err(_) => fail("presenter_construction_failed"),
    };
    if presenter.width() != display_width as usize || presenter.height() != display_height as usize
    {
        fail("fixture_geometry_unavailable");
    }
    let framebuffer_address = framebuffer.address;
    let framebuffer_aperture_len = framebuffer
        .pitch
        .checked_mul(framebuffer.height)
        .and_then(|length| u64::try_from(length).ok())
        .unwrap_or_else(|| fail("fixture_aperture_length_invalid"));

    let kernel_cr3 = paging::read_cr3();
    let result = memory::with_frame_allocator(move |allocator| {
        run_with_allocator(
            allocator,
            kernel_cr3,
            presenter,
            framebuffer_address,
            framebuffer_aperture_len,
        )
    });
    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact root captured before the bounded proof.
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
        r#"{{"schema":1,"event":"kernel.display_vessel_test.passed","level":"info","calls":772,"rejections":3,"commits":768,"terminal":true}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    mut presenter: framebuffer_presenter,
    framebuffer_address: u64,
    framebuffer_aperture_len: u64,
) -> Result<(), &'static str> {
    if !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || !paging::protections_are_enabled()
        || !maskable_interrupts_are_disabled()
    {
        return Err("initial_display_state_invalid");
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
        r#"{{"schema":1,"event":"kernel.display_vessel_test.ready","level":"info","arch":"x86_64","grant":20,"calls":772,"patches":768,"width":1024,"height":768,"format":"rgb8"}}"#
    );

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let circle = circles
        .create()
        .map_err(|_| "display_circle_create_failed")?;
    let mut space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_display_vessel_user_start),
        ptr::addr_of!(x86_64_display_vessel_user_end),
    )?;
    space
        .root
        .validate_borrowed_supervisor_range(framebuffer_address, framebuffer_aperture_len)
        .map_err(|_| "display_aperture_user_accessible")?;
    attach_task_space(&mut circles, circle)?;
    let task = create_task(&mut circles, &mut tasks, circle, space.payload_length)?;
    let channel = rite_channel::try_new(task, rite_rights::display_update | rite_rights::task_exit)
        .map_err(|_| "display_channel_create_failed")?;

    let task_snapshot = tasks
        .snapshot(task)
        .map_err(|_| "display_task_snapshot_failed")?;
    let circle_snapshot = circles
        .snapshot(circle)
        .map_err(|_| "display_circle_snapshot_failed")?;
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
        return Err("display_task_ready_state_invalid");
    }

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.display_vessel_test.task.entered","level":"info","origin":"user","mappings":3,"code_permissions":"user_rx","mailbox_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true,"aperture_user_accessible":false}}"#
    );

    presenter.initialize_proof_canary();
    if !presenter.proof_canary_is_intact() {
        return Err("display_rejection_canary_initialization_failed");
    }

    let mut context = display_context {
        circles,
        tasks,
        channel: Some(channel),
        presenter,
        current_task: None,
        phase: gate_phase::denied,
        mailbox_alias: space.mailbox_alias(),
        active_root: 0,
        transition_stack_top: 0,
        scratch: [0; page_size as usize],
        calls: 0,
        body_copy_count: 0,
        body_bytes_copied: 0,
        reply_copy_count: 0,
        rejection_statuses: [rite_status::faulted; 3],
        rejection_surface_checks: 0,
        patches_committed: 0,
        exit_code: None,
        exit_status: rite_status::faulted,
        terminal_reply_copied: false,
    };

    execute_task(&mut context, &space, task, kernel_cr3)?;

    let exited = context
        .tasks
        .snapshot(task)
        .map_err(|_| "display_task_final_snapshot_failed")?;
    if exited.state() != task_state::exited
        || context.phase != gate_phase::idle
        || context.calls != call_count
        || context.body_copy_count != expected_body_copies
        || context.body_bytes_copied != expected_body_bytes
        || context.reply_copy_count != returning_call_count
        || context.rejection_statuses
            != [
                rite_status::denied,
                rite_status::bad_payload,
                rite_status::bad_payload,
            ]
        || context.rejection_surface_checks != 3
        || context.patches_committed != display_patch_count
        || context.exit_code != Some(expected_exit_code)
        || context.exit_status != rite_status::ok
        || context.terminal_reply_copied
        || !scratch_is_zero(&context.scratch)
    {
        return Err("display_completion_evidence_invalid");
    }

    context
        .channel
        .take()
        .ok_or("display_channel_missing_at_retirement")?;
    if !clear_and_verify_bytes(&mut context.scratch) {
        return Err("display_private_storage_clear_failed");
    }

    release_task_space(allocator, &mut context.circles, circle, &mut space)?;
    context
        .tasks
        .destroy(&mut context.circles, task)
        .map_err(|_| "display_task_destroy_failed")?;
    context
        .circles
        .destroy(circle)
        .map_err(|_| "display_circle_destroy_failed")?;

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
    {
        return Err("final_display_state_mismatch");
    }

    emit_outcomes(&mut serial, &context)?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.display_vessel_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":{},"table_owned_frames":{},"active_circles":{},"active_tasks":{},"active_mappings":{},"channel_retired":true,"mailbox_wiped":true,"transition_stack_wiped":true,"scratch_wiped":true}}"#,
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

fn emit_outcomes(serial: &mut serial_port, context: &display_context) -> Result<(), &'static str> {
    let labels = ["authority", "malformed", "bounds"];
    for (index, status) in context.rejection_statuses.iter().copied().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.display_vessel_test.call.rejected","level":"info","sequence":{},"status":"{}","kind":"{}","state_unchanged":true,"surface_unchanged":true}}"#,
            index + 1,
            status_label(status)?,
            labels[index]
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.display_vessel_test.frame.committed","level":"info","patches":768,"width":1024,"height":768,"format":"rgb8"}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.display_vessel_test.task.exited","level":"info","sequence":772,"status":"ok","state":"exited","returned_from_user":true,"reply_copied":false}}"#
    );
    Ok(())
}

fn status_label(status: rite_status) -> Result<&'static str, &'static str> {
    match status.raw() {
        value if value == rite_status::ok.raw() => Ok("ok"),
        value if value == rite_status::denied.raw() => Ok("denied"),
        value if value == rite_status::bad_payload.raw() => Ok("bad_payload"),
        _ => Err("unexpected_display_status"),
    }
}

fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    code_length: usize,
) -> Result<task_identity, &'static str> {
    let code_length = u64::try_from(code_length).map_err(|_| "display_code_length_invalid")?;
    let code_end = user_code_address
        .checked_add(code_length)
        .filter(|end| *end <= user_code_address + page_size)
        .ok_or("display_code_length_invalid")?;
    let bounds = task_bounds::new(
        user_code_address,
        code_end,
        user_code_address,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("display_task_bounds_invalid")?;
    let principal = fixture_principal_id::new(1).ok_or("display_principal_invalid")?;
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
        .map_err(|_| "display_task_create_failed")
}

fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    payload_start: *const u8,
    payload_end: *const u8,
) -> Result<task_space, &'static str> {
    let code = allocate_data(allocator, "display_code_frame_allocation_failed")?;
    let mailbox = allocate_data(allocator, "display_mailbox_frame_allocation_failed")?;
    let stack = allocate_data(allocator, "display_stack_frame_allocation_failed")?;
    let code_range = code.range().ok_or("display_code_frame_extent_missing")?;
    let mailbox_range = mailbox
        .range()
        .ok_or("display_mailbox_frame_extent_missing")?;
    let stack_range = stack.range().ok_or("display_stack_frame_extent_missing")?;
    if code_range == mailbox_range
        || code_range == stack_range
        || mailbox_range == stack_range
        || !range_is_zero(code_range)
        || !range_is_zero(mailbox_range)
        || !range_is_zero(stack_range)
    {
        return Err("display_private_frames_not_distinct_and_clear");
    }
    // SAFETY: The exclusive code lease receives one immutable, bounded payload.
    let payload_length = unsafe { copy_payload(payload_start, payload_end, code_range)? };

    // SAFETY: The allocator serializes this proof and clears each table extent.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "display_address_space_create_failed")?;
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
    .map_err(|_| "display_code_mapping_failed")?;
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
    .map_err(|_| "display_mailbox_mapping_failed")?;
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
    .map_err(|_| "display_stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "display_address_space_validation_failed")?;
    validate_mapping(
        root.validate_mapping(user_code_address)
            .map_err(|_| "display_code_mapping_walk_failed")?,
        code_range.base,
        false,
        true,
    )?;
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "display_mailbox_mapping_walk_failed")?,
        mailbox_range.base,
        true,
        false,
    )?;
    validate_mapping(
        root.validate_mapping(user_stack_address)
            .map_err(|_| "display_stack_mapping_walk_failed")?,
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
            return Err("display_guard_mapping_present");
        }
    }
    if !payload_matches(payload_start, payload_length, code_range.base) {
        return Err("display_payload_copy_mismatch");
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
        .map_err(|_| "display_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "display_root_attachment_rollback_failed")?;
        return Err("display_mapping_attachment_failed");
    }
    Ok(())
}

fn allocate_data<'allocator>(
    allocator: &mut frame_allocator<'allocator>,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator grants this callback the exclusive complete extent.
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
        .ok_or("display_payload_extent_invalid")?;
    if length == 0 || length > page_size as usize || destination.length != page_size {
        return Err("display_payload_extent_invalid");
    }
    let destination = usize::try_from(destination.base)
        .map_err(|_| "display_payload_destination_invalid")? as *mut u8;
    // SAFETY: The immutable linker range fits wholly in the exclusive page.
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
        return Err("display_mapping_permissions_invalid");
    }
    Ok(())
}

fn execute_task(
    context: &mut display_context,
    space: &task_space,
    task: task_identity,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    // SAFETY: This single-BSP feature exclusively owns both continuation cells.
    let saved_stack_clear = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) == 0 };
    // SAFETY: The same exclusive interval protects the saved TS cell.
    let saved_task_switched_clear =
        unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) == 0 };
    // SAFETY: IF is clear and NMI is masked during this proof.
    let transition_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    if context.current_task.is_some()
        || context.mailbox_alias == 0
        || context.channel.is_none()
        || context.phase != gate_phase::denied
        || !active_context.load(Ordering::Acquire).is_null()
        || space.payload_length == 0
        || space.payload_length > page_size as usize
        || !saved_stack_clear
        || !saved_task_switched_clear
        || transition_stack_top == 0
        || transition_stack_top & 0x0f != 0
    {
        return Err("display_task_entry_state_invalid");
    }

    context
        .tasks
        .start(task)
        .map_err(|_| "display_task_start_failed")?;
    context.current_task = Some(task);
    context.active_root = space
        .root
        .root_address()
        .map_err(|_| "display_task_root_extent_missing")?;
    context.transition_stack_top = transition_stack_top;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The gate cannot run until publication and IRETQ below.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(saved_task_switched),
            original_task_switched,
        )
    };
    // SAFETY: The validated root borrows the live supervisor branch.
    unsafe { space.root.activate() }.map_err(|_| "display_root_activation_failed")?;
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
        // SAFETY: User entry did not occur, so immediate restoration is safe.
        unsafe { paging::write_cr3(kernel_cr3) };
        return Err("display_context_publication_failed");
    }

    // SAFETY: Identity, mappings, selectors, and gate were validated above.
    let status = unsafe {
        x86_64_display_vessel_enter_task(
            user_code_address,
            user_stack_top,
            u64::from(descriptors::user_data_selector),
            u64::from(descriptors::user_code_selector),
            ptr::addr_of_mut!(saved_kernel_stack),
        )
    };

    active_context.store(ptr::null_mut(), Ordering::Release);
    let task_switched_restored = paging::read_cr0() & cr0_task_switched == original_task_switched;
    // SAFETY: The terminal trampoline restored the original Rust stack.
    unsafe { paging::write_cr3(kernel_cr3) };
    let kernel_root_restored = paging::read_cr3() == kernel_cr3;
    context.current_task = None;
    context.mailbox_alias = 0;
    context.active_root = 0;
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    context.transition_stack_top = 0;
    // SAFETY: Completion consumed both continuation values.
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
        return Err("display_task_completion_restore_failed");
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
        .map_err(|_| "display_code_mapping_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "display_mailbox_mapping_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "display_stack_mapping_withdrawal_failed")?;
    // SAFETY: The inactive root owns no mappings and exclusively owns tables.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "display_address_space_destroy_failed")?;
    // SAFETY: Unmapping returned three exclusive, inactive data leases.
    unsafe { allocator.release_with(&mut code, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "display_code_frame_release_failed")?;
    // SAFETY: The mailbox lease is inactive and exclusive.
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "display_mailbox_frame_release_failed")?;
    // SAFETY: The stack lease is inactive and exclusive.
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "display_stack_frame_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "display_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "display_root_detachment_failed")?;
    Ok(())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_display_vessel_gate_dispatch(frame: *mut rite_gate_frame) {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<u64>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("display_gate_reentry_or_frame_invalid");
    }
    let context_pointer = active_context.load(Ordering::Acquire);
    if context_pointer.is_null() {
        fail("display_gate_without_active_context");
    }
    // SAFETY: Publication identifies the unique live kernel-stack context.
    let context = unsafe { &mut *context_pointer };
    // SAFETY: Assembly supplies exactly 22 aligned qwords for this dispatch.
    let frame = unsafe { &mut *frame };
    if !valid_gate_frame(context, frame) {
        fail("display_gate_frame_validation_failed");
    }

    let reply_copies_before = context.reply_copy_count;
    match dispatch_gate(context) {
        Ok(gate_action::return_to_user(reply)) => {
            // SAFETY: The active task retains its exclusive mailbox alias.
            if unsafe { write_reply(context.mailbox_alias, reply) }.is_err() {
                fail("display_reply_copy_failed");
            }
            context.reply_copy_count += 1;
            compiler_fence(Ordering::SeqCst);
            gate_active.store(false, Ordering::Release);
        }
        Ok(gate_action::complete) => {
            context.terminal_reply_copied = context.reply_copy_count != reply_copies_before;
            if context.terminal_reply_copied {
                fail("display_terminal_reply_was_copied");
            }
            // SAFETY: Both values were written once immediately before IRETQ.
            let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
            // SAFETY: The same single-BSP publication protects the TS value.
            let original_task_switched =
                unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
            if saved_stack == 0
                || saved_stack & (mem::align_of::<u64>() as u64 - 1) != 0
                || !matches!(original_task_switched, 0 | cr0_task_switched)
                || paging::read_cr0() & cr0_task_switched == 0
            {
                fail("display_completion_continuation_invalid");
            }
            gate_active.store(false, Ordering::Release);
            active_context.store(ptr::null_mut(), Ordering::Release);
            // SAFETY: The terminal task is exited and no reply was copied.
            unsafe {
                x86_64_display_vessel_resume_task(
                    status_completed,
                    saved_stack,
                    original_task_switched,
                )
            }
        }
        Err(reason) => fail(reason),
    }
}

enum gate_action {
    return_to_user(rite_reply),
    complete,
}

fn valid_gate_frame(context: &display_context, frame: &rite_gate_frame) -> bool {
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
    // SAFETY: IF is clear and NMI is masked for this single-BSP proof.
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
        gate_phase::denied => ptr::addr_of!(x86_64_display_vessel_denied_return),
        gate_phase::malformed => ptr::addr_of!(x86_64_display_vessel_malformed_return),
        gate_phase::bounds => ptr::addr_of!(x86_64_display_vessel_bounds_return),
        gate_phase::row(y) if y < display_height => {
            ptr::addr_of!(x86_64_display_vessel_row_return)
        }
        gate_phase::task_exit => ptr::addr_of!(x86_64_display_vessel_exit_return),
        gate_phase::row(_) | gate_phase::idle => return None,
    };
    let start = ptr::addr_of!(x86_64_display_vessel_user_start);
    let offset = (symbol as usize).checked_sub(start as usize)?;
    (offset < page_size as usize).then_some(user_code_address + offset as u64)
}

fn dispatch_gate(context: &mut display_context) -> Result<gate_action, &'static str> {
    let phase = context.phase;
    let expected_sequence = phase.sequence().ok_or("display_gate_idle_phase")?;
    if context.calls as u64 + 1 != expected_sequence {
        return Err("display_call_order_invalid");
    }
    // SAFETY: The active task retains its exclusive identity-mapped mailbox.
    let call = unsafe { read_call(context.mailbox_alias) }?;
    validate_fixture_call(phase, &call)?;

    let copies_before = context.body_copy_count;
    let completion = {
        let channel = context.channel.as_mut().ok_or("display_channel_not_live")?;
        let decision = channel
            .assess(
                &context.tasks,
                context.current_task.ok_or("display_task_missing")?,
                call,
            )
            .map_err(|_| "display_assessment_failed")?;
        match decision {
            rite_decision::rejected(reply) => (reply, false),
            rite_decision::admitted(admission) => complete_admitted(
                phase,
                admission,
                &mut context.tasks,
                &mut context.presenter,
                context.mailbox_alias,
                &mut context.scratch,
                &mut context.body_copy_count,
                &mut context.body_bytes_copied,
                &mut context.patches_committed,
                &mut context.exit_code,
            )?,
        }
    };
    let (reply, terminal) = completion;
    let expected_copies = expected_copy_count_after(phase).ok_or("display_copy_count_missing")?;
    if context.body_copy_count != expected_copies
        || context.body_copy_count < copies_before
        || !scratch_is_zero(&context.scratch)
    {
        return Err("display_body_copy_order_invalid");
    }
    validate_reply(phase, reply)?;

    if matches!(
        phase,
        gate_phase::denied | gate_phase::malformed | gate_phase::bounds
    ) {
        if !context.presenter.proof_canary_is_intact() {
            return Err("display_rejection_mutated_surface");
        }
        context.rejection_surface_checks += 1;
    }

    match phase {
        gate_phase::denied => context.rejection_statuses[0] = reply.status,
        gate_phase::malformed => context.rejection_statuses[1] = reply.status,
        gate_phase::bounds => context.rejection_statuses[2] = reply.status,
        gate_phase::task_exit => context.exit_status = reply.status,
        gate_phase::row(_) => {}
        gate_phase::idle => return Err("display_gate_idle_phase"),
    }
    context.calls += 1;

    if terminal {
        if phase != gate_phase::task_exit
            || context
                .tasks
                .snapshot(context.current_task.ok_or("display_task_missing")?)
                .map_err(|_| "display_terminal_snapshot_failed")?
                .state()
                != task_state::exited
        {
            return Err("display_terminal_state_invalid");
        }
        context.phase = gate_phase::idle;
        Ok(gate_action::complete)
    } else {
        context.phase = phase.next().ok_or("display_phase_advanced_past_terminal")?;
        Ok(gate_action::return_to_user(reply))
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_admitted(
    phase: gate_phase,
    admission: rite_admission<'_>,
    tasks: &mut task_table,
    presenter: &mut framebuffer_presenter,
    mailbox: u64,
    scratch: &mut rite_scratch,
    body_copy_count: &mut usize,
    body_bytes_copied: &mut usize,
    patches_committed: &mut usize,
    exit_code: &mut Option<i32>,
) -> Result<(rite_reply, bool), &'static str> {
    let vessel = admission.vessel();
    match phase {
        gate_phase::denied | gate_phase::idle => Err("display_unexpected_admission"),
        gate_phase::malformed => {
            // SAFETY: `mailbox` is the exact active task's exclusive alias.
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
            let patch = decode_display_patch(scratch);
            let pixels = display_patch_pixels(scratch, admission.payload_size())?;
            if patch.has_supported_body() || presenter.prepare_patch(&patch, pixels).is_ok() {
                return Err("display_malformed_patch_was_supported");
            }
            let reply = admission
                .reject_after_revalidation(tasks, rite_status::bad_payload)
                .map_err(|_| "display_malformed_rejection_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("display_malformed_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::bounds => {
            // SAFETY: `mailbox` is the exact active task's exclusive alias.
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
            let patch = decode_display_patch(scratch);
            let pixels = display_patch_pixels(scratch, admission.payload_size())?;
            if !patch.has_supported_body()
                || patch.x() != display_width
                || patch.y() != 0
                || patch.width() != 1
                || patch.height() != 1
                || presenter.prepare_patch(&patch, pixels).is_ok()
            {
                return Err("display_bounds_fixture_invalid");
            }
            let reply = admission
                .reject_after_revalidation(tasks, rite_status::bad_payload)
                .map_err(|_| "display_bounds_rejection_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("display_bounds_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::row(y) if y < display_height => {
            // SAFETY: `mailbox` is the exact active task's exclusive alias.
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
            let patch = decode_display_patch(scratch);
            let pixels = display_patch_pixels(scratch, admission.payload_size())?;
            if patch.x() != 0
                || patch.y() != y
                || patch.width() != display_width
                || patch.height() != 1
                || patch.row_bytes() != display_row_pixel_bytes
                || patch.pixel_byte_length() != display_row_pixel_bytes
            {
                return Err("display_row_fixture_invalid");
            }
            let prepared = presenter
                .prepare_patch(&patch, pixels)
                .map_err(|_| "display_row_prepare_failed")?;
            let reply = admission
                .finish_ok(tasks, |_, exact_vessel| {
                    if exact_vessel != vessel {
                        fail("display_row_identity_changed");
                    }
                    prepared.commit();
                    *patches_committed += 1;
                })
                .map_err(|_| "display_row_completion_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("display_row_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::row(_) => Err("display_row_index_invalid"),
        gate_phase::task_exit => {
            // SAFETY: `mailbox` is the exact active task's exclusive alias.
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
                return Err("display_exit_body_invalid");
            }
            let reply = admission
                .finish_ok(tasks, |tasks, exact_vessel| {
                    if exact_vessel != vessel || tasks.exit(exact_vessel).is_err() {
                        fail("display_task_exit_commit_failed");
                    }
                })
                .map_err(|_| "display_task_exit_completion_failed")?;
            *exit_code = Some(payload.code);
            if !clear_and_verify_bytes(scratch) {
                return Err("display_terminal_scratch_clear_failed");
            }
            Ok((reply, true))
        }
    }
}

const fn expected_copy_count_after(phase: gate_phase) -> Option<usize> {
    match phase {
        gate_phase::denied => Some(0),
        gate_phase::malformed => Some(1),
        gate_phase::bounds => Some(2),
        gate_phase::row(y) if y < display_height => Some(y as usize + 3),
        gate_phase::task_exit => Some(expected_body_copies),
        gate_phase::row(_) | gate_phase::idle => None,
    }
}

fn validate_fixture_call(phase: gate_phase, call: &rite_call) -> Result<(), &'static str> {
    if call.header_size as usize != call_size
        || call.reserved != 0
        || call.trace_id != phase.trace().ok_or("display_fixture_trace_phase_invalid")?
    {
        return Err("display_fixture_header_invalid");
    }
    let exact = match phase {
        gate_phase::denied => {
            call.operation == rite_operation::display_update
                && call.payload_kind == rite_payload_kind::display_patch
                && call.rights_present == rite_rights::console_write
                && call.payload_size == display_small_payload_size
                && call.payload_limit == display_small_payload_size
        }
        gate_phase::malformed | gate_phase::bounds => {
            call.operation == rite_operation::display_update
                && call.payload_kind == rite_payload_kind::display_patch
                && call.rights_present == rite_rights::display_update
                && call.payload_size == display_small_payload_size
                && call.payload_limit == display_small_payload_size
        }
        gate_phase::row(y) if y < display_height => {
            call.operation == rite_operation::display_update
                && call.payload_kind == rite_payload_kind::display_patch
                && call.rights_present == rite_rights::display_update
                && call.payload_size == display_row_payload_size
                && call.payload_limit == display_row_payload_size
        }
        gate_phase::task_exit => {
            call.operation == rite_operation::task_exit
                && call.payload_kind == rite_payload_kind::exit_status
                && call.rights_present == rite_rights::task_exit
                && call.payload_size == mem::size_of::<rite_exit_payload>() as u32
                && call.payload_limit == mem::size_of::<rite_exit_payload>() as u32
        }
        gate_phase::row(_) | gate_phase::idle => return Err("display_fixture_idle_phase"),
    };
    if !exact {
        return Err(match phase {
            gate_phase::denied => "display_fixture_denied_shape_invalid",
            gate_phase::malformed => "display_fixture_malformed_shape_invalid",
            gate_phase::bounds => "display_fixture_bounds_shape_invalid",
            gate_phase::row(_) => "display_fixture_row_shape_invalid",
            gate_phase::task_exit => "display_fixture_exit_shape_invalid",
            gate_phase::idle => "display_fixture_idle_phase",
        });
    }
    Ok(())
}

fn validate_reply(phase: gate_phase, reply: rite_reply) -> Result<(), &'static str> {
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status != phase.expected_status().ok_or("display_status_missing")?
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence != phase.sequence().ok_or("display_sequence_missing")?
        || reply.trace_id != phase.trace().ok_or("display_trace_missing")?
    {
        return Err("display_reply_invalid");
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
        // SAFETY: The allocator extent remains identity mapped and exclusive.
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
        r#"{{"schema":1,"event":"kernel.display_vessel_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
