//! Opt-in workspace over one retained low-address launch. The optional eager
//! CPU profile owns one legacy FP image; the default retains GPR-only TS entry.
//! Input/window policy remains in the existing vessel. This proves admission,
//! guarded cleanup and rendering, not three-owner scheduling or editor IPC.

#[cfg(feature = "owned_workspace_cpu_test")]
use super::application_cpu::{x86_64_application_enter, x86_64_application_resume_kernel};
use super::owned_execution::{owned_execution_group, tls_bases_are_zero, user_stack_top};
use super::physical_access::proof_window_base;
use super::physical_window::planned_physical_window;
use super::rite_transport::{
    call_size, decode_call_bytes, decode_display_patch, decode_exit_payload, display_patch_pixels,
};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
#[cfg(not(feature = "owned_workspace_cpu_test"))]
use super::scheduler_test::{x86_64_scheduler_enter, x86_64_scheduler_resume_kernel};
#[cfg(feature = "owned_workspace_cpu_test")]
use super::workspace_cpu::{capture_kind, session};
use super::{descriptors, exceptions, interrupts, paging};
use crate::boot_omen::framebuffer_info;
use crate::circle::circle_table;
use crate::console::{screen_console, serial_port};
use crate::display::{framebuffer_presenter, rgb8_frame_byte_length, rgb8_frame_stage};
use crate::identity::task_identity;
use crate::input::{key_state, physical_input_delivery};
use crate::memory::{
    self,
    physical::{frame_allocator, frame_lease, frame_purpose, physical_range},
};
use crate::rite::{rite_channel, rite_decision};
use crate::task::{task_bounds, task_state, task_table};
use anomalous_abi::{rite_operation, rite_physical_input_v1, rite_reply, rite_rights, rite_status};
use core::arch::asm;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

#[path = "workspace_mailbox.rs"]
mod workspace_mailbox;

const page: u64 = 4096;
const image_address: u64 = 0x400000;
const mailbox_address: u64 = 0x800000;
const stack_address: u64 = user_stack_top - 16 * page;
const stack_top: u64 = user_stack_top;
const data_probe: u64 = 0x6f776e65645f7277;
const stack_probe: u64 = 0x6f776e65645f7374;
const rights: u64 = rite_rights::input_read
    | rite_rights::display_update
    | rite_rights::task_yield
    | rite_rights::task_exit;
const watchdog_ticks: u64 = 200;
const user_flags: u64 = 0x202;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_payload_start"
    )]
    static x86_64_owned_workspace_service_payload_start: u8;
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_payload_end"
    )]
    static x86_64_owned_workspace_service_payload_end: u8;
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_data_start"
    )]
    static x86_64_owned_workspace_service_data_start: u8;
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_data_end"
    )]
    static x86_64_owned_workspace_service_data_end: u8;
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_fault_instruction"
    )]
    static x86_64_owned_workspace_service_fault_instruction: u8;
    #[cfg_attr(
        feature = "owned_workspace_cpu_test",
        link_name = "x86_64_owned_workspace_cpu_service_guard_instruction"
    )]
    static x86_64_owned_workspace_service_guard_instruction: u8;
    #[cfg(feature = "owned_workspace_cpu_test")]
    static x86_64_owned_workspace_cpu_service_cpu_probe_return: u8;
    fn x86_64_workspace_rite_gate_entry();
}

struct runtime {
    tasks: task_table,
    task: task_identity,
    channel: rite_channel,
    execution: owned_execution_group<1>,
    circles: circle_table,
    mailbox: physical_range,
    frame: *mut scheduler_frame,
    bounds: task_bounds,
    kernel_stack: u64,
    #[cfg(not(feature = "owned_workspace_cpu_test"))]
    original_ts: u64,
    #[cfg(feature = "owned_workspace_cpu_test")]
    cpu: &'static mut session,
    #[cfg(feature = "owned_workspace_cpu_test")]
    cpu_probe_return: u64,
    scratch: [u8; page as usize],
    stage: Option<rgb8_frame_stage<'static>>,
    presenter: framebuffer_presenter,
    frames: u64,
    progress_tick: u64,
    controlled_fault: u64,
    guard_fault: u64,
    scenario: u64,
    stopped: Option<&'static str>,
    prefer_pointer: bool,
}

pub(super) fn run(framebuffer: framebuffer_info) -> ! {
    let mut serial = serial_port::com1();
    if !interrupts::interrupts_enabled() || !active.load(Ordering::Acquire).is_null() {
        fatal("workspace_entry_state");
    }
    interrupts::disable_interrupts();
    // These are trusted build-time admission scenarios, never user authority.
    // Each fault is followed by complete retirement before another owner exists.
    for scenario in [1, 2] {
        let (reason, frames) =
            memory::with_frame_allocator(|allocator| run_owned(allocator, framebuffer, scenario))
                .unwrap_or_else(|_| fatal("workspace_allocator"));
        if reason
            != if scenario == 1 {
                "guard_fault"
            } else {
                "nx_fault"
            }
            || frames != 0
        {
            fatal("workspace_fault_probe_outcome");
        }
    }
    loop {
        interrupts::disable_interrupts();
        let (reason, frames) =
            memory::with_frame_allocator(|allocator| run_owned(allocator, framebuffer, 0))
                .unwrap_or_else(|_| fatal("workspace_allocator"));
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"workspace_service.withdrawn","level":"info","reason":"{reason}","frames_presented":{frames},"resources_restored":true}}"#
        );
        if !interrupts::enable_external_interrupts() {
            fatal("workspace_interrupt_restore");
        }
        // SAFETY: Every launch/stage/presenter owner has retired before shell.
        let screen = unsafe { screen_console::with_framebuffer(framebuffer) };
        crate::shell::run_workspace_session(
            screen,
            &mut serial,
            interrupts::workspace_input_available(),
        );
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
    scenario: u64,
) -> (&'static str, u64) {
    let initial = allocator.accounting();
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: IF is clear on the sole BSP with no native task active.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    if interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || scenario > 2
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
        || !active.load(Ordering::Acquire).is_null()
    {
        fatal("owned_workspace_initial_state");
    }
    let payload = symbol(ptr::addr_of!(x86_64_owned_workspace_service_payload_start));
    let length = symbol(ptr::addr_of!(x86_64_owned_workspace_service_payload_end))
        .checked_sub(payload)
        .unwrap_or_else(|| fatal("workspace_payload_extent"));
    let data = symbol(ptr::addr_of!(x86_64_owned_workspace_service_data_start));
    let data_length = symbol(ptr::addr_of!(x86_64_owned_workspace_service_data_end))
        .checked_sub(data)
        .unwrap_or_else(|| fatal("workspace_data_extent"));
    let fault_offset = symbol(ptr::addr_of!(
        x86_64_owned_workspace_service_fault_instruction
    ))
    .checked_sub(payload)
    .unwrap_or_else(|| fatal("workspace_fault_symbol"));
    let guard_offset = symbol(ptr::addr_of!(
        x86_64_owned_workspace_service_guard_instruction
    ))
    .checked_sub(payload)
    .unwrap_or_else(|| fatal("workspace_guard_symbol"));
    if length == 0
        || length > 32 * page
        || data_length > 3 * page
        || fault_offset.checked_add(2).is_none_or(|end| end > length)
        || guard_offset.checked_add(3).is_none_or(|end| end > length)
    {
        fatal("workspace_payload_contract");
    }
    #[cfg(feature = "owned_workspace_cpu_test")]
    let cpu_probe_return = {
        let offset = symbol(ptr::addr_of!(
            x86_64_owned_workspace_cpu_service_cpu_probe_return
        ))
        .checked_sub(payload)
        .unwrap_or_else(|| fatal("workspace_cpu_probe_symbol"));
        if offset < 2 || offset >= length || data_length > page {
            fatal("workspace_cpu_probe_extent");
        }
        // SAFETY: Immutable linked RX span contains the complete marker prefix.
        if unsafe { ptr::read((payload + offset - 2) as *const [u8; 2]) } != [0xcd, 0xf3] {
            fatal("workspace_cpu_probe_marker");
        }
        image_address + offset
    };
    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let mut execution = owned_execution_group::<1>::new();
    // SAFETY: The controller selects this exact role independently of artifact
    // fields. Source extents are immutable linked rodata and every acquired
    // owner stays in this diverging frame on error; no failure drops ownership.
    unsafe {
        execution.prepare_workspace(
            allocator,
            &mut circles,
            (payload, length),
            (data, data_length),
        )
    }
    .unwrap_or_else(|reason| fatal(reason));
    let prepared = allocator.accounting();
    if prepared.free.checked_add(69) != Some(initial.free)
        || prepared.data_owned != 60
        || prepared.table_owned != 9
        || prepared.active_leases != 5
        || prepared.bound_leases != 5
    {
        fatal("owned_workspace_prepared_accounting");
    }
    // Population checked every RX/RW byte and zero tail. Probe slots must be
    // fresh on every launch, including after both intentional fault retirements.
    if execution.read_data(0, 0x4000) != Some(0)
        || execution.read_data(0, 0x7ff8) != Some(0)
        || execution.read_stack(0, 0) != Some(0)
    {
        fatal("owned_workspace_probe_not_zero");
    }
    let task = execution
        .commit(0, &mut tasks, &mut circles, 91)
        .unwrap_or_else(|reason| fatal(reason));
    let bounds = execution
        .admission(0)
        .unwrap_or_else(|reason| fatal(reason))
        .bounds();
    if bounds.code_start() != image_address
        || bounds.entry() != image_address
        || bounds.initial_stack_pointer() != stack_top
        || bounds.stack_start() != stack_address
        || bounds.stack_end() != stack_top
    {
        fatal("owned_workspace_bounds");
    }
    let frame = execution.frame(0).unwrap_or_else(|reason| fatal(reason));
    // SAFETY: Complete retained frame; no execution consumer exists yet.
    unsafe {
        (*frame).flags = user_flags;
        (*frame).destination_index = scenario;
    }
    tasks
        .start(task)
        .unwrap_or_else(|_| fatal("workspace_task_start"));
    let channel =
        rite_channel::try_new(task, rights).unwrap_or_else(|_| fatal("workspace_channel"));
    let mut stage_owner = allocate(allocator, 576, 1).unwrap_or_else(|reason| fatal(reason));
    let stage_range = stage_owner
        .range()
        .unwrap_or_else(|| fatal("workspace_stage_range"));
    let stage_alias = planned_physical_window::new(
        physical_range::new(0, paging::identity_map_limit),
        proof_window_base,
    )
    .and_then(|window| window.plan_virtual_extent(stage_range))
    .unwrap_or_else(|_| fatal("workspace_stage_alias"));
    // SAFETY: The private stage lease remains live until this alias borrow is
    // retired and cleared. The retained physical-access owner maps this exact
    // supervisor-only, NX alias in the bootstrap and every admitted private root.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(stage_alias.base() as *mut u8, rgb8_frame_byte_length)
    };
    let stage = rgb8_frame_stage::try_new(storage).unwrap_or_else(|_| fatal("workspace_stage"));
    // SAFETY: IF is clear; consume the only diagnostic writer registration.
    let withdrawn = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fatal("workspace_failure_surface_missing"));
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
        fatal("workspace_failure_surface_mismatch");
    }
    // SAFETY: Sole aperture owner; all access is scoped to the bootstrap root.
    let presenter = unsafe { framebuffer_presenter::try_new(framebuffer) }
        .unwrap_or_else(|_| fatal("workspace_presenter"));
    // SAFETY: Complete retained residency covers code, state, bootstrap stack,
    // descriptors and the high-alias privilege stack. No user has entered yet.
    unsafe {
        execution
            .activate(allocator)
            .unwrap_or_else(|reason| fatal(reason));
        execution.select(0).unwrap_or_else(|reason| fatal(reason));
    }
    let mailbox = execution
        .workspace_mailbox_alias(0)
        .unwrap_or_else(|reason| fatal(reason));
    let owned = allocator.accounting();
    if owned.free.checked_add(645) != Some(initial.free)
        || owned.data_owned != 636
        || owned.table_owned != 9
        || owned.active_leases != 6
        || owned.bound_leases != 5
    {
        fatal("owned_workspace_stage_accounting");
    }
    let mut state = runtime {
        tasks,
        task,
        channel,
        execution,
        circles,
        mailbox,
        frame,
        bounds,
        kernel_stack: 0,
        #[cfg(not(feature = "owned_workspace_cpu_test"))]
        original_ts: 0,
        #[cfg(feature = "owned_workspace_cpu_test")]
        // SAFETY: Resident CPU storage is acquired before any user entry, under
        // the sole-BSP IF-clear execution owner. The active owner never moves.
        cpu: unsafe { session::acquire() }.unwrap_or_else(|reason| fatal(reason)),
        #[cfg(feature = "owned_workspace_cpu_test")]
        cpu_probe_return,
        scratch: [0; page as usize],
        stage: Some(stage),
        presenter,
        frames: 0,
        progress_tick: 0,
        controlled_fault: image_address + fault_offset,
        guard_fault: image_address + guard_offset,
        scenario,
        stopped: None,
        prefer_pointer: false,
    };
    let mut serial = serial_port::com1();
    let scenario_name = match scenario {
        1 => "guard",
        2 => "nx",
        _ => "desktop",
    };
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"owned_workspace.prepared","level":"info","scenario":"{scenario_name}","code_pages":32,"data_pages":8,"stack_pages":16,"transition_pages":4,"table_frames":9,"owner_frames":69,"owner_leases":5,"stage_frames":576,"populated":true,"zero_tail":true,"guards":true}}"#
    );
    #[cfg(feature = "owned_workspace_cpu_test")]
    {
        let vendor = state.cpu.vendor();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"owned_workspace_cpu.ready","level":"info","scenario":"{scenario_name}","task_images":1,"image_bytes":512,"observation_bytes":512,"cpu_vendor":"{vendor}","initial_clean":true}}"#
        );
    }
    if scenario == 0 {
        let (pointer_available, pointer_generation) =
            interrupts::pointer::initialize().unwrap_or_else(|reason| fatal(reason));
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"workspace_pointer.ready","level":"info","available":{pointer_available},"generation":{pointer_generation}}}"#
        );
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"workspace_service.ready","level":"info","image_pages":32,"data_pages":8,"stack_pages":16,"stage_frames":576,"rights":30}}"#
        );
    }
    state.progress_tick = unsafe { crate::time::now_if_disabled() }.tick();
    active.store(ptr::from_mut(&mut state), Ordering::Release);
    // SAFETY: TS-only GPR execution and the exact owned frame/continuation remain
    // live; stop restores the bootstrap root and RSP0 before the nonlocal return.
    #[cfg(not(feature = "owned_workspace_cpu_test"))]
    unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!(state.kernel_stack),
            ptr::addr_of_mut!(state.original_ts),
        )
    };
    #[cfg(feature = "owned_workspace_cpu_test")]
    unsafe {
        if !state.cpu.restore() {
            fatal("workspace_cpu_initial_restore");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!(state.kernel_stack));
    }
    active.store(ptr::null_mut(), Ordering::Release);
    if interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || gate_active.load(Ordering::Acquire)
    {
        fatal("workspace_continuation_restore");
    }
    if scenario == 0 && !interrupts::pointer::stop() {
        fatal("workspace_pointer_restore");
    }
    if state.execution.read_data(0, 0x4000) != Some(data_probe)
        || state.execution.read_data(0, 0x7ff8) != Some(data_probe)
        || state.execution.read_stack(0, 0) != Some(stack_probe)
    {
        fatal("owned_workspace_wide_probes");
    }
    let reason = state
        .stopped
        .unwrap_or_else(|| fatal("workspace_missing_outcome"));
    let frames = state.frames;
    state
        .channel
        .close_retired()
        .unwrap_or_else(|_| fatal("workspace_channel_close"));
    if !state
        .stage
        .take()
        .unwrap_or_else(|| fatal("workspace_stage_missing"))
        .clear_and_verify()
    {
        fatal("workspace_stage_clear");
    }
    state.scratch.fill(0);
    // SAFETY: Both stage borrow and every hardware/context consumer are gone.
    // The restored bootstrap still maps physical RAM after alias withdrawal.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
            .unwrap_or_else(|reason| fatal(reason));
        allocator
            .release_with(&mut stage_owner, |range| memory::clear_and_verify(range))
            .unwrap_or_else(|_| fatal("workspace_stage_release"));
    }
    if allocator.accounting() != initial {
        fatal("workspace_accounting");
    }
    #[allow(clippy::drop_non_drop)]
    drop(state.presenter);
    // SAFETY: All task/stage/presenter owners retired; IF remains clear.
    if !unsafe { crate::console::restore_retired_interrupt_failure_surface(framebuffer) } {
        fatal("workspace_failure_surface_restore");
    }
    #[cfg(feature = "owned_workspace_cpu_test")]
    {
        let value = state.cpu.evidence();
        let complete = state.cpu.complete();
        // SAFETY: Every execution owner and alias has retired. This is the last
        // access to the exclusive CPU borrow, before this runtime frame returns.
        if !unsafe { state.cpu.release() } {
            fatal("workspace_cpu_release");
        }
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"owned_workspace_cpu.released","level":"info","scenario":"{scenario_name}","gates":{},"timers":{},"inputs":{},"other_irqs":{},"faults":{},"probes":{},"clean_checks":{},"user_checks":{},"x87_timers":{},"mmx_timers":{},"probe_complete":{complete},"controls_restored":true,"storage_cleared":true}}"#,
            value.gates,
            value.timers,
            value.inputs,
            value.other_irqs,
            value.faults,
            value.probes,
            value.clean_checks,
            value.user_checks,
            value.x87_timers,
            value.mmx_timers
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"owned_workspace.released","level":"info","scenario":"{scenario_name}","reason":"{reason}","frames_presented":{frames},"wide_data_probe":true,"wide_stack_probe":true,"root_restored":true,"resources_restored":true}}"#
    );
    (reason, frames)
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}

/// Validate kernel-owned context identity independently of user register values.
/// A stack overflow or malicious flags are service faults, not kernel defects.
fn frame_owned(state: &runtime, frame: *const scheduler_frame) -> bool {
    if !state.execution.frame_owned(0, frame)
        || paging::read_cr3() != state.execution.root(0).unwrap_or(0)
        || unsafe { descriptors::selected_privilege_stack_top() }
            != state.execution.transition_stack_top(0).unwrap_or(0)
        || state.execution.workspace_mailbox_alias(0).ok() != Some(state.mailbox)
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

/// Capture before IRQ source processing, after integer-only frame provenance.
/// CPL0 idle interrupts never call this function or borrow the live runtime.
#[cfg(feature = "owned_workspace_cpu_test")]
pub(super) fn capture_interrupt(frame: *mut interrupts::interrupt_frame, irq: u8) {
    let state = unsafe { &mut *active.load(Ordering::Acquire) };
    if !frame_owned(state, frame.cast())
        || !unsafe {
            state.cpu.capture(match irq {
                0 => capture_kind::timer,
                1 | 12 => capture_kind::input,
                _ => capture_kind::other_irq,
            })
        }
    {
        fatal("workspace_cpu_interrupt_capture");
    }
}

/// Capture a user exception before any exception policy or terminal mutation.
#[cfg(feature = "owned_workspace_cpu_test")]
pub(super) fn capture_fault(frame: *mut exceptions::exception_frame) {
    let state = unsafe { &mut *active.load(Ordering::Acquire) };
    if !frame_owned(state, frame.cast()) || !unsafe { state.cpu.capture(capture_kind::fault) } {
        fatal("workspace_cpu_fault_capture");
    }
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
    #[cfg(feature = "owned_workspace_cpu_test")]
    {
        if !state.cpu.check_stable() {
            fatal("workspace_cpu_user_image");
        }
        if !unsafe { state.cpu.restore() } {
            fatal("workspace_cpu_interrupt_restore");
        }
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
    #[cfg(feature = "owned_workspace_cpu_test")]
    {
        if !state.cpu.check_stable() {
            fatal("workspace_cpu_user_image");
        }
        if let Some(value) = state
            .cpu
            .timer_tick()
            .unwrap_or_else(|reason| fatal(reason))
        {
            state
                .execution
                .write_data(0, super::workspace_cpu::tick_offset, value)
                .unwrap_or_else(|reason| fatal(reason));
        }
        if !unsafe { state.cpu.restore() } {
            fatal("workspace_cpu_interrupt_restore");
        }
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
    #[cfg(feature = "owned_workspace_cpu_test")]
    if !state.cpu.check_stable() {
        fatal("workspace_cpu_fault_image");
    }
    let reason = if let Some(reason) = super::owned_workspace_fault::classify_probe(
        state.scenario,
        &snapshot,
        state.guard_fault,
        stack_top,
    ) {
        reason
    } else if snapshot.vector == 6
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
        state
            .execution
            .restore_bootstrap()
            .unwrap_or_else(|_| fatal("workspace_stack_restore"));
    }
    #[cfg(feature = "owned_workspace_cpu_test")]
    if !unsafe { state.cpu.finish() } {
        fatal("workspace_cpu_finish");
    }
    gate_active.store(false, Ordering::Release);
    active.store(ptr::null_mut(), Ordering::Release);
    #[cfg(not(feature = "owned_workspace_cpu_test"))]
    unsafe {
        x86_64_scheduler_resume_kernel(1, state.kernel_stack, state.original_ts)
    }
    #[cfg(feature = "owned_workspace_cpu_test")]
    unsafe {
        x86_64_application_resume_kernel(1, state.kernel_stack)
    }
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
    #[cfg(feature = "owned_workspace_cpu_test")]
    if !unsafe { state.cpu.capture(capture_kind::gate) } {
        fatal("workspace_cpu_gate_capture");
    }
    if !user_return_valid(state, frame) || unsafe { (*frame).register_15 } != mailbox_address {
        stop(state, "user_fault", true);
    }
    state.frame = frame;
    #[cfg(feature = "owned_workspace_cpu_test")]
    {
        // This build-time marker is not a Rite operation or a user grant.
        let value = unsafe { ptr::read(frame) };
        if value.instruction_pointer == state.cpu_probe_return {
            if value.accumulator != super::workspace_cpu::probe_magic
                || !state.cpu.probe(value.destination_index)
            {
                stop(state, "user_fault", true);
            }
            if matches!(value.destination_index, 1 | 3) {
                state
                    .execution
                    .write_data(0, super::workspace_cpu::tick_offset, 0)
                    .unwrap_or_else(|reason| fatal(reason));
            }
            unsafe {
                (*frame).accumulator = 0;
            }
            gate_active.store(false, Ordering::Release);
            if !unsafe { state.cpu.restore() } {
                fatal("workspace_cpu_probe_restore");
            }
            return frame;
        }
        if !state.cpu.complete() || !state.cpu.check_stable() {
            stop(state, "user_fault", true);
        }
    }
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
    #[cfg(feature = "owned_workspace_cpu_test")]
    if !unsafe { state.cpu.restore() } {
        fatal("workspace_cpu_gate_restore");
    }
    frame
}

fn handle_call(state: &mut runtime) -> Result<bool, &'static str> {
    // All pointer ranges are fixed and rederived from current task-owned
    // mappings. Adjacent private data is excluded from every mailbox copy.
    // The legacy reader requires an identity-mapped physical page. This owner
    // instead retains a supervisor high alias, revalidated by frame_owned before
    // this gate. Copy only the fixed header while IF is clear and the sole
    // userspace writer is stopped; no borrowed user data reaches the decoder.
    let mut prefix = [0_u8; call_size];
    for (offset, byte) in prefix.iter_mut().enumerate() {
        // SAFETY: Exact retained mailbox validation covers this entire prefix,
        // independent of numeric physical-address bounds used by legacy rites.
        *byte = unsafe { ptr::read_volatile((state.mailbox.base as *const u8).add(offset)) };
    }
    let call = decode_call_bytes(&prefix);
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
                let completion = stage.completion().map_err(|_| "workspace_stage_complete")?;
                // SAFETY: The exact unscheduled caller was checked before this
                // integer-only gate. The callback neither enables IF nor touches
                // user mappings; the stage retains its shared supervisor alias.
                let presented = unsafe {
                    state.execution.with_workspace_bootstrap(0, || {
                        state
                            .presenter
                            .prepare_complete_rgb8_frame(completion)
                            .map(|update| update.commit_and_verify())
                    })
                }
                .unwrap_or_else(|_| fatal("workspace_present_root"));
                if !presented.map_err(|_| "workspace_present_preflight")? {
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
