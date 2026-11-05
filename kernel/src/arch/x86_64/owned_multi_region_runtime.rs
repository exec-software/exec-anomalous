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

use super::owned_execution::{owned_execution, tls_bases_are_zero};
#[cfg(feature = "owned_source_test")]
use super::owned_source_runtime::{self as source_proof, source_phase, source_set};
#[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
use super::owned_tls_runtime as tls_proof;
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

const user_code_base: u64 = 0x60001000;
const main_code_base: u64 = 0x40002000;
const user_data_base: u64 = 0x40007100;
const user_stack_top: u64 = 0x7fff00010000;
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
const stack_canary_offset: u64 = 16 * 4096 - 56;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_owned_multi_region_gate_entry();
    static x86_64_owned_multi_region_main_faulting_start: u8;
    static x86_64_owned_multi_region_main_faulting_end: u8;
    static x86_64_owned_multi_region_main_faulting_cross_return: u8;
    static x86_64_owned_multi_region_main_peer_start: u8;
    static x86_64_owned_multi_region_main_peer_end: u8;
    static x86_64_owned_multi_region_main_peer_cross_return: u8;
    static x86_64_owned_multi_region_faulting_start: u8;
    static x86_64_owned_multi_region_faulting_end: u8;
    static x86_64_owned_multi_region_faulting_ready_return: u8;
    static x86_64_owned_multi_region_fault_instruction: u8;
    static x86_64_owned_multi_region_peer_start: u8;
    static x86_64_owned_multi_region_peer_end: u8;
    static x86_64_owned_multi_region_peer_ready_return: u8;
    static x86_64_owned_multi_region_peer_completion_return: u8;
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
    crossed: bool,
    quanta: u64,
    progress: u64,
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    admitted: Option<[admitted_task; 2]>,
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
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    tls: tls_proof::proof,
    // The outer run_owned frame retains this owner across both lifetimes.
    // While published, only the IRQ/gate nonreentry latch may dereference this
    // pointer: sole BSP, IF clear, NMI masked. No allocator or second mutable
    // source borrow survives publication, and the pointer is retired on return.
    #[cfg(feature = "owned_source_test")]
    sources: *mut source_set,
    #[cfg(feature = "owned_source_test")]
    source_phase: source_phase,
}

pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot fault-path initialization owns the IDT with IF clear.
    unsafe {
        super::idt::install_gate(
            gate_vector as usize,
            x86_64_owned_multi_region_gate_entry as *const () as u64,
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
        fail("owned_multi_region_initial_state");
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
    #[cfg(not(any(feature = "owned_source_test", feature = "owned_tls_test")))]
    emit_evidence(initial);
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    tls_proof::emit_evidence(initial);
    #[cfg(feature = "owned_source_test")]
    source_proof::emit_evidence(initial);
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
        fail("owned_multi_region_setup_state");
    }
    let initial = allocator.accounting();
    if initial.free
        < if cfg!(feature = "owned_source_test") {
            208
        } else {
            204
        }
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        fail("owned_multi_region_initial_accounting");
    }
    let payloads = [
        (
            symbol_address!(x86_64_owned_multi_region_faulting_start),
            symbol_address!(x86_64_owned_multi_region_faulting_end),
        ),
        (
            symbol_address!(x86_64_owned_multi_region_peer_start),
            symbol_address!(x86_64_owned_multi_region_peer_end),
        ),
    ]
    .map(|(start, end)| {
        let length = end
            .checked_sub(start)
            .filter(|length| {
                *length > 0
                    && *length
                        <= if cfg!(feature = "owned_source_test") {
                            1024
                        } else {
                            4096
                        }
            })
            .unwrap_or_else(|| fail("owned_multi_region_payload_extent"));
        (start, length)
    });
    prove_multi_rollback(allocator, payloads).unwrap_or_else(|reason| fail(reason));
    if allocator.accounting() != initial || paging::read_cr3() != kernel_cr3 {
        fail("owned_multi_region_rollback_restore");
    }
    #[cfg(not(feature = "owned_source_test"))]
    {
        run_dispatch(allocator, kernel_cr3, bootstrap_stack, payloads, initial)
    }
    #[cfg(feature = "owned_source_test")]
    {
        let mut sources = source_set::new();
        run_dispatch(
            allocator,
            kernel_cr3,
            bootstrap_stack,
            payloads,
            initial,
            ptr::from_mut(&mut sources),
            source_phase::producer,
        );
        let retained = source_proof::retained_accounting(initial)
            .unwrap_or_else(|| fail("owned_source_retained_overflow"));
        if allocator.accounting() != retained
            || !sources.complete(source_phase::producer)
            // SAFETY: Producer tasks are retired, bootstrap restored, IF clear.
            || !unsafe { interrupts::restore_source_consumer_timer_after_retirement() }
        {
            fail("owned_source_producer_retirement");
        }
        run_dispatch(
            allocator,
            kernel_cr3,
            bootstrap_stack,
            payloads,
            retained,
            ptr::from_mut(&mut sources),
            source_phase::consumer,
        );
        // SAFETY: Both complete dispatch frames and their aliases are gone;
        // only the outer sealed source owners remain, on restored bootstrap.
        unsafe { source_proof::cleanup(&mut sources, allocator, initial, fail) }
            .unwrap_or_else(|reason| fail(reason));
        initial
    }
}

// Keep rollback's temporary owners out of the live dispatch frame. Both paths
// retain the existing 64KiB bootstrap stack and 16KiB preparation headroom.
#[inline(never)]
fn run_dispatch(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    payloads: [(u64, u64); 2],
    initial: frame_accounting,
    #[cfg(feature = "owned_source_test")] sources: *mut source_set,
    #[cfg(feature = "owned_source_test")] source_phase: source_phase,
) -> frame_accounting {
    // Prepare directly in the retained owner. Keeping separate local owners
    // and then moving them into this runtime doubles their bootstrap storage.
    let mut state = runtime {
        circles: circle_table::new_multi_region(),
        tasks: task_table::new(),
        scheduler: scheduler_table::new(2).unwrap_or_else(|_| fail("owned_multi_region_scheduler")),
        admitted: None,
        execution: owned_execution::new(),
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
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        tls: tls_proof::proof::new(),
        #[cfg(feature = "owned_source_test")]
        sources,
        #[cfg(feature = "owned_source_test")]
        source_phase,
    };
    // SAFETY: The sole BSP has IF clear and NMI masked. The support owner
    // retains every acquired lease/registration on failure; all error paths
    // below halt in this owning scope instead of returning and losing it.
    #[cfg(not(feature = "owned_source_test"))]
    unsafe {
        prepare_owner(
            &mut state.execution,
            allocator,
            &mut state.circles,
            payloads,
        )
    }
    .unwrap_or_else(|reason| fail(reason));
    #[cfg(feature = "owned_source_test")]
    {
        if sources.is_null() {
            fail("owned_source_pointer");
        }
        // SAFETY: Before publication, this call exclusively borrows the outer
        // owner. Preparation returns before any asynchronous callback can run.
        let result = unsafe {
            match source_phase {
                source_phase::producer => prepare_owner(
                    &mut state.execution,
                    allocator,
                    &mut state.circles,
                    payloads,
                ),
                source_phase::consumer => source_proof::prepare_consumer(
                    &mut state.execution,
                    allocator,
                    &mut state.circles,
                    &*sources,
                    main_lengths(),
                    payloads.map(|(_, length)| length),
                ),
            }
        };
        result.unwrap_or_else(|reason| fail(reason));
    }
    if state.tasks.active_count() != 0 {
        fail("owned_multi_region_preparation_published_task");
    }
    let identities = core::array::from_fn::<_, 2, _>(|index| {
        state
            .execution
            .commit(
                index,
                &mut state.tasks,
                &mut state.circles,
                index as u64 + 101,
            )
            .unwrap_or_else(|reason| fail(reason))
    });
    state.admitted = Some(core::array::from_fn(|index| admitted_task {
        identity: identities[index],
        payload_start: payloads[index].0,
        payload_length: payloads[index].1,
        ready: false,
        crossed: false,
        quanta: 0,
        progress: 0,
    }));
    #[cfg(feature = "owned_source_test")]
    {
        if source_phase == source_phase::producer {
            // SAFETY: Exact committed producer IDs, exclusive live alias; the
            // outer source owner is retained if any allocation fails.
            unsafe { (&mut *sources).allocate(&mut state.execution, allocator, identities) }
                .unwrap_or_else(|reason| fail(reason));
        }
        for index in 0..2 {
            state
                .execution
                .multi_write(index, source_proof::mailbox_phase, source_phase.word())
                .unwrap_or_else(|reason| fail(reason));
        }
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    // SAFETY: Exact committed identities; no user execution or publication yet.
    unsafe { state.tls.prepare(identities, &state.execution) }
        .unwrap_or_else(|reason| fail(reason));
    for identity in identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("owned_multi_region_admit"));
    }
    // SAFETY: Committed low-root owners and transition stacks remain retained;
    // activation admission happens under bootstrap before scheduler execution.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|reason| fail(reason));
    validate_ready(&state, initial, allocator.accounting()).unwrap_or_else(|reason| fail(reason));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("owned_multi_region_initial_dispatch"));
    if first.task() != identities[0]
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        fail("owned_multi_region_initial_selection");
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
        fail("owned_multi_region_publication");
    }
    let original_ts = paging::read_cr0() & task_switched;
    // SAFETY: All slow preparation/activation is complete; IF is still clear
    // and no user instruction has run. Keep ordinary PIT reinjection enabled,
    // but do not charge unpublished setup's timer debt to admitted quanta.
    #[cfg(not(feature = "owned_source_test"))]
    let rearmed = unsafe { interrupts::rearm_multi_region_timer_before_entry() };
    #[cfg(feature = "owned_source_test")]
    let rearmed = unsafe {
        match source_phase {
            source_phase::producer => interrupts::rearm_multi_region_timer_before_entry(),
            source_phase::consumer => interrupts::rearm_source_consumer_timer_before_entry(),
        }
    };
    if !rearmed {
        fail("owned_multi_region_timer_rearm");
    }
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
    let admitted = state
        .admitted
        .as_ref()
        .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
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
        || admitted
            .iter()
            .any(|task| !task.ready || !task.crossed || task.quanta != 32 || task.progress <= 1)
        || admitted[1].progress <= state.peer_progress_at_fault
    {
        fail("owned_multi_region_continuation_restore");
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    // SAFETY: Terminal assembly returned to restored bootstrap with a clean
    // capsule. Retained task mappings still exist until execution.finish below.
    unsafe { state.tls.finish() }.unwrap_or_else(|reason| fail(reason));
    for task in admitted {
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, task.identity)
            .unwrap_or_else(|_| fail("owned_multi_region_withdraw_context"));
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
    #[cfg(feature = "owned_source_test")]
    let expected_final = if source_phase == source_phase::producer {
        source_proof::retained_accounting(initial)
            .unwrap_or_else(|| fail("owned_source_retained_overflow"))
    } else {
        initial
    };
    #[cfg(not(feature = "owned_source_test"))]
    let expected_final = initial;
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("owned_multi_region_final_scheduler"));
    if allocator.accounting() != expected_final
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
        fail("owned_multi_region_final_accounting");
    }
    #[cfg(feature = "owned_source_test")]
    {
        // SAFETY: Publication and the nonreentry latch are withdrawn, so this
        // synchronous borrow cannot race either dispatch lifetime.
        if !unsafe { (&*sources).complete(source_phase) } {
            fail("owned_source_phase_incomplete");
        }
        state.sources = ptr::null_mut();
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
    let admitted = state
        .admitted
        .as_ref()
        .ok_or("owned_multi_region_admitted_missing")?;
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .map_err(|_| "owned_multi_region_ready_scheduler")?;
    #[cfg(feature = "owned_source_test")]
    let baseline = if state.source_phase == source_phase::producer {
        source_proof::retained_accounting(initial).ok_or("owned_source_retained_overflow")?
    } else {
        initial
    };
    #[cfg(feature = "owned_source_test")]
    let accounting_matches = baseline.free.checked_sub(204) == Some(ready.free)
        && baseline.data_owned.checked_add(184) == Some(ready.data_owned)
        && baseline.table_owned.checked_add(20) == Some(ready.table_owned)
        && baseline.active_leases.checked_add(22) == Some(ready.active_leases)
        && baseline.bound_leases.checked_add(22) == Some(ready.bound_leases)
        && baseline.unavailable == ready.unavailable;
    #[cfg(not(feature = "owned_source_test"))]
    let accounting_matches = initial.free.checked_sub(204) == Some(ready.free)
        && ready.data_owned == 184
        && ready.table_owned == 20
        && ready.active_leases == 22
        && ready.bound_leases == 22;
    if !accounting_matches
        || circles.circles != 2
        || circles.tasks != 2
        || circles.task_contexts != 2
        || circles.address_spaces != 2
        || circles.mappings != 18
        || snapshot.admitted() != 2
        || snapshot.runnable() != 2
        || snapshot.current().is_some()
    {
        return Err("owned_multi_region_ready_accounting");
    }
    if state.execution.root(0)? == state.execution.root(1)?
        || state.execution.frame(0)? == state.execution.frame(1)?
        || state.execution.transition_stack_top(0)? == state.execution.transition_stack_top(1)?
    {
        return Err("owned_multi_region_aliased_owner");
    }
    for (index, task) in admitted.iter().enumerate() {
        let admission = state.execution.admission(index)?;
        if admission.bounds().entry() != user_code_base + 32
            || admission.bounds().initial_stack_pointer() != user_stack_top
            || !state
                .tasks
                .snapshot(task.identity)
                .is_ok_and(|task| task.bounds() == admission.bounds())
        {
            return Err("owned_multi_region_ready_admission");
        }
    }
    let bounds = state.execution.admission(0)?;
    for forbidden in [
        0x40002000,
        0x40002036,
        0x40003000,
        0x40009000,
        0x60001000,
        0x6000101f,
        0x60002000,
        user_data_base,
    ] {
        if bounds.accepts_return(forbidden, user_stack_top) {
            return Err("owned_multi_region_return_hole");
        }
    }
    Ok(())
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame, require_ts: bool) -> bool {
        let Some(task) = self
            .admitted
            .as_ref()
            .and_then(|admitted| admitted.get(self.selected))
        else {
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
        let Some(task) = self
            .admitted
            .as_ref()
            .and_then(|admitted| admitted.get(self.selected))
        else {
            return false;
        };
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
            return Err("owned_multi_region_selected_frame");
        }
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        self.stage_tls(frame)?;
        Ok(frame)
    }

    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    fn consume_tls(&mut self, frame: *const scheduler_frame) -> Result<(), &'static str> {
        let identity = self
            .admitted
            .as_ref()
            .and_then(|tasks| tasks.get(self.selected))
            .ok_or("owned_tls_current_identity")?
            .identity;
        // SAFETY: Every caller first authenticated the current frame/root/TSS.
        unsafe { self.tls.consume(identity, frame) }
    }

    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    fn stage_tls(&mut self, frame: *const scheduler_frame) -> Result<(), &'static str> {
        if !self.frame_valid(frame, false) {
            return Err("owned_tls_selected_frame");
        }
        let identity = self
            .admitted
            .as_ref()
            .and_then(|tasks| tasks.get(self.selected))
            .ok_or("owned_tls_selected_identity")?
            .identity;
        // SAFETY: The selected frame/root/TSS and attached live task agree.
        unsafe { self.tls.stage(identity, frame) }
    }

    fn sample(&mut self, frame: scheduler_frame) -> Result<(), &'static str> {
        let task = self
            .admitted
            .as_mut()
            .and_then(|admitted| admitted.get_mut(self.selected))
            .ok_or("owned_multi_region_admitted_missing")?;
        let a = canary_a + self.selected as u64;
        let b = canary_b + self.selected as u64;
        let progress = self
            .execution
            .multi_read(self.selected, mailbox_progress)
            .ok_or("owned_multi_region_progress_read")?;
        if frame.register_12 != a
            || frame.register_13 != b
            || frame.register_15 != user_data_base
            || frame.stack_pointer != nested_stack_pointer
            || self.execution.multi_read(self.selected, 0) != Some(a)
            || self.execution.multi_read(self.selected, 8) != Some(b)
            || self
                .execution
                .read_stack(self.selected, stack_canary_offset)
                != Some(a)
            || progress < task.progress
            || frame.register_14 < progress
            || frame.register_14 - progress > 1
        {
            return Err("owned_multi_region_private_state");
        }
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        if task.ready {
            self.tls.sample(
                self.selected,
                task.identity,
                frame.register_14,
                progress,
                &self.execution,
            )?;
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
        fail("owned_multi_region_reentry");
    }
    let state = active.load(Ordering::Acquire);
    if state.is_null() || interrupts::interrupts_enabled() {
        fail("owned_multi_region_inactive");
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
        fail("owned_multi_region_timer_frame");
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    state
        .consume_tls(frame.cast())
        .unwrap_or_else(|reason| fail(reason));
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    let admitted = state
        .admitted
        .as_ref()
        .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
    if value.vector != 0x20
        || !admitted[state.selected].ready
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("owned_multi_region_timer_observation");
    }
    state.last_tick = observed.tick();
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let selected = if !state.faulted {
        if state.switches >= switch_limit {
            fail("owned_multi_region_fault_deadline");
        }
        let admitted = state
            .admitted
            .as_mut()
            .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
        admitted[state.selected].quanta += 1;
        let identity = admitted[state.selected].identity;
        let next = 1 - state.selected;
        let transition = state
            .scheduler
            .preempt_current(&mut state.tasks, identity)
            .unwrap_or_else(|_| fail("owned_multi_region_preemption"));
        if transition.task() != identity
            || transition.sequence() != state.switches
            || transition.dispatch().task() != admitted[next].identity
            || transition.dispatch().sequence() != state.switches + 1
            || transition.dispatch().context()
                != if state.switches == 0 {
                    scheduler_context::initial
                } else {
                    scheduler_context::saved
                }
        {
            fail("owned_multi_region_preemption_identity");
        }
        state.switches += 1;
        if state.switches == switch_limit {
            if next != 0
                || admitted
                    .iter()
                    .any(|task| task.quanta != 32 || task.progress <= 1)
            {
                let _ = writeln!(
                    serial_port::com1(),
                    "{{\"schema\":1,\"event\":\"kernel.owned_multi_region_test.progress_rejected\",\"level\":\"error\",\"next\":{},\"task_0_quanta\":{},\"task_1_quanta\":{},\"task_0_progress\":{},\"task_1_progress\":{},\"rip\":{}}}",
                    next,
                    admitted[0].quanta,
                    admitted[1].quanta,
                    admitted[0].progress,
                    admitted[1].progress,
                    value.instruction_pointer
                );
                fail("owned_multi_region_sustained_progress");
            }
            state
                .execution
                .multi_write(0, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        state
            .select(next, true)
            .unwrap_or_else(|reason| fail(reason))
    } else {
        if state.selected != 1 || state.survivor_ticks >= survivor_limit {
            fail("owned_multi_region_exit_deadline");
        }
        let snapshot = state
            .scheduler
            .snapshot(&state.tasks)
            .unwrap_or_else(|_| fail("owned_multi_region_survivor_scheduler"));
        if snapshot.runnable() != 0
            || snapshot.next_dispatch_sequence() != Some(66)
            || snapshot.next_preemption_sequence() != Some(64)
        {
            fail("owned_multi_region_survivor_dispatch_changed");
        }
        state.survivor_ticks += 1;
        if state.survivor_ticks == survivor_limit {
            let admitted = state
                .admitted
                .as_ref()
                .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
            if admitted[1].progress <= state.peer_progress_at_fault {
                fail("owned_multi_region_survivor_no_progress");
            }
            state
                .execution
                .multi_write(1, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        state
            .stage_tls(frame.cast())
            .unwrap_or_else(|reason| fail(reason));
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
        fail("owned_multi_region_unexpected_fault");
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    state
        .consume_tls(frame.cast())
        .unwrap_or_else(|reason| fail(reason));
    let admitted = state
        .admitted
        .as_ref()
        .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
    let expected = relocated_user_address(
        &admitted[0],
        symbol_address!(x86_64_owned_multi_region_fault_instruction),
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
        || state.execution.multi_read(0, mailbox_arm) != Some(1)
    {
        fail("owned_multi_region_fault_predicate");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    let admitted = state
        .admitted
        .as_ref()
        .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, admitted[0].identity)
        .unwrap_or_else(|_| fail("owned_multi_region_fault_transition"));
    if transition.task() != admitted[0].identity
        || transition.dispatch().task() != admitted[1].identity
        || transition.dispatch().sequence() != 65
        || transition.dispatch().context() != scheduler_context::saved
    {
        fail("owned_multi_region_fault_dispatch");
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    state
        .tls
        .retire(0, admitted[0].identity)
        .unwrap_or_else(|reason| fail(reason));
    state.faulted = true;
    state.peer_progress_at_fault = admitted[1].progress;
    let selected = state.select(1, true).unwrap_or_else(|reason| fail(reason));
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_owned_multi_region_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    // SAFETY: The active latch grants exclusive access to the live runtime.
    let state = unsafe { &mut *enter_dispatch() };
    if !state.frame_valid(frame, true) {
        fail("owned_multi_region_gate_frame");
    }
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    state
        .consume_tls(frame)
        .unwrap_or_else(|reason| fail(reason));
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector || value.error_code != 0 {
        fail("owned_multi_region_gate_vector");
    }
    state.sample(value).unwrap_or_else(|reason| fail(reason));
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    if matches!(value.accumulator, 12 | 13) {
        let task = state
            .admitted
            .as_ref()
            .and_then(|tasks| tasks.get(state.selected))
            .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
        if task.ready || !task.crossed || task.progress != 1 || value.flags != 2 {
            fail("owned_tls_request_phase");
        }
        let result = state
            .tls
            .request(
                state.selected,
                task.identity,
                value,
                task.payload_start,
                &state.execution,
            )
            .unwrap_or_else(|reason| fail(reason));
        // SAFETY: Exact authenticated retained frame; only the result register changes.
        unsafe { ptr::addr_of_mut!((*frame).accumulator).write(result) };
        compiler_fence(Ordering::SeqCst);
        state.stage_tls(frame).unwrap_or_else(|reason| fail(reason));
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    #[cfg(feature = "owned_source_test")]
    if matches!(value.accumulator, 10 | 11) {
        let task = state
            .admitted
            .as_ref()
            .and_then(|admitted| admitted.get(state.selected))
            .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
        if task.ready
            || !task.crossed
            || task.progress != 1
            || value.flags != 2
            || state.sources.is_null()
            || state
                .execution
                .multi_read(state.selected, source_proof::mailbox_phase)
                != Some(state.source_phase.word())
        {
            fail("owned_source_request_frame");
        }
        // SAFETY: The outer owner lives across both dispatches. Publication's
        // nonreentry latch, IF clear and masked NMI grant this sole short borrow.
        let status = unsafe {
            (&mut *state.sources).request(
                state.source_phase,
                state.selected,
                &mut state.execution,
                &state.tasks,
                &state.circles,
                &state.scheduler,
                task.identity,
                value,
            )
        }
        .unwrap_or_else(|reason| fail(reason));
        unsafe { ptr::addr_of_mut!((*frame).accumulator).write(status) };
        compiler_fence(Ordering::SeqCst);
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    let admitted = state
        .admitted
        .as_mut()
        .unwrap_or_else(|| fail("owned_multi_region_admitted_missing"));
    if value.accumulator == 3 {
        let (start, expected) = if state.selected == 0 {
            (
                symbol_address!(x86_64_owned_multi_region_main_faulting_start),
                symbol_address!(x86_64_owned_multi_region_main_faulting_cross_return),
            )
        } else {
            (
                symbol_address!(x86_64_owned_multi_region_main_peer_start),
                symbol_address!(x86_64_owned_multi_region_main_peer_cross_return),
            )
        };
        let task = &mut admitted[state.selected];
        if task.ready
            || task.crossed
            || value.flags != 2
            || task.progress != 1
            || value.instruction_pointer != main_code_base + (expected - start)
        {
            fail("owned_multi_region_cross_gate");
        }
        task.crossed = true;
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        state.stage_tls(frame).unwrap_or_else(|reason| fail(reason));
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    if value.accumulator == 1 {
        let ready = if state.selected == 0 {
            symbol_address!(x86_64_owned_multi_region_faulting_ready_return)
        } else {
            symbol_address!(x86_64_owned_multi_region_peer_ready_return)
        };
        let task = &mut admitted[state.selected];
        if task.ready
            || !task.crossed
            || value.flags != 2
            || task.progress != 1
            || Some(value.instruction_pointer) != relocated_user_address(task, ready)
            || state.execution.multi_read(state.selected, mailbox_arm) != Some(0)
        {
            fail("owned_multi_region_ready_gate");
        }
        #[cfg(feature = "owned_source_test")]
        {
            if state.sources.is_null() {
                fail("owned_source_pointer");
            }
            // SAFETY: Same active, nonreentrant exclusive source borrow as the
            // upload gate; readiness follows the exact successful call sequence.
            unsafe {
                (&mut *state.sources).ready(
                    state.source_phase,
                    state.selected,
                    &mut state.execution,
                )
            }
            .unwrap_or_else(|reason| fail(reason));
        }
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        state
            .tls
            .ready(state.selected, task.identity, &state.execution)
            .unwrap_or_else(|reason| fail(reason));
        task.ready = true;
        unsafe { ptr::addr_of_mut!((*frame).flags).write(0x202) };
        compiler_fence(Ordering::SeqCst);
        #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
        state.stage_tls(frame).unwrap_or_else(|reason| fail(reason));
        dispatch_active.store(false, Ordering::Release);
        return frame;
    }
    let expected = relocated_user_address(
        &admitted[1],
        symbol_address!(x86_64_owned_multi_region_peer_completion_return),
    );
    if value.accumulator != 2
        || state.selected != 1
        || !state.faulted
        || state.exited
        || state.survivor_ticks != survivor_limit
        || value.flags != 0x246
        || Some(value.instruction_pointer) != expected
        || state.execution.multi_read(1, mailbox_arm) != Some(1)
    {
        fail("owned_multi_region_exit_gate");
    }
    state
        .scheduler
        .complete_current(&mut state.tasks, admitted[1].identity)
        .unwrap_or_else(|_| fail("owned_multi_region_exit_transition"));
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    state
        .tls
        .retire(1, admitted[1].identity)
        .unwrap_or_else(|reason| fail(reason));
    state.exited = true;
    #[cfg(not(feature = "owned_source_test"))]
    interrupts::disable_external_interrupts();
    #[cfg(feature = "owned_source_test")]
    match state.source_phase {
        source_phase::producer => {
            // SAFETY: Both tasks terminal; IF clear, sole BSP, NMI masked.
            if !unsafe { interrupts::quiesce_source_producer_timer_before_retirement() } {
                fail("owned_source_producer_timer_quiesce");
            }
        }
        source_phase::consumer => interrupts::disable_external_interrupts(),
    }
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("owned_multi_region_mask_restore");
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
        fail("owned_multi_region_restore_continuation");
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

#[cfg(not(any(feature = "owned_source_test", feature = "owned_tls_test")))]
fn emit_evidence(initial: frame_accounting) {
    let mut serial = serial_port::com1();
    for record in [
        r#"{"schema":1,"event":"kernel.owned_multi_region_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
        r#"{"schema":1,"event":"kernel.owned_multi_region_test.ready","level":"info","tasks":2,"quantum_switches":64,"survivor_ticks":16,"data_frames":184,"table_frames":20,"active_leases":22,"bound_leases":22,"regions_per_task":9,"executable_regions":2,"initialized_readback":true,"rejected_return_sites":8,"owned_low_roots":true,"prepared_launch":true,"high_alias_stacks":true}"#,
        r#"{"schema":1,"event":"kernel.owned_multi_region_test.progress","level":"info","preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"task_0_progress":true,"task_1_progress":true,"canaries_intact":true,"noninitial_stacks":true,"cross_region_gates":2}"#,
        r#"{"schema":1,"event":"kernel.owned_multi_region_test.fault_contained","level":"info","vector":6,"task":0,"peer":1,"peer_continued":true}"#,
        r#"{"schema":1,"event":"kernel.owned_multi_region_test.survivor","level":"info","ticks":16,"progress":true,"dispatches_unchanged":true}"#,
    ] {
        let _ = writeln!(serial, "{record}");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_multi_region_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_multi_region_test.passed","level":"info","timer_ticks":80,"dispatches":66,"preemptions":64,"faults":1,"exits":1}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    #[cfg(not(any(feature = "owned_source_test", feature = "owned_tls_test")))]
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_multi_region_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    #[cfg(feature = "owned_source_test")]
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_source_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    #[cfg(all(feature = "owned_tls_test", not(feature = "owned_source_test")))]
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_tls_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}

#[cfg(feature = "owned_source_test")]
fn main_lengths() -> [u64; 2] {
    [
        (
            symbol_address!(x86_64_owned_multi_region_main_faulting_start),
            symbol_address!(x86_64_owned_multi_region_main_faulting_end),
        ),
        (
            symbol_address!(x86_64_owned_multi_region_main_peer_start),
            symbol_address!(x86_64_owned_multi_region_main_peer_end),
        ),
    ]
    .map(|(start, end)| {
        end.checked_sub(start)
            .filter(|length| *length > 55 && *length <= 1024)
            .unwrap_or_else(|| fail("owned_source_main_extent"))
    })
}

// The region geometry matches the previously inspected true/interpreter pair;
// bytes are native mechanism fixtures, not an execution claim for those files.
unsafe fn prepare_owner(
    execution: &mut owned_execution,
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    payloads: [(u64, u64); 2],
) -> Result<(), &'static str> {
    use super::mapping_plan::{mapping_budget, mapping_request, reserved_request};
    use super::multi_region_plan::{launch_input, multi_region_image, source_span};
    use crate::task::{executable_interval, executable_interval_set};
    let mains = [
        (
            symbol_address!(x86_64_owned_multi_region_main_faulting_start),
            symbol_address!(x86_64_owned_multi_region_main_faulting_end),
        ),
        (
            symbol_address!(x86_64_owned_multi_region_main_peer_start),
            symbol_address!(x86_64_owned_multi_region_main_peer_end),
        ),
    ];
    let main_bytes: [&[u8]; 2] = core::array::from_fn(|index| {
        let (start, end) = mains[index];
        let length = end
            .checked_sub(start)
            .filter(|length| {
                *length > 55
                    && *length
                        <= if cfg!(feature = "owned_source_test") {
                            1024
                        } else {
                            4096
                        }
            })
            .unwrap_or_else(|| fail("multi_main_source"));
        // SAFETY: Bounds are exact immutable linked rodata labels. Preparation
        // independently verifies linked-rodata containment before reading them.
        unsafe { core::slice::from_raw_parts(start as *const u8, length as usize) }
    });
    let interpreter_bytes: [&[u8]; 2] = core::array::from_fn(|index| {
        // SAFETY: run_owned validated exact nonempty immutable linked labels.
        unsafe {
            core::slice::from_raw_parts(payloads[index].0 as *const u8, payloads[index].1 as usize)
        }
    });
    static initialization: [u8; 20] = *b"unaligned-copy-test!";
    #[cfg(not(feature = "owned_source_test"))]
    const span_count: usize = 3;
    #[cfg(feature = "owned_source_test")]
    const span_count: usize = 4;
    let sources: [[source_span<'_>; span_count]; 2] = core::array::from_fn(|index| {
        [
            source_span {
                destination: main_code_base,
                bytes: main_bytes[index],
            },
            source_span {
                destination: user_code_base,
                bytes: interpreter_bytes[index],
            },
            source_span {
                destination: 0x40007028,
                bytes: &initialization,
            },
            #[cfg(feature = "owned_source_test")]
            source_span {
                destination: source_proof::pattern_address,
                bytes: &source_proof::pattern,
            },
        ]
    });
    let requests = [
        (0x40000000, 2, false, false),
        (0x40002000, 4, false, true),
        (0x40006000, 1, false, false),
        (0x40007000, 2, true, false),
        (0x60000000, 1, false, false),
        (0x60001000, 47, false, true),
        (0x60030000, 11, false, false),
        (0x6003b000, 4, true, false),
        (user_stack_top - 16 * 4096, 16, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: paging::page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    let reservations = [
        (0, 512),
        (user_stack_top - 17 * 4096, 1),
        (user_stack_top, 1),
    ]
    .map(|(virtual_address, page_count)| reserved_request {
        virtual_address,
        page_count,
    });
    let images = core::array::from_fn(|index| {
        let executable = executable_interval_set::new(&[
            executable_interval {
                start: main_code_base + 55,
                end: main_code_base + main_bytes[index].len() as u64,
            },
            executable_interval {
                start: user_code_base + 32,
                end: user_code_base + interpreter_bytes[index].len() as u64,
            },
        ])
        .unwrap_or_else(|_| fail("multi_interval_plan"));
        multi_region_image::new(launch_input {
            regions: &requests,
            reservations: &reservations,
            budget: mapping_budget {
                data_pages: 88,
                table_frames: 32,
            },
            sources: &sources[index],
            executable,
            entry: user_code_base + 32,
            stack_region: 8,
            stack_pointer: user_stack_top,
        })
        .unwrap_or_else(|_| fail("multi_source_plan"))
    });
    // SAFETY: Forward the same exclusive bootstrap and retained-owner contract.
    unsafe { execution.prepare_multi_region(allocator, circles, images) }
}

#[inline(never)]
fn prove_multi_rollback(
    allocator: &mut frame_allocator<'_>,
    payloads: [(u64, u64); 2],
) -> Result<(), &'static str> {
    let initial = allocator.accounting();
    for fail_index in 0..2 {
        let mut execution = owned_execution::new();
        let mut circles = circle_table::new_multi_region();
        let mut tasks = task_table::new();
        // SAFETY: Exclusive bootstrap, no consumer/task is published.
        unsafe { prepare_owner(&mut execution, allocator, &mut circles, payloads) }?;
        if tasks.active_count() != 0 || circles.accounting().mappings != 18 {
            return Err("multi_rollback_publication");
        }
        // SAFETY: No scheduler context exists; inject failure only at the chosen
        // transition-stack clearing boundary and retain the owner for retry.
        let first = unsafe {
            execution.multi_finish_with_stack_failure(
                allocator,
                &mut tasks,
                &mut circles,
                fail_index,
            )
        };
        if first != Err("owned_stack_clear") || allocator.accounting().active_leases == 0 {
            return Err("multi_rollback_retention");
        }
        // SAFETY: Retry retained cleanup progress after removing the injection.
        unsafe { execution.finish(allocator, &mut tasks, &mut circles) }?;
        if allocator.accounting() != initial
            || circles.accounting() != circle_table::new_multi_region().accounting()
        {
            return Err("multi_rollback_restore");
        }
    }
    Ok(())
}
