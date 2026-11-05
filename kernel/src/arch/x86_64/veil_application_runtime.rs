//! Independent native veil serving one blocked raw-trap caller at a time.
//! Linux numbers, descriptors, errno and poll bytes are opaque to this owner.
use super::super::owned_execution::owned_execution_group;
use super::*;
use crate::scheduler::{scheduler_call_authorization, scheduler_veil_wait_authorization};
use core::sync::atomic::AtomicU64;

#[cfg(feature = "linux_tls_test")]
#[path = "linux_tls_runtime.rs"]
mod tls_proof;
#[cfg(feature = "linux_tls_test")]
use super::super::task_tls;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
static next_request: AtomicU64 = AtomicU64::new(1);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
// SAFETY: Sole BSP, IF clear. Storage remains reserved until explicit teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

#[cfg(not(feature = "linux_tls_test"))]
unsafe extern "C" {
    static x86_64_veil_application_payload_start: u8;
    static x86_64_veil_application_payload_end: u8;
    static x86_64_veil_application_data_start: u8;
    static x86_64_veil_application_data_end: u8;
    static x86_64_veil_application_user_fault_instruction: u8;
}
#[derive(Clone, Copy, Default)]
struct veil_evidence {
    raw_calls: u64,
    completions: u64,
    stages: u64,
    rejected: u64,
    aborted: u64,
    #[cfg(feature = "linux_tls_test")]
    tls: tls_proof::tls_evidence,
}
struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 3],
    execution: owned_execution_group<3>,
    cpu: application_cpu::cpu_session_group<3>,
    #[cfg(feature = "linux_tls_test")]
    tls: task_tls::session_group<3>,
    #[cfg(feature = "linux_tls_test")]
    tls_update: task_tls::suspended_fs_update,
    channels: [application_channel; 2],
    last_ack: [u64; 2],
    image: artifact,
    veil_fault: u64,
    scenario: usize,
    selected: usize,
    started: [bool; 3],
    terminal: [bool; 3],
    evidence: evidence,
    veil_evidence: veil_evidence,
    pending: Option<scheduler_call_authorization>,
    waiting: Option<scheduler_veil_wait_authorization>,
    request: [u8; 80],
    sequence: u64,
    stage_kind: u64,
    stage_address: u64,
    stage_length: usize,
    stage_bytes: [u8; application_packet_capacity],
    audit_bytes: [u8; application_packet_capacity],
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
        #[cfg(feature = "linux_application_test")]
        if self.image.linux && !self.image.loader {
            let stack = self
                .execution
                .transition_stack_top(index)
                .unwrap_or_else(|e| fail(e));
            // SAFETY: IF/NMI excluded, old CPU image saved; TSS already selects
            // this retained supervisor stack. Enable SCE only for the Linux task.
            if !unsafe { self.cpu.select_linux_entry(stack, index == 0) }
                || !self.cpu.linux_entry_matches(stack, index == 0)
            {
                fail("linux_entry_selection");
            }
        }
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
        let Ok(index) = usize::try_from(value.destination_index.wrapping_sub(1)) else {
            return (2, 0, 0);
        };
        if index >= 2 {
            return (2, 0, 0);
        }
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
}

#[cfg(not(feature = "linux_tls_test"))]
fn veil_image() -> artifact {
    let rx = elf_profile::span(
        address!(x86_64_veil_application_payload_start),
        address!(x86_64_veil_application_payload_end),
        code_pages * 4096,
    );
    let rw = elf_profile::span(
        address!(x86_64_veil_application_data_start),
        address!(x86_64_veil_application_data_end),
        8192,
    );
    let fault = address!(x86_64_veil_application_user_fault_instruction);
    let offset = fault
        .checked_sub(user_code_base)
        .unwrap_or_else(|| fail("veil_fault_marker")) as usize;
    if rx.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
        fail("veil_fault_marker");
    }
    artifact {
        source: rx.as_ptr() as u64,
        length: rx.len() as u64,
        fault,
        data: (rw.as_ptr() as u64, rw.len() as u64),
        loader: false,
        linux: false,
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator,
    images: [artifact; 3],
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut runtime {
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("veil_storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    // SAFETY: Exclusive uninitialized/fully retired BSS. Every field is written
    // before a typed reference exists; owners are never moved after activation.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(3).unwrap_or_else(|_| fail("veil_scheduler")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution_group::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session_group::new());
        #[cfg(feature = "linux_tls_test")]
        {
            ptr::addr_of_mut!((*pointer).tls).write(task_tls::session_group::new());
            ptr::addr_of_mut!((*pointer).tls_update).write(task_tls::suspended_fs_update::new());
        }
        ptr::addr_of_mut!((*pointer).last_ack).write([0; 2]);
        ptr::addr_of_mut!((*pointer).image).write(images[0]);
        ptr::addr_of_mut!((*pointer).veil_fault).write(images[2].fault);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([true, false, false]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; 3]);
        ptr::addr_of_mut!((*pointer).evidence).write(evidence::default());
        ptr::addr_of_mut!((*pointer).veil_evidence).write(veil_evidence::default());
        ptr::addr_of_mut!((*pointer).pending).write(None);
        ptr::addr_of_mut!((*pointer).waiting).write(None);
        ptr::addr_of_mut!((*pointer).request).write([0; 80]);
        ptr::addr_of_mut!((*pointer).sequence).write(0);
        ptr::addr_of_mut!((*pointer).stage_kind).write(0);
        ptr::addr_of_mut!((*pointer).stage_address).write(0);
        ptr::addr_of_mut!((*pointer).stage_length).write(0);
        ptr::addr_of_mut!((*pointer).stage_bytes).write([0; application_packet_capacity]);
        ptr::addr_of_mut!((*pointer).audit_bytes).write([0; application_packet_capacity]);
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
        // Initialize identities and channels only after retained preparation.
        // No runtime reference is formed while these fields are uninitialized.
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        execution
            .prepare_segments_with_initial_stack(
                allocator,
                circles,
                images.map(|i| (i.source, i.length)),
                images.map(|i| i.data),
                code_pages,
                [user_stack_top - 256, user_stack_top, user_stack_top],
            )
            .unwrap_or_else(|e| fail(e));
        for (index, image) in images.into_iter().enumerate() {
            for offset in (0..8192).step_by(8) {
                if execution.read_data(index, offset)
                    != Some(elf_profile::data_word(image, offset, 0))
                    || execution.read_stack(index, offset) != Some(0)
                {
                    fail("veil_initial_storage");
                }
            }
            if index == 0 {
                linux_profile::populate_stack(execution, 0, usize::from(scenario == 1));
            } else {
                execution
                    .write_data(index, 0, index as u64)
                    .unwrap_or_else(|e| fail(e));
                execution
                    .write_data(
                        index,
                        8,
                        if index == 1 {
                            u64::from(scenario != 0)
                        } else {
                            scenario as u64
                        },
                    )
                    .unwrap_or_else(|e| fail(e));
            }
        }
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 501 + index as u64)
                .unwrap_or_else(|e| fail(e))
        });
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        for index in 0..2 {
            let binding = application_channel_binding::try_new(
                endpoint_id::try_from_parts(index, scenario as u64 + 1).unwrap(),
                identities[index],
                identities[1 - index],
            )
            .unwrap_or_else(|_| fail("veil_channel_binding"));
            ptr::addr_of_mut!((*pointer).channels)
                .cast::<application_channel>()
                .add(index)
                .write(
                    application_channel::try_new(binding).unwrap_or_else(|_| fail("veil_channel")),
                );
        }
        &mut *pointer
    }
}

#[inline(never)]
fn run_three(
    allocator: &mut frame_allocator,
    images: [artifact; 3],
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
) -> (&'static str, evidence, veil_evidence) {
    if is_active()
        || super::is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || !stage_is_clear()
    {
        fail("veil_setup");
    }
    let state = initialize(allocator, images, scenario, kernel_cr3, bootstrap_stack);
    for id in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, id)
            .unwrap_or_else(|_| fail("veil_admit"));
    }
    // SAFETY: All three owners are initialized at their final address and retained.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let accounting = allocator.accounting();
    if accounting.free.checked_add(75) != Some(initial.free)
        || accounting.data_owned != 48
        || accounting.table_owned != 27
        || accounting.active_leases != 15
        || accounting.bound_leases != 15
    {
        fail("veil_live_ownership");
    }
    for index in 0..3 {
        for other in 0..index {
            if state.execution.root(index).ok() == state.execution.root(other).ok()
                || state.execution.transition_stack_top(index).ok()
                    == state.execution.transition_stack_top(other).ok()
            {
                fail("veil_shared_owner");
            }
        }
    }
    // SAFETY: Stable CPU storage and all CPU/entry controls remain retained.
    unsafe { state.cpu.prepare_linux() }.unwrap_or_else(|e| fail(e));
    #[cfg(feature = "linux_tls_test")]
    unsafe { state.tls.prepare(state.identities) }.unwrap_or_else(|e| fail(e.reason()));
    if (0..3).any(|index| {
        !state
            .cpu
            .task_image(index)
            .is_some_and(|image| image.is_initial_clean())
    }) {
        fail("veil_initial_cpu_images");
    }
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("veil_start"));
    if first.task() != state.identities[0] || first.context() != scheduler_context::initial {
        fail("veil_first");
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
        fail("veil_publication");
    }
    // SAFETY: Only IF-clear handlers borrow the stable runtime until continuation.
    let status = unsafe {
        #[cfg(feature = "linux_tls_test")]
        (*pointer).stage_tls(frame);
        if !(*pointer).cpu.restore_task(0) {
            fail("veil_initial_cpu");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    #[cfg(feature = "linux_tls_test")]
    {
        // Bootstrap is restored but every task mapping remains retained.
        unsafe { state.tls.finish() }.unwrap_or_else(|e| fail(e.reason()));
        state.validate_tls_finished();
    }
    if status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || paging::read_cr0() & 8 != state.original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || !state.cpu.linux_entry_is_restored()
        || !state.cpu.linux_entry_storage_is_cleared()
        || state.terminal != [true; 3]
        || state.started != [true; 3]
        || state.pending.is_some()
        || state.waiting.is_some()
        || state.sequence != 0
        || state.stage_kind != 0
        || state.stage_bytes.iter().any(|b| *b != 0)
        || state.audit_bytes.iter().any(|b| *b != 0)
        || state
            .channels
            .iter()
            .any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        fail("veil_continuation");
    }
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if (scenario == 1 && index == 0) || (scenario == 2 && index != 1) {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|s| s.state() == expected)
        {
            fail("veil_terminal");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("veil_withdraw"));
    }
    // SAFETY: Bootstrap active, all contexts withdrawn and owners still retained.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|e| fail(e));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("veil_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
    {
        fail("veil_final_accounting");
    }
    let result = (state.cpu.cpu_vendor(), state.evidence, state.veil_evidence);
    storage_busy.store(false, Ordering::Release);
    result
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}
pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let pointer = active.load(Ordering::Acquire);
    // SAFETY: Published storage outlives every serialized sole-BSP handler.
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}
fn enter_dispatch() -> &'static mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("veil_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("veil_inactive");
    }
    // SAFETY: Sole IF-clear handler owns the live stable runtime until latch release.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    #[cfg(feature = "linux_tls_test")]
    state.stage_tls(frame);
    // SAFETY: Selected task owns its stable eager CPU image.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("veil_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = enter_dispatch();
    #[cfg(feature = "linux_tls_test")]
    {
        if !state.frame_valid(frame.cast()) {
            fail("linux_tls_timer_frame");
        }
        state.consume_tls(frame.cast());
    }
    // SAFETY: Exclusive handler owns stable eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.evidence.timer_ticks >= max_calls
    {
        fail("veil_timer");
    }
    state.last_tick = observed.tick();
    state.evidence.timer_ticks += 1;
    #[cfg(feature = "linux_tls_test")]
    state.observe_tls_timer();
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn syscall(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    #[cfg(feature = "linux_tls_test")]
    {
        if !state.frame_valid(frame) {
            fail("linux_tls_syscall_frame");
        }
        state.consume_tls(frame);
    }
    // SAFETY: Hardware entry has switched from untrusted RSP to owned stack.
    if state.selected != 0
        || state.sequence != 0
        || state.pending.is_some()
        || state.stage_kind != 0
        || !unsafe { state.cpu.save_current(0) }
        || !state.frame_valid(frame)
        || state.veil_evidence.raw_calls >= max_calls
    {
        fail("veil_raw_frame");
    }
    #[cfg(feature = "linux_tls_test")]
    state.observe_caller_fp();
    // SAFETY: Complete retained frame checked above; no syscall interpretation.
    let value = unsafe { ptr::read(frame) };
    if !super::super::linux_syscall::entry_frame_matches_hardware(value) {
        fail("veil_raw_hardware");
    }
    let sequence = next_request
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            if v == 0 { None } else { v.checked_add(1) }
        })
        .unwrap_or_else(|_| fail("veil_request_exhausted"));
    let request = anomalous_veil_protocol::invocation::new(
        sequence,
        value.accumulator,
        [
            value.destination_index,
            value.source_index,
            value.data,
            value.register_10,
            value.register_8,
            value.register_9,
        ],
    )
    .unwrap_or_else(|_| fail("veil_request_geometry"))
    .encode();
    let prepared = state
        .scheduler
        .prepare_call_handoff(
            &mut state.tasks,
            state.identities[0],
            state.identities[2],
            &mut state.waiting,
            &mut state.pending,
        )
        .unwrap_or_else(|_| fail("veil_handoff_prepare"));
    state.sequence = sequence;
    state.request = request;
    if prepared.commit().task() != state.identities[2] {
        fail("veil_handoff_peer");
    }
    state.veil_evidence.raw_calls += 1;
    let selected = state.select(2);
    leave_dispatch(state, selected)
}

impl runtime {
    fn authenticated_request(&self, sequence: u64) -> bool {
        self.selected == 2
            && sequence != 0
            && sequence == self.sequence
            && self.pending.as_ref().is_some_and(|authority| {
                self.execution
                    .validate_suspended_caller(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        authority,
                    )
                    .is_ok()
            })
    }
    // Proof observation around rejected native completions. The destination is
    // inspected while its caller is still blocked, before an eventual valid
    // completion could conceal an early copyout. This grants no userspace data.
    fn effect_observation(&mut self) -> [u64; 14] {
        let mut destination = 0;
        if matches!(self.stage_kind, 2 | 3) {
            self.execution
                .copy_from_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.pending.as_ref().unwrap(),
                    self.stage_address,
                    &mut self.audit_bytes[..self.stage_length],
                )
                .unwrap_or_else(|_| fail("veil_effect_observation"));
            destination = packet_hash(&self.audit_bytes[..self.stage_length]);
            self.audit_bytes.fill(0);
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("veil_observation_frame");
        }
        // SAFETY: Full retained caller frame is pinned by pending call authority.
        let frame_bytes = unsafe {
            core::slice::from_raw_parts(frame.cast::<u8>(), core::mem::size_of::<scheduler_frame>())
        };
        [
            self.sequence,
            self.stage_kind,
            self.stage_address,
            self.stage_length as u64,
            packet_hash(&self.stage_bytes),
            destination,
            packet_hash(frame_bytes),
            self.channels[0].phase() as u64,
            self.channels[0].queued_length() as u64,
            self.channels[0].next_sequence().unwrap_or(0),
            self.channels[1].phase() as u64,
            self.channels[1].queued_length() as u64,
            self.channels[1].next_sequence().unwrap_or(0),
            packet_hash(&self.request),
        ]
    }
    fn clear_request(&mut self) {
        #[cfg(feature = "linux_tls_test")]
        self.tls_update.discard();
        self.sequence = 0;
        self.request.fill(0);
        self.stage_kind = 0;
        self.stage_address = 0;
        self.stage_length = 0;
        self.stage_bytes.fill(0);
    }
    fn query(&self, capability: u64) -> (u64, u64, u64) {
        let Ok(index) = usize::try_from(capability) else {
            return (2, 0, 0);
        };
        let Some(channel) = self.channels.get(index) else {
            return (2, 0, 0);
        };
        let phase = match channel.phase() {
            channel_phase::empty => anomalous_veil_protocol::channel_phase::empty,
            channel_phase::queued => anomalous_veil_protocol::channel_phase::queued,
            channel_phase::delivered => anomalous_veil_protocol::channel_phase::delivered,
            channel_phase::closed => anomalous_veil_protocol::channel_phase::closed,
        };
        let snapshot = anomalous_veil_protocol::channel_snapshot::new(
            phase,
            channel.next_sequence().is_none(),
            channel.queued_length(),
        )
        .unwrap_or_else(|_| fail("veil_snapshot"));
        (0, snapshot.encode(), 0)
    }
    fn caller_read(&mut self, source: u64, destination: u64, length: u64) -> (u64, u64, u64) {
        let Ok(length) = usize::try_from(length) else {
            return (4, 0, 0);
        };
        if !(1..=application_packet_capacity).contains(&length) {
            return (4, 0, 0);
        }
        let mut scratch = [0; application_packet_capacity];
        let authority = self.pending.as_ref().unwrap();
        let result = self
            .execution
            .copy_from_suspended_caller(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                authority,
                source,
                &mut scratch[..length],
            )
            .and_then(|_| {
                self.execution.copy_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[2],
                    destination,
                    &scratch[..length],
                )
            });
        scratch.fill(0);
        if result.is_ok() {
            (0, length as u64, 0)
        } else {
            (3, 0, 0)
        }
    }
    fn stage(&mut self, kind: u64, address: u64, capacity: u64, source: u64) -> (u64, u64, u64) {
        if self.stage_kind != 0
            || !(1..=3).contains(&kind)
            || !(1..=application_packet_capacity as u64).contains(&capacity)
        {
            return (4, 0, 0);
        }
        let mut length = capacity as usize;
        let authority = self.pending.as_ref().unwrap();
        if kind == 1 {
            if self.channels[0].phase() == channel_phase::closed {
                return (5, 0, 0);
            }
            let Some(sequence) = self.channels[0].next_sequence() else {
                return (7, 0, 0);
            };
            if let Err(e) = self.channels[0].prepare_submit(
                self.channels[0].binding(),
                self.identities[0],
                sequence,
                length,
            ) {
                return (veil_channel_status(e), 0, 0);
            }
            if self
                .execution
                .copy_from_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    authority,
                    address,
                    &mut self.stage_bytes[..length],
                )
                .is_err()
            {
                self.stage_bytes.fill(0);
                return (3, 0, 0);
            }
        } else {
            if kind == 2 {
                if let Err(e) = self.channels[1].prepare_delivery(
                    self.channels[1].binding(),
                    self.identities[0],
                    length,
                ) {
                    return (veil_channel_status(e), 0, 0);
                }
                length = self.channels[1].queued_length();
            }
            if self
                .execution
                .preflight_to_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    authority,
                    address,
                    length,
                )
                .is_err()
            {
                return (3, 0, 0);
            }
            if kind == 3
                && self
                    .execution
                    .copy_from_user(
                        &self.tasks,
                        &self.circles,
                        &self.scheduler,
                        self.identities[2],
                        source,
                        &mut self.stage_bytes[..length],
                    )
                    .is_err()
            {
                self.stage_bytes.fill(0);
                return (3, 0, 0);
            }
        }
        self.stage_kind = kind;
        self.stage_address = address;
        self.stage_length = length;
        self.veil_evidence.stages += 1;
        (0, self.sequence, length as u64)
    }
    fn complete(&mut self, value: scheduler_frame) -> Result<*mut scheduler_frame, u64> {
        let action = value.data;
        if action > 2
            || value.register_10
                != if self.stage_kind == 0 {
                    0
                } else {
                    self.sequence
                }
            || (action != 0 && self.stage_kind != 0)
        {
            return Err(4);
        }
        if action == 2 && value.source_index != 0 {
            fail("veil_application_status");
        }
        // Validate both saved frames before any effect. The blocked caller's
        // stack/root remain owned; userspace cannot supply a return address.
        let caller_frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, caller_frame) {
            return Err(4);
        }
        // SAFETY: Exact retained caller frame remains pinned by pending authority.
        let caller_value = unsafe { ptr::read(caller_frame) };
        let bounds = self
            .tasks
            .snapshot(self.identities[0])
            .map_err(|_| 4u64)?
            .bounds();
        if !user_return_fields_are_valid_within_stack(caller_value, bounds)
            || !self.execution.admission(0).is_ok_and(|a| {
                a.accepts_return(caller_value.instruction_pointer, caller_value.stack_pointer)
            })
        {
            return Err(4);
        }
        let prepared = self
            .scheduler
            .prepare_call_reply(&mut self.tasks, &mut self.pending, &mut self.waiting)
            .map_err(|_| 4u64)?;
        let authority = prepared.authorization();
        if self.stage_kind == 1 {
            let sequence = self.channels[0].next_sequence().ok_or(7u64)?;
            let submission = self.channels[0]
                .prepare_submit(
                    self.channels[0].binding(),
                    self.identities[0],
                    sequence,
                    self.stage_length,
                )
                .map_err(veil_channel_status)?;
            // Preparation above plus exclusive channel ownership makes commit
            // infallible; no foreign task can run inside this exclusion interval.
            let receipt = self.channels[0]
                .commit_submit(submission, &self.stage_bytes[..self.stage_length])
                .unwrap_or_else(|_| fail("veil_submit_commit"));
            self.evidence.sends[0] += 1;
            if !(1..=7).contains(&receipt.sequence()) {
                fail("veil_surface_bound");
            }
            self.evidence.surface_hashes[receipt.sequence() as usize - 1] =
                packet_hash(&self.stage_bytes[..self.stage_length]);
        } else if self.stage_kind == 2 {
            let delivery = self.channels[1]
                .prepare_delivery(
                    self.channels[1].binding(),
                    self.identities[0],
                    self.stage_length,
                )
                .map_err(veil_channel_status)?;
            let receipt = self.channels[1]
                .consume_with(delivery, |bytes| {
                    self.execution.copy_to_suspended_caller(
                        prepared.tasks(),
                        &self.circles,
                        prepared.scheduler(),
                        authority,
                        self.stage_address,
                        bytes,
                    )
                })
                .map_err(veil_channel_status)?;
            self.last_ack[1] = receipt.sequence();
            self.evidence.deliveries[1] += 1;
            self.evidence.acknowledgements[1] += 1;
        } else if self.stage_kind == 3 {
            self.execution
                .copy_to_suspended_caller(
                    prepared.tasks(),
                    &self.circles,
                    prepared.scheduler(),
                    authority,
                    self.stage_address,
                    &self.stage_bytes[..self.stage_length],
                )
                .map_err(|_| 3u64)?;
        }
        #[cfg(feature = "linux_tls_test")]
        if self.stage_kind == 4 {
            self.execution
                .validate_suspended_caller(
                    prepared.tasks(),
                    &self.circles,
                    prepared.scheduler(),
                    authority,
                )
                .map_err(|_| 4u64)?;
            // SAFETY: Revalidated retained caller and exclusive prepared reply;
            // successful metadata commit is followed only by infallible reply.
            unsafe {
                self.tls.commit_suspended_fs(
                    prepared.tasks(),
                    prepared.scheduler(),
                    authority,
                    &mut self.tls_update,
                )
            }
            .map_err(|_| 4u64)?;
            self.veil_evidence.tls.commits += 1;
            if self.stage_address == super::super::linux_tls_fixture::full_width_base {
                self.veil_evidence.tls.full_width_commits += 1;
            }
        }
        // All fallible checks are complete. Reply wake/select commits without
        // failure after the effect, and RAX is the only changed caller register.
        if prepared.commit().task() != self.identities[0] {
            fail("veil_reply_peer");
        }
        unsafe {
            (*caller_frame).accumulator = value.source_index;
        }
        self.veil_evidence.completions += 1;
        self.clear_request();
        // The veil resumes this gate only on its next exact invocation.
        let veil_frame = self.execution.frame(2).unwrap_or_else(|e| fail(e));
        unsafe {
            (*veil_frame).accumulator = 0;
            (*veil_frame).data = 0;
            (*veil_frame).counter = 0;
        }
        let mut selected = self.select(0);
        if action == 1 && !self.terminal[1] {
            let transition = self
                .scheduler
                .preempt_current(&mut self.tasks, self.identities[0])
                .unwrap_or_else(|_| fail("veil_yield"));
            if transition.dispatch().task() != self.identities[1] {
                fail("veil_yield_peer");
            }
            self.evidence.switches += 1;
            selected = self.select(1);
        } else if action == 2 {
            selected = self.retire_current(0);
        }
        Ok(selected)
    }
    fn retire_veil(&mut self) {
        self.scheduler
            .retire_waiting_veil(&mut self.tasks, &mut self.waiting)
            .unwrap_or_else(|_| fail("veil_retire_wait"));
        #[cfg(feature = "linux_tls_test")]
        self.retire_tls(2);
        self.terminal[2] = true;
        self.evidence.exits += 1;
    }
    fn retire_current(&mut self, index: usize) -> *mut scheduler_frame {
        self.close_channels();
        self.terminal[index] = true;
        self.evidence.exits += 1;
        if index == 0 {
            self.retire_veil();
        }
        if self.terminal.iter().all(|v| *v) {
            self.scheduler
                .complete_current(&mut self.tasks, self.identities[index])
                .unwrap_or_else(|_| fail("veil_complete_last"));
            #[cfg(feature = "linux_tls_test")]
            self.retire_tls(index);
            self.finish();
        }
        let transition = self
            .scheduler
            .complete_current_and_dispatch(&mut self.tasks, self.identities[index])
            .unwrap_or_else(|_| fail("veil_complete"));
        #[cfg(feature = "linux_tls_test")]
        self.retire_tls(index);
        let peer = 1 - index;
        if transition.dispatch().task() != self.identities[peer] {
            fail("veil_complete_peer");
        }
        self.select(peer)
    }
}
fn veil_channel_status(error: channel_error) -> u64 {
    match error {
        channel_error::insufficient_capacity => 6,
        channel_error::sequence_exhausted => 7,
        e => channel_status(e),
    }
}

pub(super) fn gate(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    #[cfg(feature = "linux_tls_test")]
    {
        if !state.frame_valid(frame) {
            fail("linux_tls_gate_frame");
        }
        state.consume_tls(frame);
    }
    // SAFETY: Only native service/veil have an enabled native gate contract.
    if state.selected == 0
        || !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame)
        || state.evidence.calls >= max_calls
    {
        fail("veil_gate_frame");
    }
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector
        || value.error_code != 0
        || (value.accumulator != 0 && !state.started[state.selected])
    {
        fail("veil_gate_vector");
    }
    state.evidence.calls += 1;
    let mut selected = frame;
    // Native service convention is RAX status/RCX sequence/RDX length; veil
    // convention uses RAX status/RDX value/RCX aux. Keep this conversion explicit.
    let mut reply = (0, 0, 0);
    if value.accumulator == 0 {
        if state.started[state.selected]
            || value.flags != 2
            || value.stack_pointer != user_stack_top
            || !state
                .cpu
                .task_image(state.selected)
                .is_some_and(|i| i.is_initial_clean())
        {
            fail("veil_startup");
        }
        state.started[state.selected] = true;
        unsafe {
            (*frame).flags = 0x202;
        }
        if state.selected == 2 {
            reply = (0, 2, state.scenario as u64);
        }
    } else if state.selected == 2 {
        let operation = value.accumulator;
        if operation == 16 {
            if !state.authenticated_request(state.sequence) || value.source_index != 80 {
                reply.0 = 4;
            } else if state
                .execution
                .copy_to_user(
                    &state.tasks,
                    &state.circles,
                    &state.scheduler,
                    state.identities[2],
                    value.destination_index,
                    &state.request,
                )
                .is_err()
            {
                reply.0 = 3;
            } else {
                reply = (0, state.sequence, 80);
            }
        } else if !state.authenticated_request(value.destination_index) {
            reply.0 = 4;
        } else {
            reply = match operation {
                17 => state.query(value.source_index),
                18 => state.caller_read(value.source_index, value.data, value.register_10),
                19 => state.stage(
                    value.source_index,
                    value.data,
                    value.register_10,
                    value.register_8,
                ),
                #[cfg(feature = "linux_tls_test")]
                21 | 22 => state.tls_operation(value),
                20 => {
                    let before = state.effect_observation();
                    #[cfg(feature = "linux_tls_test")]
                    let tls_before = (
                        state.caller_fs(),
                        state.tls_update.is_pending(),
                        state.tls_update.storage_is_cleared(),
                    );
                    match state.complete(value) {
                        Ok(next) => {
                            selected = next;
                            (0, 0, 0)
                        }
                        Err(status) => {
                            #[cfg(feature = "linux_tls_test")]
                            if (
                                state.caller_fs(),
                                state.tls_update.is_pending(),
                                state.tls_update.storage_is_cleared(),
                            ) != tls_before
                            {
                                fail("linux_tls_rejected_effect_changed");
                            }
                            if state.effect_observation() != before {
                                fail("veil_rejected_effect_changed");
                            }
                            (status, 0, 0)
                        }
                    }
                }
                _ => (4, 0, 0),
            };
        }
        if reply.0 != 0 {
            state.veil_evidence.rejected += 1;
        }
    } else {
        match value.accumulator {
            1 | 2 | 3 | 6 => {
                let r = state.packet_call(value);
                reply = (r.0, r.2, r.1);
                match r.0 {
                    2 => state.evidence.denials += 1,
                    3 => state.evidence.bad_copies += 1,
                    4 => state.evidence.invalid += 1,
                    5 => state.evidence.closed += 1,
                    _ => {}
                }
            }
            4 => {
                if !state.terminal[0] {
                    let transition = state
                        .scheduler
                        .preempt_current(&mut state.tasks, state.identities[1])
                        .unwrap_or_else(|_| fail("veil_service_yield"));
                    if transition.dispatch().task() != state.identities[0] {
                        fail("veil_service_peer");
                    }
                    unsafe {
                        (*frame).accumulator = 0;
                        (*frame).counter = 0;
                        (*frame).data = 0;
                    }
                    selected = state.select(0);
                    state.evidence.switches += 1;
                }
            }
            5 => {
                if value.destination_index != 0 {
                    fail("veil_service_status");
                }
                selected = state.retire_current(1);
            }
            7 => {
                if state.scenario != 0
                    || !state.terminal[0]
                    || state.last_ack != [7, 6]
                    || value.destination_index != state.evidence.rows
                    || value.destination_index >= stage_height as u64
                    || value.data != stage_row_bytes as u64
                {
                    reply.0 = 4;
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
                    } else {
                        let start = state.evidence.rows as usize * stage_row_bytes;
                        // SAFETY: Sole service authority; complete copied row.
                        unsafe {
                            (&mut *display_stage.0.get())[start..start + stage_row_bytes]
                                .copy_from_slice(&row);
                        }
                        state.evidence.rows += 1;
                    }
                    row.fill(0);
                }
            }
            _ => {
                reply.0 = 2;
                state.evidence.denials += 1;
            }
        }
    }
    if selected == frame {
        // SAFETY: Current owned frame remains exclusively borrowed.
        unsafe {
            (*frame).accumulator = reply.0;
            (*frame).data = reply.1;
            (*frame).counter = reply.2;
        }
    }
    leave_dispatch(state, selected)
}

pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    #[cfg(feature = "linux_tls_test")]
    {
        if !state.frame_owned(frame.cast()) {
            fail("linux_tls_fault_frame");
        }
        state.consume_tls(frame.cast());
    }
    // SAFETY: Exclusive handler owns the interrupted task's eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.flags & 0x202 != 0x202
    {
        fail("veil_unexpected_fault");
    }
    if state.scenario == 1
        && state.selected == 0
        && snapshot.instruction_pointer == state.image.fault
        && state.pending.is_none()
        && state.evidence.sends == [1, 0]
        && state.evidence.deliveries == [0, 0]
    {
        #[cfg(feature = "linux_tls_test")]
        {
            state.observe_caller_fp();
        }
        state.close_channels();
        state.retire_veil();
        let transition = state
            .scheduler
            .fault_current(&mut state.tasks, state.identities[0])
            .unwrap_or_else(|_| fail("veil_producer_fault"));
        #[cfg(feature = "linux_tls_test")]
        state.retire_tls(0);
        if transition.dispatch().task() != state.identities[1] {
            fail("veil_fault_peer");
        }
        state.terminal[0] = true;
        state.evidence.faults += 1;
    } else if state.scenario == 2
        && state.selected == 2
        && snapshot.instruction_pointer == state.veil_fault
        && state.authenticated_request(state.sequence)
        && state.stage_kind
            == if cfg!(feature = "linux_tls_test") {
                4
            } else {
                1
            }
        && state.evidence.sends == [0, 0]
        && state.evidence.deliveries == [0, 0]
        && state.channels[0].phase() == channel_phase::empty
    {
        #[cfg(feature = "linux_tls_test")]
        if state.caller_fs() != Ok(0) || !state.tls_update.is_pending() {
            fail("linux_tls_abort_effect");
        }
        state.clear_request();
        #[cfg(feature = "linux_tls_test")]
        {
            if state.caller_fs() != Ok(0) || !state.tls_update.storage_is_cleared() {
                fail("linux_tls_abort_clear");
            }
        }
        state.veil_evidence.aborted += 1;
        let transition = state
            .scheduler
            .prepare_call_fault(&mut state.tasks, &mut state.pending)
            .unwrap_or_else(|_| fail("veil_dependency_fault"))
            .commit();
        #[cfg(feature = "linux_tls_test")]
        {
            state.retire_tls(0);
            state.retire_tls(2);
        }
        if transition.task() != state.identities[1] {
            fail("veil_dependency_peer");
        }
        state.close_channels();
        state.terminal[0] = true;
        state.terminal[2] = true;
        state.evidence.faults += 2;
        if state.evidence.discarded != 0 {
            fail("veil_abort_published");
        }
    } else {
        fail("veil_fault_site");
    }
    let selected = state.select(1);
    Some(leave_dispatch(state, selected).cast())
}

pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("veil_initial_state");
    }
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
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
        let native = elf_profile::application_images(elf_profile::expected_plan())[0];
        #[cfg(not(feature = "linux_tls_test"))]
        let linux = linux_profile::application_image(linux_profile::expected_plan());
        #[cfg(not(feature = "linux_tls_test"))]
        let veil = veil_image();
        #[cfg(feature = "linux_tls_test")]
        let (linux, veil) = tls_proof::images();
        let (vendor, normal) = run_pair(
            allocator,
            [native, native],
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        let patch = presenter
            .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                &*display_stage.0.get()
            })
            .unwrap_or_else(|_| fail("veil_baseline_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
            fail("veil_baseline_pixels");
        }
        clear_stage();
        let presented = framebuffer_hash(info, false);
        let (fault_vendor, faulted) = run_pair(
            allocator,
            [native, native],
            kernel_cr3,
            bootstrap_stack,
            1,
            initial,
        );
        if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
            fail("veil_baseline_fault");
        }
        clear_stage();
        let mut records = [(evidence::default(), veil_evidence::default()); 3];
        for (scenario, record) in records.iter_mut().enumerate() {
            let (current_vendor, e, v) = run_three(
                allocator,
                [linux, native, veil],
                kernel_cr3,
                bootstrap_stack,
                scenario,
                initial,
            );
            if current_vendor != vendor {
                fail("veil_cpu_comparison");
            }
            if scenario == 0 {
                if e.surface_hashes != normal.surface_hashes
                    || e.sends != [7, 6]
                    || e.deliveries != [7, 6]
                    || e.acknowledgements != [7, 6]
                    || e.rows != 24
                    || e.faults != 0
                    || e.exits != 3
                    || e.discarded != 0
                    || v.raw_calls != v.completions
                {
                    fail("veil_editor_comparison");
                }
                let patch = presenter
                    .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                        &*display_stage.0.get()
                    })
                    .unwrap_or_else(|_| fail("veil_patch"));
                if !patch.matches_pixels() || !patch.commit_and_verify() {
                    fail("veil_pixel_comparison");
                }
            } else if scenario == 1 {
                if e.sends != [1, 0]
                    || e.deliveries != [0, 0]
                    || e.faults != 1
                    || e.exits != 2
                    || e.discarded != 1
                    || e.rows != 0
                    || v.raw_calls != v.completions
                {
                    fail("veil_producer_containment");
                }
            } else if e.sends != [0, 0]
                || e.deliveries != [0, 0]
                || e.faults != 2
                || e.exits != 1
                || e.discarded != 0
                || e.rows != 0
                || v.aborted != 1
                || v.raw_calls != v.completions + 1
            {
                fail("veil_dependency_containment");
            }
            clear_stage();
            if framebuffer_hash(info, true) != outside || framebuffer_hash(info, false) != presented
            {
                fail("veil_preserved_pixels");
            }
            *record = (e, v);
        }
        (initial, vendor, [normal, faulted], records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("veil_final_interrupts");
    }
    emit_evidence(initial, vendor, baseline);
    #[cfg(feature = "linux_tls_test")]
    tls_proof::emit(records);
    #[cfg(not(feature = "linux_tls_test"))]
    {
        let mut serial = serial_port::com1();
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.veil_application_test.ready","level":"info","tasks":3,"scenarios":3,"data_frames":48,"table_frames":27,"leases":15,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture"}}"#
        );
        for (scenario, (e, v)) in records.into_iter().enumerate() {
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.veil_application_test.scenario","level":"info","scenario":{},"raw_calls":{},"completions":{},"stages":{},"rejected":{},"aborted":{},"cooperative_switches":{},"timer_ticks":{},"surface_sends":{},"input_sends":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"discarded":{},"exits":{},"faults":{},"rows":{}}}"#,
                scenario,
                v.raw_calls,
                v.completions,
                v.stages,
                v.rejected,
                v.aborted,
                e.switches,
                e.timer_ticks,
                e.sends[0],
                e.sends[1],
                e.deliveries[0],
                e.deliveries[1],
                e.acknowledgements[0],
                e.acknowledgements[1],
                e.discarded,
                e.exits,
                e.faults,
                e.rows
            );
        }
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.veil_application_test.passed","level":"info","application_tasks":9,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true}}"#
        );
    }
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
