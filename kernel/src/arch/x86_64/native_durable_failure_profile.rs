//! Scoped native disk document proof with complete pair destruction.
//! Kernel-owned mappings and per-task handle bindings remain authoritative;
//! mutable startup bytes never create mappings, tasks or channel grants.
use super::super::owned_execution::user_data_base;
use super::*;
use anomalous_document_protocol::{
    arena_byte_count, client_identity,
    durable::disk_identity,
    durable_failure_startup::{startup_descriptor, startup_size},
    durable_startup,
};

use super::super::{native_durable_fixture, native_storage_device};

const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;
const arena_frames: usize = 66;
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
    static x86_64_native_durable_failure_payload_start: u8;
    static x86_64_native_durable_failure_payload_end: u8;
    static x86_64_native_durable_failure_user_fault_instruction: u8;
    static x86_64_native_durable_failure_user_cut_fault_instruction: u8;
}
#[derive(Clone, Copy)]
struct runtime_image {
    source: u64,
    length: u64,
    cut: u64,
}
impl runtime_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_durable_failure_payload_start);
        let length = address!(x86_64_native_durable_failure_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("runtime_payload_extent"));
        let fault = address!(x86_64_native_durable_failure_user_fault_instruction);
        let cut = address!(x86_64_native_durable_failure_user_cut_fault_instruction);
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > code_pages * 4096
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || fault == cut
            || [fault, cut].iter().any(|marker| {
                *marker < user_code_base
                    || marker
                        .checked_add(2)
                        .is_none_or(|end| end > user_code_base + length)
            })
        {
            fail("runtime_artifact_geometry");
        }
        for marker in [fault, cut] {
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
            cut,
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
    packet_hashes: [[u64; packet_limit]; channel_count],
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
    device: *mut native_storage_device::device,
    device_handle: u64,
    arena_bases: [u64; 2],
    handles: [u64; channel_count],
    arena_copies: u64,
    zeroed_bytes: u64,
    selected: usize,
    started: [bool; 2],
    terminal: [bool; 2],
    evidence: document_evidence,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
    phase: usize,
    case: u64,
    image: runtime_image,
    arm_seen: bool,
    armed: bool,
    completed_operations: u64,
    io_errors: u64,
    arm_counters: native_storage_device::io_counters,
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
            && (!self.started[self.selected]
                || if self.armed && self.selected == 1 && self.case != 6 {
                    value.flags & 0x202 == 2
                } else {
                    value.flags & 0x202 == 0x202
                })
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
                if !(1..=packet_limit as u64).contains(&receipt.sequence()) {
                    fail("document_packet_sequence_bound");
                }
                self.evidence.packet_hashes[index][receipt.sequence() as usize - 1] = hash;
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
                            || self.evidence.packet_hashes[index][receipt.sequence() as usize - 1]
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
    fn storage_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        // frame_valid already binds the current task, root and retained context.
        // Only this phase's exact service owner possesses the sealed grant.
        if self.selected != 1 || self.terminal[1] || value.destination_index != self.device_handle {
            return (2, 0, 0);
        }
        let flush = value.accumulator == 14;
        if (flush && (value.source_index != 0 || value.data != 0 || value.counter != 0))
            || (!flush && (value.data != 512 || value.counter >= 5))
        {
            return (4, 0, 0);
        }
        let identity = self.identities[1];
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
    fn fault_case(&self) -> bool {
        self.phase == 0 && self.case != 6
    }
    fn fault_boundary_valid(&self) -> bool {
        self.evidence.sends == [3, 0, 5, 5]
            && self.evidence.deliveries == self.evidence.sends
            && self.evidence.acknowledgements == [3, 0, 4, 5]
            && self.last_ack == self.evidence.acknowledgements
            && self.channels.iter().enumerate().all(|(index, channel)| {
                channel.phase()
                    == if index == 2 {
                        channel_phase::delivered
                    } else {
                        channel_phase::empty
                    }
            })
    }
    fn survivor_grant(&self) -> bool {
        self.fault_case()
            && self.selected == 0
            && self.terminal[1]
            && self.evidence.faults == 1
            && self.device_handle == 0
            && self
                .channels
                .iter()
                .all(|c| c.phase() == channel_phase::closed && c.storage_is_clear())
    }
    fn operation_boundary_valid(&self) -> bool {
        // SAFETY: The unique controller remains retained by the suspended
        // coordinator; this handler is serialized with IF clear.
        let now = unsafe { (&*self.device).counters() };
        let writes = self.completed_operations.div_ceil(2);
        let flushes = self.completed_operations / 2;
        let failed = u64::from(self.case == 7);
        now.commands_issued
            == self.arm_counters.commands_issued + self.completed_operations + failed
            && now.commands_completed
                == self.arm_counters.commands_completed + self.completed_operations
            && now.sectors_read == self.arm_counters.sectors_read
            && now.sectors_written == self.arm_counters.sectors_written + writes
            && now.flushes == self.arm_counters.flushes + flushes
    }
    fn arena_contains(&self, index: usize, address: u64, length: usize) -> bool {
        address >= self.arena_bases[index]
            && address
                .checked_add(length as u64)
                .is_some_and(|end| end <= self.arena_bases[index] + arena_byte_count)
    }
}

fn clear_sector_scratch(bytes: &mut [u8; 512]) {
    for byte in bytes {
        // SAFETY: Private live kernel scratch; clearing must survive removal
        // of dead stores before this gate returns to an untrusted owner.
        unsafe { ptr::write_volatile(byte, 0) };
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct runtime_record {
    application: document_evidence,
    arena_copies: u64,
    zeroed_bytes: u64,
    starts: u64,
    arm_seen: bool,
    completed_operations: u64,
    io_errors: u64,
}

// Return constructor temporaries before any mapping preparation or activation.
#[inline(never)]
unsafe fn initialize_storage(
    pointer: *mut runtime,
    device: *mut native_storage_device::device,
    scenario: (usize, u64, runtime_image),
    kernel_cr3: u64,
    bootstrap_stack: u64,
    arena_bases: [u64; 2],
    handles: [u64; channel_count],
) {
    // SAFETY: Caller owns the sole storage latch over uninitialized or fully
    // retired BSS. No whole-runtime reference exists until identities/channels
    // are written after retained preparation. No live owner is moved.
    let (phase, case, image) = scenario;
    unsafe {
        ptr::addr_of_mut!((*pointer).phase).write(phase);
        ptr::addr_of_mut!((*pointer).case).write(case);
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).arm_seen).write(false);
        ptr::addr_of_mut!((*pointer).armed).write(false);
        ptr::addr_of_mut!((*pointer).completed_operations).write(0);
        ptr::addr_of_mut!((*pointer).io_errors).write(0);
        ptr::addr_of_mut!((*pointer).arm_counters)
            .write(native_storage_device::io_counters::default());
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new_native_arenas());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(2).unwrap_or_else(|_| fail("runtime_scheduler")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session::new());
        ptr::addr_of_mut!((*pointer).last_ack).write([0; channel_count]);
        ptr::addr_of_mut!((*pointer).device).write(device);
        ptr::addr_of_mut!((*pointer).device_handle).write(0x7001 + phase as u64 * 0x100);
        ptr::addr_of_mut!((*pointer).arena_bases).write(arena_bases);
        ptr::addr_of_mut!((*pointer).handles).write(handles);
        ptr::addr_of_mut!((*pointer).arena_copies).write(0);
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(0);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([false; 2]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; 2]);
        ptr::addr_of_mut!((*pointer).evidence).write(document_evidence::default());
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
    }
}

#[inline(never)]
fn initialize(
    device: *mut native_storage_device::device,
    input: (u64, &[u8]),
    allocator: &mut frame_allocator,
    image: runtime_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut runtime {
    let (case, challenge) = input;
    if !matches!(scenario, 0 | 2)
        || !(1..=7).contains(&case)
        || storage_busy.swap(true, Ordering::AcqRel)
    {
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
    let handles = core::array::from_fn(|index| 0x301 + scenario as u64 * 0x100 + index as u64);
    // SAFETY: Exclusive BSS latch, IF clear and NMI masked. Initialize each
    // field before borrowing it; whole runtime remains unborrowed until complete.
    unsafe {
        initialize_storage(
            pointer,
            device,
            (scenario, case, image),
            kernel_cr3,
            bootstrap_stack,
            arena_bases,
            handles,
        );
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
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
            let base = durable_startup::startup_descriptor::try_new(
                index as u64,
                scenario as u64,
                [arena_base, arena_byte_count],
                handles,
                authority,
                disk_identity::try_new(0xa110, 0xd001).unwrap(),
                if index == 1 {
                    0x7001 + scenario as u64 * 0x100
                } else {
                    0
                },
                if index == 0 { challenge } else { &[] },
            )
            .unwrap_or_else(|_| fail("runtime_descriptor"));
            let descriptor = startup_descriptor::try_new(base, case)
                .unwrap_or_else(|_| fail("runtime_failure_descriptor"));
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

#[inline(never)]
fn run_arena_pair(
    device: &mut native_storage_device::device,
    input: (usize, u64, &[u8]),
    allocator: &mut frame_allocator,
    image: runtime_image,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> (&'static str, runtime_record) {
    let (scenario, case, challenge) = input;
    if super::is_active()
        || is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || !stage_is_clear()
        || !device.grant_available()
    {
        fail("runtime_setup");
    }
    let state = initialize(
        ptr::from_mut(device),
        (case, challenge),
        allocator,
        image,
        scenario,
        kernel_cr3,
        bootstrap_stack,
    );
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
        || state.terminal != [true; 2] || state.started != [true; 2]
        || state.channels.iter().any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        fail("runtime_continuation");
    }
    let e = state.evidence;
    let fault_case = state.fault_case();
    let expected_operations = if scenario == 2 {
        0
    } else if case == 7 {
        3
    } else if case == 6 {
        4
    } else {
        case - 1
    };
    if e.sends != e.deliveries
        || (fault_case && (e.sends != [3, 0, 5, 5] || e.acknowledgements != [3, 0, 4, 5]))
        || (!fault_case && e.sends != e.acknowledgements)
        || e.rows != stage_height as u64
        || e.exits != if fault_case { 1 } else { 2 }
        || e.faults != u64::from(fault_case)
        || e.discarded != u64::from(fault_case)
        || e.closed != if fault_case { 2 } else { 0 }
        || state.arm_seen != (scenario == 0)
        || state.armed
        || state.completed_operations != expected_operations
        || state.io_errors != if scenario == 0 && case == 7 { 4 } else { 0 }
        || device.is_poisoned() != (scenario == 0 && case == 7)
    {
        fail("durable_completion_evidence");
    }
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if fault_case && index == 1 {
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
        arm_seen: state.arm_seen,
        completed_operations: state.completed_operations,
        io_errors: state.io_errors,
    };
    let vendor = state.cpu.cpu_vendor();
    state.device = ptr::null_mut();
    state.device_handle = 0;
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
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.evidence.timer_ticks >= max_calls
        || (state.armed && state.case != 6)
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
    // SAFETY: Exclusive handler saves the exact interrupted service CPU owner.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || !state.fault_case()
        || !state.armed
        || state.selected != 1
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.fault_address.is_some()
        || snapshot.flags & 0x200 != 0
        || snapshot.instruction_pointer != state.image.cut
        || !state.fault_boundary_valid()
        || state.completed_operations != if state.case == 7 { 3 } else { state.case - 1 }
        || state.io_errors != if state.case == 7 { 4 } else { 0 }
        || !state.operation_boundary_valid()
    {
        fail("durable_unexpected_user_fault");
    }
    state.device_handle = 0;
    state.close_channels();
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.identities[1])
        .unwrap_or_else(|_| fail("durable_fault_transition"));
    if transition.dispatch().task() != state.identities[0] {
        fail("durable_fault_peer");
    }
    state.terminal[1] = true;
    state.evidence.faults += 1;
    state.armed = false;
    let selected = state.select(0);
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
    if state.armed && !matches!(value.accumulator, 12..=14) {
        fail("durable_armed_gate");
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
            if !state.survivor_grant()
                && (state.last_ack != state.evidence.sends
                    || state
                        .channels
                        .iter()
                        .any(|channel| channel.phase() != channel_phase::empty)
                    || (index == 1 && !state.terminal[0]))
            {
                fail("durable_terminal_pending_work");
            }
            state.terminal[index] = true;
            state.evidence.exits += 1;
            if state.terminal[1 - index] {
                if !state.survivor_grant() {
                    state.close_channels();
                }
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
            let survivor = state.survivor_grant();
            if state.selected != 1 && !survivor {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if (!survivor
                && (!state.terminal[0]
                    || state.evidence.sends[0] == 0
                    || state.last_ack != state.evidence.sends))
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
                        state.identities[state.selected],
                        value.source_index,
                        &mut row,
                    )
                    .is_err()
                {
                    reply.0 = 3;
                    state.evidence.bad_copies += 1;
                } else {
                    if !state.arena_contains(state.selected, value.source_index, row.len()) {
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
            if state.selected != 1
                || state.phase != 0
                || state.arm_seen
                || value.destination_index != state.case
                || value.source_index != 0
                || value.data != 0
                || value.counter != 0
                || !state.fault_boundary_valid()
            {
                fail("durable_arm_boundary");
            }
            // SAFETY: Sole retained owner, IF clear and no concurrent transport.
            let device = unsafe { &mut *state.device };
            if !device.grant_available() {
                fail("durable_arm_owner");
            }
            state.arm_counters = device.counters();
            if state.case == 7 {
                device
                    .arm_flush_observation_loss()
                    .unwrap_or_else(|_| fail("durable_arm_injection"));
            }
            state.arm_seen = true;
            state.armed = true;
            if state.case != 6 {
                // SAFETY: The exact inspected service return frame is retained.
                // Bounded storage calls lead directly to its distinct UD2 marker.
                unsafe {
                    (*frame).flags &= !0x200;
                }
            }
        }
        12..=14 => {
            // Device owner survives pair retirement. The sole BSP cannot access
            // it through the coordinator while this suspended entry is active.
            let before = unsafe { (&*state.device).counters() };
            if state.armed {
                let expected = if state.io_errors == 0 {
                    if state.completed_operations.is_multiple_of(2) {
                        13
                    } else {
                        14
                    }
                } else {
                    11 + state.io_errors
                };
                if value.accumulator != expected || state.io_errors > 3 {
                    fail("durable_save_operation_order");
                }
            }
            reply = state.storage_call(value);
            if state.armed {
                if reply.0 == 0 && state.io_errors == 0 && state.completed_operations < 4 {
                    state.completed_operations += 1;
                    if state.case == 6 && state.completed_operations == 4 {
                        if !state.operation_boundary_valid() {
                            fail("durable_saved_boundary");
                        }
                        state.armed = false;
                    }
                } else if reply.0 == 6 && state.case == 7 && state.completed_operations == 3 {
                    state.io_errors += 1;
                    // After the first loss, all probes must reject without I/O.
                    if state.io_errors > 1 && unsafe { (&*state.device).counters() } != before {
                        fail("durable_poison_performed_io");
                    }
                } else {
                    fail("durable_save_operation_result");
                }
            }
            match reply.0 {
                2 => state.evidence.denials += 1,
                3 => state.evidence.bad_copies += 1,
                4 => state.evidence.invalid += 1,
                _ => {}
            }
            if matches!(reply.0, 2..=4) && unsafe { (&*state.device).counters() } != before {
                fail("durable_rejection_performed_io");
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
        fail("durable_initial_state");
    }
    // SAFETY: This proof exclusively owns boot-time fw_cfg before user launch.
    let fixture = unsafe { native_durable_fixture::read() }.unwrap_or_else(|e| fail(e.as_str()));
    if !(1..=7).contains(&fixture.case()) {
        fail("durable_failure_fixture_case");
    }
    // SAFETY: Dedicated, explicitly provisioned secondary-master IDE channel;
    // owner stays on the suspended coordinator stack across every pair lifetime.
    let mut device = unsafe { native_storage_device::device::acquire(8) }
        .unwrap_or_else(|_| fail("durable_device_owner"));
    let geometry = device
        .identify()
        .unwrap_or_else(|_| fail("durable_device_geometry"));
    if device.geometry() != Some(geometry) || geometry.logical_sector_bytes != 512 {
        fail("durable_device_identity");
    }
    let first_phase = match fixture.phase() {
        native_durable_fixture::fixture_phase::provision_save => 0,
        native_durable_fixture::fixture_phase::cold_recovery => 2,
    };
    let phases = [first_phase];
    let baseline_image = artifact::inspect();
    let image = runtime_image::inspect();
    // SAFETY: Unique framebuffer owner is withdrawn for the whole proof.
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
            .unwrap_or_else(|_| fail("durable_baseline_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
            fail("durable_baseline_pixels");
        }
        clear_stage();
        let before_fault = framebuffer_hash(info, false);
        let (fault_vendor, faulted) = run_owned(
            allocator,
            baseline_image,
            kernel_cr3,
            bootstrap_stack,
            1,
            initial,
        );
        if fault_vendor != vendor || framebuffer_hash(info, false) != before_fault {
            fail("durable_baseline_fault");
        }
        clear_stage();
        let mut records = [runtime_record::default(); 1];
        for (position, phase) in phases.iter().copied().enumerate() {
            let challenge = if phase == 0 { fixture.challenge() } else { &[] };
            let (current_vendor, record) = run_arena_pair(
                &mut device,
                (phase, fixture.case(), challenge),
                allocator,
                image,
                kernel_cr3,
                bootstrap_stack,
                initial,
            );
            if current_vendor != vendor {
                fail("durable_cpu_comparison");
            }
            let patch = presenter
                .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                    &*display_stage.0.get()
                })
                .unwrap_or_else(|_| fail("durable_patch"));
            if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
                fail("durable_pixels");
            }
            clear_stage();
            records[position] = record;
        }
        (initial, vendor, [normal, faulted], records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("durable_final_interrupts");
    }
    let quarantine_denied = first_phase == 0 && fixture.case() == 7;
    if device.grant_available() == quarantine_denied || device.is_poisoned() != quarantine_denied {
        fail("durable_retired_device_grant");
    }
    emit_evidence(initial, vendor, baseline);
    let mut serial = serial_port::com1();
    for (phase, record) in phases.iter().zip(records.iter()) {
        let e = record.application;
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_durable_failure_test.phase","level":"info","phase":{},"case":{},"starts":{},"zeroed_bytes":{},"arena_copies":{},"calls":{},"switches":{},"timer_ticks":{},"denials":{},"bad_copies":{},"invalid":{},"rows":{},"exits":{},"faults":{},"closed":{},"discarded":{},"arm_seen":{},"completed_operations":{},"io_errors":{},"resources_restored":true}}"#,
            phase,
            fixture.case(),
            record.starts,
            record.zeroed_bytes,
            record.arena_copies,
            e.calls,
            e.switches,
            e.timer_ticks,
            e.denials,
            e.bad_copies,
            e.invalid,
            e.rows,
            e.exits,
            e.faults,
            e.closed,
            e.discarded,
            record.arm_seen,
            record.completed_operations,
            record.io_errors
        );
        for channel in 0..channel_count {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.native_durable_failure_test.channel","level":"info","phase":{},"case":{},"channel":{},"sends":{},"deliveries":{},"acks":{}}}"#,
                phase,
                fixture.case(),
                channel,
                e.sends[channel],
                e.deliveries[channel],
                e.acknowledgements[channel]
            );
            for index in 0..e.sends[channel] as usize {
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_durable_failure_test.packet","level":"info","phase":{},"case":{},"channel":{},"sequence":{},"hash":{}}}"#,
                    phase,
                    fixture.case(),
                    channel,
                    index + 1,
                    e.packet_hashes[channel][index]
                );
            }
        }
    }
    let c = device.counters();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_durable_failure_test.device","level":"info","phase":{},"case":{},"port_reads":{},"port_writes":{},"commands_issued":{},"commands_completed":{},"sectors_read":{},"sectors_written":{},"flushes":{},"poisoned":{}}}"#,
        first_phase,
        fixture.case(),
        c.port_reads,
        c.port_writes,
        c.commands_issued,
        c.commands_completed,
        c.sectors_read,
        c.sectors_written,
        c.flushes,
        device.is_poisoned()
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_durable_failure_test.passed","level":"info","case":{},"phases":{},"zeroed_bytes":{},"resources_restored":true,"authority_revoked":true,"quarantine_denied":{},"store_scope":"dedicated_disk"}}"#,
        fixture.case(),
        phases.len(),
        phases.len() as u64 * 2 * arena_byte_count,
        quarantine_denied
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
