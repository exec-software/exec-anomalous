//! Sustained dispatch over retained prepared-launch owners and low user roots.
//!
//! Two GPR-only tasks execute inside nested calls across real PIT switches.
//! Every mapping, high-alias transition stack and activation registration stays
//! owned through terminal restoration. This feature does not define an app ABI.

use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::owned_execution::{
    owned_execution, prove_rollback, tls_bases_are_zero, user_code_base, user_data_base,
    user_stack_top,
};
use super::scheduler_test::{
    scheduler_frame, user_return_fields_are_valid_within_stack, x86_64_scheduler_enter,
    x86_64_scheduler_resume_kernel,
};
use super::{descriptors, exceptions, interrupts, paging};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::task_identity;
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

const switch_limit: u64 = 64;
const survivor_limit: u64 = 16;
const gate_vector: u64 = 0xf2;
const task_switched: u64 = 8;
const completion_status: u64 = 0xd15;
const canary_a: u64 = 0x61a7_c0de_f17e_0001;
const canary_b: u64 = 0x9ee7_c0de_ba5e_0002;
const mailbox_arm: u64 = 16;
const mailbox_progress: u64 = 24;
const nested_stack_pointer: u64 = user_stack_top - 24;
const stack_canary_offset: u64 = 2 * 4096 - 56;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_owned_dispatcher_gate_entry();
    static x86_64_owned_dispatcher_faulting_start: u8;
    static x86_64_owned_dispatcher_faulting_end: u8;
    static x86_64_owned_dispatcher_faulting_ready_return: u8;
    static x86_64_owned_dispatcher_fault_instruction: u8;
    static x86_64_owned_dispatcher_peer_start: u8;
    static x86_64_owned_dispatcher_peer_end: u8;
    static x86_64_owned_dispatcher_peer_ready_return: u8;
    static x86_64_owned_dispatcher_peer_completion_return: u8;
}

macro_rules! symbol_address {
    ($symbol:ident) => {{
        let address: u64;
        // SAFETY: Materialize a linked marker's numeric address without treating
        // adjacent linker symbols as distinct nonzero-sized Rust objects.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) address,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        address
    }};
}

struct admitted_task {
    identity: task_identity,
    payload_start: u64,
    payload_length: u64,
    ready: bool,
    quanta: u64,
    progress: u64,
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    admitted: [admitted_task; 2],
    execution: owned_execution,
    selected: usize,
    switches: u64,
    survivor_ticks: u64,
    last_tick: u64,
    faulted: bool,
    exited: bool,
    peer_progress_at_fault: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot fault-path initialization owns the IDT with IF clear.
    unsafe {
        super::idt::install_gate(
            gate_vector as usize,
            x86_64_owned_dispatcher_gate_entry as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}

pub(super) fn run() -> ! {
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("owned_dispatcher_initial_state");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Timer restriction leaves IF clear on the sole BSP; NMI is masked.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let result =
        memory::with_frame_allocator(|allocator| run_owned(allocator, kernel_cr3, bootstrap_stack));
    let initial = match result {
        Ok(initial) => initial,
        Err(error) => fail(error.reason()),
    };
    emit_evidence(initial);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_owned(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> frame_accounting {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || bootstrap_stack == 0
        || !tls_bases_are_zero()
    {
        fail("owned_dispatcher_setup_state");
    }
    let initial = allocator.accounting();
    if initial.free < 36
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        fail("owned_dispatcher_initial_accounting");
    }
    let payloads = [
        (
            symbol_address!(x86_64_owned_dispatcher_faulting_start),
            symbol_address!(x86_64_owned_dispatcher_faulting_end),
        ),
        (
            symbol_address!(x86_64_owned_dispatcher_peer_start),
            symbol_address!(x86_64_owned_dispatcher_peer_end),
        ),
    ]
    .map(|(start, end)| {
        let length = end
            .checked_sub(start)
            .filter(|length| *length > 0 && *length <= 4096)
            .unwrap_or_else(|| fail("owned_dispatcher_payload_extent"));
        (start, length)
    });
    prove_rollback(allocator, payloads).unwrap_or_else(|reason| fail(reason));
    if allocator.accounting() != initial || paging::read_cr3() != kernel_cr3 {
        fail("owned_dispatcher_rollback_restore");
    }
    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let mut execution = owned_execution::new();
    // SAFETY: The sole BSP has IF clear and NMI masked. The support owner
    // retains every acquired lease/registration on failure; all error paths
    // below halt in this owning scope instead of returning and losing it.
    unsafe { execution.prepare(allocator, &mut circles, payloads) }
        .unwrap_or_else(|reason| fail(reason));
    if tasks.active_count() != 0 {
        fail("owned_dispatcher_preparation_published_task");
    }
    let identities = core::array::from_fn::<_, 2, _>(|index| {
        execution
            .commit(index, &mut tasks, &mut circles, index as u64 + 101)
            .unwrap_or_else(|reason| fail(reason))
    });
    let mut scheduler =
        scheduler_table::new(2).unwrap_or_else(|_| fail("owned_dispatcher_scheduler"));
    for identity in identities {
        scheduler
            .admit(&mut circles, &mut tasks, identity)
            .unwrap_or_else(|_| fail("owned_dispatcher_admit"));
    }
    // SAFETY: Committed low-root owners and transition stacks remain retained;
    // activation admission happens under bootstrap before scheduler execution.
    unsafe { execution.activate(allocator) }.unwrap_or_else(|reason| fail(reason));
    let mut state = runtime {
        circles,
        tasks,
        scheduler,
        admitted: core::array::from_fn(|index| admitted_task {
            identity: identities[index],
            payload_start: payloads[index].0,
            payload_length: payloads[index].1,
            ready: false,
            quanta: 0,
            progress: 0,
        }),
        execution,
        selected: 0,
        switches: 0,
        survivor_ticks: 0,
        last_tick: 0,
        faulted: false,
        exited: false,
        peer_progress_at_fault: 0,
        kernel_cr3,
        bootstrap_stack,
        kernel_stack: 0,
        original_ts: u64::MAX,
    };
    validate_ready(&state, initial, allocator.accounting()).unwrap_or_else(|reason| fail(reason));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("owned_dispatcher_initial_dispatch"));
    if first.task() != identities[0]
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        fail("owned_dispatcher_initial_selection");
    }
    let frame = state.select(0, false).unwrap_or_else(|reason| fail(reason));
    let runtime_pointer = ptr::from_mut(&mut state);
    if active
        .compare_exchange(
            ptr::null_mut(),
            runtime_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        fail("owned_dispatcher_publication");
    }
    let original_ts = paging::read_cr0() & task_switched;
    // SAFETY: The runtime remains at this stable bootstrap-stack address until
    // the single-use continuation returns. IRQ callbacks exclusively borrow it
    // through the nonreentry latch; no allocator reference is exposed to them.
    let status = unsafe {
        x86_64_scheduler_enter(
            frame,
            ptr::addr_of_mut!((*runtime_pointer).kernel_stack),
            ptr::addr_of_mut!((*runtime_pointer).original_ts),
        )
    };
    if status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || paging::read_cr0() & task_switched != original_ts
        || state.original_ts != original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || !interrupts::scheduler_interrupts_are_masked()
        || !tls_bases_are_zero()
        || state.switches != switch_limit
        || state.survivor_ticks != survivor_limit
        || !state.faulted
        || !state.exited
        || state
            .admitted
            .iter()
            .any(|task| !task.ready || task.quanta != 32 || task.progress <= 1)
        || state.admitted[1].progress <= state.peer_progress_at_fault
    {
        fail("owned_dispatcher_continuation_restore");
    }
    for task in &state.admitted {
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, task.identity)
            .unwrap_or_else(|_| fail("owned_dispatcher_withdraw_context"));
    }
    // SAFETY: Both tasks are terminal, their scheduler contexts are withdrawn,
    // and the original bootstrap stack/root and TSS selection were restored.
    // Failure retains the runtime and all unreleased resources in this scope.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|reason| fail(reason));
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("owned_dispatcher_final_scheduler"));
    if allocator.accounting() != initial
        || circles != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || snapshot.next_dispatch_sequence() != Some(66)
        || snapshot.next_preemption_sequence() != Some(64)
        || paging::read_cr3() != kernel_cr3
        || identities
            .iter()
            .any(|identity| state.tasks.validate(*identity).is_ok())
    {
        fail("owned_dispatcher_final_accounting");
    }
    // SAFETY: The execution owner has cleared/released all private storage and
    // retired its address-bearing fields. Retire the consumed continuation too.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.bootstrap_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.original_ts), 0);
    }
    initial
}

fn validate_ready(
    state: &runtime,
    initial: frame_accounting,
    ready: frame_accounting,
) -> Result<(), &'static str> {
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .map_err(|_| "owned_dispatcher_ready_scheduler")?;
    if initial.free.checked_sub(36) != Some(ready.free)
        || ready.data_owned != 18
        || ready.table_owned != 18
        || ready.active_leases != 10
        || ready.bound_leases != 10
        || circles.circles != 2
        || circles.tasks != 2
        || circles.task_contexts != 2
        || circles.address_spaces != 2
        || circles.mappings != 6
        || snapshot.admitted() != 2
        || snapshot.runnable() != 2
        || snapshot.current().is_some()
    {
        return Err("owned_dispatcher_ready_accounting");
    }
    if state.execution.root(0)? == state.execution.root(1)?
        || state.execution.frame(0)? == state.execution.frame(1)?
        || state.execution.transition_stack_top(0)? == state.execution.transition_stack_top(1)?
    {
        return Err("owned_dispatcher_aliased_owner");
    }
    for index in 0..2 {
        let admission = state.execution.admission(index)?;
        if admission.bounds().code_start() != user_code_base
            || admission.bounds().initial_stack_pointer() != user_stack_top
            || !state
                .tasks
                .snapshot(state.admitted[index].identity)
                .is_ok_and(|task| task.bounds() == admission.bounds())
        {
            return Err("owned_dispatcher_ready_admission");
        }
    }
    Ok(())
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame, require_ts: bool) -> bool {
        let Some(task) = self.admitted.get(self.selected) else {
            return false;
        };
        self.execution.frame_owned(self.selected, frame)
            && self.execution.root(self.selected).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(self.selected).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && (!require_ts || paging::read_cr0() & task_switched != 0)
            && self
                .scheduler
                .snapshot(&self.tasks)
                .is_ok_and(|s| s.current() == Some(task.identity))
            && self
                .tasks
                .snapshot(task.identity)
                .is_ok_and(|s| s.state() == task_state::running && s.context_attached())
    }

    fn frame_valid(&self, frame: *const scheduler_frame, require_ts: bool) -> bool {
        if !self.frame_owned(frame, require_ts) {
            return false;
        }
        let task = &self.admitted[self.selected];
        let Ok(snapshot) = self.tasks.snapshot(task.identity) else {
            return false;
        };
        // SAFETY: Ownership checks above validate the complete retained frame.
        let frame = unsafe { ptr::read(frame) };
        user_return_fields_are_valid_within_stack(frame, snapshot.bounds())
            && self
                .execution
                .admission(self.selected)
                .is_ok_and(|admission| {
                    admission.accepts_return(frame.instruction_pointer, frame.stack_pointer)
                })
            && frame.flags & !0x0000_0000_0025_0fd7 == 0
            && (!task.ready || frame.flags & 0x202 == 0x202)
    }

    fn select(
        &mut self,
        index: usize,
        require_ts: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        // SAFETY: The retained support owner validates both registered roots,
        // the current supervisor stack and the selected transition stack. IF
        // and NMI are excluded; the new root retains the live Rust stack too.
        unsafe { self.execution.select(index) }?;
        self.selected = index;
        let frame = self.execution.frame(index)?;
        if !self.frame_valid(frame, require_ts) {
            return Err("owned_dispatcher_selected_frame");
        }
        Ok(frame)
    }

    fn sample(&mut self, frame: scheduler_frame) -> Result<(), &'static str> {
        let task = &mut self.admitted[self.selected];
        let a = canary_a + self.selected as u64;
        let b = canary_b + self.selected as u64;
        let progress = self
            .execution
            .read_data(self.selected, mailbox_progress)
            .ok_or("owned_dispatcher_progress_read")?;
        if frame.register_12 != a
            || frame.register_13 != b
            || frame.register_15 != user_data_base
            || frame.stack_pointer != nested_stack_pointer
            || self.execution.read_data(self.selected, 0) != Some(a)
            || self.execution.read_data(self.selected, 8) != Some(b)
            || self
                .execution
                .read_stack(self.selected, stack_canary_offset)
                != Some(a)
            || progress < task.progress
            || frame.register_14 < progress
            || frame.register_14 - progress > 1
        {
            return Err("owned_dispatcher_private_state");
        }
        task.progress = progress;
        Ok(())
    }
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}

pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let state = active.load(Ordering::Acquire);
    !state.is_null() && unsafe { (&*state).frame_valid(frame.cast(), true) }
}

fn enter_dispatch() -> *mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) {
        fail("owned_dispatcher_reentry");
    }
    let state = active.load(Ordering::Acquire);
    if state.is_null() || interrupts::interrupts_enabled() {
        fail("owned_dispatcher_inactive");
    }
    // SAFETY: The sole BSP runs with IF clear and NMI masked. The nonreentrant
    // latch grants this exclusive borrow, which ends before returning or the
    // terminal bootstrap continuation. No allocator reference is exposed.
    state
}

pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    // SAFETY: The active latch grants exclusive access to the live runtime.
    let state = unsafe { &mut *enter_dispatch() };
    if !state.frame_valid(frame.cast(), true) {
        fail("owned_dispatcher_timer_frame");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if value.vector != 0x20
        || !state.admitted[state.selected].ready
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("owned_dispatcher_timer_observation");
    }
    state.last_tick = observed.tick();
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let selected = if !state.faulted {
        if state.switches >= switch_limit {
            fail("owned_dispatcher_fault_deadline");
        }
        state.admitted[state.selected].quanta += 1;
        let identity = state.admitted[state.selected].identity;
        let next = 1 - state.selected;
        let transition = state
            .scheduler
            .preempt_current(&mut state.tasks, identity)
            .unwrap_or_else(|_| fail("owned_dispatcher_preemption"));
        if transition.task() != identity
            || transition.sequence() != state.switches
            || transition.dispatch().task() != state.admitted[next].identity
            || transition.dispatch().sequence() != state.switches + 1
            || transition.dispatch().context()
                != if state.switches == 0 {
                    scheduler_context::initial
                } else {
                    scheduler_context::saved
                }
        {
            fail("owned_dispatcher_preemption_identity");
        }
        state.switches += 1;
        if state.switches == switch_limit {
            if next != 0
                || state
                    .admitted
                    .iter()
                    .any(|task| task.quanta != 32 || task.progress <= 1)
            {
                fail("owned_dispatcher_sustained_progress");
            }
            state
                .execution
                .write_data(0, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        state
            .select(next, true)
            .unwrap_or_else(|reason| fail(reason))
    } else {
        if state.selected != 1 || state.survivor_ticks >= survivor_limit {
            fail("owned_dispatcher_exit_deadline");
        }
        let snapshot = state
            .scheduler
            .snapshot(&state.tasks)
            .unwrap_or_else(|_| fail("owned_dispatcher_survivor_scheduler"));
        if snapshot.runnable() != 0
            || snapshot.next_dispatch_sequence() != Some(66)
            || snapshot.next_preemption_sequence() != Some(64)
        {
            fail("owned_dispatcher_survivor_dispatch_changed");
        }
        state.survivor_ticks += 1;
        if state.survivor_ticks == survivor_limit {
            if state.admitted[1].progress <= state.peer_progress_at_fault {
                fail("owned_dispatcher_survivor_no_progress");
            }
            state
                .execution
                .write_data(1, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        frame.cast()
    };
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    // SAFETY: The active latch grants exclusive access to the live runtime.
    let state = unsafe { &mut *enter_dispatch() };
    if state.selected != 0
        || state.faulted
        || state.switches != switch_limit
        || !state.frame_valid(frame.cast(), true)
    {
        fail("owned_dispatcher_unexpected_fault");
    }
    let expected = relocated_user_address(
        &state.admitted[0],
        symbol_address!(x86_64_owned_dispatcher_fault_instruction),
    );
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.fault_address.is_some()
        || !matches!(snapshot.flags, 0x246 | 0x10246)
        || Some(snapshot.instruction_pointer) != expected
        || value.vector != snapshot.vector
        || value.error_code != snapshot.error_code
        || value.instruction_pointer != snapshot.instruction_pointer
        || value.flags != snapshot.flags
        || snapshot.stack_pointer != nested_stack_pointer
        || state.execution.read_data(0, mailbox_arm) != Some(1)
    {
        fail("owned_dispatcher_fault_predicate");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.admitted[0].identity)
        .unwrap_or_else(|_| fail("owned_dispatcher_fault_transition"));
    if transition.task() != state.admitted[0].identity
        || transition.dispatch().task() != state.admitted[1].identity
        || transition.dispatch().sequence() != 65
        || transition.dispatch().context() != scheduler_context::saved
    {
        fail("owned_dispatcher_fault_dispatch");
    }
    state.faulted = true;
    state.peer_progress_at_fault = state.admitted[1].progress;
    let selected = state.select(1, true).unwrap_or_else(|reason| fail(reason));
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_owned_dispatcher_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    // SAFETY: The active latch grants exclusive access to the live runtime.
    let state = unsafe { &mut *enter_dispatch() };
    if !state.frame_valid(frame, true) {
        fail("owned_dispatcher_gate_frame");
    }
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector || value.error_code != 0 {
        fail("owned_dispatcher_gate_vector");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    if value.accumulator == 1 {
        let ready = if state.selected == 0 {
            symbol_address!(x86_64_owned_dispatcher_faulting_ready_return)
        } else {
            symbol_address!(x86_64_owned_dispatcher_peer_ready_return)
        };
        let task = &mut state.admitted[state.selected];
        if task.ready
            || value.flags != 2
            || task.progress != 1
            || Some(value.instruction_pointer) != relocated_user_address(task, ready)
            || state.execution.read_data(state.selected, mailbox_arm) != Some(0)
        {
            fail("owned_dispatcher_ready_gate");
        }
        task.ready = true;
        unsafe { ptr::addr_of_mut!((*frame).flags).write(0x202) };
        compiler_fence(Ordering::SeqCst);
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    let expected = relocated_user_address(
        &state.admitted[1],
        symbol_address!(x86_64_owned_dispatcher_peer_completion_return),
    );
    if value.accumulator != 2
        || state.selected != 1
        || !state.faulted
        || state.exited
        || state.survivor_ticks != survivor_limit
        || value.flags != 0x246
        || Some(value.instruction_pointer) != expected
        || state.execution.read_data(1, mailbox_arm) != Some(1)
    {
        fail("owned_dispatcher_exit_gate");
    }
    state
        .scheduler
        .complete_current(&mut state.tasks, state.admitted[1].identity)
        .unwrap_or_else(|_| fail("owned_dispatcher_exit_transition"));
    state.exited = true;
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("owned_dispatcher_mask_restore");
    }
    // SAFETY: Both scheduled tasks are terminal; the current transition stack
    // remains owned and reachable through the shared alias during restoration.
    unsafe { state.execution.restore_bootstrap() }.unwrap_or_else(|reason| fail(reason));
    if paging::read_cr3() != state.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
        || state.kernel_stack == 0
        || !(state.kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(state.original_ts, 0 | task_switched)
    {
        fail("owned_dispatcher_restore_continuation");
    }
    let kernel_stack = state.kernel_stack;
    let original_ts = state.original_ts;
    dispatch_active.store(false, Ordering::Release);
    active.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: All tasks are terminal; this consumes the saved continuation and
    // abandons the still-owned current privilege stack before any reclamation.
    unsafe { x86_64_scheduler_resume_kernel(completion_status, kernel_stack, original_ts) }
}

fn relocated_user_address(task: &admitted_task, symbol: u64) -> Option<u64> {
    let offset = symbol.checked_sub(task.payload_start)?;
    (offset < task.payload_length).then_some(user_code_base.checked_add(offset)?)
}

fn emit_evidence(initial: frame_accounting) {
    let mut serial = serial_port::com1();
    for record in [
        r#"{"schema":1,"event":"kernel.owned_dispatcher_test.rollback","level":"info","partial_admissions":1,"unstarted_withdrawals":1,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
        r#"{"schema":1,"event":"kernel.owned_dispatcher_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":18,"table_frames":18,"active_leases":10,"bound_leases":10,"owned_low_roots":true,"prepared_launch":true,"high_alias_stacks":true}"#,
        r#"{"schema":1,"event":"kernel.owned_dispatcher_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true,"noninitial_stacks":true}"#,
        r#"{"schema":1,"event":"kernel.owned_dispatcher_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true}"#,
        r#"{"schema":1,"event":"kernel.owned_dispatcher_test.survivor","level":"info","ticks":16,"progress":true,"dispatches_unchanged":true}"#,
    ] {
        let _ = writeln!(serial, "{record}");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_dispatcher_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_dispatcher_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_dispatcher_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
