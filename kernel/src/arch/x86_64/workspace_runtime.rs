//! One persistent, isolated UI owner. Only execution, copied input, and complete
//! frame presentation live here; window and keyboard policy live in the vessel.

use super::address_space::address_space;
use super::rite_transport::{
    decode_display_patch, decode_exit_payload, display_patch_pixels, read_call,
};
use super::scheduler_test::{
    frame_address_matches_stack, scheduler_frame, user_return_fields_are_valid_within_stack,
    x86_64_scheduler_enter, x86_64_scheduler_resume_kernel,
};
use super::{descriptors, exceptions, interrupts, paging};
use crate::boot_omen::framebuffer_info;
use crate::circle::{circle_child_kind, circle_table};
use crate::console::{screen_console, serial_port};
use crate::display::{framebuffer_presenter, rgb8_frame_byte_length, rgb8_frame_stage};
use crate::identity::{fixture_principal_id, task_identity};
use crate::input::{key_state, physical_input_delivery};
use crate::memory::{
    self,
    physical::{frame_allocator, frame_lease, frame_purpose, physical_range},
};
use crate::rite::{rite_channel, rite_decision};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};
use anomalous_abi::{rite_operation, rite_physical_input_v1, rite_reply, rite_rights, rite_status};
use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

#[path = "workspace_mailbox.rs"]
mod workspace_mailbox;

const page: u64 = 4096;
const image_address: u64 = 0x0000_6000_0000_1000;
const mailbox_address: u64 = 0x0000_6000_0008_0000;
const stack_address: u64 = 0x0000_6000_0009_0000;
const stack_top: u64 = stack_address + 16 * page;
const rights: u64 = rite_rights::input_read
    | rite_rights::display_update
    | rite_rights::task_yield
    | rite_rights::task_exit;
const watchdog_ticks: u64 = 200;
const user_flags: u64 = 0x202;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    static x86_64_workspace_service_payload_start: u8;
    static x86_64_workspace_service_payload_end: u8;
    static x86_64_workspace_service_data_start: u8;
    static x86_64_workspace_service_data_end: u8;
    static x86_64_workspace_service_fault_instruction: u8;
    fn x86_64_workspace_rite_gate_entry();
}

struct runtime {
    tasks: task_table,
    task: task_identity,
    channel: rite_channel,
    root: address_space,
    mailbox: physical_range,
    privilege: physical_range,
    frame: *mut scheduler_frame,
    bounds: task_bounds,
    kernel_cr3: u64,
    kernel_stack: u64,
    original_ts: u64,
    scratch: [u8; page as usize],
    stage: Option<rgb8_frame_stage<'static>>,
    presenter: framebuffer_presenter,
    frames: u64,
    progress_tick: u64,
    controlled_fault: u64,
    stopped: Option<&'static str>,
    prefer_pointer: bool,
}

pub(super) fn run(framebuffer: framebuffer_info) -> ! {
    let mut serial = serial_port::com1();
    loop {
        if !interrupts::interrupts_enabled() || !active.load(Ordering::Acquire).is_null() {
            fatal("workspace_entry_state");
        }
        interrupts::disable_interrupts();
        let result = memory::with_frame_allocator(|allocator| run_owned(allocator, framebuffer));
        let (reason, frames) = match result {
            Ok(Ok(value)) => value,
            Ok(Err(reason)) => fatal(reason),
            Err(_) => fatal("workspace_allocator"),
        };
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"workspace_service.withdrawn","level":"info","reason":"{reason}","frames_presented":{frames},"resources_restored":true}}"#
        );
        if !interrupts::enable_external_interrupts() {
            fatal("workspace_interrupt_restore");
        }
        // SAFETY: Every native owner has retired before this exclusive console
        // starts. Returning from the shell drops it before the next presenter.
        let screen = unsafe { screen_console::with_framebuffer(framebuffer) };
        crate::shell::run_workspace_session(
            screen,
            &mut serial,
            interrupts::workspace_input_available(),
        );
        // `desktop` requests a fresh service, not recovery of the old windows.
        // Reuse this continuation without recursively growing the kernel stack.
    }
}

fn allocate(
    allocator: &mut frame_allocator<'_>,
    count: usize,
    alignment: usize,
) -> Result<frame_lease, &'static str> {
    // SAFETY: Allocation grants one exclusive extent to the clearing callback.
    unsafe {
        allocator.allocate_with(count, alignment, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "workspace_allocate")
}

fn symbol(value: *const u8) -> u64 {
    core::hint::black_box(value) as u64
}

fn run_owned(
    allocator: &mut frame_allocator<'_>,
    framebuffer: framebuffer_info,
) -> Result<(&'static str, u64), &'static str> {
    let initial = allocator.accounting();
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: IF is clear on the sole BSP with no native task active.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let payload = symbol(ptr::addr_of!(x86_64_workspace_service_payload_start));
    let payload_end = symbol(ptr::addr_of!(x86_64_workspace_service_payload_end));
    let length = payload_end
        .checked_sub(payload)
        .ok_or("workspace_payload_extent")?;
    let data = symbol(ptr::addr_of!(x86_64_workspace_service_data_start));
    let data_length = symbol(ptr::addr_of!(x86_64_workspace_service_data_end))
        .checked_sub(data)
        .ok_or("workspace_data_extent")?;
    let fault_offset = symbol(ptr::addr_of!(x86_64_workspace_service_fault_instruction))
        .checked_sub(payload)
        .ok_or("workspace_fault_symbol")?;
    if length == 0 || length > 0x40000 || data_length > 0x7000 || fault_offset >= length {
        return Err("workspace_payload_contract");
    }
    let image_pages = length.div_ceil(page) as usize;
    let image = allocate(allocator, image_pages, 1)?;
    let data_owner = allocate(allocator, 8, 1)?;
    let stack = allocate(allocator, 16, 1)?;
    let mut privilege_owner = allocate(allocator, 4, 4)?;
    let mut stage_owner = allocate(allocator, 576, 1)?;
    let image_range = image.range().ok_or("workspace_image_range")?;
    let mailbox = data_owner.range().ok_or("workspace_data_range")?;
    let stack_range = stack.range().ok_or("workspace_stack_range")?;
    let privilege = privilege_owner.range().ok_or("workspace_privilege_range")?;
    let stage_range = stage_owner.range().ok_or("workspace_stage_range")?;
    // SAFETY: Inspected linked payloads and exclusive zeroed allocations have
    // checked lengths. Only the data segment's file bytes overwrite zero BSS.
    unsafe {
        ptr::copy_nonoverlapping(
            payload as *const u8,
            image_range.base as *mut u8,
            length as usize,
        );
        ptr::copy_nonoverlapping(
            data as *const u8,
            (mailbox.base + page) as *mut u8,
            data_length as usize,
        );
    }
    // SAFETY: This caller owns the allocator and initializes each new table.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "workspace_root")?;
    for (address, owner, writable, executable) in [
        (image_address, image, false, true),
        (mailbox_address, data_owner, true, false),
        (stack_address, stack, true, false),
    ] {
        root.map_owned_extent(
            allocator,
            address,
            owner,
            paging::page_permissions {
                user: true,
                writable,
                executable,
            },
        )
        .map_err(|_| "workspace_mapping")?;
    }
    root.validate_all_mappings()
        .map_err(|_| "workspace_mapping_validation")?;
    for (virtual_base, range, writable, executable) in [
        (image_address, image_range, false, true),
        (mailbox_address, mailbox, true, false),
        (stack_address, stack_range, true, false),
    ] {
        for offset in (0..range.length).step_by(page as usize) {
            let mapping = root
                .validate_mapping(virtual_base + offset)
                .map_err(|_| "workspace_mapping_walk")?;
            if mapping.physical_address != range.base + offset
                || !mapping.permissions.user
                || mapping.permissions.writable != writable
                || mapping.permissions.executable != executable
            {
                return Err("workspace_mapping_permissions");
            }
        }
    }
    for guard in [
        image_address - page,
        image_address + image_pages as u64 * page,
        mailbox_address - page,
        mailbox_address + 8 * page,
        stack_address - page,
        stack_top,
    ] {
        root.validate_absent(guard).map_err(|_| "workspace_guard")?;
    }
    // Both the stage and hardware aperture remain supervisor-only at every
    // page, including aliases borrowed from the boot kernel root.
    for range in [
        stage_range,
        physical_range::new(
            framebuffer.address,
            (framebuffer.pitch * framebuffer.height) as u64,
        ),
    ] {
        let start = range.base & !(page - 1);
        let end = range.end().ok_or("workspace_private_range")?.div_ceil(page) * page;
        for address in (start..end).step_by(page as usize) {
            root.validate_borrowed_supervisor_range(address, page)
                .map_err(|_| "workspace_private_mapping")?;
        }
    }
    allocator
        .bind(&privilege_owner)
        .map_err(|_| "workspace_privilege_bind")?;
    let frame = (privilege.end().ok_or("workspace_privilege_extent")?
        - mem::size_of::<scheduler_frame>() as u64) as *mut scheduler_frame;
    let mut context = scheduler_frame::initial(image_address);
    context.stack_pointer = stack_top;
    context.flags = user_flags;
    // SAFETY: The bound privilege extent owns this aligned complete frame.
    unsafe { ptr::write(frame, context) };
    let bounds = task_bounds::new(
        image_address,
        image_address + length,
        image_address,
        stack_address,
        stack_top,
        stack_top,
    )
    .ok_or("workspace_bounds")?;
    let mut circles = circle_table::new();
    let circle = circles.create().map_err(|_| "workspace_circle")?;
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "workspace_circle_root")?;
    circles
        .attach_children(circle, circle_child_kind::mapping, 3)
        .map_err(|_| "workspace_circle_mappings")?;
    let mut tasks = task_table::new();
    let task = tasks
        .create(
            &mut circles,
            task_binding::new(
                circle,
                fixture_principal_id::new(91).ok_or("workspace_principal")?,
                synthetic_origin_metadata::receiver_role_fixture,
                bounds,
            ),
        )
        .map_err(|_| "workspace_task")?;
    tasks.start(task).map_err(|_| "workspace_task_start")?;
    let channel = rite_channel::try_new(task, rights).map_err(|_| "workspace_channel")?;
    // SAFETY: The private allocation remains owned until stage destruction
    // after task exit. No mapping or other Rust reference exposes these bytes.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(stage_range.base as *mut u8, rgb8_frame_byte_length)
    };
    let stage = rgb8_frame_stage::try_new(storage).map_err(|_| "workspace_stage")?;
    // SAFETY: IF is clear on the BSP. Consuming the diagnostic registration
    // removes the only other path that could construct an aperture writer.
    let withdrawn = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .ok_or("workspace_failure_surface_missing")?;
    if withdrawn.address != framebuffer.address
        || withdrawn.pitch != framebuffer.pitch
        || withdrawn.width != framebuffer.width
        || withdrawn.height != framebuffer.height
        || withdrawn.bits_per_pixel != framebuffer.bits_per_pixel
        || withdrawn.red.position != framebuffer.red.position
        || withdrawn.red.size != framebuffer.red.size
        || withdrawn.green.position != framebuffer.green.position
        || withdrawn.green.size != framebuffer.green.size
        || withdrawn.blue.position != framebuffer.blue.position
        || withdrawn.blue.size != framebuffer.blue.size
    {
        return Err("workspace_failure_surface_mismatch");
    }
    // SAFETY: Boot drawing released its writer and the diagnostic registration
    // was consumed. This owner lasts until teardown, before shell fallback.
    let presenter = unsafe { framebuffer_presenter::try_new(framebuffer) }
        .map_err(|_| "workspace_presenter")?;
    let mut state = runtime {
        tasks,
        task,
        channel,
        root,
        mailbox,
        privilege,
        frame,
        bounds,
        kernel_cr3,
        kernel_stack: 0,
        original_ts: 0,
        scratch: [0; page as usize],
        stage: Some(stage),
        presenter,
        frames: 0,
        progress_tick: 0,
        controlled_fault: image_address + fault_offset,
        stopped: None,
        prefer_pointer: false,
    };
    // SAFETY: IF is clear while selecting the live task root and privilege
    // stack. Its frame explicitly restores IF only after ring-3 entry.
    unsafe {
        if !descriptors::select_privilege_stack_top(
            privilege.end().ok_or("workspace_privilege_extent")?,
        ) {
            return Err("workspace_select_stack");
        }
        paging::write_cr3(
            state
                .root
                .root_address()
                .map_err(|_| "workspace_root_address")?,
        );
    }
    let (pointer_available, pointer_generation) = interrupts::pointer::initialize()?;
    state.progress_tick = unsafe { crate::time::now_if_disabled() }.tick();
    active.store(ptr::from_mut(&mut state), Ordering::Release);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"workspace_pointer.ready","level":"info","available":{pointer_available},"generation":{pointer_generation}}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"workspace_service.ready","level":"info","image_pages":{image_pages},"data_pages":8,"stack_pages":16,"stage_frames":576,"rights":30}}"#
    );
    // SAFETY: State, saved continuation and owned context remain live for the
    // complete persistent vessel lifetime; only stop() returns to this frame.
    unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!(state.kernel_stack),
            ptr::addr_of_mut!(state.original_ts),
        )
    };
    active.store(ptr::null_mut(), Ordering::Release);
    if interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || gate_active.load(Ordering::Acquire)
    {
        return Err("workspace_continuation_restore");
    }
    if !interrupts::pointer::stop() {
        return Err("workspace_pointer_restore");
    }
    let reason = state.stopped.ok_or("workspace_missing_outcome")?;
    let frames = state.frames;
    state
        .channel
        .close_retired()
        .map_err(|_| "workspace_channel_close")?;
    if !state
        .stage
        .take()
        .ok_or("workspace_stage_missing")?
        .clear_and_verify()
    {
        return Err("workspace_stage_clear");
    }
    state.scratch.fill(0);
    for address in [image_address, mailbox_address, stack_address] {
        let mut owner = state
            .root
            .unmap_owned(allocator, address)
            .map_err(|_| "workspace_unmap")?;
        // SAFETY: The inactive root returned one exclusive unbound lease.
        unsafe { allocator.release_with(&mut owner, |range| memory::clear_and_verify(range)) }
            .map_err(|_| "workspace_release")?;
    }
    unsafe {
        state
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "workspace_root_destroy")?;
    allocator
        .unbind(&privilege_owner)
        .map_err(|_| "workspace_privilege_unbind")?;
    unsafe {
        allocator.release_with(&mut privilege_owner, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "workspace_privilege_release")?;
    unsafe { allocator.release_with(&mut stage_owner, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "workspace_stage_release")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, 3)
        .map_err(|_| "workspace_circle_unmap")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "workspace_circle_unroot")?;
    state
        .tasks
        .destroy(&mut circles, state.task)
        .map_err(|_| "workspace_task_destroy")?;
    circles
        .destroy(circle)
        .map_err(|_| "workspace_circle_destroy")?;
    if allocator.accounting() != initial {
        return Err("workspace_accounting");
    }
    // End the aperture owner before restoring the diagnostic metadata. The
    // inactive task/root/stage and every borrowed presentation owner are gone.
    #[allow(clippy::drop_non_drop)] // Retire the unique aperture owner before restoring metadata.
    drop(state.presenter);
    // SAFETY: IF remains clear on the BSP and no framebuffer writer remains.
    if !unsafe { crate::console::restore_retired_interrupt_failure_surface(framebuffer) } {
        return Err("workspace_failure_surface_restore");
    }
    Ok((reason, frames))
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}

/// Validate kernel-owned context identity independently of user register values.
/// A stack overflow or malicious flags are service faults, not kernel defects.
fn frame_owned(state: &runtime, frame: *const scheduler_frame) -> bool {
    if !frame_address_matches_stack(frame, state.privilege)
        || paging::read_cr3() != state.root.root_address().unwrap_or(0)
        || unsafe { descriptors::selected_privilege_stack_top() }
            != state.privilege.end().unwrap_or(0)
        || state
            .tasks
            .snapshot(state.task)
            .map_or(true, |task| task.state() != task_state::running)
    {
        return false;
    }
    true
}

fn user_return_valid(state: &runtime, frame: *const scheduler_frame) -> bool {
    // SAFETY: Every caller first validates the exact complete owned frame.
    let value = unsafe { ptr::read(frame) };
    value.code_segment == u64::from(descriptors::user_code_selector)
        && value.stack_segment == u64::from(descriptors::user_data_selector)
        && value.flags & ((3 << 12) | (1 << 14) | (1 << 10)) == 0
        && value.flags & 0x202 == 0x202
        && state.bounds.contains_stack_pointer(value.stack_pointer)
        && user_return_fields_are_valid_within_stack(value, state.bounds)
}

pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let current = active.load(Ordering::Acquire);
    !current.is_null() && unsafe { frame_owned(&*current, frame.cast()) }
}

/// Called after the external source has been acknowledged. Withdrawing before
/// PIC acknowledgement would leave that source blocked in the fallback shell.
pub(super) fn resume_interrupt(
    frame: *mut interrupts::interrupt_frame,
) -> *mut interrupts::interrupt_frame {
    let state = unsafe { &mut *active.load(Ordering::Acquire) };
    if !frame_owned(state, frame.cast()) {
        fatal("workspace_interrupt_frame");
    }
    if !user_return_valid(state, frame.cast()) {
        stop(state, "user_fault", true);
    }
    frame
}

pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    tick: u64,
) -> *mut interrupts::interrupt_frame {
    let state = unsafe { &mut *active.load(Ordering::Acquire) };
    if !frame_owned(state, frame.cast()) {
        fatal("workspace_timer_frame");
    }
    if !user_return_valid(state, frame.cast()) {
        stop(state, "user_fault", true);
    }
    if tick.saturating_sub(state.progress_tick) > watchdog_ticks {
        stop(state, "watchdog", true);
    }
    frame
}

/// Called only after the exception dispatcher has cleared its reentry guard.
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> ! {
    let state = unsafe { &mut *active.load(Ordering::Acquire) };
    if !frame_owned(state, frame.cast()) {
        fatal("workspace_fault_frame");
    }
    let reason = if snapshot.vector == 6
        && snapshot.instruction_pointer == state.controlled_fault
        && snapshot.error_code == 0
    {
        "controlled_fault"
    } else {
        "user_fault"
    };
    stop(state, reason, true)
}

fn stop(state: &mut runtime, reason: &'static str, faulted: bool) -> ! {
    interrupts::disable_interrupts();
    if faulted {
        state
            .tasks
            .fault_unscheduled(state.task)
            .unwrap_or_else(|_| fatal("workspace_fault_transition"));
    } else {
        state
            .tasks
            .exit(state.task)
            .unwrap_or_else(|_| fatal("workspace_exit_transition"));
    }
    state.stopped = Some(reason);
    // SAFETY: The original root and bootstrap privilege stack remain owned;
    // this abandons the vessel stack only after all borrowed rite owners end.
    unsafe {
        paging::write_cr3(state.kernel_cr3);
        if !descriptors::restore_bootstrap_privilege_stack() {
            fatal("workspace_stack_restore");
        }
    }
    gate_active.store(false, Ordering::Release);
    active.store(ptr::null_mut(), Ordering::Release);
    unsafe { x86_64_scheduler_resume_kernel(1, state.kernel_stack, state.original_ts) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_workspace_rite_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let current = active.load(Ordering::Acquire);
    if current.is_null() || gate_active.swap(true, Ordering::AcqRel) {
        fatal("workspace_gate_state");
    }
    let state = unsafe { &mut *current };
    if !frame_owned(state, frame) {
        fatal("workspace_gate_frame");
    }
    if !user_return_valid(state, frame) || unsafe { (*frame).register_15 } != mailbox_address {
        stop(state, "user_fault", true);
    }
    state.frame = frame;
    // A malformed request belongs to this already validated isolated task.
    // Return to the bootstrap continuation before reclaiming its resources.
    let exit = match handle_call(state) {
        Ok(exit) => exit,
        Err(_) => stop(state, "user_fault", true),
    };
    if exit {
        stop(state, "task_exit", false);
    }
    state.scratch.fill(0);
    gate_active.store(false, Ordering::Release);
    frame
}

fn handle_call(state: &mut runtime) -> Result<bool, &'static str> {
    // All pointer ranges are fixed and rederived from current task-owned
    // mappings. Adjacent private data is excluded from every mailbox copy.
    let call = unsafe { read_call(state.mailbox.base) }?;
    let decision = state
        .channel
        .assess(&state.tasks, state.task, call)
        .map_err(|_| "workspace_admission")?;
    let admission = match decision {
        rite_decision::rejected(reply) => {
            write_reply(state.mailbox.base, reply, &[]);
            return Ok(false);
        }
        rite_decision::admitted(value) => value,
    };
    let operation = admission.operation();
    let mut exit = false;
    let mut body = None;
    let reply = if operation == rite_operation::input_read {
        if let Some(encoded) = poll_input(&mut state.prefer_pointer)? {
            body = Some(encoded);
            admission
                .finish_physical_input_delivery(&state.tasks)
                .map_err(|_| "workspace_input_completion")?
        } else {
            admission
                .finish_would_block(&state.tasks)
                .map_err(|_| "workspace_input_empty")?
        }
    } else if operation == rite_operation::task_yield {
        let reply = admission
            .finish_ok(&mut state.tasks, |_, _| {})
            .map_err(|_| "workspace_yield_completion")?;
        // No user-runtime reference is accessed by kernel-origin IRQs. The
        // next timer bounds this idle interval, and STI/HLT closes lost wake.
        if !unsafe { crate::input::ps2_delivery_ready_if_disabled() }
            && !interrupts::pointer::ready()
        {
            unsafe { asm!("sti", "hlt", "cli", options(nomem, nostack)) };
            state.progress_tick = unsafe { crate::time::now_if_disabled() }.tick();
        }
        reply
    } else if operation == rite_operation::display_update {
        copy_body(
            state.mailbox.base,
            admission.payload_size(),
            &mut state.scratch,
        )?;
        let patch = decode_display_patch(&state.scratch);
        let pixels = display_patch_pixels(&state.scratch, admission.payload_size())?;
        let stage = state.stage.as_mut().ok_or("workspace_stage_missing")?;
        if !patch.has_supported_body()
            || admission.payload_size() != 3112
            || patch.x() != 0
            || patch.width() != 1024
            || patch.height() != 1
            || stage.stage_row(patch.y() as usize, pixels).is_err()
        {
            admission
                .reject_after_revalidation(&state.tasks, rite_status::bad_payload)
                .map_err(|_| "workspace_row_rejection")?
        } else {
            if stage.is_complete() {
                if !state
                    .presenter
                    .prepare_complete_rgb8_frame(
                        stage.completion().map_err(|_| "workspace_stage_complete")?,
                    )
                    .map_err(|_| "workspace_present_preflight")?
                    .commit_and_verify()
                {
                    return Err("workspace_frame_readback");
                }
                state.frames = state
                    .frames
                    .checked_add(1)
                    .ok_or("workspace_frame_sequence")?;
                let mut serial = serial_port::com1();
                let sequence = state.frames;
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"workspace_service.frame_presented","level":"info","sequence":{sequence},"rows":768,"width":1024,"height":768}}"#
                );
                state.stage = Some(
                    state
                        .stage
                        .take()
                        .ok_or("workspace_stage_missing")?
                        .recycle()
                        .map_err(|_| "workspace_stage_recycle")?,
                );
            }
            state.progress_tick = unsafe { crate::time::now_if_disabled() }.tick();
            admission
                .finish_ok(&mut state.tasks, |_, _| {})
                .map_err(|_| "workspace_display_completion")?
        }
    } else if operation == rite_operation::task_exit {
        copy_body(
            state.mailbox.base,
            admission.payload_size(),
            &mut state.scratch,
        )?;
        let payload = decode_exit_payload(&state.scratch)?;
        if !payload.has_supported_body() {
            let reply = admission
                .reject_after_revalidation(&state.tasks, rite_status::bad_payload)
                .map_err(|_| "workspace_exit_rejection")?;
            write_reply(state.mailbox.base, reply, &[]);
            return Ok(false);
        }
        let reply = admission
            .finish_ok(&mut state.tasks, |_, _| {})
            .map_err(|_| "workspace_exit_completion")?;
        exit = true;
        reply
    } else {
        admission
            .reject_after_revalidation(&state.tasks, rite_status::denied)
            .map_err(|_| "workspace_operation_rejection")?
    };
    if !exit {
        write_reply(
            state.mailbox.base,
            reply,
            body.as_ref().map_or(&[], |value| value.encode()),
        );
    }
    Ok(exit)
}

fn poll_input(preference: &mut bool) -> Result<Option<rite_physical_input_v1>, &'static str> {
    // Alternating first choice bounds starvation while preserving each source's
    // own FIFO and generation order. No cross-source ordering is invented.
    let prefer_pointer = *preference;
    *preference = !prefer_pointer;
    if prefer_pointer {
        match interrupts::pointer::poll()? {
            Some(value) => Ok(Some(value)),
            None => poll_keyboard(),
        }
    } else {
        match poll_keyboard()? {
            Some(value) => Ok(Some(value)),
            None => interrupts::pointer::poll(),
        }
    }
}

fn poll_keyboard() -> Result<Option<rite_physical_input_v1>, &'static str> {
    // SAFETY: The native rite gate holds IF clear throughout copied delivery.
    let Some(delivery) = (unsafe { crate::input::poll_physical_input_if_disabled() }) else {
        return Ok(None);
    };
    let encoded = match delivery {
        physical_input_delivery::transition(record) => {
            let transition = record.payload();
            rite_physical_input_v1::key(
                record.source().generation(),
                record.sequence().get(),
                transition.key().make_code(),
                transition.key().is_extended(),
                transition.state() == key_state::pressed,
            )
        }
        physical_input_delivery::loss(loss) => {
            let new_generation =
                interrupts::rearm_keyboard_input_after_loss().ok_or("workspace_input_rearm")?;
            rite_physical_input_v1::loss(
                loss.source().generation(),
                new_generation,
                loss.first_sequence()
                    .ok_or("workspace_input_loss_identity")?
                    .get(),
                loss.last_sequence()
                    .ok_or("workspace_input_loss_identity")?
                    .get(),
                loss.lost_count(),
            )
        }
    }
    .ok_or("workspace_input_encoding")?;
    Ok(Some(encoded))
}

fn write_reply(mailbox: u64, reply: rite_reply, body: &[u8]) {
    // SAFETY: Runtime validation proves this current task's complete mailbox
    // is writable. Invalidate first and publish the supported header last.
    unsafe {
        let target = mailbox as *mut u8;
        for offset in 0..40 {
            ptr::write_volatile(target.add(offset), 0);
        }
        for offset in 0..48 {
            ptr::write_volatile(
                target.add(40 + offset),
                body.get(offset).copied().unwrap_or(0),
            );
        }
        let prefix = super::rite_transport::encode_reply_bytes(reply);
        for (offset, byte) in prefix.iter().enumerate().skip(2) {
            ptr::write_volatile(target.add(offset), *byte);
        }
        ptr::write_volatile(target.add(1), prefix[1]);
        ptr::write_volatile(target, prefix[0]);
    }
}

pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot fault-path initialization owns the IDT with IF clear.
    unsafe {
        super::idt::install_gate(
            0xf3,
            x86_64_workspace_rite_gate_entry as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}

fn fatal(reason: &'static str) -> ! {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"workspace_service.failed","level":"fatal","reason":"{reason}"}}"#
    );
    interrupts::disable_external_interrupts();
    super::qemu_exit_failure();
    super::halt_forever()
}

fn copy_body(mailbox: u64, size: u32, scratch: &mut [u8; 4096]) -> Result<(), &'static str> {
    // SAFETY: Admission and the exact owned normal-memory mailbox mapping
    // bound this copy. The sole vessel is stopped in its rite, and scratch is
    // disjoint kernel memory. No user references escape into decoding.
    unsafe { workspace_mailbox::copy_body(mailbox as *const u8, size as usize, scratch) }
}
