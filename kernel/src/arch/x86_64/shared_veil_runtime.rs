//! One native veil serves two sealed callers over independent application routes.
//! Linux numbers, descriptors, errno and poll bytes are opaque to this owner.
use super::super::owned_execution::owned_execution_group;
use super::*;
use crate::scheduler::{
    scheduler_call_authorization, scheduler_call_service, scheduler_veil_wait_authorization,
};

const veil_index: usize = 4;
const task_count: usize = 5;
use core::sync::atomic::AtomicU64;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
static next_request: AtomicU64 = AtomicU64::new(1);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
// SAFETY: Sole BSP, IF clear. Storage remains reserved until explicit teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

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
}
struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; task_count],
    execution: owned_execution_group<task_count>,
    cpu: application_cpu::cpu_session_group<task_count>,
    channels: [[application_channel; 2]; 2],
    last_ack: [[u64; 2]; 2],
    image: artifact,
    veil_fault: u64,
    scenario: usize,
    selected: usize,
    started: [bool; task_count],
    terminal: [bool; task_count],
    evidence: [evidence; 2],
    veil_evidence: [veil_evidence; 2],
    service: Option<scheduler_call_service>,
    request_pair: usize,
    previous_pair: Option<usize>,
    previous_sequence: u64,
    shared_evidence: shared_evidence,
    row_hashes: [u64; stage_height],
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

#[derive(Clone, Copy, Default)]
struct shared_evidence {
    cross_handoffs: u64,
    stale_rejected: u64,
    peer_checks: u64,
    member_retirements: u64,
    veil_exits: u64,
    veil_faults: u64,
}

impl runtime {
    fn pair(&self) -> usize {
        if self.selected == veil_index {
            self.request_pair
        } else {
            self.selected / 2
        }
    }
    fn index_of(&self, identity: task_identity) -> usize {
        self.identities
            .iter()
            .position(|id| *id == identity)
            .unwrap_or_else(|| fail("shared_dispatch_identity"))
    }
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
            if !unsafe { self.cpu.select_linux_entry(stack, matches!(index, 0 | 2)) }
                || !self.cpu.linux_entry_matches(stack, matches!(index, 0 | 2))
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
    fn close_channels(&mut self, pair: usize) {
        for channel in &mut self.channels[pair] {
            let receipt = channel
                .close(channel.binding())
                .unwrap_or_else(|_| fail("channel_close"));
            self.evidence[pair].discarded += u64::from(receipt.discarded());
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
        let pair = self.selected / 2;
        let binding = self.channels[pair][index].binding();
        let caller = self.identities[self.selected];
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        match operation {
            1 => {
                let prepared = match self.channels[pair][index]
                    .prepare_submit(binding, caller, sequence, length)
                {
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
                let receipt = self.channels[pair][index]
                    .commit_submit(prepared, &scratch[..length])
                    .unwrap_or_else(|_| fail("submit_commit"));
                scratch.fill(0);
                self.evidence[pair].sends[index] += 1;
                if index == 0 {
                    if !(1..=7).contains(&receipt.sequence()) {
                        fail("surface_sequence_bound");
                    }
                    self.evidence[pair].surface_hashes[receipt.sequence() as usize - 1] = hash;
                }
                (0, receipt.sequence(), receipt.length() as u64)
            }
            2 => {
                let prepared =
                    match self.channels[pair][index].prepare_delivery(binding, caller, length) {
                        Ok(p) => p,
                        Err(e) => return (channel_status(e), 0, 0),
                    };
                let mut hash = 0;
                let result = self.channels[pair][index].deliver_with(prepared, |bytes| {
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
                            && self.evidence[pair].surface_hashes[receipt.sequence() as usize - 1]
                                != hash
                        {
                            fail("copied_snapshot_changed");
                        }
                        self.evidence[pair].deliveries[index] += 1;
                        (0, receipt.sequence(), receipt.length() as u64)
                    }
                    Err(error) => (channel_status(error), 0, 0),
                }
            }
            3 => match self.channels[pair][index].acknowledge(binding, caller, sequence) {
                Ok(receipt) => {
                    self.last_ack[pair][index] = receipt.sequence();
                    self.evidence[pair].acknowledgements[index] += 1;
                    (0, receipt.sequence(), 0)
                }
                Err(error) => (channel_status(error), 0, 0),
            },
            6 => {
                if caller != binding.producer() {
                    return (2, 0, 0);
                }
                if self.channels[pair][index].phase() == channel_phase::closed {
                    return (5, 0, 0);
                }
                if sequence == 0
                    || self.channels[pair][index]
                        .next_sequence()
                        .is_none_or(|next| sequence >= next)
                {
                    return (4, 0, 0);
                }
                if self.last_ack[pair][index] == sequence {
                    (0, sequence, 0)
                } else {
                    (1, 0, 0)
                }
            }
            _ => (4, 0, 0),
        }
    }
}

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
        .unwrap_or_else(|| fail("shared_veil_fault_marker")) as usize;
    if rx.get(offset..offset.saturating_add(2)) != Some(&[0x0f, 0x0b]) {
        fail("shared_veil_fault_marker");
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

// Keep constructor temporaries out of the frame that performs mapping preparation.
// No typed runtime reference exists until initialize finishes the remaining fields.
#[inline(never)]
unsafe fn initialize_storage(
    pointer: *mut runtime,
    image: artifact,
    veil_fault: u64,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    row_hashes: [u64; stage_height],
) {
    // SAFETY: Caller holds the sole storage latch over this uninitialized or fully
    // retired BSS. Every pointer addresses its own field; identities and channels
    // deliberately remain uninitialized until preparation commits their owners.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler).write(
            scheduler_table::new(task_count).unwrap_or_else(|_| fail("shared_veil_scheduler")),
        );
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution_group::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session_group::new());
        ptr::addr_of_mut!((*pointer).last_ack).write([[0; 2]; 2]);
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).veil_fault).write(veil_fault);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([true, false, true, false, false]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; task_count]);
        ptr::addr_of_mut!((*pointer).evidence).write([evidence::default(); 2]);
        ptr::addr_of_mut!((*pointer).veil_evidence).write([veil_evidence::default(); 2]);
        ptr::addr_of_mut!((*pointer).service).write(None);
        ptr::addr_of_mut!((*pointer).request_pair).write(0);
        ptr::addr_of_mut!((*pointer).previous_pair).write(None);
        ptr::addr_of_mut!((*pointer).previous_sequence).write(0);
        ptr::addr_of_mut!((*pointer).shared_evidence).write(shared_evidence::default());
        ptr::addr_of_mut!((*pointer).row_hashes).write(row_hashes);
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
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator,
    images: [artifact; task_count],
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    row_hashes: [u64; stage_height],
) -> &'static mut runtime {
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("shared_veil_storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    // SAFETY: Exclusive uninitialized/fully retired BSS. Every field is written
    // before a typed reference exists; owners are never moved after activation.
    unsafe {
        initialize_storage(
            pointer,
            images[0],
            images[veil_index].fault,
            scenario,
            kernel_cr3,
            bootstrap_stack,
            row_hashes,
        );
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
                [
                    user_stack_top - 256,
                    user_stack_top,
                    user_stack_top - 256,
                    user_stack_top,
                    user_stack_top,
                ],
            )
            .unwrap_or_else(|e| fail(e));
        for (index, image) in images.into_iter().enumerate() {
            for offset in (0..8192).step_by(8) {
                if execution.read_data(index, offset)
                    != Some(elf_profile::data_word(image, offset, 0))
                    || execution.read_stack(index, offset) != Some(0)
                {
                    fail("shared_veil_initial_storage");
                }
            }
            if matches!(index, 0 | 2) {
                linux_profile::populate_stack(
                    execution,
                    index,
                    usize::from(scenario == 1 && index == 0),
                );
            } else {
                execution
                    .write_data(index, 0, if index == veil_index { 2 } else { 1 })
                    .unwrap_or_else(|e| fail(e));
                let fixture_scenario = if index == veil_index {
                    if scenario == 2 { 2 } else { 0 }
                } else {
                    u64::from(scenario == 2 || (scenario == 1 && index == 1))
                };
                execution
                    .write_data(index, 8, fixture_scenario)
                    .unwrap_or_else(|e| fail(e));
            }
        }
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 501 + index as u64)
                .unwrap_or_else(|e| fail(e))
        });
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        for pair in 0..2 {
            for index in 0..2 {
                let binding = application_channel_binding::try_new(
                    endpoint_id::try_from_parts(pair * 2 + index, scenario as u64 + 1).unwrap(),
                    identities[pair * 2 + index],
                    identities[pair * 2 + 1 - index],
                )
                .unwrap_or_else(|_| fail("shared_channel_binding"));
                ptr::addr_of_mut!((*pointer).channels)
                    .cast::<application_channel>()
                    .add(pair * 2 + index)
                    .write(
                        application_channel::try_new(binding)
                            .unwrap_or_else(|_| fail("shared_channel")),
                    );
            }
        }
        &mut *pointer
    }
}

#[inline(never)]
fn run_five(
    allocator: &mut frame_allocator,
    images: [artifact; task_count],
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
    row_hashes: [u64; stage_height],
) -> (
    &'static str,
    [evidence; 2],
    [veil_evidence; 2],
    shared_evidence,
) {
    if is_active()
        || super::is_active()
        || interrupts::interrupts_enabled()
        || paging::read_cr3() != kernel_cr3
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || !stage_is_clear()
    {
        fail("shared_veil_setup");
    }
    let state = initialize(
        allocator,
        images,
        scenario,
        kernel_cr3,
        bootstrap_stack,
        row_hashes,
    );
    for id in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, id)
            .unwrap_or_else(|_| fail("shared_veil_admit"));
    }
    state.service = Some(
        state
            .scheduler
            .register_call_service(
                &state.tasks,
                state.identities[veil_index],
                [state.identities[0], state.identities[2]],
            )
            .unwrap_or_else(|_| fail("shared_service_register")),
    );
    // SAFETY: All five owners are initialized at their final address and retained.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let accounting = allocator.accounting();
    if accounting.free.checked_add(125) != Some(initial.free)
        || accounting.data_owned != 80
        || accounting.table_owned != 45
        || accounting.active_leases != 25
        || accounting.bound_leases != 25
    {
        fail("shared_veil_live_ownership");
    }
    for index in 0..task_count {
        for other in 0..index {
            if state.execution.root(index).ok() == state.execution.root(other).ok()
                || state.execution.transition_stack_top(index).ok()
                    == state.execution.transition_stack_top(other).ok()
            {
                fail("shared_veil_shared_owner");
            }
        }
    }
    // SAFETY: Stable CPU storage and all CPU/entry controls remain retained.
    unsafe { state.cpu.prepare_linux() }.unwrap_or_else(|e| fail(e));
    if (0..task_count).any(|index| {
        !state
            .cpu
            .task_image(index)
            .is_some_and(|image| image.is_initial_clean())
    }) {
        fail("shared_veil_initial_cpu_images");
    }
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("shared_veil_start"));
    if first.task() != state.identities[0] || first.context() != scheduler_context::initial {
        fail("shared_veil_first");
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
        fail("shared_veil_publication");
    }
    // SAFETY: Only IF-clear handlers borrow the stable runtime until continuation.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("shared_veil_initial_cpu");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
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
        || state.terminal != [true; task_count]
        || state.started != [true; task_count]
        || state.pending.is_some()
        || state.waiting.is_some()
        || state.sequence != 0
        || !state
            .service
            .as_ref()
            .is_some_and(|service| service.is_revoked())
        || state.stage_kind != 0
        || state.stage_bytes.iter().any(|b| *b != 0)
        || state.audit_bytes.iter().any(|b| *b != 0)
        || state
            .channels
            .iter()
            .flatten()
            .any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        fail("shared_veil_continuation");
    }
    for (index, identity) in state.identities.into_iter().enumerate() {
        let expected = if (scenario == 1 && index == 0)
            || (scenario == 2 && matches!(index, 0 | 2 | veil_index))
        {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|s| s.state() == expected)
        {
            fail("shared_veil_terminal");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("shared_veil_withdraw"));
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
        .unwrap_or_else(|_| fail("shared_veil_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
    {
        fail("shared_veil_final_accounting");
    }
    let result = (
        state.cpu.cpu_vendor(),
        state.evidence,
        state.veil_evidence,
        state.shared_evidence,
    );
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
        fail("shared_veil_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("shared_veil_inactive");
    }
    // SAFETY: Sole IF-clear handler owns the live stable runtime until latch release.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // SAFETY: Selected task owns its stable eager CPU image.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("shared_veil_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = enter_dispatch();
    let pair = state.pair();
    // SAFETY: Exclusive handler owns stable eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.evidence[pair].timer_ticks >= max_calls
    {
        fail("shared_timer");
    }
    state.last_tick = observed.tick();
    state.evidence[pair].timer_ticks += 1;
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn syscall(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    let caller = state.selected;
    let pair = caller / 2;
    // SAFETY: Hardware entry switched from untrusted RSP to this owned stack.
    if !matches!(caller, 0 | 2)
        || state.sequence != 0
        || state.pending.is_some()
        || state.stage_kind != 0
        || !unsafe { state.cpu.save_current(caller) }
        || !state.frame_valid(frame)
        || state.veil_evidence[pair].raw_calls >= max_calls
        || !state
            .service
            .as_ref()
            .is_some_and(|service| service.contains(state.identities[caller]))
    {
        fail("shared_raw_frame");
    }
    // SAFETY: Complete retained frame checked above; no syscall interpretation.
    let value = unsafe { ptr::read(frame) };
    if !super::super::linux_syscall::entry_frame_matches_hardware(value) {
        fail("shared_raw_hardware");
    }
    let sequence = next_request
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            if v == 0 { None } else { v.checked_add(1) }
        })
        .unwrap_or_else(|_| fail("shared_request_exhausted"));
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
    .unwrap_or_else(|_| fail("shared_request_geometry"))
    .encode();
    let prepared = state
        .scheduler
        .prepare_service_call_handoff(
            &mut state.tasks,
            state.identities[caller],
            state.service.as_ref().unwrap(),
            &mut state.waiting,
            &mut state.pending,
        )
        .unwrap_or_else(|_| fail("shared_handoff_prepare"));
    state.sequence = sequence;
    state.request = request;
    state.request_pair = pair;
    if prepared.commit().task() != state.identities[veil_index] {
        fail("shared_handoff_peer");
    }
    state.veil_evidence[pair].raw_calls += 1;
    let selected = state.select(veil_index);
    if state.previous_pair.is_some_and(|previous| previous != pair) {
        state.shared_evidence.cross_handoffs += 1;
        // Artifact stays byte-identical. These are privileged proof proposals
        // through the same dispatch method as its gate, with the former caller's
        // completed request and stage token; they are not userspace probes.
        let peer = state.peer_observation(pair);
        let effect = state.effect_observation();
        for operation in [17, 20] {
            let mut stale = value;
            stale.accumulator = operation;
            stale.destination_index = state.previous_sequence;
            stale.source_index = 0;
            stale.data = 0;
            stale.register_10 = state.previous_sequence;
            let (reply, next) = state.veil_call(stale);
            if reply != (4, 0, 0)
                || next.is_some()
                || state.effect_observation() != effect
                || state.peer_observation(pair) != peer
            {
                fail("shared_cross_caller_stale_effect");
            }
            state.shared_evidence.stale_rejected += 1;
        }
    }
    state.previous_pair = Some(pair);
    state.previous_sequence = sequence;
    leave_dispatch(state, selected)
}

impl runtime {
    fn authenticated_request(&self, sequence: u64) -> bool {
        self.selected == veil_index
            && sequence != 0
            && sequence == self.sequence
            && self.pending.as_ref().is_some_and(|authority| {
                authority.caller() == self.identities[self.request_pair * 2]
                    && authority.veil() == self.identities[veil_index]
                    && self
                        .service
                        .as_ref()
                        .is_some_and(|service| service.contains(authority.caller()))
                    && self
                        .execution
                        .validate_suspended_caller(
                            &self.tasks,
                            &self.circles,
                            &self.scheduler,
                            authority,
                        )
                        .is_ok()
            })
    }
    // Observe the other application's actual retained storage, never via a
    // userspace-supplied task/root. Timer and cooperative peers cannot run inside
    // this IF-clear gate. Hash comparison is evidence, not copy authorization.
    fn peer_observation(&self, pair: usize) -> [u64; 24] {
        let other = 1 - pair;
        let mut observed = [0; 24];
        for slot in 0..2 {
            let index = other * 2 + slot;
            let frame = self.execution.frame(index).unwrap_or_else(|e| fail(e));
            if !self.execution.frame_owned(index, frame) {
                fail("shared_peer_frame");
            }
            // SAFETY: Complete frame is inside this retained owner's transition stack.
            observed[slot * 3] = packet_hash(unsafe {
                core::slice::from_raw_parts(
                    frame.cast::<u8>(),
                    core::mem::size_of::<scheduler_frame>(),
                )
            });
            let [data, stack] = self
                .execution
                .proof_private_hashes(index)
                .unwrap_or_else(|| fail("shared_peer_storage"));
            observed[slot * 3 + 1] = data;
            observed[slot * 3 + 2] = stack;
        }
        for index in 0..2 {
            let channel = &self.channels[other][index];
            observed[6 + index * 5] = channel.phase() as u64;
            observed[7 + index * 5] = channel.queued_length() as u64;
            observed[8 + index * 5] = channel.next_sequence().unwrap_or(0);
            observed[9 + index * 5] = channel.proof_storage_hash();
            observed[10 + index * 5] = self.last_ack[other][index];
        }
        let e = self.evidence[other];
        observed[16..].copy_from_slice(&[
            e.sends[0],
            e.sends[1],
            e.deliveries[0],
            e.deliveries[1],
            e.acknowledgements[0],
            e.acknowledgements[1],
            e.rows,
            e.discarded,
        ]);
        observed
    }
    fn veil_call(
        &mut self,
        value: scheduler_frame,
    ) -> ((u64, u64, u64), Option<*mut scheduler_frame>) {
        if value.accumulator == 16 {
            let reply = if !self.authenticated_request(self.sequence) || value.source_index != 80 {
                (4, 0, 0)
            } else if self
                .execution
                .copy_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[veil_index],
                    value.destination_index,
                    &self.request,
                )
                .is_err()
            {
                (3, 0, 0)
            } else {
                (0, self.sequence, 80)
            };
            return (reply, None);
        }
        if !self.authenticated_request(value.destination_index) {
            return ((4, 0, 0), None);
        }
        let reply = match value.accumulator {
            17 => self.query(value.source_index),
            18 => self.caller_read(value.source_index, value.data, value.register_10),
            19 => self.stage(
                value.source_index,
                value.data,
                value.register_10,
                value.register_8,
            ),
            20 => {
                let before = self.effect_observation();
                match self.complete(value) {
                    Ok(next) => return ((0, 0, 0), Some(next)),
                    Err(status) => {
                        if self.effect_observation() != before {
                            fail("shared_rejected_effect_changed");
                        }
                        (status, 0, 0)
                    }
                }
            }
            _ => (4, 0, 0),
        };
        (reply, None)
    }
    // Proof observation around rejected native completions. The destination is
    // inspected while its caller is still blocked, before an eventual valid
    // completion could conceal an early copyout. This grants no userspace data.
    fn effect_observation(&mut self) -> [u64; 16] {
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
                .unwrap_or_else(|_| fail("shared_veil_effect_observation"));
            destination = packet_hash(&self.audit_bytes[..self.stage_length]);
            self.audit_bytes.fill(0);
        }
        let frame = self
            .execution
            .frame(self.request_pair * 2)
            .unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(self.request_pair * 2, frame) {
            fail("shared_veil_observation_frame");
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
            self.channels[self.request_pair][0].phase() as u64,
            self.channels[self.request_pair][0].queued_length() as u64,
            self.channels[self.request_pair][0]
                .next_sequence()
                .unwrap_or(0),
            self.channels[self.request_pair][1].phase() as u64,
            self.channels[self.request_pair][1].queued_length() as u64,
            self.channels[self.request_pair][1]
                .next_sequence()
                .unwrap_or(0),
            packet_hash(&self.request),
            self.channels[self.request_pair][0].proof_storage_hash(),
            self.channels[self.request_pair][1].proof_storage_hash(),
        ]
    }
    fn clear_request(&mut self) {
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
        let Some(channel) = self.channels[self.request_pair].get(index) else {
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
        .unwrap_or_else(|_| fail("shared_veil_snapshot"));
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
                    self.identities[veil_index],
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
            if self.channels[self.request_pair][0].phase() == channel_phase::closed {
                return (5, 0, 0);
            }
            let Some(sequence) = self.channels[self.request_pair][0].next_sequence() else {
                return (7, 0, 0);
            };
            if let Err(e) = self.channels[self.request_pair][0].prepare_submit(
                self.channels[self.request_pair][0].binding(),
                self.identities[self.request_pair * 2],
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
                if let Err(e) = self.channels[self.request_pair][1].prepare_delivery(
                    self.channels[self.request_pair][1].binding(),
                    self.identities[self.request_pair * 2],
                    length,
                ) {
                    return (veil_channel_status(e), 0, 0);
                }
                length = self.channels[self.request_pair][1].queued_length();
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
                        self.identities[veil_index],
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
        self.veil_evidence[self.request_pair].stages += 1;
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
            fail("shared_veil_application_status");
        }
        // Validate both saved frames before any effect. The blocked caller's
        // stack/root remain owned; userspace cannot supply a return address.
        let caller_frame = self
            .execution
            .frame(self.request_pair * 2)
            .unwrap_or_else(|e| fail(e));
        if !self
            .execution
            .frame_owned(self.request_pair * 2, caller_frame)
        {
            return Err(4);
        }
        // SAFETY: Exact retained caller frame remains pinned by pending authority.
        let caller_value = unsafe { ptr::read(caller_frame) };
        let bounds = self
            .tasks
            .snapshot(self.identities[self.request_pair * 2])
            .map_err(|_| 4u64)?
            .bounds();
        if !user_return_fields_are_valid_within_stack(caller_value, bounds)
            || !self
                .execution
                .admission(self.request_pair * 2)
                .is_ok_and(|a| {
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
            let sequence = self.channels[self.request_pair][0]
                .next_sequence()
                .ok_or(7u64)?;
            let submission = self.channels[self.request_pair][0]
                .prepare_submit(
                    self.channels[self.request_pair][0].binding(),
                    self.identities[self.request_pair * 2],
                    sequence,
                    self.stage_length,
                )
                .map_err(veil_channel_status)?;
            // Preparation above plus exclusive channel ownership makes commit
            // infallible; no foreign task can run inside this exclusion interval.
            let receipt = self.channels[self.request_pair][0]
                .commit_submit(submission, &self.stage_bytes[..self.stage_length])
                .unwrap_or_else(|_| fail("shared_veil_submit_commit"));
            self.evidence[self.request_pair].sends[0] += 1;
            if !(1..=7).contains(&receipt.sequence()) {
                fail("shared_veil_surface_bound");
            }
            self.evidence[self.request_pair].surface_hashes[receipt.sequence() as usize - 1] =
                packet_hash(&self.stage_bytes[..self.stage_length]);
        } else if self.stage_kind == 2 {
            let delivery = self.channels[self.request_pair][1]
                .prepare_delivery(
                    self.channels[self.request_pair][1].binding(),
                    self.identities[self.request_pair * 2],
                    self.stage_length,
                )
                .map_err(veil_channel_status)?;
            let receipt = self.channels[self.request_pair][1]
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
            self.last_ack[self.request_pair][1] = receipt.sequence();
            self.evidence[self.request_pair].deliveries[1] += 1;
            self.evidence[self.request_pair].acknowledgements[1] += 1;
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
        // All fallible checks are complete. Reply wake/select commits without
        // failure after the effect, and RAX is the only changed caller register.
        if prepared.commit().task() != self.identities[self.request_pair * 2] {
            fail("shared_veil_reply_peer");
        }
        unsafe {
            (*caller_frame).accumulator = value.source_index;
        }
        self.veil_evidence[self.request_pair].completions += 1;
        self.clear_request();
        // The veil resumes this gate only on its next exact invocation.
        let veil_frame = self.execution.frame(veil_index).unwrap_or_else(|e| fail(e));
        unsafe {
            (*veil_frame).accumulator = 0;
            (*veil_frame).data = 0;
            (*veil_frame).counter = 0;
        }
        let caller = self.request_pair * 2;
        let mut selected = self.select(caller);
        if action == 1 && self.application_peer_runnable() {
            let transition = self
                .scheduler
                .preempt_current(&mut self.tasks, self.identities[caller])
                .unwrap_or_else(|_| fail("shared_yield"));
            let next = self.index_of(transition.dispatch().task());
            if next == veil_index {
                fail("shared_veil_queued");
            }
            self.evidence[self.request_pair].switches += 1;
            selected = self.select(next);
        } else if action == 2 {
            selected = self.retire_current(caller);
        }
        Ok(selected)
    }
    fn application_peer_runnable(&self) -> bool {
        self.scheduler
            .snapshot(&self.tasks)
            .is_ok_and(|snapshot| snapshot.runnable() != 0)
    }
    fn retire_member(&mut self, index: usize) {
        self.scheduler
            .retire_service_caller(
                &self.tasks,
                self.service.as_mut().unwrap(),
                self.identities[index],
            )
            .unwrap_or_else(|_| fail("shared_member_retire"));
        self.shared_evidence.member_retirements += 1;
        if self.service.as_ref().unwrap().is_empty() {
            self.scheduler
                .retire_service_veil(
                    &mut self.tasks,
                    self.service.as_ref().unwrap(),
                    &mut self.waiting,
                )
                .unwrap_or_else(|_| fail("shared_veil_retire"));
            self.terminal[veil_index] = true;
            self.shared_evidence.veil_exits += 1;
            self.scheduler
                .retire_call_service(&self.tasks, self.service.as_mut().unwrap())
                .unwrap_or_else(|_| fail("shared_service_revoke"));
        }
    }
    fn retire_current(&mut self, index: usize) -> *mut scheduler_frame {
        self.close_channels(index / 2);
        self.terminal[index] = true;
        self.evidence[index / 2].exits += 1;
        if self.terminal.iter().all(|v| *v) {
            self.scheduler
                .complete_current(&mut self.tasks, self.identities[index])
                .unwrap_or_else(|_| fail("shared_complete_last"));
            self.finish();
        }
        let transition = self
            .scheduler
            .complete_current_and_dispatch(&mut self.tasks, self.identities[index])
            .unwrap_or_else(|_| fail("shared_complete"));
        if matches!(index, 0 | 2) {
            self.retire_member(index);
        }
        let next = self.index_of(transition.dispatch().task());
        if next == veil_index {
            fail("shared_complete_veil");
        }
        self.select(next)
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
    let pair = state.pair();
    let role = state.selected;
    // SAFETY: Only native services/veil have the enabled native gate contract.
    if matches!(role, 0 | 2)
        || !unsafe { state.cpu.save_current(role) }
        || !state.frame_valid(frame)
        || state.evidence[pair].calls >= max_calls
    {
        fail("shared_gate_frame");
    }
    // SAFETY: Complete retained frame validated before reading.
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector
        || value.error_code != 0
        || (value.accumulator != 0 && !state.started[role])
    {
        fail("shared_gate_vector");
    }
    state.evidence[pair].calls += 1;
    let mut selected = frame;
    // Native service RAX status/RCX sequence/RDX length; veil RAX status/RDX value/RCX aux.
    let mut reply = (0, 0, 0);
    if value.accumulator == 0 {
        if state.started[role]
            || value.flags != 2
            || value.stack_pointer != user_stack_top
            || !state
                .cpu
                .task_image(role)
                .is_some_and(|i| i.is_initial_clean())
        {
            fail("shared_startup");
        }
        state.started[role] = true;
        // SAFETY: Current exact startup frame exclusively owned by this handler.
        unsafe {
            (*frame).flags = 0x202;
        }
        if role == veil_index {
            reply = (0, 2, if state.scenario == 2 { 2 } else { 0 });
        }
    } else if role == veil_index {
        let peer = state.peer_observation(pair);
        let (result, next) = state.veil_call(value);
        reply = result;
        if let Some(next) = next {
            selected = next;
        }
        if state.peer_observation(pair) != peer {
            fail("shared_foreign_effect");
        }
        state.shared_evidence.peer_checks += 1;
        if reply.0 != 0 {
            state.veil_evidence[pair].rejected += 1;
        }
    } else {
        match value.accumulator {
            1 | 2 | 3 | 6 => {
                let r = state.packet_call(value);
                reply = (r.0, r.2, r.1);
                match r.0 {
                    2 => state.evidence[pair].denials += 1,
                    3 => state.evidence[pair].bad_copies += 1,
                    4 => state.evidence[pair].invalid += 1,
                    5 => state.evidence[pair].closed += 1,
                    _ => {}
                }
            }
            4 => {
                if state.application_peer_runnable() {
                    let transition = state
                        .scheduler
                        .preempt_current(&mut state.tasks, state.identities[role])
                        .unwrap_or_else(|_| fail("shared_service_yield"));
                    let next = state.index_of(transition.dispatch().task());
                    if next == veil_index {
                        fail("shared_service_veil_queued");
                    }
                    // SAFETY: Native yield reply belongs only to this retained frame.
                    unsafe {
                        (*frame).accumulator = 0;
                        (*frame).counter = 0;
                        (*frame).data = 0;
                    }
                    selected = state.select(next);
                    state.evidence[pair].switches += 1;
                }
            }
            5 => {
                if value.destination_index != 0 {
                    fail("shared_service_status");
                }
                selected = state.retire_current(role);
            }
            7 => {
                let normal_pair = state.scenario == 0 || (state.scenario == 1 && pair == 1);
                if !normal_pair
                    || !state.terminal[pair * 2]
                    || state.last_ack[pair] != [7, 6]
                    || value.destination_index != state.evidence[pair].rows
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
                            state.identities[role],
                            value.source_index,
                            &mut row,
                        )
                        .is_err()
                    {
                        reply.0 = 3;
                    } else {
                        let index = state.evidence[pair].rows as usize;
                        if packet_hash(&row) != state.row_hashes[index] {
                            fail("shared_row_comparison");
                        }
                        let start = index * stage_row_bytes;
                        // SAFETY: Complete copied row equals the native baseline; both
                        // isolated services may publish identical rows into this one stage.
                        unsafe {
                            (&mut *display_stage.0.get())[start..start + stage_row_bytes]
                                .copy_from_slice(&row);
                        }
                        state.evidence[pair].rows += 1;
                    }
                    row.fill(0);
                }
            }
            _ => {
                reply.0 = 2;
                state.evidence[pair].denials += 1;
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
    // SAFETY: Exclusive handler owns interrupted task's eager CPU image.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.flags & 0x202 != 0x202
    {
        fail("shared_unexpected_fault");
    }
    let next;
    if state.scenario == 1
        && state.selected == 0
        && snapshot.instruction_pointer == state.image.fault
        && state.pending.is_none()
        && state.evidence[0].sends == [1, 0]
        && state.evidence[0].deliveries == [0, 0]
    {
        let peer = state.peer_observation(0);
        state.close_channels(0);
        let transition = state
            .scheduler
            .fault_current(&mut state.tasks, state.identities[0])
            .unwrap_or_else(|_| fail("shared_producer_fault"));
        next = state.index_of(transition.dispatch().task());
        if next != 1 {
            fail("shared_producer_fault_peer");
        }
        state.terminal[0] = true;
        state.evidence[0].faults += 1;
        state.retire_member(0);
        if state.peer_observation(0) != peer
            || !state
                .service
                .as_ref()
                .unwrap()
                .contains(state.identities[2])
        {
            fail("shared_survivor_changed");
        }
    } else if state.scenario == 2
        && state.selected == veil_index
        && snapshot.instruction_pointer == state.veil_fault
        && state.authenticated_request(state.sequence)
        && state.request_pair == 0
        && state.stage_kind == 1
        && state
            .evidence
            .iter()
            .all(|e| e.sends == [0, 0] && e.deliveries == [0, 0])
        && state
            .channels
            .iter()
            .flatten()
            .all(|c| c.phase() == channel_phase::empty)
    {
        state.clear_request();
        state.veil_evidence[0].aborted += 1;
        let transition = state
            .scheduler
            .prepare_service_call_fault(
                &mut state.tasks,
                state.service.as_mut().unwrap(),
                &mut state.pending,
            )
            .unwrap_or_else(|_| fail("shared_dependency_fault"))
            .commit();
        next = state.index_of(transition.task());
        if next != 1 || !state.service.as_ref().unwrap().is_revoked() {
            fail("shared_dependency_peer");
        }
        for pair in 0..2 {
            state.close_channels(pair);
            state.terminal[pair * 2] = true;
            state.evidence[pair].faults += 1;
            if state.evidence[pair].discarded != 0 {
                fail("shared_abort_published");
            }
        }
        state.terminal[veil_index] = true;
        state.shared_evidence.veil_faults += 1;
        state.shared_evidence.member_retirements += 2;
    } else {
        fail("shared_fault_site");
    }
    let selected = state.select(next);
    Some(leave_dispatch(state, selected).cast())
}

pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("shared_initial_state");
    }
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: Unique withdrawn aperture remains exclusively retained for this proof.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Initialized sole-BSP descriptors are read with interrupts excluded.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let outside = framebuffer_hash(info, true);
    let (initial, vendor, baseline, records) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let native = elf_profile::application_images(elf_profile::expected_plan())[0];
        let linux = linux_profile::application_image(linux_profile::expected_plan());
        let veil = veil_image();
        let (vendor, normal) = run_pair(
            allocator,
            [native, native],
            kernel_cr3,
            bootstrap_stack,
            0,
            initial,
        );
        // SAFETY: Native reference has fully retired; stage remains kernel owned.
        let row_hashes = core::array::from_fn(|row| {
            packet_hash(unsafe {
                &(&*display_stage.0.get())[row * stage_row_bytes..(row + 1) * stage_row_bytes]
            })
        });
        let patch = presenter
            .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                &*display_stage.0.get()
            })
            .unwrap_or_else(|_| fail("shared_baseline_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != outside {
            fail("shared_baseline_pixels");
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
            fail("shared_baseline_fault");
        }
        clear_stage();
        let mut records = [(
            [evidence::default(); 2],
            [veil_evidence::default(); 2],
            shared_evidence::default(),
        ); 3];
        for (scenario, record) in records.iter_mut().enumerate() {
            let (current_vendor, pairs, veils, shared) = run_five(
                allocator,
                [linux, native, linux, native, veil],
                kernel_cr3,
                bootstrap_stack,
                scenario,
                initial,
                row_hashes,
            );
            if current_vendor != vendor
                || shared.member_retirements != 2
                || shared.stale_rejected != shared.cross_handoffs * 2
                || shared.peer_checks == 0
            {
                fail("shared_service_comparison");
            }
            for pair in 0..2 {
                let e = pairs[pair];
                let v = veils[pair];
                let normal_pair = scenario == 0 || (scenario == 1 && pair == 1);
                if normal_pair {
                    if e.surface_hashes != normal.surface_hashes
                        || e.sends != [7, 6]
                        || e.deliveries != [7, 6]
                        || e.acknowledgements != [7, 6]
                        || e.rows != stage_height as u64
                        || e.faults != 0
                        || e.exits != 2
                        || e.discarded != 0
                        || v.raw_calls != v.completions
                        || v.stages != 40
                    {
                        fail("shared_editor_comparison");
                    }
                } else if scenario == 1 {
                    if e.sends != [1, 0]
                        || e.deliveries != [0, 0]
                        || e.faults != 1
                        || e.exits != 1
                        || e.discarded != 1
                        || e.rows != 0
                        || v.raw_calls != v.completions
                        || v.stages != 1
                    {
                        fail("shared_producer_containment");
                    }
                } else if e.sends != [0, 0]
                    || e.deliveries != [0, 0]
                    || e.faults != 1
                    || e.exits != 1
                    || e.discarded != 0
                    || e.rows != 0
                    || v.aborted != u64::from(pair == 0)
                    || v.raw_calls != v.completions + v.aborted
                {
                    fail("shared_dependency_containment");
                }
            }
            if scenario < 2 {
                if shared.cross_handoffs == 0 || shared.veil_exits != 1 || shared.veil_faults != 0 {
                    fail("shared_service_lifecycle");
                }
                let patch = presenter
                    .prepare_rgb8_region(32, 32, stage_width, stage_height, unsafe {
                        &*display_stage.0.get()
                    })
                    .unwrap_or_else(|_| fail("shared_patch"));
                if !patch.matches_pixels() || !patch.commit_and_verify() {
                    fail("shared_pixel_comparison");
                }
            } else if shared.veil_exits != 0
                || shared.veil_faults != 1
                || shared.cross_handoffs != 0
            {
                fail("shared_service_containment");
            }
            clear_stage();
            if framebuffer_hash(info, true) != outside || framebuffer_hash(info, false) != presented
            {
                fail("shared_preserved_pixels");
            }
            *record = (pairs, veils, shared);
        }
        (initial, vendor, [normal, faulted], records)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("shared_final_interrupts");
    }
    emit_evidence(initial, vendor, baseline);
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shared_veil_test.ready","level":"info","tasks":5,"callers":2,"scenarios":3,"data_frames":80,"table_frames":45,"leases":25,"request_bytes":80,"policy":"userspace","startup":"fixed_fixture"}}"#
    );
    for (scenario, (pairs, veils, shared)) in records.into_iter().enumerate() {
        let a = pairs[0];
        let b = pairs[1];
        let va = veils[0];
        let vb = veils[1];
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.shared_veil_test.scenario","level":"info","scenario":{},"raw_calls":{},"completions":{},"stages":{},"rejected":{},"aborted":{},"timer_ticks":{},"cooperative_switches":{},"caller_changes":{},"isolation_checks":{},"stale_rejections":{},"exits":{},"faults":{}}}"#,
            scenario,
            va.raw_calls + vb.raw_calls,
            va.completions + vb.completions,
            va.stages + vb.stages,
            va.rejected + vb.rejected,
            va.aborted + vb.aborted,
            a.timer_ticks + b.timer_ticks,
            a.switches + b.switches,
            shared.cross_handoffs,
            shared.peer_checks,
            shared.stale_rejected,
            a.exits + b.exits + shared.veil_exits,
            a.faults + b.faults + shared.veil_faults
        );
        for (caller, e) in pairs.into_iter().enumerate() {
            let normal_pair = scenario == 0 || (scenario == 1 && caller == 1);
            let _ = writeln!(
                serial,
                r#"{{"schema":1,"event":"kernel.shared_veil_test.pair","level":"info","scenario":{},"caller":{},"surfaces":{},"inputs":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"discarded":{},"exits":{},"faults":{},"rows":{},"hashes_equal":{},"pixels_equal":{}}}"#,
                scenario,
                caller,
                e.sends[0],
                e.sends[1],
                e.deliveries[0],
                e.deliveries[1],
                e.acknowledgements[0],
                e.acknowledgements[1],
                e.discarded,
                e.exits,
                e.faults,
                e.rows,
                normal_pair,
                normal_pair
            );
        }
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.shared_veil_test.passed","level":"info","application_tasks":15,"hashes_equal":true,"pixels_equal":true,"abort_unpublished":true,"resources_restored":true,"authority_revoked":true}}"#
    );
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
