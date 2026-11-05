//! Native mixed-layout coordination using three isolated CPL3 probe programs.
//! This is a mechanism proof, not the workspace renderer or a document service.
//! One owner governs both gates, all roots, eager CPU images and four routes.

use core::arch::asm;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use super::application_cpu::{
    cpu_session_group, fp_image, x86_64_application_enter, x86_64_application_resume_kernel,
};
use super::owned_execution::{owned_execution_group, tls_bases_are_zero, user_code_base};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
use super::{descriptors, exceptions, interrupts, paging};
use crate::application_channel::{
    application_channel, application_channel_binding, application_channel_error,
    application_channel_phase,
};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::{endpoint_id, task_identity};
use crate::memory;
use crate::memory::physical::frame_allocator;
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

const routes: [(usize, usize); 4] = [(0, 1), (1, 0), (1, 2), (2, 1)];
const arena_bases: [u64; 3] = [0x810000, 0x804000, 0x814000];
const arena_lengths: [u64; 3] = [36864, 32768, 32768];
const completion_status: u64 = 0x6d6978;
const pattern_base: u64 = 0x7171_6d69_7865_6400;
const tick_offset: u64 = 0x200;
const call_limit: u64 = 128;

unsafe extern "C" {
    fn x86_64_mixed_workspace_gate_f3();
    fn x86_64_mixed_workspace_gate_f4();
    static x86_64_mixed_workspace_workspace_start: u8;
    static x86_64_mixed_workspace_workspace_end: u8;
    static x86_64_mixed_workspace_editor_start: u8;
    static x86_64_mixed_workspace_editor_end: u8;
    static x86_64_mixed_workspace_service_start: u8;
    static x86_64_mixed_workspace_service_end: u8;
    static x86_64_mixed_workspace_service_cut_instruction: u8;
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
    stable_timers: [u64; 3],
    captures: u64,
    clean_checks: u64,
    user_checks: u64,
    switches: u64,
    denials: u64,
    bad_copies: u64,
    closed: u64,
    sends: [u64; 4],
    deliveries: [u64; 4],
    acknowledgements: [u64; 4],
    post_service_packets: u64,
    arena_checks: u64,
    service_checks: u64,
}
struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 3],
    execution: owned_execution_group<3>,
    cpu: cpu_session_group<3>,
    observed: fp_image,
    channels: [application_channel; 4],
    last_ack: [u64; 4],
    selected: usize,
    started: [bool; 3],
    sealed: [bool; 3],
    terminal: [bool; 3],
    faulted: [bool; 3],
    evidence: evidence,
    last_tick: u64,
    kernel_root: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
    service_fault: u64,
}
struct retained(UnsafeCell<MaybeUninit<runtime>>);
// SAFETY: Sole BSP; the busy latch covers construction, published dispatch,
// terminal restoration and complete teardown. Never move an active CPU owner.
unsafe impl Sync for retained {}
static storage: retained = retained(UnsafeCell::new(MaybeUninit::uninit()));
static busy: AtomicBool = AtomicBool::new(false);
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

fn fail(reason: &'static str) -> ! {
    interrupts::disable_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.mixed_workspace_test.failed","level":"fatal","reason":"{reason}"}}"#
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
    fn check_user_cpu(&mut self) {
        let role = self.selected;
        let image = self
            .cpu
            .task_image(role)
            .unwrap_or_else(|| fail("cpu_image_missing"));
        let bytes = image.bytes();
        let (significand, exponent): (u64, u16) = match role {
            0 => (0x8000_0000_0000_0000, 0x3fff),
            1 => (0, 0),
            _ => (0xc90f_daa2_2168_c235, 0x4000),
        };
        if image.control_word() != 0x37f
            || image.mxcsr() != 0x1f80
            || bytes[2..4] != 0x3800_u16.to_le_bytes()
            || bytes[4] != 0x80
            || bytes[32..40] != significand.to_le_bytes()
            || bytes[40..42] != exponent.to_le_bytes()
            || bytes[160..168] != (pattern_base + role as u64).to_le_bytes()
            || bytes[168..416].iter().any(|b| *b != 0)
        {
            fail("cpu_user_pattern");
        }
        increment(&mut self.evidence.user_checks);
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
        if self.sealed[self.selected] {
            self.check_user_cpu();
        }
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
                if value.data != 8 {
                    return (4, 0, 0);
                }
                let mut bytes = [0; 8];
                if self
                    .execution
                    .copy_from_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        caller,
                        value.source_index,
                        &mut bytes,
                    )
                    .is_err()
                {
                    return (3, 0, 0);
                }
                let receipt = self.channels[channel]
                    .commit_submit(prepared, &bytes)
                    .unwrap_or_else(|_| fail("submit_commit"));
                increment(&mut self.evidence.sends[channel]);
                if self.terminal[2] {
                    increment(&mut self.evidence.post_service_packets);
                }
                let mut serial = serial_port::com1();
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.mixed_workspace_test.packet","level":"info","channel":{},"sequence":{},"producer":{},"consumer":{},"word":{}}}"#,
                    channel,
                    receipt.sequence(),
                    routes[channel].0,
                    routes[channel].1,
                    u64::from_le_bytes(bytes)
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
                if value.data != 8 {
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
    fn workspace_probe(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if self.selected != 0 {
            return (2, 0, 0);
        }
        if value.accumulator != 1 || !no_arguments(value) {
            return (4, 0, 0);
        }
        if self.evidence.arena_checks >= 2 {
            fail("workspace_arena_check_count");
        }
        // Full checked copies cover both 18,432-byte prospective frame slots.
        // The second pass verifies retention across peer execution and service
        // retirement, allowing only the two exact packet words to have changed.
        // No frame storage or array is placed on a userspace stack.
        let mut page = [0; 4096];
        for offset in (0..arena_lengths[0]).step_by(page.len()) {
            self.execution
                .copy_from_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[0],
                    arena_bases[0] + offset,
                    &mut page,
                )
                .unwrap_or_else(|_| fail("workspace_arena_copy"));
            for (index, word) in page.chunks_exact(8).enumerate() {
                let expected = match (self.evidence.arena_checks, offset + index as u64 * 8) {
                    (1, 64) => 0x574f5232_u64,
                    (1, 256) => 0x45444932_u64,
                    _ => pattern_base,
                };
                if word != expected.to_le_bytes() {
                    fail("workspace_arena_readback");
                }
            }
        }
        increment(&mut self.evidence.arena_checks);
        (0, 0, 0)
    }
    fn retire(&mut self, faulted: bool) -> Option<*mut scheduler_frame> {
        let index = self.selected;
        let identity = self.identities[index];
        if !self.sealed[index] || self.evidence.stable_timers[index] < 2 {
            fail("terminal_cpu_unproven");
        }
        let mut closed = 0;
        for channel in &mut self.channels {
            let binding = channel.binding();
            if channel.phase() == application_channel_phase::closed
                || (binding.producer() != identity && binding.service() != identity)
            {
                continue;
            }
            if channel.phase() != application_channel_phase::empty {
                fail("terminal_pending_packet");
            }
            if channel
                .close(binding)
                .unwrap_or_else(|_| fail("route_close"))
                .discarded()
                || !channel.storage_is_clear()
            {
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
            r#"{{"schema":1,"event":"kernel.mixed_workspace_test.retired","level":"info","role":{},"faulted":{},"closed_routes":{},"survivors":{}}}"#,
            index, faulted, closed, survivors
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
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = enter_dispatch();
    state.capture(frame.cast());
    if !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("timer_frame");
    }
    state.last_tick = observed.tick();
    increment(&mut state.evidence.timers[state.selected]);
    if state.sealed[state.selected] {
        increment(&mut state.evidence.stable_timers[state.selected]);
        state
            .execution
            .write_data(
                state.selected,
                tick_offset,
                state.evidence.stable_timers[state.selected],
            )
            .unwrap_or_else(|e| fail(e));
    }
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    state.capture(frame.cast());
    if state.selected != 2
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.instruction_pointer != state.service_fault
        || !super::application_fault::classify_user_fault(snapshot)
        || state.evidence.acknowledgements != [1, 1, 1, 1]
    {
        fail("unexpected_user_fault");
    }
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
#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_mixed_workspace_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = enter_dispatch();
    state.capture(frame);
    if !state.frame_valid(frame) {
        fail("gate_return_frame");
    }
    // SAFETY: Complete current-owned normalized frame validated above.
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
        state.workspace_probe(value)
    } else {
        match value.accumulator {
            0 => {
                if state.started[state.selected]
                    || value.destination_index != state.selected as u64
                    || value.source_index != 0
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
                    r#"{{"schema":1,"event":"kernel.mixed_workspace_test.started","level":"info","role":{},"cpu_clean":true}}"#,
                    state.selected
                );
                (0, 0, 0)
            }
            1 | 2 | 3 | 6 => state.packet(value),
            7 => {
                if !no_arguments(value) || state.sealed[state.selected] {
                    fail("cpu_seal");
                }
                state.check_user_cpu();
                state.sealed[state.selected] = true;
                (0, 0, 0)
            }
            8 => {
                if state.selected != 2 {
                    (2, 0, 0)
                } else if !no_arguments(value) {
                    (4, 0, 0)
                } else {
                    increment(&mut state.evidence.service_checks);
                    (0, 0, 0)
                }
            }
            4 => {
                if !no_arguments(value) {
                    fail("yield_arguments");
                }
                // Save the outgoing reply before selecting any peer's root.
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
                next = state.select(dispatch.task());
                (0, 0, 0)
            }
            5 => {
                if !no_arguments(value) {
                    fail("exit_arguments");
                }
                if let Some(selected) = state.retire(false) {
                    next = selected;
                } else {
                    state.finish();
                }
                (0, 0, 0)
            }
            _ => (4, 0, 0),
        }
    };
    match reply.0 {
        2 => increment(&mut state.evidence.denials),
        3 => increment(&mut state.evidence.bad_copies),
        5 => increment(&mut state.evidence.closed),
        0 => {}
        _ => fail("unexpected_probe_status"),
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
            x86_64_mixed_workspace_gate_f3 as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        ) && super::idt::install_gate(
            0xf4,
            x86_64_mixed_workspace_gate_f4 as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}

#[inline(never)]
fn initialize(allocator: &mut frame_allocator<'_>, root: u64, stack: u64) -> &'static mut runtime {
    if busy.swap(true, Ordering::AcqRel) {
        fail("storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    let spans = [
        (
            address!(x86_64_mixed_workspace_workspace_start),
            address!(x86_64_mixed_workspace_workspace_end),
        ),
        (
            address!(x86_64_mixed_workspace_editor_start),
            address!(x86_64_mixed_workspace_editor_end),
        ),
        (
            address!(x86_64_mixed_workspace_service_start),
            address!(x86_64_mixed_workspace_service_end),
        ),
    ];
    let payloads = spans.map(|(start, end)| {
        (
            start,
            end.checked_sub(start)
                .unwrap_or_else(|| fail("payload_span")),
        )
    });
    let service_fault =
        user_code_base + address!(x86_64_mixed_workspace_service_cut_instruction) - spans[2].0;
    // SAFETY: The latch exclusively reserves final resident storage. Write each
    // field separately; no aggregate runtime temporary uses bootstrap stack.
    unsafe {
        macro_rules! field {
            ($name:ident, $value:expr) => {
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
        field!(sealed, [false; 3]);
        field!(terminal, [false; 3]);
        field!(faulted, [false; 3]);
        field!(evidence, evidence::default());
        field!(last_tick, 0);
        field!(kernel_root, root);
        field!(bootstrap_stack, stack);
        field!(kernel_stack, 0);
        field!(original_ts, paging::read_cr0() & 8);
        field!(service_fault, service_fault);
        (*pointer)
            .execution
            .prepare_workspace_editor_service(
                allocator,
                &mut (*pointer).circles,
                payloads,
                [(0, 0); 3],
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
        &mut *pointer
    }
}

fn run_owned(allocator: &mut frame_allocator<'_>, root: u64, stack: u64) {
    let initial = allocator.accounting();
    if initial.free < 144
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
        || !tls_bases_are_zero()
    {
        fail("initial_ownership");
    }
    let state = initialize(allocator, root, stack);
    for identity in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("scheduler_admit"));
    }
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let held = allocator.accounting();
    if held.free.checked_add(144) != Some(initial.free)
        || held.data_owned != 117
        || held.table_owned != 27
        || held.active_leases != 18
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
        r#"{{"schema":1,"event":"kernel.mixed_workspace_test.ready","level":"info","tasks":3,"channels":4,"workspace_rx_pages":32,"workspace_rw_pages":8,"workspace_arena_pages":9,"workspace_stack_pages":16,"application_rx_pages":8,"application_rw_pages":2,"application_arena_pages":8,"application_stack_pages":2,"owned_frames":144,"data_frames":117,"table_frames":27,"leases":18,"zeroed_arena_bytes":102400,"free_before":{},"free_live":{}}}"#,
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
    // SAFETY: Final resident owner remains exclusively retained until the one
    // terminal nonlocal continuation restores bootstrap root/TSS/CPU controls.
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
        || state.sealed != [true; 3]
        || state.faulted != [false, false, true]
        || state
            .channels
            .iter()
            .any(|c| c.phase() != application_channel_phase::closed || !c.storage_is_clear())
    {
        fail("terminal_continuation");
    }
    let e = state.evidence;
    if e.calls != [23, 24, 9]
        || e.switches != 10
        || e.denials != 6
        || e.bad_copies != 5
        || e.closed != 2
        || e.sends != [2, 2, 1, 1]
        || e.deliveries != e.sends
        || e.acknowledgements != e.sends
        || state.last_ack != [2, 2, 1, 1]
        || e.post_service_packets != 2
        || e.arena_checks != 2
        || e.service_checks != 1
        || e.stable_timers.iter().any(|ticks| *ticks < 2)
        || e.captures != 57 + e.timers.iter().sum::<u64>()
        || e.clean_checks != e.captures
        || e.user_checks != 54 + e.stable_timers.iter().sum::<u64>()
    {
        fail("final_evidence");
    }
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if index == 2 {
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
    }
    .unwrap_or_else(|e| fail(e));
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
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.mixed_workspace_test.passed","level":"info","calls":56,"workspace_calls":23,"editor_calls":24,"service_calls":9,"switches":10,"denials":6,"bad_copies":5,"closed":2,"sends":6,"deliveries":6,"acknowledgements":6,"post_service_packets":2,"arena_checks":2,"arena_checked_bytes":73728,"service_checks":1,"exits":2,"faults":1,"workspace_timers":{},"editor_timers":{},"service_timers":{},"workspace_stable_timers":{},"editor_stable_timers":{},"service_stable_timers":{},"cpu_captures":{},"cpu_clean_checks":{},"cpu_user_checks":{},"free_after":{},"resources_restored":true,"authority_revoked":true,"cpu_restored":true,"source_language":"assembly","personality":"native_f3_f4"}}"#,
        e.timers[0],
        e.timers[1],
        e.timers[2],
        e.stable_timers[0],
        e.stable_timers[1],
        e.stable_timers[2],
        e.captures,
        e.clean_checks,
        e.user_checks,
        initial.free
    );
    busy.store(false, Ordering::Release);
}
pub(super) fn run() -> ! {
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
        || !paging::protections_are_enabled()
    {
        fail("initial_state");
    }
    let root = paging::read_cr3();
    let stack = unsafe { descriptors::selected_privilege_stack_top() };
    memory::with_frame_allocator(|allocator| run_owned(allocator, root, stack))
        .unwrap_or_else(|error| fail(error.reason()));
    super::qemu_exit_success();
    super::halt_forever()
}
