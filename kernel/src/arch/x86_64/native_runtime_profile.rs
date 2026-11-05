//! Experimental copied native startup and private precommitted arena proof.
//! Kernel-owned mappings and per-task handle bindings remain authoritative;
//! mutable startup bytes never create mappings, tasks or channel grants.
use super::super::owned_execution::user_data_base;
use super::*;
use anomalous_native_runtime::{arena_byte_count, startup_descriptor, startup_size};

const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;
const arena_frames: usize = 66;
const execution_order: [usize; 4] = [0, 2, 3, 1];
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
// SAFETY: Sole BSP, IF clear. The latch covers initialization through teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

unsafe extern "C" {
    static x86_64_native_runtime_payload_start: u8;
    static x86_64_native_runtime_payload_end: u8;
    static x86_64_native_runtime_user_guard_fault_instruction: u8;
    static x86_64_native_runtime_user_nx_fault_instruction: u8;
    static x86_64_native_runtime_user_fault_instruction: u8;
}
#[derive(Clone, Copy)]
struct runtime_image {
    source: u64,
    length: u64,
    guard: u64,
    nx: u64,
}
impl runtime_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_runtime_payload_start);
        let length = address!(x86_64_native_runtime_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("runtime_payload_extent"));
        let guard = address!(x86_64_native_runtime_user_guard_fault_instruction);
        let nx = address!(x86_64_native_runtime_user_nx_fault_instruction);
        let fault = address!(x86_64_native_runtime_user_fault_instruction);
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > code_pages * 4096
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || [guard, nx, fault].iter().any(|marker| {
                *marker < user_code_base
                    || marker
                        .checked_add(2)
                        .is_none_or(|end| end > user_code_base + length)
            })
        {
            fail("runtime_artifact_geometry");
        }
        // SAFETY: Inspected complete two-byte marker in immutable resident payload.
        if unsafe { ptr::read_unaligned((source + fault - user_code_base) as *const [u8; 2]) }
            != [0x0f, 0x0b]
        {
            fail("runtime_artifact_fault_marker");
        }
        Self {
            source,
            length,
            guard,
            nx,
        }
    }
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 2],
    execution: owned_execution,
    cpu: application_cpu::cpu_session,
    channels: [application_channel; 2],
    last_ack: [u64; 2],
    image: runtime_image,
    arena_bases: [u64; 2],
    handles: [[u64; 2]; 2],
    arena_copies: u64,
    zeroed_bytes: u64,
    scenario: usize,
    selected: usize,
    started: [bool; 2],
    terminal: [bool; 2],
    failure_armed: bool,
    evidence: evidence,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        // SAFETY: The sole BSP reads its initialized TSS with interrupts excluded.
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
                .is_ok_and(|s| s.state() == task_state::running && s.context_attached())
    }
    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(self.identities[self.selected]) else {
            return false;
        };
        // SAFETY: frame_owned validated the complete aligned retained frame.
        let value = unsafe { ptr::read(frame) };
        user_return_fields_are_valid_within_stack(value, snapshot.bounds())
            && self
                .execution
                .admission(self.selected)
                .is_ok_and(|a| a.accepts_return(value.instruction_pointer, value.stack_pointer))
            && value.flags & !0x25_0fd7 == 0
            && (!self.started[self.selected] || value.flags & 0x202 == 0x202)
    }
    fn select(&mut self, index: usize) -> *mut scheduler_frame {
        // SAFETY: Serialized dispatch retains both roots, stacks and aliases.
        unsafe { self.execution.select(index) }.unwrap_or_else(|reason| fail(reason));
        self.selected = index;
        let frame = self
            .execution
            .frame(index)
            .unwrap_or_else(|reason| fail(reason));
        if !self.frame_valid(frame) {
            fail("selected_frame");
        }
        frame
    }
    fn close_channels(&mut self) {
        for channel in &mut self.channels {
            let receipt = channel
                .close(channel.binding())
                .unwrap_or_else(|_| fail("channel_close"));
            self.evidence.discarded += u64::from(receipt.discarded());
            if !channel.storage_is_clear() {
                fail("channel_clear");
            }
        }
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
            fail("bootstrap_restore");
        }
        let stack = self.kernel_stack;
        dispatch_active.store(false, Ordering::Release);
        active.store(ptr::null_mut(), Ordering::Release);
        // SAFETY: stack is the exact suspended enter continuation; bootstrap root
        // and CPU are restored and no handler can access the withdrawn runtime.
        unsafe { x86_64_application_resume_kernel(completion_status, stack) }
    }
    fn packet_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        let Some(index) = self.handles[self.selected]
            .iter()
            .position(|handle| *handle == value.destination_index)
        else {
            return (2, 0, 0);
        };
        self.packet_operation(
            value.accumulator,
            index,
            value.source_index,
            value.data,
            value.counter,
        )
    }
    fn packet_operation(
        &mut self,
        operation: u64,
        index: usize,
        address: u64,
        length: u64,
        sequence: u64,
    ) -> (u64, u64, u64) {
        let binding = self.channels[index].binding();
        let caller = self.identities[self.selected];
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        match operation {
            1 => {
                let prepared =
                    match self.channels[index].prepare_submit(binding, caller, sequence, length) {
                        Ok(p) => p,
                        Err(e) => return (channel_status(e), 0, 0),
                    };
                let mut scratch = [0; application_packet_capacity];
                if self
                    .execution
                    .copy_from_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        caller,
                        address,
                        &mut scratch[..length],
                    )
                    .is_err()
                {
                    return (3, 0, 0);
                }
                if !self.arena_contains(self.selected, address, length) {
                    fail("runtime_submit_not_arena");
                }
                self.arena_copies += 1;
                let hash = packet_hash(&scratch[..length]);
                let receipt = self.channels[index]
                    .commit_submit(prepared, &scratch[..length])
                    .unwrap_or_else(|_| fail("submit_commit"));
                scratch.fill(0);
                self.evidence.sends[index] += 1;
                if index == 0 {
                    if !(1..=7).contains(&receipt.sequence()) {
                        fail("surface_sequence_bound");
                    }
                    self.evidence.surface_hashes[receipt.sequence() as usize - 1] = hash;
                }
                (0, receipt.sequence(), receipt.length() as u64)
            }
            2 => {
                let prepared = match self.channels[index].prepare_delivery(binding, caller, length)
                {
                    Ok(p) => p,
                    Err(e) => return (channel_status(e), 0, 0),
                };
                let mut hash = 0;
                let result = self.channels[index].deliver_with(prepared, |bytes| {
                    hash = packet_hash(bytes);
                    self.execution.copy_to_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        caller,
                        address,
                        bytes,
                    )
                });
                match result {
                    Ok(receipt) => {
                        if !self.arena_contains(self.selected, address, receipt.length()) {
                            fail("runtime_delivery_not_arena");
                        }
                        self.arena_copies += 1;
                        if index == 0
                            && self.evidence.surface_hashes[receipt.sequence() as usize - 1] != hash
                        {
                            fail("copied_snapshot_changed");
                        }
                        self.evidence.deliveries[index] += 1;
                        (0, receipt.sequence(), receipt.length() as u64)
                    }
                    Err(error) => (channel_status(error), 0, 0),
                }
            }
            3 => match self.channels[index].acknowledge(binding, caller, sequence) {
                Ok(receipt) => {
                    self.last_ack[index] = receipt.sequence();
                    self.evidence.acknowledgements[index] += 1;
                    (0, receipt.sequence(), 0)
                }
                Err(error) => (channel_status(error), 0, 0),
            },
            6 => {
                if caller != binding.producer() {
                    return (2, 0, 0);
                }
                if self.channels[index].phase() == channel_phase::closed {
                    return (5, 0, 0);
                }
                if sequence == 0
                    || self.channels[index]
                        .next_sequence()
                        .is_none_or(|next| sequence >= next)
                {
                    return (4, 0, 0);
                }
                if self.last_ack[index] == sequence {
                    (0, sequence, 0)
                } else {
                    (1, 0, 0)
                }
            }
            _ => (4, 0, 0),
        }
    }
    fn arena_contains(&self, index: usize, address: u64, length: usize) -> bool {
        address >= self.arena_bases[index]
            && address
                .checked_add(length as u64)
                .is_some_and(|end| end <= self.arena_bases[index] + arena_byte_count)
    }
}

#[derive(Clone, Copy, Default)]
struct runtime_record {
    application: evidence,
    arena_copies: u64,
    zeroed_bytes: u64,
    starts: u64,
}

// Return constructor temporaries before any mapping preparation or activation.
#[inline(never)]
unsafe fn initialize_storage(
    pointer: *mut runtime,
    image: runtime_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    arena_bases: [u64; 2],
    handles: [[u64; 2]; 2],
) {
    // SAFETY: Caller owns the sole storage latch over uninitialized or fully
    // retired BSS. No whole-runtime reference exists until identities/channels
    // are written after retained preparation. No live owner is moved.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new_native_arenas());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(2).unwrap_or_else(|_| fail("runtime_scheduler")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session::new());
        ptr::addr_of_mut!((*pointer).last_ack).write([0; 2]);
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).arena_bases).write(arena_bases);
        ptr::addr_of_mut!((*pointer).handles).write(handles);
        ptr::addr_of_mut!((*pointer).arena_copies).write(0);
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(0);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([false; 2]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; 2]);
        ptr::addr_of_mut!((*pointer).failure_armed).write(false);
        ptr::addr_of_mut!((*pointer).evidence).write(evidence::default());
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
    image: runtime_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut runtime {
    if scenario >= 4 || storage_busy.swap(true, Ordering::AcqRel) {
        fail("runtime_storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    let arena_bases = if scenario.is_multiple_of(2) {
        [0x804000, 0x814000]
    } else {
        [0x814000, 0x804000]
    };
    let handles = if scenario.is_multiple_of(2) {
        [[0x31, 0x52]; 2]
    } else {
        [[0x91, 0xb2]; 2]
    };
    let document: &[u8] = if scenario.is_multiple_of(2) {
        b"const before = 1;"
    } else {
        b"const before = 2;"
    };
    // SAFETY: Exclusive BSS latch, IF clear and NMI masked. Initialize each
    // field before borrowing it; whole runtime remains unborrowed until complete.
    unsafe {
        initialize_storage(
            pointer,
            image,
            scenario,
            kernel_cr3,
            bootstrap_stack,
            arena_bases,
            handles,
        );
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        execution
            .prepare_native_arenas(
                allocator,
                circles,
                [(image.source, image.length); 2],
                [(0, 0); 2],
                arena_bases,
            )
            .unwrap_or_else(|e| fail(e));
        let mut zeroed_bytes = 0;
        for index in 0..2 {
            if execution.arena_extent(index) != Some((arena_bases[index], arena_byte_count)) {
                fail("runtime_arena_extent");
            }
            for offset in (0..8192).step_by(8) {
                if execution.read_data(index, offset) != Some(0)
                    || execution.read_stack(index, offset) != Some(0)
                {
                    fail("runtime_initial_storage");
                }
            }
            for offset in (0..arena_byte_count).step_by(8) {
                if execution.read_arena(index, offset) != Some(0) {
                    fail("runtime_initial_arena");
                }
                zeroed_bytes += 8;
            }
            let descriptor = startup_descriptor::try_new(
                index as u64,
                scenario as u64,
                arena_bases[index],
                arena_byte_count,
                handles[index][index],
                handles[index][1 - index],
                document,
            )
            .unwrap_or_else(|_| fail("runtime_descriptor"));
            let bytes = descriptor.encode();
            let mut readback = [0; startup_size];
            for (word, bytes) in bytes.chunks_exact(8).enumerate() {
                let offset = descriptor_offset + word as u64 * 8;
                execution
                    .write_data(index, offset, u64::from_le_bytes(bytes.try_into().unwrap()))
                    .unwrap_or_else(|e| fail(e));
                readback[word * 8..word * 8 + 8].copy_from_slice(
                    &execution
                        .read_data(index, offset)
                        .unwrap_or_else(|| fail("runtime_descriptor_copy"))
                        .to_le_bytes(),
                );
            }
            if startup_descriptor::decode(&readback) != Ok(descriptor) {
                fail("runtime_descriptor_readback");
            }
            let frame = execution.frame(index).unwrap_or_else(|e| fail(e));
            if !execution.frame_owned(index, frame)
                || (*frame).flags != 2
                || (*frame).stack_pointer != user_stack_top
            {
                fail("runtime_initial_frame");
            }
            (*frame).destination_index = descriptor_address;
            (*frame).source_index = startup_size as u64;
        }
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(zeroed_bytes);
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 601 + index as u64)
                .unwrap_or_else(|e| fail(e))
        });
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        for index in 0..2 {
            let binding = application_channel_binding::try_new(
                endpoint_id::try_from_parts(index, 101 + scenario as u64).unwrap(),
                identities[index],
                identities[1 - index],
            )
            .unwrap_or_else(|_| fail("runtime_channel_binding"));
            ptr::addr_of_mut!((*pointer).channels)
                .cast::<application_channel>()
                .add(index)
                .write(
                    application_channel::try_new(binding)
                        .unwrap_or_else(|_| fail("runtime_channel")),
                );
        }
        &mut *pointer
    }
}

#[inline(never)]
fn run_arena_pair(
    allocator: &mut frame_allocator,
    image: runtime_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> (&'static str, runtime_record) {
    if super::is_active()
        || is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || !stage_is_clear()
    {
        fail("runtime_setup");
    }
    let state = initialize(allocator, image, scenario, kernel_cr3, bootstrap_stack);
    for id in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, id)
            .unwrap_or_else(|_| fail("runtime_admit"));
    }
    // SAFETY: Both mapping owners remain at their final retained BSS location.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let accounting = allocator.accounting();
    if accounting.free.checked_add(arena_frames) != Some(initial.free)
        || accounting.data_owned != 48
        || accounting.table_owned != 18
        || accounting.active_leases != 12
        || accounting.bound_leases != 12
        || state.execution.root(0).ok() == state.execution.root(1).ok()
        || state.execution.transition_stack_top(0).ok()
            == state.execution.transition_stack_top(1).ok()
        || state.zeroed_bytes != 2 * arena_byte_count
    {
        fail("runtime_live_ownership");
    }
    // SAFETY: Stable sole-BSP CPU owner and unchanged native entry profile.
    unsafe { state.cpu.prepare() }.unwrap_or_else(|e| fail(e));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("runtime_start"));
    if first.task() != state.identities[0] || first.context() != scheduler_context::initial {
        fail("runtime_first");
    }
    let frame = state.select(0);
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
    // SAFETY: Serialized handlers alone borrow this stable owner until resume.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("runtime_initial_cpu");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    if status != completion_status || is_active() || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3 || paging::read_cr0() & 8 != state.original_ts
        // SAFETY: Sole-BSP TSS remains initialized and bootstrap restored.
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || interrupts::interrupts_enabled() || !tls_bases_are_zero() || !state.cpu.storage_is_cleared()
        || state.terminal != [true; 2] || state.started != [true; 2] || state.failure_armed
        || state.channels.iter().any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        fail("runtime_continuation");
    }
    let e = state.evidence;
    if scenario < 2 {
        if e.calls != 114
            || e.switches != 14
            || e.denials != 4
            || e.bad_copies != 2
            || e.invalid != 1
            || e.closed != 0
            || e.sends != [7, 6]
            || e.deliveries != [7, 6]
            || e.acknowledgements != [7, 6]
            || e.rows != stage_height as u64
            || e.exits != 2
            || e.faults != 0
            || e.discarded != 0
            || state.arena_copies != 50
        {
            fail("runtime_normal_evidence");
        }
    } else if e.calls != 14
        || e.switches != 0
        || e.denials != 6
        || e.bad_copies != 1
        || e.invalid != 0
        || e.closed != 2
        || e.sends != [1, 0]
        || e.deliveries != [0, 0]
        || e.acknowledgements != [0, 0]
        || e.exits != 1
        || e.faults != 1
        || e.discarded != 1
        || e.rows != 0
        || state.arena_copies != 1
    {
        fail("runtime_fault_evidence");
    }
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if scenario >= 2 && index == 0 {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|s| s.state() == expected)
        {
            fail("runtime_terminal");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("runtime_withdraw"));
    }
    // SAFETY: Bootstrap restored, tasks terminal and all contexts withdrawn;
    // every mapping/arena/alias lease stays retained until explicit teardown.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|e| fail(e));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("runtime_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || state
            .identities
            .iter()
            .any(|id| state.tasks.validate(*id).is_ok())
    {
        fail("runtime_final_accounting");
    }
    let record = runtime_record {
        application: e,
        arena_copies: state.arena_copies,
        zeroed_bytes: state.zeroed_bytes,
        starts: state.started.iter().filter(|started| **started).count() as u64,
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
fn enter_dispatch() -> &'static mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("runtime_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("runtime_inactive");
    }
    // SAFETY: Sole BSP, IF clear, nonreentrant handler owns the retained runtime.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // SAFETY: Selected eager image belongs to this stable exclusive owner.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("runtime_cpu_restore");
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
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame.cast())
        || state.failure_armed
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.evidence.timer_ticks >= max_calls
    {
        fail("runtime_timer");
    }
    state.last_tick = observed.tick();
    state.evidence.timer_ticks += 1;
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    // Validate ownership, not return admission: a deliberate NX fetch points
    // outside the admitted executable image and must still be containable.
    // SAFETY: Exclusive handler owns the interrupted task's eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || !state.failure_armed
        || state.selected != 0
        || snapshot.vector != 14
        || snapshot.flags & 0x200 != 0
        || state.evidence.sends != [1, 0]
        || state.evidence.deliveries != [0, 0]
        || state.channels[0].phase() != channel_phase::queued
    {
        fail("runtime_unexpected_fault");
    }
    let exact = if state.scenario == 2 {
        snapshot.error_code == 6
            && snapshot.instruction_pointer == state.image.guard
            && snapshot.fault_address == Some(state.arena_bases[0] - 8)
    } else if state.scenario == 3 {
        snapshot.error_code == 0x15
            && snapshot.instruction_pointer == state.arena_bases[0]
            && snapshot.fault_address == Some(state.arena_bases[0])
    } else {
        false
    };
    if !exact {
        fail("runtime_fault_site");
    }
    state.close_channels();
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.identities[0])
        .unwrap_or_else(|_| fail("runtime_fault_transition"));
    if transition.dispatch().task() != state.identities[1] {
        fail("runtime_fault_peer");
    }
    state.terminal[0] = true;
    state.evidence.faults += 1;
    state.failure_armed = false;
    let selected = state.select(1);
    Some(leave_dispatch(state, selected).cast())
}
pub(super) fn gate(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    // SAFETY: Stable session exclusively owns the current aligned CPU image.
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame) {
        fail("runtime_gate_frame");
    }
    // SAFETY: Complete retained frame ownership validated above.
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector
        || value.error_code != 0
        || state.evidence.calls >= max_calls
        || (value.accumulator != 0 && !state.started[state.selected])
    {
        fail("runtime_gate_vector");
    }
    state.evidence.calls += 1;
    let mut selected = frame;
    let mut reply = (0, 0, 0);
    match value.accumulator {
        0 => {
            if state.started[state.selected]
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || value.destination_index != descriptor_address
                || value.source_index != startup_size as u64
                || !state
                    .cpu
                    .task_image(state.selected)
                    .is_some_and(|image| image.is_initial_clean())
            {
                fail("runtime_startup");
            }
            state.started[state.selected] = true;
            // SAFETY: Exact startup frame remains exclusively owned. RDI/RSI
            // continue to carry the descriptor span; they are never reply fields.
            unsafe {
                (*frame).flags = 0x202;
            }
        }
        1 | 2 | 3 | 6 => {
            reply = state.packet_call(value);
            match reply.0 {
                2 => state.evidence.denials += 1,
                3 => state.evidence.bad_copies += 1,
                4 => state.evidence.invalid += 1,
                5 => state.evidence.closed += 1,
                _ => {}
            }
        }
        4 => {
            if !state.terminal[1 - state.selected] {
                let transition = state
                    .scheduler
                    .preempt_current(&mut state.tasks, state.identities[state.selected])
                    .unwrap_or_else(|_| fail("runtime_yield"));
                if transition.dispatch().task() != state.identities[1 - state.selected] {
                    fail("runtime_yield_peer");
                }
                // SAFETY: Reply saved in the outgoing owner's retained frame.
                unsafe {
                    (*frame).accumulator = 0;
                    (*frame).counter = 0;
                    (*frame).data = 0;
                }
                selected = state.select(1 - state.selected);
                state.evidence.switches += 1;
            }
        }
        5 => {
            if value.destination_index != 0 {
                fail("runtime_user_status");
            }
            let index = state.selected;
            state.close_channels();
            state.terminal[index] = true;
            state.evidence.exits += 1;
            if state.terminal[1 - index] {
                state
                    .scheduler
                    .complete_current(&mut state.tasks, state.identities[index])
                    .unwrap_or_else(|_| fail("runtime_complete_last"));
                state.finish();
            }
            let transition = state
                .scheduler
                .complete_current_and_dispatch(&mut state.tasks, state.identities[index])
                .unwrap_or_else(|_| fail("runtime_complete_peer"));
            if transition.dispatch().task() != state.identities[1 - index] {
                fail("runtime_complete_wrong_peer");
            }
            selected = state.select(1 - index);
        }
        7 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.scenario >= 2
                || !state.terminal[0]
                || state.last_ack != [7, 6]
                || value.destination_index != state.evidence.rows
                || value.destination_index >= stage_height as u64
                || value.data != stage_row_bytes as u64
            {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else {
                let mut row = [0; stage_row_bytes];
                if state
                    .execution
                    .copy_from_user(
                        &state.tasks,
                        &state.circles,
                        &state.scheduler,
                        state.identities[1],
                        value.source_index,
                        &mut row,
                    )
                    .is_err()
                {
                    reply.0 = 3;
                    state.evidence.bad_copies += 1;
                } else {
                    if !state.arena_contains(1, value.source_index, row.len()) {
                        fail("runtime_row_not_arena");
                    }
                    state.arena_copies += 1;
                    let start = state.evidence.rows as usize * stage_row_bytes;
                    // SAFETY: Complete copied row, exclusive service grant and
                    // retained kernel-only stage; no userspace alias escapes.
                    unsafe {
                        (&mut *display_stage.0.get())[start..start + stage_row_bytes]
                            .copy_from_slice(&row);
                    }
                    state.evidence.rows += 1;
                }
                row.fill(0);
            }
        }
        8 => {
            let (target, marker) = if state.scenario == 2 {
                (state.arena_bases[0] - 8, state.image.guard)
            } else if state.scenario == 3 {
                (state.arena_bases[0], state.image.nx)
            } else {
                fail("runtime_fault_scenario");
            };
            if state.selected != 0
                || state.failure_armed
                || value.destination_index != state.scenario as u64
                || value.source_index != target
                || value.instruction_pointer != marker
                || state.evidence.sends != [1, 0]
                || state.evidence.deliveries != [0, 0]
                || state.channels[0].phase() != channel_phase::queued
            {
                fail("runtime_fault_arm");
            }
            state.failure_armed = true;
            // SAFETY: Disable IF only on this exact guard/NX fault-probe return.
            unsafe {
                (*frame).flags = value.flags & !0x200;
            }
        }
        _ => {
            reply.0 = 4;
            state.evidence.invalid += 1;
        }
    }
    if selected == frame {
        // SAFETY: Current exact frame remains exclusively retained until IRETQ.
        unsafe {
            (*frame).accumulator = reply.0;
            (*frame).counter = reply.1;
            (*frame).data = reply.2;
        }
    }
    leave_dispatch(state, selected)
}

pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("runtime_initial_state");
    }
    let baseline_image = artifact::inspect();
    let image = runtime_image::inspect();
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: Unique withdrawn aperture is retained exclusively through this proof.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Initialized sole-BSP descriptors remain exclusively owned.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    let (initial, vendor, baseline, records) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (vendor, normal) = run_owned(
            allocator,
            baseline_image,
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        let patch = presenter
            .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                &*display_stage.0.get()
            })
            .unwrap_or_else(|_| fail("runtime_baseline_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
            fail("runtime_baseline_pixels");
        }
        clear_stage();
        let mut presented = framebuffer_hash(info, false);
        let (fault_vendor, faulted) = run_owned(
            allocator,
            baseline_image,
            kernel_cr3,
            bootstrap_stack,
            1,
            initial,
        );
        if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
            fail("runtime_baseline_fault");
        }
        clear_stage();
        let mut records = [runtime_record::default(); 4];
        for (position, scenario) in execution_order.into_iter().enumerate() {
            let (current_vendor, record) = run_arena_pair(
                allocator,
                image,
                scenario,
                kernel_cr3,
                bootstrap_stack,
                initial,
            );
            if current_vendor != vendor {
                fail("runtime_cpu_comparison");
            }
            if scenario < 2 {
                if (record.application.surface_hashes == normal.surface_hashes) != (scenario == 0) {
                    fail("runtime_document_comparison");
                }
                let patch = presenter
                    .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                        &*display_stage.0.get()
                    })
                    .unwrap_or_else(|_| fail("runtime_patch"));
                if scenario == 0 && !patch.matches_pixels() {
                    fail("runtime_baseline_comparison");
                }
                if !patch.commit_and_verify() {
                    fail("runtime_pixel_readback");
                }
                let changed = framebuffer_hash(info, false);
                if (changed == presented) != (scenario == 0) {
                    fail("runtime_document_pixels");
                }
                presented = changed;
            }
            clear_stage();
            if framebuffer_hash(info, true) != outside || framebuffer_hash(info, false) != presented
            {
                fail("runtime_preserved_pixels");
            }
            records[position] = record;
        }
        (initial, vendor, [normal, faulted], records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("runtime_final_interrupts");
    }
    emit_evidence(initial, vendor, baseline);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_runtime_test.ready","level":"info","tasks":2,"scenarios":4,"descriptor_bytes":128,"arena_bytes":32768,"mappings_per_task":4,"data_frames":48,"table_frames":18,"leases":12}}"#
    );
    let mut zeroed_bytes = 0;
    for (scenario, record) in execution_order.into_iter().zip(records) {
        let e = record.application;
        zeroed_bytes += record.zeroed_bytes;
        let fault_kind = match scenario {
            2 => "guard",
            3 => "nx",
            _ => "none",
        };
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_runtime_test.scenario","level":"info","scenario":{},"document":{},"layout":{},"starts":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"cooperative_switches":{},"surfaces":{},"inputs":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"rows":{},"exits":{},"faults":{},"fault_kind":"{}","arena_copies":{},"resources_restored":true}}"#,
            scenario,
            scenario % 2,
            scenario % 2,
            record.starts,
            record.zeroed_bytes,
            e.calls,
            e.timer_ticks,
            e.switches,
            e.sends[0],
            e.sends[1],
            e.deliveries[0],
            e.deliveries[1],
            e.acknowledgements[0],
            e.acknowledgements[1],
            e.denials,
            e.bad_copies,
            e.invalid,
            e.closed,
            e.discarded,
            e.rows,
            e.exits,
            e.faults,
            fault_kind,
            record.arena_copies
        );
        if scenario < 2 {
            for (index, hash) in e.surface_hashes.into_iter().enumerate() {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_runtime_test.surface","level":"info","scenario":{},"sequence":{},"hash":{}}}"#,
                    scenario,
                    index + 1,
                    hash
                );
            }
        }
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_runtime_test.passed","level":"info","application_tasks":8,"descriptor_bytes":128,"arena_bytes":32768,"documents":2,"layouts":2,"normal_runs":2,"contained_faults":2,"zeroed_bytes":{},"resources_restored":true,"authority_revoked":true,"source_language":"rust"}}"#,
        zeroed_bytes
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
