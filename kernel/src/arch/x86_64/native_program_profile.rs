//! A program-bearing launch: one task, one RX page, a 64 KiB arena (ADR 0094).
//!
//! What this proves is the launch contract, not an interpreter. There is no
//! NIL interpreter here or in the vessel yet, so no edition is executed and
//! none is mapped: the descriptor's edition fields are carried end to end and
//! stated back, not backed by bytes. That is the honest extent of it.
//!
//! The task receives an `ANPG` descriptor in its own writable data, states
//! back what it was granted at the head of its arena, and exits. The kernel
//! compares that statement against what it actually wrote. A descriptor is
//! launch data and never a capability: the profile is selected here, by the
//! controller, before anything is acquired.
//!
//! Single task, honestly. ADR 0094 widened the single-task gate rather than
//! padding the launch with a peer that exists only to satisfy a legacy count,
//! so there is no second task, no channel pair and no display here.
//!
//! The console grant has no transport behind it yet. A scenario that grants
//! `console_write` mints the handle and the task reports it, but every gate
//! operation except startup and exit is refused, so the grant is not yet
//! reachable. `console_transport` is what makes it reachable; until then the
//! emitted evidence says so rather than implying a working console.
//!
//! The parent module lowers `dead_code` and `unused_imports` under this
//! feature, because a program build leaves its two-task half unreachable.
//! Re-denied here so the lowered level never reaches the code that does run.
#![deny(dead_code, unused_imports)]

use super::super::arena_profile::program_arena_bases;
use super::super::owned_execution::{owned_execution_group, user_data_base};
use super::*;
use anomalous_native_runtime::{
    minimum_arena_bytes, program_descriptor, program_startup_size, service,
};

/// Where the descriptor sits in the task's private RW data. The vessel's
/// linker script pins the same address, and startup admission compares the
/// registers against it.
const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;

/// The arena length this profile expects to be given.
///
/// Taken from the ABI's own stated minimum rather than from `arena_profile`.
/// Comparing the mapping against the profile that produced it would prove
/// only that the two agree, and they can drift together while it passes; the
/// contract `program_descriptor` enforces is the independent authority.
const arena_bytes: u64 = minimum_arena_bytes;
const program_code_pages: u64 = 1;

/// The arena offset and word count the vessel writes its grant record at.
/// Offset zero: a program's first act is to state what it was given.
const proof_offset: u64 = 0;
const proof_words: usize = 6;

/// Frame accounting for one program task, derived rather than measured.
///
/// `data_pages` for this geometry is 1 code + 2 data + 16 arena + 2 stack =
/// 21, and `prepare_profile` then allocates one `transition_pages` stack of
/// 4, so 25 data frames. Nine table frames is the resident count the host
/// fixture brackets: the user plan needs eight and `resident_plan` adds one.
/// Six leases is one per mapped region plus the table lease plus the
/// transition stack.
///
/// These are expectations, not receipts. The run below is what measures them,
/// and it fails closed on any of them rather than reporting what it found.
const expected_data_frames: usize = 25;
const expected_table_frames: usize = 9;
const expected_leases: usize = 6;
const program_frames: usize = expected_data_frames + expected_table_frames;

/// What a single run grants.
///
/// Two runs, one per admitted base, because the base must not be baked into
/// the launch. The console pairing is proved in both directions: granted with
/// a handle, and absent with none. `program_descriptor::try_new` refuses
/// either one without the other, and the vessel checks the pairing again on
/// the side of the boundary that would be lied to.
struct scenario_profile {
    base: u64,
    console_handle: u64,
    granted_services: u64,
    edition_handle: u64,
    edition_length: u64,
    arguments: &'static [u8],
}
const scenarios: [scenario_profile; 2] = [
    scenario_profile {
        base: program_arena_bases[0],
        console_handle: 0x62,
        granted_services: service::console_write,
        edition_handle: 0x41,
        edition_length: 4096,
        arguments: b"hello\0--once",
    },
    scenario_profile {
        base: program_arena_bases[1],
        console_handle: 0,
        granted_services: 0,
        edition_handle: 0x9f,
        edition_length: 64,
        arguments: b"",
    },
];

static active: AtomicPtr<program> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<program>>);
// SAFETY: Sole BSP, IF clear. The latch covers initialization through teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

unsafe extern "C" {
    static x86_64_native_program_payload_start: u8;
    static x86_64_native_program_payload_end: u8;
    static x86_64_native_program_user_fault_instruction: u8;
}

#[derive(Clone, Copy)]
struct program_image {
    source: u64,
    length: u64,
}
impl program_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_program_payload_start);
        let length = address!(x86_64_native_program_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("program_payload_extent"));
        let fault = address!(x86_64_native_program_user_fault_instruction);
        // One code page, not the native application's eight. An image that
        // outgrew its single mapped page must be refused here rather than
        // copied into a mapping that cannot hold it.
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > program_code_pages * paging::page_size
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || fault < user_code_base
            || fault
                .checked_add(2)
                .is_none_or(|end| end > user_code_base + length)
        {
            fail("program_artifact_geometry");
        }
        // SAFETY: Inspected complete two-byte marker in immutable resident payload.
        if unsafe { ptr::read_unaligned((source + fault - user_code_base) as *const [u8; 2]) }
            != [0x0f, 0x0b]
        {
            fail("program_artifact_fault_marker");
        }
        Self { source, length }
    }
}

struct program {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identity: task_identity,
    execution: owned_execution_group<1>,
    cpu: application_cpu::cpu_session_group<1>,
    image: program_image,
    /// The six words granted, in the order the vessel states them back.
    granted: [u64; proof_words],
    arena_base: u64,
    scenario: usize,
    zeroed_bytes: u64,
    calls: u64,
    timer_ticks: u64,
    invalid: u64,
    exits: u64,
    started: bool,
    terminal: bool,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

#[derive(Clone, Copy, Default)]
struct program_record {
    calls: u64,
    timer_ticks: u64,
    invalid: u64,
    exits: u64,
    zeroed_bytes: u64,
    granted: [u64; proof_words],
    stated: [u64; proof_words],
}

impl program {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        // SAFETY: The sole BSP reads its initialized TSS with interrupts excluded.
        self.execution.frame_owned(0, frame)
            && self.execution.root(0).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(0).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self.cpu.controls_are_active()
            && self
                .scheduler
                .snapshot(&self.tasks)
                .is_ok_and(|s| s.current() == Some(self.identity))
            && self
                .tasks
                .snapshot(self.identity)
                .is_ok_and(|s| s.state() == task_state::running && s.context_attached())
    }
    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(self.identity) else {
            return false;
        };
        // SAFETY: frame_owned validated the complete aligned retained frame.
        let value = unsafe { ptr::read(frame) };
        user_return_fields_are_valid_within_stack(value, snapshot.bounds())
            && self
                .execution
                .admission(0)
                .is_ok_and(|a| a.accepts_return(value.instruction_pointer, value.stack_pointer))
            && value.flags & !0x25_0fd7 == 0
            && (!self.started || value.flags & 0x202 == 0x202)
    }
    /// The grant record the task wrote, read back through the kernel's own
    /// alias. Nothing in userspace is trusted to report it.
    fn stated_grant(&self) -> [u64; proof_words] {
        core::array::from_fn(|word| {
            self.execution
                .read_arena(0, proof_offset + word as u64 * 8)
                .unwrap_or_else(|| fail("program_proof_readback"))
        })
    }
    fn finish(&mut self) -> ! {
        // SAFETY: Serialized terminal dispatch retains bootstrap mappings and TSS.
        unsafe { self.execution.restore_bootstrap() }.unwrap_or_else(|reason| fail(reason));
        // SAFETY: Stable CPU storage and the initialized sole-BSP TSS stay live.
        if paging::read_cr3() != self.kernel_cr3
            || unsafe { descriptors::selected_privilege_stack_top() } != self.bootstrap_stack
            || self.kernel_stack == 0
            || !unsafe { self.cpu.finish() }
        {
            fail("program_bootstrap_restore");
        }
        let stack = self.kernel_stack;
        dispatch_active.store(false, Ordering::Release);
        active.store(ptr::null_mut(), Ordering::Release);
        // SAFETY: stack is the exact suspended enter continuation; bootstrap root
        // and CPU are restored and no handler can access the withdrawn program.
        unsafe { x86_64_application_resume_kernel(completion_status, stack) }
    }
}

// Return constructor temporaries before any mapping preparation or activation.
#[inline(never)]
unsafe fn initialize_storage(
    pointer: *mut program,
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    arena_base: u64,
    granted: [u64; proof_words],
) {
    // SAFETY: Caller owns the sole storage latch over uninitialized or fully
    // retired BSS. No whole-program reference exists until the identity is
    // written after retained preparation. No live owner is moved.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new_native_arenas());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(1).unwrap_or_else(|_| fail("program_scheduler")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution_group::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session_group::new());
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).granted).write(granted);
        ptr::addr_of_mut!((*pointer).arena_base).write(arena_base);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(0);
        ptr::addr_of_mut!((*pointer).calls).write(0);
        ptr::addr_of_mut!((*pointer).timer_ticks).write(0);
        ptr::addr_of_mut!((*pointer).invalid).write(0);
        ptr::addr_of_mut!((*pointer).exits).write(0);
        ptr::addr_of_mut!((*pointer).started).write(false);
        ptr::addr_of_mut!((*pointer).terminal).write(false);
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator,
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut program {
    let selected = scenarios
        .get(scenario)
        .unwrap_or_else(|| fail("program_scenario"));
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("program_storage_busy");
    }
    let pointer = storage.0.get().cast::<program>();
    // Built before anything is acquired, so an unsatisfiable launch costs
    // nothing. The descriptor is the contract; its own validation decides
    // whether this geometry and these grants are admissible at all.
    let descriptor = program_descriptor::try_new(
        selected.base,
        arena_bytes,
        selected.edition_handle,
        selected.edition_length,
        selected.console_handle,
        selected.granted_services,
        selected.arguments,
    )
    .unwrap_or_else(|_| fail("program_descriptor"));
    let granted = [
        descriptor.arena_length(),
        descriptor.edition_length(),
        descriptor.granted_services(),
        descriptor.arguments().len() as u64,
        u64::from(descriptor.console().is_some()),
        descriptor.edition_handle(),
    ];
    // SAFETY: Exclusive BSS latch, IF clear and NMI masked. Initialize each
    // field before borrowing it; the whole program remains unborrowed until
    // complete.
    unsafe {
        initialize_storage(
            pointer,
            image,
            scenario,
            kernel_cr3,
            bootstrap_stack,
            selected.base,
            granted,
        );
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        execution
            .prepare_program_arena(
                allocator,
                circles,
                (image.source, image.length),
                selected.base,
            )
            .unwrap_or_else(|e| fail(e));
        // The arena the launch promised must be the arena the task got. A
        // native application owner at this same base would report 32 KiB.
        if execution.arena_extent(0) != Some((selected.base, arena_bytes)) {
            fail("program_arena_extent");
        }
        for offset in (0..8192).step_by(8) {
            if execution.read_data(0, offset) != Some(0)
                || execution.read_stack(0, offset) != Some(0)
            {
                fail("program_initial_storage");
            }
        }
        let mut zeroed_bytes = 0;
        for offset in (0..arena_bytes).step_by(8) {
            if execution.read_arena(0, offset) != Some(0) {
                fail("program_initial_arena");
            }
            zeroed_bytes += 8;
        }
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(zeroed_bytes);
        let bytes = descriptor.encode();
        let mut readback = [0; program_startup_size];
        for (word, bytes) in bytes.chunks_exact(8).enumerate() {
            let offset = descriptor_offset + word as u64 * 8;
            execution
                .write_data(0, offset, u64::from_le_bytes(bytes.try_into().unwrap()))
                .unwrap_or_else(|e| fail(e));
            readback[word * 8..word * 8 + 8].copy_from_slice(
                &execution
                    .read_data(0, offset)
                    .unwrap_or_else(|| fail("program_descriptor_copy"))
                    .to_le_bytes(),
            );
        }
        // Decoded from what was actually written, not from what was intended:
        // the task will decode these same bytes and nothing else.
        if program_descriptor::decode(&readback) != Ok(descriptor) {
            fail("program_descriptor_readback");
        }
        let frame = execution.frame(0).unwrap_or_else(|e| fail(e));
        if !execution.frame_owned(0, frame)
            || (*frame).flags != 2
            || (*frame).stack_pointer != user_stack_top
        {
            fail("program_initial_frame");
        }
        (*frame).destination_index = descriptor_address;
        (*frame).source_index = program_startup_size as u64;
        let identity = execution
            .commit(0, tasks, circles, 701 + scenario as u64)
            .unwrap_or_else(|e| fail(e));
        ptr::addr_of_mut!((*pointer).identity).write(identity);
        &mut *pointer
    }
}

#[inline(never)]
fn run_program(
    allocator: &mut frame_allocator,
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> (&'static str, program_record) {
    if super::is_active()
        || is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
    {
        fail("program_setup");
    }
    let state = initialize(allocator, image, scenario, kernel_cr3, bootstrap_stack);
    state
        .scheduler
        .admit(&mut state.circles, &mut state.tasks, state.identity)
        .unwrap_or_else(|_| fail("program_admit"));
    // SAFETY: The mapping owner remains at its final retained BSS location.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let accounting = allocator.accounting();
    if accounting.free.checked_add(program_frames) != Some(initial.free)
        || accounting.data_owned != expected_data_frames
        || accounting.table_owned != expected_table_frames
        || accounting.active_leases != expected_leases
        || accounting.bound_leases != expected_leases
        || state.zeroed_bytes != arena_bytes
    {
        fail("program_live_ownership");
    }
    // SAFETY: Stable sole-BSP CPU owner and unchanged native entry profile.
    unsafe { state.cpu.prepare() }.unwrap_or_else(|e| fail(e));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("program_start"));
    if first.task() != state.identity || first.context() != scheduler_context::initial {
        fail("program_first");
    }
    // SAFETY: Serialized dispatch retains the root, stack and alias.
    unsafe { state.execution.select(0) }.unwrap_or_else(|reason| fail(reason));
    let frame = state
        .execution
        .frame(0)
        .unwrap_or_else(|reason| fail(reason));
    if !state.frame_valid(frame) {
        fail("program_selected_frame");
    }
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
        fail("program_publication");
    }
    // SAFETY: Serialized handlers alone borrow this stable owner until resume.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("program_initial_cpu");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    if status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || paging::read_cr0() & 8 != state.original_ts
        // SAFETY: Sole-BSP TSS remains initialized and bootstrap restored.
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || !state.terminal
        || !state.started
    {
        fail("program_continuation");
    }
    // Exactly the two operations this gate admits: startup and exit. An
    // interpreter will add more; until it does, a third call is a defect.
    if state.calls != 2 || state.exits != 1 || state.invalid != 0 {
        fail("program_evidence");
    }
    let stated = state.stated_grant();
    if stated != state.granted {
        fail("program_grant_mismatch");
    }
    if !state
        .tasks
        .snapshot(state.identity)
        .is_ok_and(|s| s.state() == task_state::exited)
    {
        fail("program_terminal");
    }
    state
        .scheduler
        .withdraw_terminal_context(&mut state.circles, &mut state.tasks, state.identity)
        .unwrap_or_else(|_| fail("program_withdraw"));
    // SAFETY: Bootstrap restored, the task is terminal and its context is
    // withdrawn; every mapping, arena and alias lease stays retained until
    // this explicit teardown.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|e| fail(e));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("program_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || state.tasks.validate(state.identity).is_ok()
    {
        fail("program_final_accounting");
    }
    let record = program_record {
        calls: state.calls,
        timer_ticks: state.timer_ticks,
        invalid: state.invalid,
        exits: state.exits,
        zeroed_bytes: state.zeroed_bytes,
        granted: state.granted,
        stated,
    };
    let vendor = state.cpu.cpu_vendor();
    storage_busy.store(false, Ordering::Release);
    (vendor, record)
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}
pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let pointer = active.load(Ordering::Acquire);
    // SAFETY: Published owner remains retained through every IF-clear handler.
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}
fn enter_dispatch() -> &'static mut program {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("program_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("program_inactive");
    }
    // SAFETY: Sole BSP, IF clear, nonreentrant handler owns the retained program.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut program, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // SAFETY: The selected eager image belongs to this stable exclusive owner.
    if !unsafe { state.cpu.restore_task(0) } {
        fail("program_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = enter_dispatch();
    // SAFETY: Exclusive handler captures this exact current owner's CPU image.
    if !unsafe { state.cpu.save_current(0) }
        || !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.timer_ticks >= max_calls
    {
        fail("program_timer");
    }
    state.last_tick = observed.tick();
    state.timer_ticks += 1;
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    // No fault is expected and none is armed. The vessel's panic handler ends
    // in `ud2`, so a fault here is a failed proof and is reported as one
    // rather than contained: there is nothing to contain it on behalf of.
    let _ = (state, frame, snapshot);
    fail("program_unexpected_fault")
}
pub(super) fn gate(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    // SAFETY: Stable session exclusively owns the current aligned CPU image.
    if !unsafe { state.cpu.save_current(0) } || !state.frame_valid(frame) {
        fail("program_gate_frame");
    }
    // SAFETY: Complete retained frame ownership validated above.
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector
        || value.error_code != 0
        || state.calls >= max_calls
        || (value.accumulator != 0 && !state.started)
    {
        fail("program_gate_vector");
    }
    state.calls += 1;
    let mut reply = 0;
    match value.accumulator {
        0 => {
            if state.started
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || value.destination_index != descriptor_address
                || value.source_index != program_startup_size as u64
                || !state
                    .cpu
                    .task_image(0)
                    .is_some_and(|image| image.is_initial_clean())
            {
                fail("program_startup");
            }
            state.started = true;
            // SAFETY: The exact startup frame remains exclusively owned. RDI/RSI
            // continue to carry the descriptor span; they are never reply fields.
            unsafe {
                (*frame).flags = 0x202;
            }
        }
        5 => {
            if value.destination_index != 0 {
                fail("program_user_status");
            }
            state.terminal = true;
            state.exits += 1;
            state
                .scheduler
                .complete_current(&mut state.tasks, state.identity)
                .unwrap_or_else(|_| fail("program_complete"));
            state.finish();
        }
        // Everything else is refused, including the console write the grant
        // names: there is no transport behind it yet. A refusal is the honest
        // answer, and the task's own evidence records that it was refused.
        _ => {
            reply = 4;
            state.invalid += 1;
        }
    }
    // SAFETY: The current exact frame remains exclusively retained until IRETQ.
    unsafe {
        (*frame).accumulator = reply;
        (*frame).counter = 0;
        (*frame).data = 0;
    }
    leave_dispatch(state, frame)
}

pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("program_initial_state");
    }
    let image = program_image::inspect();
    // Withdrawn for the same reason the native runtime profile withdraws it:
    // the interrupt failure surface paints through the bootstrap root, and a
    // user root is selected for the whole of this proof. Nothing presents
    // here, so the aperture is consumed and never handed on. Required rather
    // than skipped when absent, so a missing aperture is reported as one.
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
    let _aperture = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("program_framebuffer_unavailable"));
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Initialized sole-BSP descriptors remain exclusively owned.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let (initial, vendor, records) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let mut records = [program_record::default(); scenarios.len()];
        let mut vendor = "";
        for (scenario, slot) in records.iter_mut().enumerate() {
            let (current_vendor, record) = run_program(
                allocator,
                image,
                scenario,
                kernel_cr3,
                bootstrap_stack,
                initial,
            );
            if scenario == 0 {
                vendor = current_vendor;
            } else if current_vendor != vendor {
                fail("program_cpu_comparison");
            }
            *slot = record;
        }
        (initial, vendor, records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("program_final_interrupts");
    }
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_program_test.ready","level":"info","vendor":"{}","tasks":1,"scenarios":{},"code_pages":{},"descriptor_bytes":{},"arena_bytes":{},"mappings":4,"data_frames":{},"table_frames":{},"leases":{},"free_frames":{},"edition_mapped":false,"console_transport":false}}"#,
        vendor,
        scenarios.len(),
        program_code_pages,
        program_startup_size,
        arena_bytes,
        expected_data_frames,
        expected_table_frames,
        expected_leases,
        initial.free
    );
    let mut zeroed_bytes = 0;
    for (scenario, record) in records.into_iter().enumerate() {
        zeroed_bytes += record.zeroed_bytes;
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":{},"arena_base":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"invalid":{},"exits":{},"granted_services":{},"console_granted":{},"argument_bytes":{},"edition_bytes":{},"grant_restated":{}}}"#,
            scenario,
            scenarios[scenario].base,
            record.zeroed_bytes,
            record.calls,
            record.timer_ticks,
            record.invalid,
            record.exits,
            record.granted[2],
            record.granted[4],
            record.granted[3],
            record.granted[1],
            record.stated == record.granted
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_program_test.passed","level":"info","program_tasks":{},"descriptor_bytes":{},"arena_bytes":{},"zeroed_bytes":{},"grants_restated":true,"editions_executed":0,"resources_restored":true,"authority_revoked":true,"source_language":"rust"}}"#,
        scenarios.len(),
        program_startup_size,
        arena_bytes,
        zeroed_bytes
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
