//! Isolated interactive native editor and durable service. Policy stays in userspace.
//! Kernel-owned mappings and per-task handle bindings remain authoritative;
//! mutable startup bytes never create mappings, tasks or channel grants.
use super::super::owned_execution::user_data_base;
use super::*;
use anomalous_document_protocol::{
    client_identity,
    durable::disk_identity,
    interactive_startup::{
        arena_byte_count, display_height, display_width, row_bytes, startup_descriptor,
        startup_size,
    },
};

use super::super::{native_interactive_fixture, native_storage_device};
use crate::input::{key_state, physical_input_delivery};
use anomalous_abi::{rite_physical_input_v1, rite_physical_input_v1_size};

const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;
const arena_frames: usize = 66;
const channel_count: usize = 4;
const interactive_stage_size: usize = row_bytes * display_height;
struct interactive_display_storage(UnsafeCell<[u8; interactive_stage_size]>);
// SAFETY: Sole BSP, IF clear, exclusive active runtime or its bootstrap owner.
unsafe impl Sync for interactive_display_storage {}
static interactive_stage: interactive_display_storage =
    interactive_display_storage(UnsafeCell::new([0; interactive_stage_size]));
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
// SAFETY: Sole BSP, IF clear. The latch covers initialization through teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

unsafe extern "C" {
    static x86_64_native_durable_payload_start: u8;
    static x86_64_native_durable_payload_end: u8;
    static x86_64_native_durable_user_fault_instruction: u8;
}
#[derive(Clone, Copy)]
struct runtime_image {
    source: u64,
    length: u64,
}
impl runtime_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_durable_payload_start);
        let length = address!(x86_64_native_durable_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("runtime_payload_extent"));
        let fault = address!(x86_64_native_durable_user_fault_instruction);
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > code_pages * 4096
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || [fault].iter().any(|marker| {
                *marker < user_code_base
                    || marker
                        .checked_add(2)
                        .is_none_or(|end| end > user_code_base + length)
            })
        {
            fail("runtime_artifact_geometry");
        }
        for marker in [fault] {
            // SAFETY: Each complete marker lies in the immutable resident payload.
            if unsafe { ptr::read_unaligned((source + marker - user_code_base) as *const [u8; 2]) }
                != [0x0f, 0x0b]
            {
                fail("document_artifact_fault_marker");
            }
        }
        Self { source, length }
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
    inputs: u64,
    input_losses: u64,
    frames: u64,
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
    packet_hashes: [u64; channel_count],
    row_cursor: usize,
    presenter: *mut framebuffer_presenter,
    framebuffer: framebuffer_info,
    outside: u64,
    faulted: [bool; 2],
    input_empty: bool,
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
            if channel.phase() == channel_phase::closed {
                continue;
            }
            let receipt = channel
                .close(channel.binding())
                .unwrap_or_else(|_| fail("channel_close"));
            if receipt.discarded() {
                increment(&mut self.evidence.discarded);
            }
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
                if !self.arena_contains(self.selected, address, length) {
                    return (3, 0, 0);
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
                increment(&mut self.arena_copies);
                let hash = packet_hash(&scratch[..length]);
                let receipt = self.channels[index]
                    .commit_submit(prepared, &scratch[..length])
                    .unwrap_or_else(|_| fail("submit_commit"));
                scratch.fill(0);
                increment(&mut self.evidence.sends[index]);
                self.packet_hashes[index] = hash;
                let mut serial = serial_port::com1();
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"kernel.native_interactive_test.packet","level":"info","channel":{},"sequence":{},"length":{},"hash":{}}}"#,
                    index,
                    receipt.sequence(),
                    receipt.length(),
                    hash
                );
                (0, receipt.sequence(), receipt.length() as u64)
            }
            2 => {
                let prepared = match self.channels[index].prepare_delivery(binding, caller, length)
                {
                    Ok(p) => p,
                    Err(e) => return (channel_status(e), 0, 0),
                };
                if !self.arena_contains(self.selected, address, length) {
                    return (3, 0, 0);
                }
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
                        increment(&mut self.arena_copies);
                        if self.packet_hashes[index] != hash {
                            fail("copied_snapshot_changed");
                        }
                        increment(&mut self.evidence.deliveries[index]);
                        (0, receipt.sequence(), receipt.length() as u64)
                    }
                    Err(error) => (channel_status(error), 0, 0),
                }
            }
            3 => match self.channels[index].acknowledge(binding, caller, sequence) {
                Ok(receipt) => {
                    self.last_ack[index] = receipt.sequence();
                    increment(&mut self.evidence.acknowledgements[index]);
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
    fn input_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if self.selected != 0 {
            return (2, 0, 0);
        }
        if value.destination_index != 0
            || value.counter != 0
            || value.data != rite_physical_input_v1_size as u64
        {
            return (4, 0, 0);
        }
        if !self.arena_contains(0, value.source_index, rite_physical_input_v1_size)
            || self
                .execution
                .preflight_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[0],
                    value.source_index,
                    rite_physical_input_v1_size,
                )
                .is_err()
        {
            return (3, 0, 0);
        }
        // SAFETY: Complete writable destination validated before consuming any
        // source state. IF is clear and the exact selected owner cannot change.
        let Some(delivery) = (unsafe { crate::input::poll_physical_input_if_disabled() }) else {
            self.input_empty = true;
            return (1, 0, 0);
        };
        let encoded = match delivery {
            physical_input_delivery::transition(record) => {
                let key = record.payload();
                rite_physical_input_v1::key(
                    record.source().generation(),
                    record.sequence().get(),
                    key.key().make_code(),
                    key.key().is_extended(),
                    key.state() == key_state::pressed,
                )
            }
            physical_input_delivery::loss(loss) => {
                let generation = interrupts::rearm_keyboard_input_after_loss()
                    .unwrap_or_else(|| fail("interactive_input_rearm"));
                increment(&mut self.evidence.input_losses);
                rite_physical_input_v1::loss(
                    loss.source().generation(),
                    generation,
                    loss.first_sequence()
                        .unwrap_or_else(|| fail("input_loss_first"))
                        .get(),
                    loss.last_sequence()
                        .unwrap_or_else(|| fail("input_loss_last"))
                        .get(),
                    loss.lost_count(),
                )
            }
        }
        .unwrap_or_else(|| fail("interactive_input_encoding"));
        self.execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[0],
                value.source_index,
                encoded.encode(),
            )
            .unwrap_or_else(|_| fail("interactive_input_after_preflight"));
        self.input_empty = false;
        increment(&mut self.evidence.inputs);
        increment(&mut self.arena_copies);
        (0, encoded.sequence(), rite_physical_input_v1_size as u64)
    }
    fn row_call(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if self.selected != 0 {
            return (2, 0, 0);
        }
        if value.counter != 0
            || value.destination_index != self.row_cursor as u64
            || self.row_cursor >= display_height
            || value.data != row_bytes as u64
        {
            return (4, 0, 0);
        }
        let mut row = [0; row_bytes];
        if !self.arena_contains(0, value.source_index, row.len())
            || self
                .execution
                .copy_from_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[0],
                    value.source_index,
                    &mut row,
                )
                .is_err()
        {
            clear_scratch(&mut row);
            return (3, 0, 0);
        }
        let start = self.row_cursor * row_bytes;
        // SAFETY: Exact bounded row in private BSS, under the sole dispatch latch.
        unsafe {
            (&mut *interactive_stage.0.get())[start..start + row_bytes].copy_from_slice(&row);
        }
        clear_scratch(&mut row);
        self.row_cursor += 1;
        increment(&mut self.evidence.rows);
        increment(&mut self.arena_copies);
        (0, 0, 0)
    }
    fn commit_frame(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if self.selected != 0 {
            return (2, 0, 0);
        }
        if !no_arguments(value) || self.row_cursor != display_height {
            return (4, 0, 0);
        }
        let next = self
            .evidence
            .frames
            .checked_add(1)
            .unwrap_or_else(|| fail("frame_sequence"));
        // SAFETY: Coordinator retains the sole presenter while suspended. All
        // rows are private copies; no userspace mapping or borrow is published.
        let pixels = unsafe { &*interactive_stage.0.get() };
        let hash = packet_hash(pixels);
        let presenter = self.presenter;
        let framebuffer = self.framebuffer;
        let outside = self.outside;
        // SAFETY: The selected eager CPU image was captured before dispatch.
        // The callback only presents resident copied pixels through the boot
        // framebuffer mapping. IF stays clear; all task ownership is retained.
        let verified = unsafe {
            self.execution.with_interactive_bootstrap(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[self.selected],
                || {
                    let patch = (&mut *presenter)
                        .prepare_rgb8_region(32, 32, display_width, display_height, pixels)
                        .unwrap_or_else(|_| fail("interactive_present_preflight"));
                    patch.commit_and_verify() && outside_hash(framebuffer) == outside
                },
            )
        }
        .unwrap_or_else(|reason| fail(reason));
        if !verified || !self.cpu.controls_are_active() {
            fail("interactive_present_readback");
        }
        self.evidence.frames = next;
        clear_interactive_stage();
        self.row_cursor = 0;
        let mut serial = serial_port::com1();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_interactive_test.frame","level":"info","sequence":{},"hash":{}}}"#,
            next, hash
        );
        (0, next, 0)
    }
    fn peer_status(&self) -> u64 {
        let peer = 1 - self.selected;
        if self.faulted[peer] {
            2
        } else {
            u64::from(self.terminal[peer])
        }
    }
    fn retire_current(&mut self, faulted: bool) -> Option<*mut scheduler_frame> {
        let index = self.selected;
        self.close_channels();
        self.terminal[index] = true;
        self.faulted[index] = faulted;
        if faulted {
            increment(&mut self.evidence.faults);
        } else {
            increment(&mut self.evidence.exits);
        }
        if index == 1 {
            self.device_handle = 0;
        }
        if self.terminal[1 - index] {
            if faulted {
                self.scheduler
                    .fault_last_current(&mut self.tasks, self.identities[index])
            } else {
                self.scheduler
                    .complete_current(&mut self.tasks, self.identities[index])
            }
            .unwrap_or_else(|_| fail("interactive_complete_last"));
            return None;
        }
        let transition = if faulted {
            self.scheduler
                .fault_current(&mut self.tasks, self.identities[index])
                .map(|result| result.dispatch())
        } else {
            self.scheduler
                .complete_current_and_dispatch(&mut self.tasks, self.identities[index])
                .map(|result| result.dispatch())
        }
        .unwrap_or_else(|_| fail("interactive_complete_peer"));
        if transition.task() != self.identities[1 - index] {
            fail("interactive_wrong_peer");
        }
        Some(self.select(1 - index))
    }
    fn arena_contains(&self, index: usize, address: u64, length: usize) -> bool {
        address >= self.arena_bases[index]
            && address
                .checked_add(length as u64)
                .is_some_and(|end| end <= self.arena_bases[index] + arena_byte_count)
    }
}

fn clear_sector_scratch(bytes: &mut [u8; 512]) {
    clear_scratch(bytes);
}

fn clear_scratch(bytes: &mut [u8]) {
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
}

// Return constructor temporaries before any mapping preparation or activation.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
unsafe fn initialize_storage(
    pointer: *mut runtime,
    device: *mut native_storage_device::device,
    scenario: usize,
    presenter: *mut framebuffer_presenter,
    framebuffer: framebuffer_info,
    outside: u64,
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
        ptr::addr_of_mut!((*pointer).packet_hashes).write([0; channel_count]);
        ptr::addr_of_mut!((*pointer).row_cursor).write(0);
        ptr::addr_of_mut!((*pointer).presenter).write(presenter);
        ptr::addr_of_mut!((*pointer).framebuffer).write(framebuffer);
        ptr::addr_of_mut!((*pointer).outside).write(outside);
        ptr::addr_of_mut!((*pointer).faulted).write([false; 2]);
        ptr::addr_of_mut!((*pointer).input_empty).write(false);
        ptr::addr_of_mut!((*pointer).device).write(device);
        ptr::addr_of_mut!((*pointer).device_handle).write(0x7001 + scenario as u64 * 0x100);
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
#[allow(clippy::too_many_arguments)]
fn initialize(
    device: *mut native_storage_device::device,
    proof_mode: u64,
    presenter: *mut framebuffer_presenter,
    framebuffer: framebuffer_info,
    outside: u64,
    allocator: &mut frame_allocator,
    image: runtime_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut runtime {
    if scenario >= 2 || proof_mode > 2 || storage_busy.swap(true, Ordering::AcqRel) {
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
            scenario,
            presenter,
            framebuffer,
            outside,
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
            let descriptor = startup_descriptor::try_new(
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
                proof_mode,
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

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn run_arena_pair(
    device: &mut native_storage_device::device,
    input: (usize, u64),
    presenter: &mut framebuffer_presenter,
    framebuffer: framebuffer_info,
    outside: u64,
    allocator: &mut frame_allocator,
    image: runtime_image,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> (&'static str, runtime_record) {
    let (scenario, proof_mode) = input;
    if super::is_active()
        || is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || !interactive_stage_clear()
    {
        fail("runtime_setup");
    }
    let state = initialize(
        ptr::from_mut(device),
        proof_mode,
        ptr::from_mut(presenter),
        framebuffer,
        outside,
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
    if e.exits + e.faults != 2 || state.device_handle != 0 {
        fail("interactive_completion_evidence");
    }
    clear_interactive_stage();
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if state.faulted[index] {
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
    state.presenter = ptr::null_mut();
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
    {
        fail("runtime_timer");
    }
    state.last_tick = observed.tick();
    increment(&mut state.evidence.timer_ticks);
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn resume_input_interrupt(
    frame: *mut interrupts::interrupt_frame,
) -> *mut interrupts::interrupt_frame {
    let state = enter_dispatch();
    // SAFETY: Exact eager CPU owner; IRQ stubs and handler use only integers.
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame.cast()) {
        fail("interactive_input_frame");
    }
    leave_dispatch(state, frame.cast()).cast()
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    // SAFETY: Check frame ownership separately; a faulting RIP need not be a
    // valid return address. CPU storage still belongs to this selected owner.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
    {
        fail("interactive_fault_frame");
    }
    state
        .retire_current(true)
        .map(|next| leave_dispatch(state, next).cast())
}
pub(super) fn resume_kernel_after_fault() -> ! {
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() || !dispatch_active.load(Ordering::Acquire) {
        fail("interactive_fault_continuation");
    }
    // SAFETY: The exception dispatcher has withdrawn its latch before this
    // nonlocal return; the current runtime remains exclusively borrowed.
    let state = unsafe { &mut *pointer };
    if state.terminal != [true; 2] {
        fail("interactive_fault_live_peer");
    }
    state.finish()
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
        || (value.accumulator != 0 && !state.started[state.selected])
    {
        fail("runtime_gate_vector");
    }
    increment(&mut state.evidence.calls);
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
                2 => increment(&mut state.evidence.denials),
                3 => increment(&mut state.evidence.bad_copies),
                4 => increment(&mut state.evidence.invalid),
                5 => increment(&mut state.evidence.closed),
                _ => {}
            }
        }
        4 => {
            if !no_arguments(value) {
                reply.0 = 4;
                increment(&mut state.evidence.invalid);
            } else {
                reply.1 = state.peer_status();
                // This is an idle interval, never a storage or copy operation.
                // Kernel-origin IRQs cannot borrow the active runtime. A timer
                // bounds one sleep even if the peer has private pending work.
                if state.selected == 0
                    && state.input_empty
                    && state
                        .channels
                        .iter()
                        .all(|c| matches!(c.phase(), channel_phase::empty | channel_phase::closed))
                    && !unsafe { crate::input::ps2_delivery_ready_if_disabled() }
                {
                    unsafe { asm!("sti", "hlt", "cli", options(nostack)) };
                }
                if !state.terminal[1 - state.selected] {
                    let transition = state
                        .scheduler
                        .preempt_current(&mut state.tasks, state.identities[state.selected])
                        .unwrap_or_else(|_| fail("runtime_yield"));
                    if transition.dispatch().task() != state.identities[1 - state.selected] {
                        fail("runtime_yield_peer");
                    }
                    // SAFETY: Complete outgoing frame remains privately owned.
                    unsafe {
                        (*frame).accumulator = 0;
                        (*frame).counter = reply.1;
                        (*frame).data = 0;
                    }
                    selected = state.select(1 - state.selected);
                    increment(&mut state.evidence.switches);
                }
            }
        }
        5 => {
            if !no_arguments(value) {
                reply.0 = 4;
                increment(&mut state.evidence.invalid);
            } else if let Some(next) = state.retire_current(false) {
                selected = next;
            } else {
                state.finish();
            }
        }
        7 | 15 | 16 => {
            reply = match value.accumulator {
                7 => state.row_call(value),
                15 => state.input_call(value),
                16 => state.commit_frame(value),
                _ => unreachable!(),
            };
            match reply.0 {
                2 => increment(&mut state.evidence.denials),
                3 => increment(&mut state.evidence.bad_copies),
                4 => increment(&mut state.evidence.invalid),
                _ => {}
            }
        }
        12..=14 => {
            // Device owner survives pair retirement. The sole BSP cannot access
            // it through the coordinator while this suspended entry is active.
            let before = unsafe { (&*state.device).counters() };
            reply = state.storage_call(value);
            match reply.0 {
                2 => increment(&mut state.evidence.denials),
                3 => increment(&mut state.evidence.bad_copies),
                4 => increment(&mut state.evidence.invalid),
                _ => {}
            }
            if matches!(reply.0, 2..=4) && unsafe { (&*state.device).counters() } != before {
                fail("durable_rejection_performed_io");
            }
        }
        _ => {
            reply.0 = 4;
            increment(&mut state.evidence.invalid);
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

fn increment(value: &mut u64) {
    *value = value
        .checked_add(1)
        .unwrap_or_else(|| fail("interactive_counter_exhausted"));
}
fn no_arguments(value: scheduler_frame) -> bool {
    value.destination_index == 0 && value.source_index == 0 && value.data == 0 && value.counter == 0
}
fn interactive_stage_clear() -> bool {
    // SAFETY: Sole BSP owns the complete supervisor-only retained stage.
    unsafe {
        (&*interactive_stage.0.get())
            .iter()
            .all(|v| ptr::read_volatile(v) == 0)
    }
}
fn clear_interactive_stage() {
    // SAFETY: Exclusive stage owner, with no live user mapping or pixel borrow.
    unsafe {
        clear_scratch(&mut *interactive_stage.0.get());
    }
    if !interactive_stage_clear() {
        fail("interactive_stage_clear");
    }
}
fn outside_hash(info: framebuffer_info) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let pixel_bytes = info.bits_per_pixel as usize / 8;
    for row in 0..info.height {
        for offset in 0..info.pitch {
            if (32..32 + display_height).contains(&row)
                && (32 * pixel_bytes..(32 + display_width) * pixel_bytes).contains(&offset)
            {
                continue;
            }
            // SAFETY: Presenter validated the complete uniquely retained aperture.
            let byte = unsafe {
                ptr::read_volatile((info.address + (row * info.pitch + offset) as u64) as *const u8)
            };
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}
pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_interactive_input()
    {
        fail("interactive_initial_state");
    }
    // SAFETY: Exclusive fw_cfg boot reader, then fixed explicitly provisioned
    // secondary IDE owner; neither is exposed to either userspace task.
    let fixture =
        unsafe { native_interactive_fixture::read() }.unwrap_or_else(|e| fail(e.as_str()));
    let mut device = unsafe { native_storage_device::device::acquire(8) }
        .unwrap_or_else(|_| fail("interactive_device_owner"));
    let geometry = device
        .identify()
        .unwrap_or_else(|_| fail("interactive_device_geometry"));
    if device.geometry() != Some(geometry)
        || geometry.logical_sector_bytes != 512
        || device.is_poisoned()
    {
        fail("interactive_device_identity");
    }
    let image = runtime_image::inspect();
    // SAFETY: This coordinator owns the sole framebuffer until both tasks exit.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = outside_hash(info);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_interactive_test.ready","level":"info","tasks":2,"phase":{},"proof_mode":{},"descriptor_bytes":192,"arena_bytes":32768,"data_frames":48,"table_frames":18,"leases":12,"x":32,"y":32,"width":384,"height":144}}"#,
        fixture.phase, fixture.proof_mode
    );
    let (_, record) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        run_arena_pair(
            &mut device,
            (fixture.phase as usize, fixture.proof_mode),
            &mut presenter,
            info,
            outside,
            allocator,
            image,
            kernel_cr3,
            bootstrap_stack,
            initial,
        )
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    let e = record.application;
    if e.inputs
        .checked_add(e.rows)
        .is_none_or(|minimum| record.arena_copies < minimum)
    {
        fail("interactive_copy_accounting");
    }
    for channel in 0..channel_count {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_interactive_test.channel","level":"info","channel":{},"sends":{},"deliveries":{},"acks":{}}}"#,
            channel, e.sends[channel], e.deliveries[channel], e.acknowledgements[channel]
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_interactive_test.passed","level":"info","phase":{},"proof_mode":{},"starts":{},"calls":{},"switches":{},"timer_ticks":{},"inputs":{},"input_losses":{},"frames":{},"rows":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"exits":{},"faults":{},"zeroed_bytes":{},"resources_restored":true,"authority_revoked":true,"device_poisoned":{}}}"#,
        fixture.phase,
        fixture.proof_mode,
        record.starts,
        e.calls,
        e.switches,
        e.timer_ticks,
        e.inputs,
        e.input_losses,
        e.frames,
        e.rows,
        e.denials,
        e.bad_copies,
        e.invalid,
        e.closed,
        e.discarded,
        e.exits,
        e.faults,
        record.zeroed_bytes,
        device.is_poisoned()
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
