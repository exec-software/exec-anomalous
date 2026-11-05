//! Native editor replacement with one retained userspace RAM service.
//! Kernel-owned mappings and per-task handle bindings remain authoritative;
//! mutable startup bytes never create mappings, tasks or channel grants.
use super::super::owned_execution::user_data_base;
use super::*;
use anomalous_document_protocol::{
    arena_byte_count, client_identity, startup_descriptor, startup_size,
};

#[cfg(feature = "elf_relaunch_test")]
#[path = "elf_relaunch_profile.rs"]
mod elf_relaunch_profile;

const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;
const arena_frames: usize = 66;
const execution_order: [usize; 3] = [0, 1, 2];
const checkpoint_status: u64 = 0x4152;
const channel_count: usize = 4;
const packet_limit: usize = 32;
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
// SAFETY: Sole BSP, IF clear. The latch covers initialization through teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

unsafe extern "C" {
    static x86_64_native_relaunch_payload_start: u8;
    static x86_64_native_relaunch_payload_end: u8;
    static x86_64_native_relaunch_user_before_fault_instruction: u8;
    static x86_64_native_relaunch_user_after_fault_instruction: u8;
    static x86_64_native_relaunch_user_fault_instruction: u8;
}
#[derive(Clone, Copy)]
struct runtime_image {
    source: u64,
    length: u64,
    before: u64,
    after: u64,
    #[cfg(feature = "elf_relaunch_test")]
    data: (u64, u64),
}
impl runtime_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_relaunch_payload_start);
        let length = address!(x86_64_native_relaunch_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("runtime_payload_extent"));
        let before = address!(x86_64_native_relaunch_user_before_fault_instruction);
        let after = address!(x86_64_native_relaunch_user_after_fault_instruction);
        let fault = address!(x86_64_native_relaunch_user_fault_instruction);
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > code_pages * 4096
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || [before, after, fault].iter().any(|marker| {
                *marker < user_code_base
                    || marker
                        .checked_add(2)
                        .is_none_or(|end| end > user_code_base + length)
            })
        {
            fail("runtime_artifact_geometry");
        }
        for marker in [before, after, fault] {
            // SAFETY: Each complete marker lies in the immutable resident payload.
            if unsafe { ptr::read_unaligned((source + marker - user_code_base) as *const [u8; 2]) }
                != [0x0f, 0x0b]
            {
                fail("document_artifact_fault_marker");
            }
        }
        Self {
            source,
            length,
            before,
            after,
            #[cfg(feature = "elf_relaunch_test")]
            data: (0, 0),
        }
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct document_evidence {
    calls: u64,
    switches: u64,
    timer_ticks: u64,
    sends: [u64; channel_count],
    deliveries: [u64; channel_count],
    acknowledgements: [u64; channel_count],
    denials: u64,
    bad_copies: u64,
    invalid: u64,
    closed: u64,
    discarded: u64,
    exits: u64,
    faults: u64,
    rows: u64,
    packet_hashes: [[[u64; packet_limit]; channel_count]; 2],
    route_sends: [[u64; channel_count]; 2],
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 2],
    execution: owned_execution,
    cpu: application_cpu::cpu_session,
    channels: [application_channel; channel_count],
    last_ack: [u64; channel_count],
    image: runtime_image,
    arena_bases: [u64; 2],
    handles: [u64; channel_count],
    arena_copies: u64,
    zeroed_bytes: u64,
    scenario: usize,
    selected: usize,
    started: [bool; 2],
    terminal: [bool; 2],
    failure_armed: bool,
    evidence: document_evidence,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
    incarnation: usize,
    starts: u64,
    checkpointed: bool,
    receipt_copied: bool,
    rebind_complete: bool,
    successor: [u8; startup_size],
    service_checks: u64,
    release_failures: u64,
    release_retries: u64,
    prepare_failures: u64,
    prepare_rollbacks: u64,
    publication_failures: u64,
    publication_rollbacks: u64,
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
    fn fault_boundary_valid(&self) -> bool {
        self.incarnation == 0
            && self.evidence.route_sends[0]
                == if self.scenario == 1 {
                    [4, 5, 3, 2]
                } else {
                    [4, 6, 3, 3]
                }
            && self.evidence.deliveries
                == if self.scenario == 1 {
                    [4, 5, 2, 2]
                } else {
                    [4, 6, 3, 2]
                }
            && self.evidence.deliveries == self.evidence.acknowledgements
            && self.channels[if self.scenario == 1 { 2 } else { 3 }].phase()
                == channel_phase::queued
            && [0, 1]
                .into_iter()
                .all(|index| self.channels[index].phase() == channel_phase::empty)
            && self.channels[if self.scenario == 1 { 3 } else { 2 }].phase() == channel_phase::empty
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
        let Some(index) = self
            .handles
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
                if !(1..=packet_limit as u64).contains(&sequence) {
                    return (4, 0, 0);
                }
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
                self.evidence.route_sends[self.incarnation][index] += 1;
                if !(1..=packet_limit as u64).contains(&receipt.sequence()) {
                    fail("document_packet_sequence_bound");
                }
                self.evidence.packet_hashes[self.incarnation][index]
                    [receipt.sequence() as usize - 1] = hash;
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
                        if !(1..=packet_limit as u64).contains(&receipt.sequence())
                            || self.evidence.packet_hashes[self.incarnation][index]
                                [receipt.sequence() as usize - 1]
                                != hash
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

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct runtime_record {
    application: document_evidence,
    arena_copies: u64,
    zeroed_bytes: u64,
    starts: u64,
    service_checks: u64,
    release_failures: u64,
    release_retries: u64,
    prepare_failures: u64,
    prepare_rollbacks: u64,
    publication_failures: u64,
    publication_rollbacks: u64,
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
    handles: [u64; channel_count],
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
        ptr::addr_of_mut!((*pointer).last_ack).write([0; channel_count]);
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
        ptr::addr_of_mut!((*pointer).evidence).write(document_evidence::default());
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
        ptr::addr_of_mut!((*pointer).incarnation).write(0);
        ptr::addr_of_mut!((*pointer).starts).write(0);
        ptr::addr_of_mut!((*pointer).checkpointed).write(false);
        ptr::addr_of_mut!((*pointer).receipt_copied).write(false);
        ptr::addr_of_mut!((*pointer).rebind_complete).write(false);
        ptr::addr_of_mut!((*pointer).successor).write([0; startup_size]);
        ptr::addr_of_mut!((*pointer).service_checks).write(0);
        ptr::addr_of_mut!((*pointer).release_failures).write(0);
        ptr::addr_of_mut!((*pointer).release_retries).write(0);
        ptr::addr_of_mut!((*pointer).prepare_failures).write(0);
        ptr::addr_of_mut!((*pointer).prepare_rollbacks).write(0);
        ptr::addr_of_mut!((*pointer).publication_failures).write(0);
        ptr::addr_of_mut!((*pointer).publication_rollbacks).write(0);
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
    if scenario >= 3 || storage_busy.swap(true, Ordering::AcqRel) {
        fail("runtime_storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    let arena_bases = if scenario.is_multiple_of(2) {
        [0x804000, 0x814000]
    } else {
        [0x814000, 0x804000]
    };
    // Each launch receives distinct task-local route values and a distinct
    // document-service identity. Neither copied value grants authority.
    let handles = core::array::from_fn(|index| 0x31 + index as u64);
    let document = 0xd101 + scenario as u64;
    let seed = b"const before = 1;";
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
        #[cfg(feature = "elf_relaunch_test")]
        let data = [image.data; 2];
        #[cfg(not(feature = "elf_relaunch_test"))]
        let data = [(0, 0); 2];
        execution
            .prepare_native_arenas(
                allocator,
                circles,
                [(image.source, image.length); 2],
                data,
                arena_bases,
            )
            .unwrap_or_else(|e| fail(e));
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 701 + index as u64)
                .unwrap_or_else(|e| fail(e))
        });
        // Tasks remain unadmitted and no active runtime is published. Their
        // actual route identity can now populate immutable launch information.
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        let authority = client_identity::try_new(
            identities[0].task().slot() as u64 + 1,
            identities[0].task().generation_value(),
        )
        .unwrap_or_else(|_| fail("document_client_identity"));
        let mut zeroed_bytes = 0;
        for (index, arena_base) in arena_bases.into_iter().enumerate() {
            if execution.arena_extent(index) != Some((arena_base, arena_byte_count)) {
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
                [arena_base, arena_byte_count],
                handles,
                document,
                authority,
                if index == 1 { seed.as_slice() } else { &[] },
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
        for index in 0..channel_count {
            let producer = index % 2;
            let binding = application_channel_binding::try_new(
                endpoint_id::try_from_parts(index, 201 + scenario as u64).unwrap(),
                identities[producer],
                identities[1 - producer],
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

#[derive(Eq, PartialEq)]
struct service_snapshot {
    identity: task_identity,
    root: u64,
    stack: u64,
    frame: [u64; 22],
    cpu: [u8; 512],
    regions: [u64; 4],
}

fn snapshot_service(state: &runtime) -> service_snapshot {
    if !state.cpu.maintenance_is_ready() {
        fail("relaunch_maintenance_cpu");
    }
    let frame = state.execution.frame(1).unwrap_or_else(|e| fail(e));
    if !state.execution.frame_owned(1, frame) {
        fail("relaunch_service_frame");
    }
    service_snapshot {
        identity: state.identities[1],
        root: state.execution.root(1).unwrap_or_else(|e| fail(e)),
        stack: state
            .execution
            .transition_stack_top(1)
            .unwrap_or_else(|e| fail(e)),
        // SAFETY: Complete retained C-layout frame has exactly22 u64 fields.
        frame: unsafe { ptr::read(frame.cast::<[u64; 22]>()) },
        cpu: *state
            .cpu
            .task_image(1)
            .unwrap_or_else(|| fail("relaunch_service_cpu"))
            .bytes(),
        regions: state
            .execution
            .native_service_hashes(&state.tasks, &state.scheduler)
            .unwrap_or_else(|| fail("relaunch_service_regions")),
    }
}

fn service_unchanged(state: &mut runtime, before: &service_snapshot) {
    if snapshot_service(state) != *before {
        fail("relaunch_service_changed");
    }
    state.service_checks += 1;
}

fn require_owner_count(allocator: &frame_allocator, initial: frame_accounting, owners: usize) {
    let a = allocator.accounting();
    if a.free.checked_add(33 * owners) != Some(initial.free)
        || a.data_owned != 24 * owners
        || a.table_owned != 9 * owners
        || a.active_leases != 6 * owners
        || a.bound_leases != 6 * owners
    {
        fail("relaunch_owner_accounting");
    }
}

fn publication_preflight(
    state: &runtime,
    editor: task_identity,
    accept: bool,
) -> Result<(), &'static str> {
    if !state
        .tasks
        .snapshot(editor)
        .is_ok_and(|s| s.state() == task_state::ready && !s.context_attached())
        || !state.scheduler.snapshot(&state.tasks).is_ok_and(|s| {
            s.current() == Some(state.identities[1]) && s.admitted() == 1 && s.runnable() == 0
        })
        || !state.cpu.maintenance_is_ready()
        || state
            .channels
            .iter()
            .any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        return Err("relaunch_publication_state");
    }
    if !accept {
        return Err("relaunch_publication_probe");
    }
    Ok(())
}

#[inline(never)]
fn replace_channels(
    state: &mut runtime,
    old_bindings: [application_channel_binding; channel_count],
) {
    for (index, old_binding) in old_bindings.into_iter().enumerate() {
        let producer = index % 2;
        let binding = application_channel_binding::try_new(
            endpoint_id::try_from_parts(index, 202 + state.scenario as u64).unwrap(),
            state.identities[producer],
            state.identities[1 - producer],
        )
        .unwrap_or_else(|_| fail("relaunch_channel_binding"));
        state.channels[index] =
            application_channel::try_new(binding).unwrap_or_else(|_| fail("relaunch_channel"));
        if state.channels[index]
            .prepare_submit(old_binding, old_binding.producer(), 1, startup_size)
            .is_ok()
            || state.channels[index].binding() == old_binding
            || state.channels[index].phase() != channel_phase::empty
            || !state.channels[index].storage_is_clear()
        {
            fail("relaunch_old_route_authority");
        }
    }
}

#[inline(never)]
fn replace_editor(state: &mut runtime, allocator: &mut frame_allocator, initial: frame_accounting) {
    if state.selected != 1
        || !state.checkpointed
        || state.incarnation != 0
        || state.terminal != [true, false]
        || dispatch_active.load(Ordering::Acquire)
        || is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != state.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
    {
        fail("relaunch_checkpoint_continuation");
    }
    require_owner_count(allocator, initial, 2);
    let before = snapshot_service(state);
    let old = state.identities[0];
    let old_bindings = core::array::from_fn::<_, channel_count, _>(|i| state.channels[i].binding());
    state
        .scheduler
        .withdraw_terminal_context(&mut state.circles, &mut state.tasks, old)
        .unwrap_or_else(|_| fail("relaunch_editor_withdraw"));
    // First clear is deliberately rejected after editor mappings/task are gone;
    // its unbound transition-stack lease remains owned for the exact retry.
    let failure = unsafe {
        state.execution.native_release_editor(
            allocator,
            &mut state.tasks,
            &mut state.circles,
            &state.scheduler,
            Some(old),
            true,
        )
    };
    if failure != Err("native_editor_stack_clear") {
        fail("relaunch_release_probe");
    }
    state.release_failures += 1;
    let retained = allocator.accounting();
    if retained.free.checked_add(37) != Some(initial.free)
        || retained.data_owned != 28
        || retained.table_owned != 9
        || retained.active_leases != 7
        || retained.bound_leases != 6
    {
        fail("relaunch_release_retained");
    }
    service_unchanged(state, &before);
    unsafe {
        state.execution.native_release_editor(
            allocator,
            &mut state.tasks,
            &mut state.circles,
            &state.scheduler,
            Some(old),
            false,
        )
    }
    .unwrap_or_else(|e| fail(e));
    state.release_retries += 1;
    require_owner_count(allocator, initial, 1);
    if state.tasks.validate(old).is_ok()
        || state.circles.snapshot(old.circle()).is_ok()
        || !unsafe { state.cpu.reset_native_editor() }
    {
        fail("relaunch_old_authority");
    }
    service_unchanged(state, &before);
    let payload = (state.image.source, state.image.length);
    let arena_base = state.arena_bases[0];
    let failure = unsafe {
        state.execution.native_prepare_editor(
            allocator,
            &mut state.circles,
            &state.tasks,
            &state.scheduler,
            #[cfg(feature = "elf_relaunch_test")]
            (payload, state.image.data, arena_base),
            #[cfg(not(feature = "elf_relaunch_test"))]
            (payload, arena_base),
            true,
        )
    };
    if failure != Err("native_editor_prepare_population") {
        fail("relaunch_prepare_probe");
    }
    state.prepare_failures += 1;
    let retained = allocator.accounting();
    if retained.free.checked_add(62) != Some(initial.free)
        || retained.data_owned != 44
        || retained.table_owned != 18
        || retained.active_leases != 11
        || retained.bound_leases != 11
        || state.tasks.active_count() != 1
        || state.tasks.attached_context_count() != 1
    {
        fail("relaunch_prepare_retained");
    }
    service_unchanged(state, &before);
    unsafe {
        state.execution.native_release_editor(
            allocator,
            &mut state.tasks,
            &mut state.circles,
            &state.scheduler,
            None,
            false,
        )
    }
    .unwrap_or_else(|e| fail(e));
    state.prepare_rollbacks += 1;
    require_owner_count(allocator, initial, 1);
    service_unchanged(state, &before);
    unsafe {
        state.execution.native_prepare_editor(
            allocator,
            &mut state.circles,
            &state.tasks,
            &state.scheduler,
            #[cfg(feature = "elf_relaunch_test")]
            (payload, state.image.data, arena_base),
            #[cfg(not(feature = "elf_relaunch_test"))]
            (payload, arena_base),
            false,
        )
    }
    .unwrap_or_else(|e| fail(e));
    let cancelled = state
        .execution
        .native_commit_editor(&mut state.tasks, &mut state.circles, &state.scheduler, 701)
        .unwrap_or_else(|e| fail(e));
    if publication_preflight(state, cancelled, false) != Err("relaunch_publication_probe") {
        fail("relaunch_publication_rejection");
    }
    state.publication_failures += 1;
    require_owner_count(allocator, initial, 2);
    service_unchanged(state, &before);
    unsafe {
        state.execution.native_release_editor(
            allocator,
            &mut state.tasks,
            &mut state.circles,
            &state.scheduler,
            Some(cancelled),
            false,
        )
    }
    .unwrap_or_else(|e| fail(e));
    state.publication_rollbacks += 1;
    require_owner_count(allocator, initial, 1);
    if state.tasks.validate(cancelled).is_ok() || state.circles.snapshot(cancelled.circle()).is_ok()
    {
        fail("relaunch_cancelled_authority");
    }
    service_unchanged(state, &before);
    unsafe {
        state.execution.native_prepare_editor(
            allocator,
            &mut state.circles,
            &state.tasks,
            &state.scheduler,
            #[cfg(feature = "elf_relaunch_test")]
            (payload, state.image.data, arena_base),
            #[cfg(not(feature = "elf_relaunch_test"))]
            (payload, arena_base),
            false,
        )
    }
    .unwrap_or_else(|e| fail(e));
    let editor = state
        .execution
        .native_commit_editor(&mut state.tasks, &mut state.circles, &state.scheduler, 701)
        .unwrap_or_else(|e| fail(e));
    publication_preflight(state, editor, true).unwrap_or_else(|e| fail(e));
    if editor.task().slot() != old.task().slot()
        || editor.task().generation_value() <= cancelled.task().generation_value()
        || cancelled.task().generation_value() <= old.task().generation_value()
        || editor.circle() == old.circle()
        || editor.circle() == cancelled.circle()
        || state.tasks.validate(old).is_ok()
        || state.tasks.validate(cancelled).is_ok()
    {
        fail("relaunch_editor_generation");
    }
    for offset in (0..8192).step_by(8) {
        if state.execution.read_data(0, offset) != Some(0)
            || state.execution.read_stack(0, offset) != Some(0)
        {
            fail("relaunch_editor_zeroed");
        }
    }
    for offset in (0..arena_byte_count).step_by(8) {
        if state.execution.read_arena(0, offset) != Some(0) {
            fail("relaunch_editor_arena_zeroed");
        }
        state.zeroed_bytes += 8;
    }
    state.identities[0] = editor;
    state.handles = state.handles.map(|handle| handle + 0x10);
    let authority = client_identity::try_new(
        editor.task().slot() as u64 + 1,
        editor.task().generation_value(),
    )
    .unwrap_or_else(|_| fail("relaunch_editor_authority"));
    let descriptor = startup_descriptor::try_new(
        0,
        state.scenario as u64,
        [arena_base, arena_byte_count],
        state.handles,
        0xd101 + state.scenario as u64,
        authority,
        &[],
    )
    .unwrap_or_else(|_| fail("relaunch_editor_descriptor"));
    state.successor = descriptor.encode();
    let mut readback = [0; startup_size];
    for (word, bytes) in state.successor.chunks_exact(8).enumerate() {
        let offset = descriptor_offset + word as u64 * 8;
        state
            .execution
            .write_data(0, offset, u64::from_le_bytes(bytes.try_into().unwrap()))
            .unwrap_or_else(|e| fail(e));
        readback[word * 8..word * 8 + 8].copy_from_slice(
            &state
                .execution
                .read_data(0, offset)
                .unwrap_or_else(|| fail("relaunch_descriptor_readback"))
                .to_le_bytes(),
        );
    }
    if readback != state.successor || startup_descriptor::decode(&readback) != Ok(descriptor) {
        fail("relaunch_descriptor_copy");
    }
    let frame = state.execution.frame(0).unwrap_or_else(|e| fail(e));
    if !state.execution.frame_owned(0, frame) || unsafe { (*frame).flags } != 2 {
        fail("relaunch_new_frame");
    }
    unsafe {
        (*frame).destination_index = descriptor_address;
        (*frame).source_index = startup_size as u64;
    }
    state.started[0] = false;
    state.terminal[0] = false;
    state.incarnation = 1;
    state.last_ack = [0; channel_count];
    service_unchanged(state, &before);
    unsafe {
        state
            .execution
            .native_activate_editor(allocator, &state.tasks, &state.scheduler, editor)
    }
    .unwrap_or_else(|e| fail(e));
    replace_channels(state, old_bindings);
    state
        .scheduler
        .admit(&mut state.circles, &mut state.tasks, editor)
        .unwrap_or_else(|_| fail("relaunch_editor_admit"));
    require_owner_count(allocator, initial, 2);
    service_unchanged(state, &before);
    if state.service_checks != 8 || state.zeroed_bytes != 3 * arena_byte_count {
        fail("relaunch_maintenance_evidence");
    }
    state
        .execution
        .native_maintenance_resume(&state.tasks, &state.scheduler, editor)
        .unwrap_or_else(|e| fail(e));
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
    let mut frame = frame;
    let status = loop {
        // SAFETY: Published stable owner is borrowed exclusively by handlers;
        // each continuation abandons its handler stack before returning here.
        let status = unsafe {
            if !(*pointer).cpu.restore_task((*pointer).selected) {
                fail("runtime_initial_cpu");
            }
            x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
        };
        if status != checkpoint_status {
            break status;
        }
        replace_editor(state, allocator, initial);
        frame = state.select(1);
        if active
            .compare_exchange(
                ptr::null_mut(),
                pointer,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            fail("relaunch_republication");
        }
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
    let expected_sends = match scenario {
        0 => [8, 7, 11, 11],
        1 => [8, 7, 12, 11],
        2 => [8, 8, 12, 12],
        _ => fail("relaunch_scenario"),
    };
    let mut delivered = expected_sends;
    if scenario == 1 {
        delivered[2] -= 1;
    }
    if scenario == 2 {
        delivered[3] -= 1;
    }
    if e.calls != [292, 293, 303][scenario]
        || e.switches != [47, 47, 49][scenario]
        || e.denials != 8
        || e.bad_copies != 1
        || e.invalid != 9
        || e.closed != 2
        || e.sends != expected_sends
        || e.deliveries != delivered
        || e.acknowledgements != delivered
        || e.rows != stage_height as u64
        || e.exits != [3, 2, 2][scenario]
        || e.faults != u64::from(scenario != 0)
        || e.discarded != u64::from(scenario != 0)
        || state.arena_copies != [99, 100, 104][scenario]
        || state.starts != 3
        || state.zeroed_bytes != 3 * arena_byte_count
        || !state.checkpointed
        || !state.receipt_copied
        || !state.rebind_complete
        || state.incarnation != 1
        || state.service_checks != 8
        || [
            state.release_failures,
            state.release_retries,
            state.prepare_failures,
            state.prepare_rollbacks,
            state.publication_failures,
            state.publication_rollbacks,
        ] != [1; 6]
    {
        fail("relaunch_evidence");
    }
    for identity in state.identities {
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|s| s.state() == task_state::exited)
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
        starts: state.starts,
        service_checks: state.service_checks,
        release_failures: state.release_failures,
        release_retries: state.release_retries,
        prepare_failures: state.prepare_failures,
        prepare_rollbacks: state.prepare_rollbacks,
        publication_failures: state.publication_failures,
        publication_rollbacks: state.publication_rollbacks,
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
    // The distinct artifact markers bound the before/after userspace commit
    // probes. Document policy and RAM-store bytes never enter the kernel.
    // SAFETY: Exclusive handler owns the interrupted editor's eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || !state.failure_armed
        || state.selected != 0
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.fault_address.is_some()
        || snapshot.flags & 0x200 != 0
        || !state.fault_boundary_valid()
    {
        fail("document_unexpected_fault");
    }
    let expected = match state.scenario {
        1 => state.image.before,
        2 => state.image.after,
        _ => fail("document_fault_scenario"),
    };
    if snapshot.instruction_pointer != expected {
        fail("document_fault_site");
    }
    state.close_channels();
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.identities[0])
        .unwrap_or_else(|_| fail("document_fault_transition"));
    if transition.dispatch().task() != state.identities[1] {
        fail("document_fault_peer");
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
            state.starts += 1;
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
            if state.incarnation == 1 && !state.rebind_complete {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else if !state.terminal[1 - state.selected] {
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
            if state.incarnation == 1 && !state.rebind_complete {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else {
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
        }
        7 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.incarnation != 1
                || !state.rebind_complete
                || !state.terminal[0]
                || state.evidence.sends[0] == 0
                || state.last_ack != state.evidence.route_sends[1]
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
            let marker = match state.scenario {
                1 => state.image.before,
                2 => state.image.after,
                _ => fail("document_fault_scenario"),
            };
            if state.selected != 0
                || state.failure_armed
                || value.destination_index != state.scenario as u64
                || value.source_index != 0
                || value.instruction_pointer != marker
                || !state.fault_boundary_valid()
            {
                fail("document_fault_arm");
            }
            state.failure_armed = true;
            // SAFETY: IF is disabled only on this exact editor UD2 return.
            unsafe {
                (*frame).flags = value.flags & !0x200;
            }
        }
        9 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.checkpointed
                || !state.terminal[0]
                || state.incarnation != 0
                || [
                    value.destination_index,
                    value.source_index,
                    value.data,
                    value.counter,
                ] != [0; 4]
                || state.channels.iter().any(|channel| {
                    channel.phase() != channel_phase::closed || !channel.storage_is_clear()
                })
            {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else {
                // The successful service return is fixed before taking any
                // preservation snapshot. No lifecycle bytes enter its RAM yet.
                unsafe {
                    (*frame).accumulator = 0;
                    (*frame).counter = 0;
                    (*frame).data = 0;
                }
                state.checkpointed = true;
                if !state.cpu.maintenance_is_ready() {
                    fail("relaunch_checkpoint_cpu");
                }
                unsafe {
                    state.execution.native_maintenance_bootstrap(
                        &state.tasks,
                        &state.scheduler,
                        state.identities[0],
                        state.identities[1],
                    )
                }
                .unwrap_or_else(|e| fail(e));
                active.store(ptr::null_mut(), Ordering::Release);
                dispatch_active.store(false, Ordering::Release);
                // SAFETY: Exact suspended enter continuation, saved service CPU,
                // clean kernel image and independent bootstrap root/TSS retained.
                unsafe {
                    application_cpu::x86_64_application_checkpoint_kernel(
                        checkpoint_status,
                        state.kernel_stack,
                    )
                }
            }
        }
        10 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.incarnation != 1
                || state.rebind_complete
                || value.destination_index != 0
                || value.counter != 0
                || value.data != startup_size as u64
            {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else if !state.arena_contains(1, value.source_index, startup_size)
                || state
                    .execution
                    .copy_to_user(
                        &state.tasks,
                        &state.circles,
                        &state.scheduler,
                        state.identities[1],
                        value.source_index,
                        &state.successor,
                    )
                    .is_err()
            {
                reply.0 = 3;
                state.evidence.bad_copies += 1;
            } else {
                state.receipt_copied = true;
                state.arena_copies += 1;
                reply = (
                    0,
                    state.identities[0].task().generation_value(),
                    startup_size as u64,
                );
            }
        }
        11 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.incarnation != 1
                || !state.receipt_copied
                || state.rebind_complete
                || value.destination_index != 0
                || value.source_index != 0
                || value.data != 0
                || value.counter != state.identities[0].task().generation_value()
            {
                reply.0 = 4;
                state.evidence.invalid += 1;
            } else {
                state.rebind_complete = true;
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

#[cfg(not(feature = "elf_relaunch_test"))]
#[inline(never)]
fn emit_record(scenario: usize, record: &runtime_record) {
    let mut serial = serial_port::com1();
    let e = &record.application;
    let fault_kind = match scenario {
        1 => "before_commit",
        2 => "after_commit",
        _ => "none",
    };
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_relaunch_test.maintenance","level":"info","scenario":{},"frames_before":66,"frames_survivor":33,"frames_after":66,"leases_before":12,"leases_survivor":6,"leases_after":12,"release_failures":{},"release_retries":{},"prepare_failures":{},"prepare_rollbacks":{},"publication_failures":{},"publication_rollbacks":{},"service_checks":{},"zeroed_bytes":32768,"old_identity_revoked":true,"cancelled_identity_revoked":true,"service_preserved":true,"route_authority_replaced":true,"editor_generation_advanced":true}}"#,
        scenario,
        record.release_failures,
        record.release_retries,
        record.prepare_failures,
        record.prepare_rollbacks,
        record.publication_failures,
        record.publication_rollbacks,
        record.service_checks
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_relaunch_test.scenario","level":"info","scenario":{},"starts":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"cooperative_switches":{},"surfaces":{},"inputs":{},"requests":{},"replies":{},"surface_deliveries":{},"input_deliveries":{},"request_deliveries":{},"reply_deliveries":{},"surface_acks":{},"input_acks":{},"request_acks":{},"reply_acks":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"rows":{},"exits":{},"faults":{},"fault_kind":"{}","arena_copies":{},"resources_restored":true}}"#,
        scenario,
        record.starts,
        record.zeroed_bytes,
        e.calls,
        e.timer_ticks,
        e.switches,
        e.sends[0],
        e.sends[1],
        e.sends[2],
        e.sends[3],
        e.deliveries[0],
        e.deliveries[1],
        e.deliveries[2],
        e.deliveries[3],
        e.acknowledgements[0],
        e.acknowledgements[1],
        e.acknowledgements[2],
        e.acknowledgements[3],
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

    for incarnation in 0..2 {
        for (index, hash) in e.packet_hashes[incarnation][0]
            [..e.route_sends[incarnation][0] as usize]
            .iter()
            .enumerate()
        {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.native_relaunch_test.surface","level":"info","scenario":{},"incarnation":{},"sequence":{},"hash":{}}}"#,
                scenario,
                incarnation,
                index + 1,
                hash
            );
        }
        for channel in 2..channel_count {
            for (index, hash) in e.packet_hashes[incarnation][channel]
                [..e.route_sends[incarnation][channel] as usize]
                .iter()
                .enumerate()
            {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_relaunch_test.packet","level":"info","scenario":{},"incarnation":{},"channel":{},"sequence":{},"hash":{}}}"#,
                    scenario,
                    incarnation,
                    channel,
                    index + 1,
                    hash
                );
            }
        }
    }
}

#[cfg(feature = "elf_relaunch_test")]
pub(super) fn run() -> ! {
    elf_relaunch_profile::run()
}

#[cfg(not(feature = "elf_relaunch_test"))]
pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("relaunch_initial_state");
    }
    let baseline_image = artifact::inspect();
    let image = runtime_image::inspect();
    // SAFETY: Sole bootstrap owner withdraws and retains the unique aperture.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (vendor, normal) = run_owned(allocator, baseline_image, kernel_cr3, bootstrap_stack, 0, initial);
        let patch = presenter.prepare_rgb8_region(32, 32, stage_width, stage_height,
            unsafe { &*display_stage.0.get() }).unwrap_or_else(|_| fail("relaunch_baseline_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
            fail("relaunch_baseline_pixels");
        }
        clear_stage();
        let presented = framebuffer_hash(info, false);
        let (fault_vendor, faulted) = run_owned(allocator, baseline_image, kernel_cr3, bootstrap_stack, 1, initial);
        if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
            fail("relaunch_baseline_fault");
        }
        clear_stage();
        emit_evidence(initial, vendor, [normal, faulted]);
        let mut serial = serial_port::com1();
        let _ = writeln!(serial,
            r#"{{"schema":1,"event":"kernel.native_relaunch_test.ready","level":"info","tasks":2,"scenarios":3,"channels":4,"descriptor_bytes":128,"arena_bytes":32768,"mappings_per_task":4,"data_frames":48,"table_frames":18,"leases":12}}"#);
        let mut zeroed_bytes = 0;
        for scenario in execution_order {
            let (current_vendor, record) = run_arena_pair(allocator, image, scenario, kernel_cr3, bootstrap_stack, initial);
            if current_vendor != vendor {
                fail("relaunch_cpu_comparison");
            }
            let patch = presenter.prepare_rgb8_region(32, 32, stage_width, stage_height,
                unsafe { &*display_stage.0.get() }).unwrap_or_else(|_| fail("relaunch_patch"));
            if scenario != 0 && !patch.matches_pixels() {
                fail("relaunch_recovered_pixels");
            }
            if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
                fail("relaunch_pixel_readback");
            }
            clear_stage();
            zeroed_bytes += record.zeroed_bytes;
            emit_record(scenario, &record);
        }
        if zeroed_bytes != 294912 || allocator.accounting() != initial {
            fail("relaunch_final_evidence");
        }
        interrupts::disable_external_interrupts();
        if !interrupts::scheduler_interrupts_are_masked() {
            fail("relaunch_final_interrupts");
        }
        let _ = writeln!(serial,
            r#"{{"schema":1,"event":"kernel.native_relaunch_test.passed","level":"info","application_tasks":9,"service_lifetimes":3,"replacements":3,"cancelled_tasks":3,"contained_faults":2,"zeroed_bytes":{},"resources_restored":true,"authority_revoked":true,"source_language":"rust","store_scope":"service_lifetime"}}"#,
            zeroed_bytes);
    }).unwrap_or_else(|e| fail(e.reason()));
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
