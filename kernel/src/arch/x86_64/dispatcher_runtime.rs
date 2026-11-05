//! Bounded sustained dispatcher evidence, separate from the accepted two-tick
//! scheduler fixture. Two integer-only tasks retain private state across real
//! timer switches; one exact fault leaves its peer running under the timer.

use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::scheduler_test::{
    attach_task_space, create_task, frame_address_matches_stack, prepare_task_space,
    ranges_overlap, read_mailbox_word, release_task_space, relocated_user_address, scheduler_frame,
    task_space, user_mailbox_address, user_return_fields_are_valid, user_stack_top,
    write_mailbox_word, x86_64_scheduler_enter, x86_64_scheduler_resume_kernel,
};
use super::{descriptors, exceptions, interrupts, paging};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::{circle_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator, physical_range};
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
const stack_canary_offset: u64 = 4096 - 32;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_dispatcher_gate_entry();
    static x86_64_dispatcher_faulting_start: u8;
    static x86_64_dispatcher_faulting_end: u8;
    static x86_64_dispatcher_faulting_ready_return: u8;
    static x86_64_dispatcher_fault_instruction: u8;
    static x86_64_dispatcher_peer_start: u8;
    static x86_64_dispatcher_peer_end: u8;
    static x86_64_dispatcher_peer_ready_return: u8;
    static x86_64_dispatcher_peer_completion_return: u8;
}

struct admitted_task {
    circle: circle_id,
    identity: task_identity,
    space: task_space,
    ready: bool,
    quanta: u64,
    progress: u64,
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    admitted: [admitted_task; 2],
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
            x86_64_dispatcher_gate_entry as *const () as u64,
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
        fail("dispatcher_initial_state");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Timer restriction leaves IF clear on the sole BSP; NMI is masked.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let result =
        memory::with_frame_allocator(|allocator| run_owned(allocator, kernel_cr3, bootstrap_stack));
    let initial = match result {
        Ok(Ok(initial)) => initial,
        Ok(Err(reason)) => fail(reason),
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
) -> Result<frame_accounting, &'static str> {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || bootstrap_stack == 0
    {
        return Err("dispatcher_setup_state");
    }
    let initial = allocator.accounting();
    if initial.free < 22
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("dispatcher_initial_accounting");
    }
    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let faulting_circle = circles.create().map_err(|_| "dispatcher_circle")?;
    let peer_circle = circles.create().map_err(|_| "dispatcher_circle")?;
    let faulting_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_dispatcher_faulting_start),
        ptr::addr_of!(x86_64_dispatcher_faulting_end),
    )?;
    let peer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_dispatcher_peer_start),
        ptr::addr_of!(x86_64_dispatcher_peer_end),
    )?;
    attach_task_space(&mut circles, faulting_circle)?;
    attach_task_space(&mut circles, peer_circle)?;
    let faulting = create_task(
        &mut circles,
        &mut tasks,
        faulting_circle,
        101,
        faulting_space.payload_length,
    )?;
    let peer = create_task(
        &mut circles,
        &mut tasks,
        peer_circle,
        102,
        peer_space.payload_length,
    )?;
    let mut scheduler = scheduler_table::new(2).map_err(|_| "dispatcher_scheduler")?;
    for identity in [faulting, peer] {
        scheduler
            .admit(&mut circles, &mut tasks, identity)
            .map_err(|_| "dispatcher_admit")?;
    }
    let mut state = runtime {
        circles,
        tasks,
        scheduler,
        admitted: [
            admitted_task {
                circle: faulting_circle,
                identity: faulting,
                space: faulting_space,
                ready: false,
                quanta: 0,
                progress: 0,
            },
            admitted_task {
                circle: peer_circle,
                identity: peer,
                space: peer_space,
                ready: false,
                quanta: 0,
                progress: 0,
            },
        ],
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
    validate_ready(&state, initial, allocator.accounting())?;
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .map_err(|_| "dispatcher_initial_dispatch")?;
    if first.task() != faulting
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        return Err("dispatcher_initial_selection");
    }
    let frame = state.select(0, false)?;
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
        return Err("dispatcher_publication");
    }
    let original_ts = paging::read_cr0() & task_switched;
    // SAFETY: Every task owner and the exclusive runtime remain on this suspended
    // bootstrap stack. IRQs borrow only state, never this allocator reference.
    // Derive continuation cells from the same published raw base; no independent
    // field borrow competes with that runtime pointer during user execution.
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
        return Err("dispatcher_continuation_restore");
    }
    for task in &state.admitted {
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, task.identity)
            .map_err(|_| "dispatcher_withdraw_context")?;
    }
    for task in &mut state.admitted {
        release_task_space(allocator, &mut state.circles, task.circle, &mut task.space)?;
        state
            .tasks
            .destroy(&mut state.circles, task.identity)
            .map_err(|_| "dispatcher_destroy_task")?;
        state
            .circles
            .destroy(task.circle)
            .map_err(|_| "dispatcher_destroy_circle")?;
    }
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .map_err(|_| "dispatcher_final_scheduler")?;
    if allocator.accounting() != initial
        || circles.circles != 0
        || circles.tasks != 0
        || circles.task_contexts != 0
        || circles.address_spaces != 0
        || circles.mappings != 0
        || circles.endpoints != 0
        || circles.sigils != 0
        || circles.supervisions != 0
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || snapshot.next_dispatch_sequence() != Some(66)
        || snapshot.next_preemption_sequence() != Some(64)
    {
        return Err("dispatcher_final_accounting");
    }
    // SAFETY: Native execution and all owners have retired. Remove address-bearing
    // continuation metadata after exporting only aggregate accounting evidence.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.bootstrap_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.original_ts), 0);
    }
    Ok(initial)
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
        .map_err(|_| "dispatcher_ready_scheduler")?;
    if initial.free.checked_sub(22) != Some(ready.free)
        || ready.data_owned != 14
        || ready.table_owned != 8
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
        return Err("dispatcher_ready_accounting");
    }
    let mut ranges = [physical_range::new(0, 0); 10];
    for (index, task) in state.admitted.iter().enumerate() {
        for page in 0..3 {
            ranges[index * 5 + page] = physical_range::new(task.space.physical_pages[page], 4096);
        }
        ranges[index * 5 + 3] = physical_range::new(
            task.space
                .root
                .root_address()
                .map_err(|_| "dispatcher_ready_root")?,
            4 * 4096,
        );
        ranges[index * 5 + 4] = task.space.privilege_stack_range;
    }
    for (index, range) in ranges.iter().enumerate() {
        if range.length == 0
            || range
                .end()
                .is_none_or(|end| end > paging::identity_map_limit)
            || ranges[index + 1..]
                .iter()
                .any(|other| ranges_overlap(*range, *other))
        {
            return Err("dispatcher_owned_overlap");
        }
    }
    Ok(())
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame, require_ts: bool) -> bool {
        let Some(task) = self.admitted.get(self.selected) else {
            return false;
        };
        frame == task.space.frame
            && frame_address_matches_stack(frame, task.space.privilege_stack_range)
            && task.space.root.root_address().ok() == Some(paging::read_cr3())
            && task.space.root.validate_all_mappings().is_ok()
            && task.space.privilege_stack.range() == Some(task.space.privilege_stack_range)
            && task.space.privilege_stack_range.end()
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
        user_return_fields_are_valid(frame, snapshot.bounds())
            && frame.flags & !0x0000_0000_0025_0fd7 == 0
            && (!task.ready || frame.flags & 0x202 == 0x202)
    }

    fn select(
        &mut self,
        index: usize,
        require_ts: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let task = &self.admitted[index];
        let stack_top = task
            .space
            .privilege_stack_range
            .end()
            .ok_or("dispatcher_stack_extent")?;
        // SAFETY: IF and NMI are excluded; the selected root retains the live
        // shared supervisor branch and owns its checked private mappings.
        unsafe { task.space.root.activate() }.map_err(|_| "dispatcher_activate_root")?;
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("dispatcher_select_stack");
        }
        self.selected = index;
        let frame = self.admitted[index].space.frame;
        if !self.frame_valid(frame, require_ts) {
            return Err("dispatcher_selected_frame");
        }
        Ok(frame)
    }

    fn sample(&mut self, frame: scheduler_frame) -> Result<(), &'static str> {
        let task = &mut self.admitted[self.selected];
        let base = task.space.physical_pages[1];
        let a = canary_a + self.selected as u64;
        let b = canary_b + self.selected as u64;
        let progress =
            read_mailbox_word(base, mailbox_progress).ok_or("dispatcher_progress_read")?;
        if frame.register_12 != a
            || frame.register_13 != b
            || frame.register_15 != user_mailbox_address
            || read_mailbox_word(base, 0) != Some(a)
            || read_mailbox_word(base, 8) != Some(b)
            || read_mailbox_word(task.space.physical_pages[2], stack_canary_offset) != Some(a)
            || progress < task.progress
            || frame.register_14 < progress
            || frame.register_14 - progress > 1
        {
            return Err("dispatcher_private_state");
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
        fail("dispatcher_reentry");
    }
    let state = active.load(Ordering::Acquire);
    if state.is_null() || interrupts::interrupts_enabled() {
        fail("dispatcher_inactive");
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
        fail("dispatcher_timer_frame");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if value.vector != 0x20
        || !state.admitted[state.selected].ready
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("dispatcher_timer_observation");
    }
    state.last_tick = observed.tick();
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let selected = if !state.faulted {
        if state.switches >= switch_limit {
            fail("dispatcher_fault_deadline");
        }
        state.admitted[state.selected].quanta += 1;
        let identity = state.admitted[state.selected].identity;
        let next = 1 - state.selected;
        let transition = state
            .scheduler
            .preempt_current(&mut state.tasks, identity)
            .unwrap_or_else(|_| fail("dispatcher_preemption"));
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
            fail("dispatcher_preemption_identity");
        }
        state.switches += 1;
        if state.switches == switch_limit {
            if next != 0
                || state
                    .admitted
                    .iter()
                    .any(|task| task.quanta != 32 || task.progress <= 1)
            {
                fail("dispatcher_sustained_progress");
            }
            write_mailbox_word(state.admitted[0].space.physical_pages[1], mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        state
            .select(next, true)
            .unwrap_or_else(|reason| fail(reason))
    } else {
        if state.selected != 1 || state.survivor_ticks >= survivor_limit {
            fail("dispatcher_exit_deadline");
        }
        let snapshot = state
            .scheduler
            .snapshot(&state.tasks)
            .unwrap_or_else(|_| fail("dispatcher_survivor_scheduler"));
        if snapshot.runnable() != 0
            || snapshot.next_dispatch_sequence() != Some(66)
            || snapshot.next_preemption_sequence() != Some(64)
        {
            fail("dispatcher_survivor_dispatch_changed");
        }
        state.survivor_ticks += 1;
        if state.survivor_ticks == survivor_limit {
            if state.admitted[1].progress <= state.peer_progress_at_fault {
                fail("dispatcher_survivor_no_progress");
            }
            write_mailbox_word(state.admitted[1].space.physical_pages[1], mailbox_arm, 1)
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
        fail("dispatcher_unexpected_fault");
    }
    let expected = relocated_user_address(
        ptr::addr_of!(x86_64_dispatcher_faulting_start),
        ptr::addr_of!(x86_64_dispatcher_fault_instruction),
        state.admitted[0].space.payload_length,
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
        || snapshot.stack_pointer != user_stack_top
        || read_mailbox_word(state.admitted[0].space.physical_pages[1], mailbox_arm) != Some(1)
    {
        fail("dispatcher_fault_predicate");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.admitted[0].identity)
        .unwrap_or_else(|_| fail("dispatcher_fault_transition"));
    if transition.task() != state.admitted[0].identity
        || transition.dispatch().task() != state.admitted[1].identity
        || transition.dispatch().sequence() != 65
        || transition.dispatch().context() != scheduler_context::saved
    {
        fail("dispatcher_fault_dispatch");
    }
    state.faulted = true;
    state.peer_progress_at_fault = state.admitted[1].progress;
    let selected = state.select(1, true).unwrap_or_else(|reason| fail(reason));
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_dispatcher_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    // SAFETY: The active latch grants exclusive access to the live runtime.
    let state = unsafe { &mut *enter_dispatch() };
    if !state.frame_valid(frame, true) {
        fail("dispatcher_gate_frame");
    }
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector || value.error_code != 0 {
        fail("dispatcher_gate_vector");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    if value.accumulator == 1 {
        let (start, ready) = if state.selected == 0 {
            (
                ptr::addr_of!(x86_64_dispatcher_faulting_start),
                ptr::addr_of!(x86_64_dispatcher_faulting_ready_return),
            )
        } else {
            (
                ptr::addr_of!(x86_64_dispatcher_peer_start),
                ptr::addr_of!(x86_64_dispatcher_peer_ready_return),
            )
        };
        let task = &mut state.admitted[state.selected];
        if task.ready
            || value.flags != 2
            || task.progress != 1
            || Some(value.instruction_pointer)
                != relocated_user_address(start, ready, task.space.payload_length)
            || read_mailbox_word(task.space.physical_pages[1], mailbox_arm) != Some(0)
        {
            fail("dispatcher_ready_gate");
        }
        task.ready = true;
        unsafe { ptr::addr_of_mut!((*frame).flags).write(0x202) };
        compiler_fence(Ordering::SeqCst);
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    let expected = relocated_user_address(
        ptr::addr_of!(x86_64_dispatcher_peer_start),
        ptr::addr_of!(x86_64_dispatcher_peer_completion_return),
        state.admitted[1].space.payload_length,
    );
    if value.accumulator != 2
        || state.selected != 1
        || !state.faulted
        || state.exited
        || state.survivor_ticks != survivor_limit
        || value.flags != 0x246
        || Some(value.instruction_pointer) != expected
        || read_mailbox_word(state.admitted[1].space.physical_pages[1], mailbox_arm) != Some(1)
    {
        fail("dispatcher_exit_gate");
    }
    state
        .scheduler
        .complete_current(&mut state.tasks, state.admitted[1].identity)
        .unwrap_or_else(|_| fail("dispatcher_exit_transition"));
    state.exited = true;
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("dispatcher_mask_restore");
    }
    unsafe { paging::write_cr3(state.kernel_cr3) };
    if !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
        || state.kernel_stack == 0
        || !(state.kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(state.original_ts, 0 | task_switched)
    {
        fail("dispatcher_restore_continuation");
    }
    let kernel_stack = state.kernel_stack;
    let original_ts = state.original_ts;
    dispatch_active.store(false, Ordering::Release);
    active.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: All tasks are terminal; this consumes the saved continuation and
    // abandons the still-owned current privilege stack before any reclamation.
    unsafe { x86_64_scheduler_resume_kernel(completion_status, kernel_stack, original_ts) }
}

fn emit_evidence(initial: frame_accounting) {
    let mut serial = serial_port::com1();
    for record in [
        r#"{"schema":1,"event":"kernel.dispatcher_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":14,"table_frames":8}"#,
        r#"{"schema":1,"event":"kernel.dispatcher_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true}"#,
        r#"{"schema":1,"event":"kernel.dispatcher_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true}"#,
        r#"{"schema":1,"event":"kernel.dispatcher_test.survivor","level":"info","ticks":16,"progress":true,"dispatches_unchanged":true}"#,
    ] {
        let _ = writeln!(serial, "{record}");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.dispatcher_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.dispatcher_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.dispatcher_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
