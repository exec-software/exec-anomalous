//! Opt-in three-task native workspace, sole editor and durable document service.
//! A single retained coordinator owns all roots, eager CPU images and routes.
//! Application policy and ANEA/ANER/ANDC parsing stay in userspace.
use super::application_cpu::{
    cpu_session_group, fp_image, x86_64_application_enter, x86_64_application_resume_kernel,
};
use super::owned_execution::{owned_execution_group, tls_bases_are_zero};
use super::physical_access::proof_window_base;
use super::physical_window::planned_physical_window;
use super::rite_transport::{
    call_size, decode_call_bytes, decode_display_patch, decode_exit_payload, display_patch_pixels,
};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
use super::{
    descriptors, exceptions, interrupts, native_interactive_fixture, native_storage_device, paging,
};
use crate::application_channel::{
    application_channel, application_channel_binding, application_channel_error,
    application_channel_phase,
};
use crate::boot_omen::framebuffer_info;
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::display::{framebuffer_presenter, rgb8_frame_byte_length, rgb8_frame_stage};
use crate::identity::{endpoint_id, task_identity};
use crate::input::{key_state, physical_input_delivery};
use crate::memory::{
    self,
    physical::{frame_allocator, frame_lease, frame_purpose, physical_range},
};
use crate::rite::{rite_channel, rite_decision};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;
use anomalous_abi::{rite_operation, rite_physical_input_v1, rite_reply, rite_rights, rite_status};
use anomalous_document_protocol::{
    client_identity, durable::disk_identity, remote_startup::startup_descriptor,
};
use core::arch::asm;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
#[path = "workspace_mailbox.rs"]
mod workspace_mailbox;
const routes: [(usize, usize); 4] = [(0, 1), (1, 0), (1, 2), (2, 1)];
const packet_lengths: [usize; 4] = [64, 192, 128, 128];
const arena_bases: [u64; 3] = [0x810000, 0x804000, 0x814000];
const arena_lengths: [u64; 3] = [36864, 32768, 32768];
const completion_status: u64 = 0x6e6577;
const descriptor_address: u64 = 0x801000;
const mailbox_address: u64 = 0x800000;
const rights: u64 = rite_rights::input_read
    | rite_rights::display_update
    | rite_rights::task_yield
    | rite_rights::task_exit;
const call_limit: u64 = 2_000_000;
unsafe extern "C" {
    fn x86_64_native_editor_workspace_gate_f3();
    fn x86_64_native_editor_workspace_gate_f4();
    static x86_64_native_editor_workspace_workspace_payload_start: u8;
    static x86_64_native_editor_workspace_workspace_payload_end: u8;
    static x86_64_native_editor_workspace_workspace_data_start: u8;
    static x86_64_native_editor_workspace_workspace_data_end: u8;
    static x86_64_native_editor_workspace_application_payload_start: u8;
    static x86_64_native_editor_workspace_application_payload_end: u8;
    static x86_64_native_editor_workspace_application_data_start: u8;
    static x86_64_native_editor_workspace_application_data_end: u8;
}
macro_rules! address {
    ($symbol:ident) => {{
        let value: u64;
        // SAFETY: Materialize linker metadata; do not dereference the symbol.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) value,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        value
    }};
}

#[derive(Clone, Copy, Default)]
struct evidence {
    calls: [u64; 3],
    timers: [u64; 3],
    user_irqs: u64,
    captures: u64,
    clean_checks: u64,
    switches: u64,
    denials: u64,
    bad_copies: u64,
    invalid: u64,
    closed: u64,
    sends: [u64; 4],
    deliveries: [u64; 4],
    acknowledgements: [u64; 4],
    discarded: u64,
    inputs: u64,
    input_losses: u64,
    rows: u64,
}
struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 3],
    task: task_identity,
    execution: owned_execution_group<3>,
    cpu: cpu_session_group<3>,
    observed: fp_image,
    channels: [application_channel; 4],
    last_ack: [u64; 4],
    selected: usize,
    started: [bool; 3],
    terminal: [bool; 3],
    faulted: [bool; 3],
    evidence: evidence,
    last_tick: u64,
    kernel_root: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
    channel: rite_channel,
    mailbox: physical_range,
    scratch: [u8; 4096],
    stage: Option<rgb8_frame_stage<'static>>,
    presenter: Option<framebuffer_presenter>,
    frames: u64,
    frame_hash: u64,
    progress_tick: u64,
    prefer_pointer: bool,
    device: *mut native_storage_device::device,
}
struct retained(UnsafeCell<MaybeUninit<runtime>>);
// SAFETY: Sole BSP; the busy latch covers construction, published dispatch,
// terminal restoration and complete teardown. Never move an active CPU owner.
unsafe impl Sync for retained {}
static runtime_storage: retained = retained(UnsafeCell::new(MaybeUninit::uninit()));
static busy: AtomicBool = AtomicBool::new(false);
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

fn fail(reason: &'static str) -> ! {
    interrupts::disable_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
fn increment(value: &mut u64) {
    *value = value
        .checked_add(1)
        .unwrap_or_else(|| fail("counter_overflow"));
}
fn no_arguments(value: scheduler_frame) -> bool {
    value.destination_index == 0 && value.source_index == 0 && value.data == 0 && value.counter == 0
}
fn status(error: application_channel_error) -> u64 {
    match error {
        application_channel_error::backpressure | application_channel_error::empty => 1,
        application_channel_error::wrong_binding
        | application_channel_error::wrong_producer
        | application_channel_error::wrong_service => 2,
        application_channel_error::copy_failed => 3,
        application_channel_error::closed => 5,
        _ => 4,
    }
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        self.execution.frame_owned(self.selected, frame)
            && self.execution.root(self.selected).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(self.selected).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self.cpu.controls_are_active()
            && self
                .scheduler
                .snapshot(&self.tasks)
                .is_ok_and(|s| s.current() == Some(self.identities[self.selected]))
            && self
                .tasks
                .snapshot(self.identities[self.selected])
                .is_ok_and(|t| t.state() == task_state::running && t.context_attached())
    }
    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        // SAFETY: Exact complete retained transition-stack frame was validated.
        let value = unsafe { ptr::read(frame) };
        self.execution
            .admission(self.selected)
            .is_ok_and(|admission| {
                user_return_fields_are_valid_within_stack(value, admission.bounds())
                    && admission.accepts_return(value.instruction_pointer, value.stack_pointer)
                    && value.flags & !0x25_0fd7 == 0
                    && if self.started[self.selected] {
                        value.flags & 0x202 == 0x202
                    } else {
                        value.flags == 2
                    }
            })
    }
    fn capture(&mut self, frame: *const scheduler_frame) {
        if !self.frame_owned(frame) {
            fail("entry_owner");
        }
        // SAFETY: The exact selected user image and clean observation are
        // resident, exclusive, aligned and never moved during activation.
        if !unsafe { self.cpu.save_current(self.selected) }
            || !unsafe { self.cpu.workspace_clean_observed(&mut self.observed) }
        {
            fail("cpu_capture");
        }
        increment(&mut self.evidence.captures);
        increment(&mut self.evidence.clean_checks);
    }
    fn select(&mut self, identity: task_identity) -> *mut scheduler_frame {
        let index = self
            .identities
            .iter()
            .position(|id| *id == identity)
            .unwrap_or_else(|| fail("dispatch_identity"));
        if self.terminal[index] {
            fail("dispatch_terminal");
        }
        // SAFETY: All roots, frames, aliases and stacks remain retained.
        unsafe { self.execution.select(index) }.unwrap_or_else(|e| fail(e));
        self.selected = index;
        let frame = self.execution.frame(index).unwrap_or_else(|e| fail(e));
        if !self.frame_valid(frame) {
            fail("selected_frame");
        }
        frame
    }
    fn packet(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        let Some(channel) = value
            .destination_index
            .checked_sub(1)
            .and_then(|v| usize::try_from(v).ok())
            .filter(|v| *v < 4)
        else {
            return (2, 0, 0);
        };
        let binding = self.channels[channel].binding();
        let caller = self.identities[self.selected];
        let sequence = value.counter;
        match value.accumulator {
            1 => {
                let prepared = match self.channels[channel].prepare_submit(
                    binding,
                    caller,
                    sequence,
                    value.data as usize,
                ) {
                    Ok(value) => value,
                    Err(error) => return (status(error), 0, 0),
                };
                if value.data != packet_lengths[channel] as u64 {
                    return (4, 0, 0);
                }
                let mut storage = [0; 192];
                let bytes = &mut storage[..packet_lengths[channel]];
                if self
                    .execution
                    .copy_from_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        caller,
                        value.source_index,
                        bytes,
                    )
                    .is_err()
                {
                    return (3, 0, 0);
                }
                let receipt = self.channels[channel]
                    .commit_submit(prepared, bytes)
                    .unwrap_or_else(|_| fail("submit_commit"));
                increment(&mut self.evidence.sends[channel]);
                let mut serial = serial_port::com1();
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.packet","level":"info","channel":{},"sequence":{},"producer":{},"consumer":{},"length":{},"hash":{}}}"#,
                    channel,
                    receipt.sequence(),
                    routes[channel].0,
                    routes[channel].1,
                    bytes.len(),
                    packet_hash(bytes)
                );
                (0, receipt.sequence(), receipt.length() as u64)
            }
            2 => {
                let prepared = match self.channels[channel].prepare_delivery(
                    binding,
                    caller,
                    value.data as usize,
                ) {
                    Ok(value) => value,
                    Err(error) => return (status(error), 0, 0),
                };
                if value.data != packet_lengths[channel] as u64 {
                    return (4, 0, 0);
                }
                let result = self.channels[channel].deliver_with(prepared, |bytes| {
                    self.execution.copy_to_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        caller,
                        value.source_index,
                        bytes,
                    )
                });
                match result {
                    Ok(receipt) => {
                        increment(&mut self.evidence.deliveries[channel]);
                        (0, receipt.sequence(), receipt.length() as u64)
                    }
                    Err(error) => (status(error), 0, 0),
                }
            }
            3 => match self.channels[channel].acknowledge(binding, caller, sequence) {
                Ok(receipt) => {
                    self.last_ack[channel] = receipt.sequence();
                    increment(&mut self.evidence.acknowledgements[channel]);
                    (0, receipt.sequence(), 0)
                }
                Err(error) => (status(error), 0, 0),
            },
            6 => {
                if caller != binding.producer() {
                    return (2, 0, 0);
                }
                if self.channels[channel].phase() == application_channel_phase::closed {
                    return (5, 0, 0);
                }
                if sequence == 0
                    || self.channels[channel]
                        .next_sequence()
                        .is_none_or(|next| sequence >= next)
                {
                    return (4, 0, 0);
                }
                if self.last_ack[channel] == sequence {
                    (0, sequence, 0)
                } else {
                    (1, 0, 0)
                }
            }
            _ => (4, 0, 0),
        }
    }
    fn storage_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        // frame_valid already binds the current task, root and retained context.
        // Only this phase's exact service owner possesses the sealed grant.
        if self.selected != 2 || self.terminal[2] || value.destination_index != 1 {
            return (2, 0, 0);
        }
        let flush = value.accumulator == 14;
        if (flush && (value.source_index != 0 || value.data != 0 || value.counter != 0))
            || (!flush && (value.data != 512 || value.counter >= 5))
        {
            return (4, 0, 0);
        }
        let identity = self.identities[2];
        let mut bytes = [0; 512];
        if value.accumulator == 12 {
            if self
                .execution
                .preflight_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    identity,
                    value.source_index,
                    bytes.len(),
                )
                .is_err()
            {
                return (3, 0, 0);
            }
        } else if value.accumulator == 13
            && self
                .execution
                .copy_from_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    identity,
                    value.source_index,
                    &mut bytes,
                )
                .is_err()
        {
            return (3, 0, 0);
        }
        // SAFETY: Coordinator retains the unique controller owner until both
        // tasks terminate. IF remains clear; no mapping/owner mutation can occur.
        let device = unsafe { &mut *self.device };
        let result = match value.accumulator {
            12 => device.read_sector(value.counter, &mut bytes),
            13 => device.write_sector(value.counter, &bytes),
            14 => device.flush(),
            _ => unreachable!(),
        };
        if result.is_err() {
            clear_sector_scratch(&mut bytes);
            return (6, 0, 0);
        }
        if value.accumulator == 12 {
            self.execution
                .copy_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    identity,
                    value.source_index,
                    &bytes,
                )
                .unwrap_or_else(|_| fail("durable_copy_after_preflight"));
        }
        clear_sector_scratch(&mut bytes);
        (0, value.counter, if flush { 0 } else { 512 })
    }
    fn retire(&mut self, faulted: bool) -> Option<*mut scheduler_frame> {
        let index = self.selected;
        let identity = self.identities[index];
        let mut closed = 0;
        for channel in &mut self.channels {
            let binding = channel.binding();
            if channel.phase() == application_channel_phase::closed
                || (binding.producer() != identity && binding.service() != identity)
            {
                continue;
            }
            let receipt = channel
                .close(binding)
                .unwrap_or_else(|_| fail("route_close"));
            if receipt.discarded() {
                increment(&mut self.evidence.discarded);
            }
            if !channel.storage_is_clear() {
                fail("route_clear");
            }
            closed += 1;
        }
        self.terminal[index] = true;
        self.faulted[index] = faulted;
        let survivors = self.terminal.iter().filter(|terminal| !**terminal).count();
        let mut serial = serial_port::com1();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.retired","level":"info","role":{},"faulted":{},"closed_routes":{},"discarded":{},"survivors":{}}}"#,
            index, faulted, closed, self.evidence.discarded, survivors
        );
        if survivors == 0 {
            if faulted {
                self.scheduler.fault_last_current(&mut self.tasks, identity)
            } else {
                self.scheduler.complete_current(&mut self.tasks, identity)
            }
            .unwrap_or_else(|_| fail("complete_last"));
            None
        } else {
            let dispatch = if faulted {
                self.scheduler
                    .fault_current(&mut self.tasks, identity)
                    .map(|r| r.dispatch())
            } else {
                self.scheduler
                    .complete_current_and_dispatch(&mut self.tasks, identity)
                    .map(|r| r.dispatch())
            }
            .unwrap_or_else(|_| fail("complete_dispatch"));
            increment(&mut self.evidence.switches);
            Some(self.select(dispatch.task()))
        }
    }
    fn finish(&mut self) -> ! {
        // SAFETY: Every user image was captured; all owners remain retained.
        unsafe { self.execution.restore_bootstrap() }.unwrap_or_else(|e| fail(e));
        if paging::read_cr3() != self.kernel_root
            || unsafe { descriptors::selected_privilege_stack_top() } != self.bootstrap_stack
            || self.kernel_stack == 0
            || !unsafe { self.cpu.finish() }
            || !self.observed.clear_workspace_observation()
        {
            fail("bootstrap_restore");
        }
        let stack = self.kernel_stack;
        dispatch_active.store(false, Ordering::Release);
        active.store(ptr::null_mut(), Ordering::Release);
        // SAFETY: The exact suspended bootstrap continuation is restored once.
        unsafe { x86_64_application_resume_kernel(completion_status, stack) }
    }
}
fn enter_dispatch() -> &'static mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("dispatch_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("dispatch_inactive");
    }
    // SAFETY: Sole BSP, IF clear, one retained published owner and one latch.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // SAFETY: All entry work completed under the clean kernel image.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}
pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}
pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let pointer = active.load(Ordering::Acquire);
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}
// IRQ provenance is checked before this captures the selected user image.
// CPL0 idle IRQs never touch this live owner or its dispatch latch.
pub(super) fn capture_interrupt(frame: *mut interrupts::interrupt_frame) {
    let state = enter_dispatch();
    state.capture(frame.cast());
    if !state.frame_valid(frame.cast()) {
        fail("interrupt_frame");
    }
    increment(&mut state.evidence.user_irqs);
}
fn captured_runtime() -> &'static mut runtime {
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null()
        || !dispatch_active.load(Ordering::Acquire)
        || interrupts::interrupts_enabled()
    {
        fail("missing_capture");
    }
    // SAFETY: The IRQ capture holds the sole IF-clear dispatch borrow.
    unsafe { &mut *pointer }
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = captured_runtime();
    if !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("timer_frame");
    }
    state.last_tick = observed.tick();
    increment(&mut state.evidence.timers[state.selected]);
    if observed.tick().saturating_sub(state.progress_tick) > 6000 {
        fail("watchdog");
    }
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn resume_interrupt(
    frame: *mut interrupts::interrupt_frame,
) -> *mut interrupts::interrupt_frame {
    let state = captured_runtime();
    if !state.frame_valid(frame.cast()) {
        fail("input_return_frame");
    }
    leave_dispatch(state, frame.cast()).cast()
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    state.capture(frame.cast());
    if !super::application_fault::classify_user_fault(snapshot) {
        fail("unexpected_user_fault");
    }
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.fault","level":"info","role":{},"vector":{},"ip":{}}}"#,
        state.selected, snapshot.vector, snapshot.instruction_pointer
    );
    state
        .retire(true)
        .map(|next| leave_dispatch(state, next).cast())
}
pub(super) fn resume_kernel_after_fault() -> ! {
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() || !dispatch_active.load(Ordering::Acquire) {
        fail("fault_continuation");
    }
    // SAFETY: The common exception latch was cleared before this continuation.
    let state = unsafe { &mut *pointer };
    if state.terminal != [true; 3] {
        fail("fault_live_owner");
    }
    state.finish()
}
fn yield_task(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // Complete the outgoing reply before selecting another private root.
    unsafe {
        (*frame).accumulator = 0;
        (*frame).counter = 0;
        (*frame).data = 0;
    }
    let dispatch = state
        .scheduler
        .preempt_current(&mut state.tasks, state.identities[state.selected])
        .unwrap_or_else(|_| fail("yield_transition"))
        .dispatch();
    increment(&mut state.evidence.switches);
    state.select(dispatch.task())
}
#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_native_editor_workspace_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = enter_dispatch();
    state.capture(frame);
    if !state.frame_valid(frame) {
        fail("gate_return_frame");
    }
    // SAFETY: The complete retained current-owner frame was validated above.
    let value = unsafe { ptr::read(frame) };
    if !matches!(value.vector, 0xf3 | 0xf4)
        || value.error_code != 0
        || (!state.started[state.selected] && (value.vector != 0xf4 || value.accumulator != 0))
    {
        fail("gate_vector");
    }
    increment(&mut state.evidence.calls[state.selected]);
    if state.evidence.calls.iter().sum::<u64>() > call_limit {
        fail("call_limit");
    }
    let mut next = frame;
    let reply = if value.vector == 0xf3 {
        if state.selected != 0 {
            (2, 0, 0)
        } else if value.register_15 != mailbox_address
            || state.execution.mixed_workspace_mailbox_alias().ok() != Some(state.mailbox)
        {
            fail("workspace_mailbox_owner");
        } else {
            match handle_call(state).unwrap_or_else(|reason| fail(reason)) {
                1 => {
                    next = yield_task(state, frame);
                }
                2 => {
                    if let Some(frame) = state.retire(false) {
                        next = frame;
                    } else {
                        state.finish();
                    }
                }
                _ => {}
            }
            (0, 0, 0)
        }
    } else {
        match value.accumulator {
            0 => {
                if state.started[state.selected]
                    || value.destination_index != descriptor_address
                    || value.source_index != 192
                    || value.data != 0
                    || value.counter != 0
                    || !state
                        .cpu
                        .task_image(state.selected)
                        .is_some_and(fp_image::is_initial_clean)
                {
                    fail("startup");
                }
                state.started[state.selected] = true;
                unsafe {
                    (*frame).flags = 0x202;
                }
                let mut serial = serial_port::com1();
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.started","level":"info","role":{},"cpu_clean":true}}"#,
                    state.selected
                );
                (0, 0, 0)
            }
            1 | 2 | 3 | 6 => state.packet(value),
            4 => {
                if !no_arguments(value) {
                    (4, 0, 0)
                } else {
                    next = yield_task(state, frame);
                    (0, 0, 0)
                }
            }
            5 => {
                if !no_arguments(value) {
                    (4, 0, 0)
                } else {
                    if let Some(frame) = state.retire(false) {
                        next = frame;
                    } else {
                        state.finish();
                    }
                    (0, 0, 0)
                }
            }
            12..=14 => state.storage_call(value),
            // No f4 display/input operation is granted, even to workspace.
            7 | 15 | 16 => (2, 0, 0),
            _ => (4, 0, 0),
        }
    };
    match reply.0 {
        2 => increment(&mut state.evidence.denials),
        3 => increment(&mut state.evidence.bad_copies),
        4 => increment(&mut state.evidence.invalid),
        5 => increment(&mut state.evidence.closed),
        _ => {}
    }
    if next == frame {
        unsafe {
            (*frame).accumulator = reply.0;
            (*frame).counter = reply.1;
            (*frame).data = reply.2;
        }
    }
    leave_dispatch(state, next)
}
pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot IF-clear initialization owns the IDT, prior to any task.
    unsafe {
        super::idt::install_gate(
            0xf3,
            x86_64_native_editor_workspace_gate_f3 as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        ) && super::idt::install_gate(
            0xf4,
            x86_64_native_editor_workspace_gate_f4 as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}

fn handle_call(state: &mut runtime) -> Result<u8, &'static str> {
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
            return Ok(0);
        }
        rite_decision::admitted(value) => value,
    };
    let operation = admission.operation();
    let mut exit = false;
    let mut yielded = false;
    let mut body = None;
    let reply = if operation == rite_operation::input_read {
        if let Some(encoded) = poll_input(&mut state.prefer_pointer)? {
            increment(&mut state.evidence.inputs);
            if encoded.is_loss() || encoded.is_pointer_loss() {
                increment(&mut state.evidence.input_losses);
            }
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
        yielded = true;
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
            increment(&mut state.evidence.rows);
            state.frame_hash = hash_continue(state.frame_hash, pixels);
            if stage.is_complete() {
                let completion = stage.completion().map_err(|_| "workspace_stage_complete")?;
                // SAFETY: The exact unscheduled caller was checked before this
                // integer-only gate. The callback neither enables IF nor touches
                // user mappings; the stage retains its shared supervisor alias.
                let presented = unsafe {
                    state.execution.with_mixed_workspace_bootstrap(|| {
                        state
                            .presenter
                            .as_mut()
                            .expect("retained presenter")
                            .prepare_complete_rgb8_frame(completion)
                            .map(|update| update.commit_and_verify())
                    })
                }
                .unwrap_or_else(|_| fail("workspace_present_root"));
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
                    r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.frame","level":"info","sequence":{sequence},"hash":{},"rows":768,"width":1024,"height":768}}"#,
                    state.frame_hash
                );
                state.frame_hash = 0xcbf29ce484222325;
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
            return Ok(0);
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
    Ok(if exit {
        2
    } else if yielded {
        1
    } else {
        0
    })
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

fn copy_body(mailbox: u64, size: u32, scratch: &mut [u8; 4096]) -> Result<(), &'static str> {
    // SAFETY: Admission and the exact owned normal-memory mailbox mapping
    // bound this copy. The sole vessel is stopped in its rite, and scratch is
    // disjoint kernel memory. No user references escape into decoding.
    unsafe { workspace_mailbox::copy_body(mailbox as *const u8, size as usize, scratch) }
}

fn hash_continue(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    hash
}
fn packet_hash(bytes: &[u8]) -> u64 {
    hash_continue(0xcbf29ce484222325, bytes)
}
fn clear_sector_scratch(bytes: &mut [u8; 512]) {
    for byte in bytes {
        unsafe {
            ptr::write_volatile(byte, 0);
        }
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator<'_>,
    root: u64,
    stack: u64,
    device: *mut native_storage_device::device,
    presenter: framebuffer_presenter,
    phase: u64,
    proof_mode: u64,
) -> &'static mut runtime {
    if busy.swap(true, Ordering::AcqRel) {
        fail("storage_busy");
    }
    let pointer = runtime_storage.0.get().cast::<runtime>();
    let w = (
        address!(x86_64_native_editor_workspace_workspace_payload_start),
        address!(x86_64_native_editor_workspace_workspace_payload_end),
    );
    let a = (
        address!(x86_64_native_editor_workspace_application_payload_start),
        address!(x86_64_native_editor_workspace_application_payload_end),
    );
    let wd = (
        address!(x86_64_native_editor_workspace_workspace_data_start),
        address!(x86_64_native_editor_workspace_workspace_data_end),
    );
    let ad = (
        address!(x86_64_native_editor_workspace_application_data_start),
        address!(x86_64_native_editor_workspace_application_data_end),
    );
    let span = |(start, end): (u64, u64)| {
        (
            start,
            end.checked_sub(start)
                .unwrap_or_else(|| fail("linked_span")),
        )
    };
    // SAFETY: The busy latch reserves final BSS storage before any owner is
    // published. Field writes avoid an aggregate temporary on bootstrap stack.
    unsafe {
        macro_rules! field {
            ($name:ident,$value:expr) => {
                ptr::addr_of_mut!((*pointer).$name).write($value)
            };
        }
        field!(circles, circle_table::new_native_arenas());
        field!(tasks, task_table::new());
        field!(
            scheduler,
            scheduler_table::new(3).unwrap_or_else(|_| fail("scheduler_construct"))
        );
        field!(execution, owned_execution_group::new());
        field!(cpu, cpu_session_group::new());
        field!(observed, fp_image::workspace_observation());
        field!(last_ack, [0; 4]);
        field!(selected, 0);
        field!(started, [false; 3]);
        field!(terminal, [false; 3]);
        field!(faulted, [false; 3]);
        field!(evidence, evidence::default());
        field!(last_tick, 0);
        field!(kernel_root, root);
        field!(bootstrap_stack, stack);
        field!(kernel_stack, 0);
        field!(original_ts, paging::read_cr0() & 8);
        field!(mailbox, physical_range::new(0, 0));
        field!(scratch, [0; 4096]);
        field!(stage, None);
        field!(presenter, Some(presenter));
        field!(frames, 0);
        field!(frame_hash, 0xcbf29ce484222325);
        field!(progress_tick, crate::time::now_if_disabled().tick());
        field!(prefer_pointer, false);
        field!(device, device);
        (*pointer)
            .execution
            .prepare_workspace_editor_service(
                allocator,
                &mut (*pointer).circles,
                [span(w), span(a), span(a)],
                [span(wd), span(ad), span(ad)],
            )
            .unwrap_or_else(|e| fail(e));
        for index in 0..3 {
            if (*pointer).execution.arena_extent(index)
                != Some((arena_bases[index], arena_lengths[index]))
            {
                fail("arena_extent");
            }
            for offset in (0..arena_lengths[index]).step_by(8) {
                if (*pointer).execution.read_arena(index, offset) != Some(0) {
                    fail("arena_initial_zero");
                }
            }
            let identity = (*pointer)
                .execution
                .commit(
                    index,
                    &mut (*pointer).tasks,
                    &mut (*pointer).circles,
                    901 + index as u64,
                )
                .unwrap_or_else(|e| fail(e));
            ptr::addr_of_mut!((*pointer).identities)
                .cast::<task_identity>()
                .add(index)
                .write(identity);
        }
        field!(task, (*pointer).identities[0]);
        field!(
            channel,
            rite_channel::try_new((*pointer).identities[0], rights)
                .unwrap_or_else(|_| fail("workspace_rite_channel"))
        );
        for (index, (producer, consumer)) in routes.into_iter().enumerate() {
            let binding = application_channel_binding::try_new(
                endpoint_id::try_from_parts(index, 901).unwrap(),
                (*pointer).identities[producer],
                (*pointer).identities[consumer],
            )
            .unwrap_or_else(|_| fail("route_binding"));
            ptr::addr_of_mut!((*pointer).channels)
                .cast::<application_channel>()
                .add(index)
                .write(
                    application_channel::try_new(binding)
                        .unwrap_or_else(|_| fail("route_construct")),
                );
        }
        let editor = (*pointer).identities[1];
        let workspace = (*pointer).identities[0];
        let authority = client_identity::try_new(
            editor.task().slot() as u64 + 1,
            editor.task().generation_value(),
        )
        .unwrap_or_else(|_| fail("editor_identity"));
        let identity = disk_identity::try_new(0x414e4f4d414c4f55, 0x52454d4f54450001)
            .unwrap_or_else(|_| fail("disk_identity"));
        for index in 0..3 {
            let descriptor = startup_descriptor::try_new(
                index as u64,
                phase + 1,
                [arena_bases[index], arena_lengths[index]],
                [1, 2, 3, 4],
                authority,
                identity,
                if index == 2 { 1 } else { 0 },
                proof_mode,
                1,
                [
                    workspace.task().slot() as u64 + 1,
                    workspace.task().generation_value(),
                ],
                [901, 901],
            )
            .unwrap_or_else(|_| fail("startup_descriptor"));
            let bytes = descriptor.encode();
            for (word, value) in bytes.chunks_exact(8).enumerate() {
                (*pointer)
                    .execution
                    .write_data(
                        index,
                        4096 + word as u64 * 8,
                        u64::from_le_bytes(value.try_into().unwrap()),
                    )
                    .unwrap_or_else(|e| fail(e));
            }
            let frame = (*pointer)
                .execution
                .frame(index)
                .unwrap_or_else(|e| fail(e));
            (*frame).destination_index = descriptor_address;
            (*frame).source_index = 192;
        }
        &mut *pointer
    }
}
fn allocate_stage(allocator: &mut frame_allocator<'_>) -> frame_lease {
    // SAFETY: The retained lease owns each byte before clearing or aliasing it.
    unsafe {
        allocator.allocate_with(576, 1, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .unwrap_or_else(|_| fail("stage_allocate"))
}
#[allow(clippy::too_many_arguments)]
fn run_owned(
    allocator: &mut frame_allocator<'_>,
    root: u64,
    stack: u64,
    device: *mut native_storage_device::device,
    presenter: framebuffer_presenter,
    framebuffer: framebuffer_info,
    phase: u64,
    proof_mode: u64,
) {
    let initial = allocator.accounting();
    if initial.free < 720
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
        || !tls_bases_are_zero()
    {
        fail("initial_ownership");
    }
    let state = initialize(allocator, root, stack, device, presenter, phase, proof_mode);
    for identity in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("scheduler_admit"));
    }
    let mut stage_owner = allocate_stage(allocator);
    let stage_range = stage_owner.range().unwrap_or_else(|| fail("stage_range"));
    let stage_alias = planned_physical_window::new(
        physical_range::new(0, paging::identity_map_limit),
        proof_window_base,
    )
    .and_then(|window| window.plan_virtual_extent(stage_range))
    .unwrap_or_else(|_| fail("stage_alias"));
    // SAFETY: The 576-frame lease stays exclusively retained until this stage
    // borrow ends. Every root retains its supervisor-only NX high alias.
    let storage = unsafe {
        core::slice::from_raw_parts_mut(stage_alias.base() as *mut u8, rgb8_frame_byte_length)
    };
    state.stage = Some(rgb8_frame_stage::try_new(storage).unwrap_or_else(|_| fail("stage_zero")));
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    state.mailbox = state
        .execution
        .mixed_workspace_mailbox_alias()
        .unwrap_or_else(|e| fail(e));
    let held = allocator.accounting();
    if held.free.checked_add(720) != Some(initial.free)
        || held.data_owned != 693
        || held.table_owned != 27
        || held.active_leases != 19
        || held.bound_leases != 18
    {
        fail("live_ownership");
    }
    for index in 0..3 {
        for other in 0..index {
            if state.execution.root(index).ok() == state.execution.root(other).ok()
                || state.execution.transition_stack_top(index).ok()
                    == state.execution.transition_stack_top(other).ok()
            {
                fail("owner_alias");
            }
        }
    }
    unsafe { state.cpu.prepare() }.unwrap_or_else(|e| fail(e));
    if !unsafe { state.cpu.workspace_clean_observed(&mut state.observed) } {
        fail("initial_cpu_clean");
    }
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("scheduler_start"));
    if first.task() != state.identities[0] || first.context() != scheduler_context::initial {
        fail("first_dispatch");
    }
    let frame = state.select(first.task());
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.ready","level":"info","tasks":3,"phase":{phase},"proof_mode":{proof_mode},"descriptor_bytes":192,"owned_frames":144,"owned_leases":18,"stage_frames":576,"connection":1,"free_before":{},"free_live":{}}}"#,
        initial.free, held.free
    );
    let pointer = ptr::from_mut(&mut *state);
    if active
        .compare_exchange(
            ptr::null_mut(),
            pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        fail("runtime_publication");
    }
    // SAFETY: The final resident runtime owns every task/CPU image until the
    // single terminal continuation restores this exact bootstrap stack and root.
    let result = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("initial_cpu_restore");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    if result != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != root
        || paging::read_cr0() & 8 != state.original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || state.terminal != [true; 3]
        || state.started != [true; 3]
        || state
            .channels
            .iter()
            .any(|c| c.phase() != application_channel_phase::closed || !c.storage_is_clear())
    {
        fail("terminal_continuation");
    }
    if !interrupts::pointer::stop() {
        fail("pointer_stop");
    }
    if !state
        .stage
        .take()
        .unwrap_or_else(|| fail("stage_missing"))
        .clear_and_verify()
    {
        fail("stage_clear");
    }
    state.scratch.fill(0);
    state
        .channel
        .close_retired()
        .unwrap_or_else(|_| fail("workspace_channel_close"));
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if state.faulted[index] {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|task| task.state() == expected)
        {
            fail("terminal_task");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("context_withdraw"));
    }
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
            .unwrap_or_else(|e| fail(e));
        allocator
            .release_with(&mut stage_owner, |range| memory::clear_and_verify(range))
            .unwrap_or_else(|_| fail("stage_release"));
    }
    let scheduler = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new_native_arenas().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || scheduler.admitted() != 0
        || scheduler.runnable() != 0
        || scheduler.current().is_some()
        || state
            .identities
            .iter()
            .any(|identity| state.tasks.validate(*identity).is_ok())
    {
        fail("final_ownership");
    }
    // The presenter is retired before returning the sole diagnostic writer.
    // This runtime remains allocated but no method can access it after active is
    // cleared; replacement construction requires the now-released busy latch.
    #[allow(clippy::drop_non_drop)]
    drop(
        state
            .presenter
            .take()
            .unwrap_or_else(|| fail("presenter_missing")),
    );
    if !unsafe { crate::console::restore_retired_interrupt_failure_surface(framebuffer) } {
        fail("failure_surface_restore");
    }
    let e = state.evidence;
    let faults = state.faulted.iter().filter(|v| **v).count();
    if e.captures != e.calls.iter().sum::<u64>() + e.user_irqs + faults as u64
        || e.clean_checks != e.captures
    {
        fail("cpu_accounting");
    }
    for channel in 0..4 {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.channel","level":"info","channel":{channel},"sends":{},"deliveries":{},"acks":{}}}"#,
            e.sends[channel], e.deliveries[channel], e.acknowledgements[channel]
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_editor_workspace_test.passed","level":"info","phase":{phase},"proof_mode":{proof_mode},"starts":3,"calls":{},"switches":{},"timer_ticks":{},"user_irqs":{},"cpu_captures":{},"cpu_clean_checks":{},"inputs":{},"input_losses":{},"frames":{},"rows":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"exits":{},"faults":{faults},"zeroed_bytes":102400,"free_frames_before":{},"free_frames_after":{},"resources_restored":true,"authority_revoked":true,"cpu_restored":true,"device_poisoned":{}}}"#,
        e.calls.iter().sum::<u64>(),
        e.switches,
        e.timers.iter().sum::<u64>(),
        e.user_irqs,
        e.captures,
        e.clean_checks,
        e.inputs,
        e.input_losses,
        state.frames,
        e.rows,
        e.denials,
        e.bad_copies,
        e.invalid,
        e.closed,
        e.discarded,
        3 - faults,
        initial.free,
        allocator.accounting().free,
        unsafe { (*device).is_poisoned() }
    );
    busy.store(false, Ordering::Release);
}
pub(super) fn run() -> ! {
    interrupts::disable_interrupts();
    if is_active() || dispatch_active.load(Ordering::Acquire) || !paging::protections_are_enabled()
    {
        fail("initial_state");
    }
    // SAFETY: Exclusive fixed QEMU fixture and explicitly provisioned secondary
    // IDE controller. Neither copied startup bytes nor user calls mint this grant.
    let fixture =
        unsafe { native_interactive_fixture::read() }.unwrap_or_else(|e| fail(e.as_str()));
    let mut device = unsafe { native_storage_device::device::acquire(8) }
        .unwrap_or_else(|_| fail("device_owner"));
    let geometry = device
        .identify()
        .unwrap_or_else(|_| fail("device_geometry"));
    if geometry.logical_sector_bytes != 512
        || device.geometry() != Some(geometry)
        || device.is_poisoned()
    {
        fail("device_identity");
    }
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    let presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let (available, generation) =
        interrupts::pointer::initialize().unwrap_or_else(|reason| fail(reason));
    if !available || !interrupts::enable_external_interrupts() {
        fail("input_initialize");
    }
    interrupts::disable_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"workspace_pointer.ready","level":"info","available":true,"generation":{generation}}}"#
    );
    let root = paging::read_cr3();
    let stack = unsafe { descriptors::selected_privilege_stack_top() };
    memory::with_frame_allocator(|allocator| {
        run_owned(
            allocator,
            root,
            stack,
            &mut device,
            presenter,
            info,
            fixture.phase,
            fixture.proof_mode,
        )
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    super::qemu_exit_success();
    super::halt_forever()
}
