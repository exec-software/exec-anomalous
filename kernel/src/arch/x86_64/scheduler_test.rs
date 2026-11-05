//! Fixed two-task x86-64 preemption and fault-containment proof.
//!
//! Every address, task, tick transition, and recovery predicate in this module
//! is feature-local. The architecture-neutral scheduler owns runnable order;
//! this harness binds it to allocator-owned roots, privilege stacks, and the
//! exact processor frames used by the proof.

use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use crate::circle::{circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::serial_port;
use crate::identity::{circle_id, fixture_principal_id, task_identity};
use crate::memory;
use crate::memory::physical::{
    frame_accounting, frame_allocator, frame_lease, frame_purpose, frame_size, physical_range,
};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};
use crate::time::monotonic_instant;

use super::address_space::{address_space, mapping_evidence};
use super::{descriptors, exceptions, interrupts, paging};

const user_window_address: u64 = 0x0000_7000_0000_0000;
pub(super) const user_code_address: u64 = user_window_address + 0x1000;
pub(super) const user_mailbox_address: u64 = user_window_address + 0x3000;
pub(super) const user_stack_address: u64 = user_window_address + 0x5000;
pub(super) const user_stack_top: u64 = user_window_address + 0x6000;
pub(super) const scheduler_gate_vector: u64 = 0xf2;
pub(super) const timer_vector: u64 = 0x20;
pub(super) const invalid_opcode_vector: u64 = 6;
pub(super) const initial_user_flags: u64 = 0x0002;
pub(super) const running_user_flags: u64 = 0x0202;
pub(super) const resume_flag: u64 = 1 << 16;
pub(super) const cr0_task_switched: u64 = 1 << 3;
pub(super) const privilege_stack_pages: usize =
    descriptors::privilege_stack_size / frame_size as usize;
const scheduler_task_count: usize = 2;
const context_word_count: usize = 22;
pub(super) const context_size: u64 = (context_word_count * mem::size_of::<u64>()) as u64;
const operation_ready: u64 = 1;
const operation_complete: u64 = 2;
const completion_status: u64 = 1;
const mailbox_canary_a_offset: u64 = 0;
const mailbox_canary_b_offset: u64 = 8;
const mailbox_arm_offset: u64 = 16;
const fault_canary_a: u64 = 0x51a7_c0de_f17e_0001;
const fault_canary_b: u64 = 0x51a7_c0de_f17e_0002;
const peer_canary_a: u64 = 0x9ee7_c0de_ba5e_0001;
const peer_canary_b: u64 = 0x9ee7_c0de_ba5e_0002;
const fault_arm: u64 = 0xf017_aa55_c0de_0001;
const peer_arm: u64 = 0x9ee7_aa55_c0de_0002;

const _: () = assert!(descriptors::privilege_stack_size == 16 * 1024);
const _: () = assert!(privilege_stack_pages == 4);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum fixture_role {
    faulting,
    peer,
}

impl fixture_role {
    const fn label(self) -> &'static str {
        match self {
            Self::faulting => "faulting_fixture",
            Self::peer => "peer_fixture",
        }
    }

    const fn canaries(self) -> (u64, u64) {
        match self {
            Self::faulting => (fault_canary_a, fault_canary_b),
            Self::peer => (peer_canary_a, peer_canary_b),
        }
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(super) struct scheduler_frame {
    pub(super) accumulator: u64,
    pub(super) base: u64,
    pub(super) counter: u64,
    pub(super) data: u64,
    pub(super) frame_pointer: u64,
    pub(super) source_index: u64,
    pub(super) destination_index: u64,
    pub(super) register_8: u64,
    pub(super) register_9: u64,
    pub(super) register_10: u64,
    pub(super) register_11: u64,
    pub(super) register_12: u64,
    pub(super) register_13: u64,
    pub(super) register_14: u64,
    pub(super) register_15: u64,
    pub(super) vector: u64,
    pub(super) error_code: u64,
    pub(super) instruction_pointer: u64,
    pub(super) code_segment: u64,
    pub(super) flags: u64,
    pub(super) stack_pointer: u64,
    pub(super) stack_segment: u64,
}

impl scheduler_frame {
    pub(super) const fn initial(instruction_pointer: u64) -> Self {
        Self {
            accumulator: 0,
            base: 0,
            counter: 0,
            data: 0,
            frame_pointer: 0,
            source_index: 0,
            destination_index: 0,
            register_8: 0,
            register_9: 0,
            register_10: 0,
            register_11: 0,
            register_12: 0,
            register_13: 0,
            register_14: 0,
            register_15: 0,
            vector: 0,
            error_code: 0,
            instruction_pointer,
            code_segment: descriptors::user_code_selector as u64,
            flags: initial_user_flags,
            stack_pointer: user_stack_top,
            stack_segment: descriptors::user_data_selector as u64,
        }
    }
}

pub(super) fn frame_address_matches_stack(
    frame: *const scheduler_frame,
    stack: physical_range,
) -> bool {
    if frame.is_null() || !(frame as usize).is_multiple_of(mem::align_of::<u64>()) {
        return false;
    }
    let address = frame as usize as u64;
    let Some(frame_end) = address.checked_add(context_size) else {
        return false;
    };
    let Some(stack_end) = stack.end() else {
        return false;
    };
    stack_end.checked_sub(context_size) == Some(address) && frame_end == stack_end
}

pub(super) fn user_return_fields_are_valid(frame: scheduler_frame, bounds: task_bounds) -> bool {
    user_return_fields_are_valid_with_stack_top(frame, bounds, user_stack_top)
}

pub(super) fn user_return_fields_are_valid_with_stack_top(
    frame: scheduler_frame,
    bounds: task_bounds,
    expected_stack_top: u64,
) -> bool {
    user_return_fields_are_valid_within_stack(frame, bounds)
        && frame.stack_pointer == expected_stack_top
}

pub(super) fn user_return_fields_are_valid_within_stack(
    frame: scheduler_frame,
    bounds: task_bounds,
) -> bool {
    frame.error_code == 0
        && frame.code_segment == u64::from(descriptors::user_code_selector)
        && frame.stack_segment == u64::from(descriptors::user_data_selector)
        && frame.flags & (1 << 1) != 0
        && frame.flags & (3 << 12) == 0
        && frame.flags & (1 << 10) == 0
        && frame.flags & (1 << 14) == 0
        && bounds.contains_code(frame.instruction_pointer)
        && bounds.contains_stack_pointer(frame.stack_pointer)
}

pub(super) fn gate_registers_are_canonical(
    frame: scheduler_frame,
    expected_register_15: u64,
) -> bool {
    frame.accumulator == 0
        && frame.base == 0
        && frame.counter == 0
        && frame.data == 0
        && frame.frame_pointer == 0
        && frame.source_index == 0
        && frame.destination_index == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
        && frame.register_15 == expected_register_15
}

fn expected_invalid_opcode_matches(
    snapshot: exceptions::exception_snapshot,
    registers: scheduler_frame,
    instruction_pointer: u64,
) -> bool {
    registers.accumulator == snapshot.accumulator
        && registers.vector == snapshot.vector
        && registers.error_code == snapshot.error_code
        && registers.instruction_pointer == snapshot.instruction_pointer
        && registers.code_segment == snapshot.code_segment
        && registers.flags == snapshot.flags
        && registers.stack_pointer == snapshot.stack_pointer
        && Some(registers.stack_segment) == snapshot.stack_segment
        && snapshot.vector == invalid_opcode_vector
        && snapshot.error_code == 0
        && snapshot.instruction_pointer == instruction_pointer
        && snapshot.code_segment == u64::from(descriptors::user_code_selector)
        && snapshot.stack_pointer == user_stack_top
        && snapshot.stack_segment == Some(u64::from(descriptors::user_data_selector))
        && snapshot.fault_address.is_none()
        && (snapshot.flags == running_user_flags
            || snapshot.flags == running_user_flags | resume_flag)
        && snapshot.accumulator == 0
        && all_general_registers_zero(registers)
}

const _: () = assert!(mem::size_of::<scheduler_frame>() == context_size as usize);
const _: () = assert!(mem::offset_of!(scheduler_frame, vector) == 15 * 8);
const _: () = assert!(mem::offset_of!(scheduler_frame, instruction_pointer) == 17 * 8);
const _: () = assert!(mem::offset_of!(scheduler_frame, code_segment) == 18 * 8);
const _: () = assert!(mem::offset_of!(scheduler_frame, stack_pointer) == 20 * 8);
const _: () = assert!(mem::offset_of!(scheduler_frame, stack_segment) == 21 * 8);

pub(super) struct task_space {
    pub(super) root: address_space,
    pub(super) physical_pages: [u64; 3],
    pub(super) privilege_stack: frame_lease,
    pub(super) privilege_stack_range: physical_range,
    pub(super) frame: *mut scheduler_frame,
    pub(super) payload_length: usize,
}

struct scheduled_task {
    circle: circle_id,
    identity: task_identity,
    space: task_space,
    ready_seen: bool,
}

#[derive(Clone, Copy)]
struct scheduler_trace {
    dispatches: [u64; 4],
    preemptions: [u64; 2],
    ticks: [u64; 2],
    fault: Option<exceptions::exception_snapshot>,
    resume_count: usize,
    fault_count: usize,
    completion_count: usize,
    peer_continued: bool,
    kernel_continuation_restored: bool,
}

impl scheduler_trace {
    const fn empty() -> Self {
        Self {
            dispatches: [u64::MAX; 4],
            preemptions: [u64::MAX; 2],
            ticks: [0; 2],
            fault: None,
            resume_count: 0,
            fault_count: 0,
            completion_count: 0,
            peer_continued: false,
            kernel_continuation_restored: false,
        }
    }
}

struct scheduler_runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    faulting: scheduled_task,
    peer: scheduled_task,
    active_role: Option<fixture_role>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
    saved_kernel_stack: u64,
    original_task_switched: u64,
    trace: scheduler_trace,
}

#[derive(Clone, Copy)]
struct scheduler_evidence {
    initial: frame_accounting,
    faulting_circle_generation: u64,
    faulting_task_generation: u64,
    peer_circle_generation: u64,
    peer_task_generation: u64,
    trace: scheduler_trace,
}

static active_runtime: AtomicPtr<scheduler_runtime> = AtomicPtr::new(ptr::null_mut());
static switch_active: AtomicBool = AtomicBool::new(false);
static gate_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    pub(super) fn x86_64_scheduler_enter(
        frame: *mut scheduler_frame,
        saved_kernel_stack: *mut u64,
        original_task_switched: *mut u64,
    ) -> u64;
    pub(super) fn x86_64_scheduler_resume_kernel(
        status: u64,
        saved_kernel_stack: u64,
        original_task_switched: u64,
    ) -> !;

    static x86_64_scheduler_faulting_start: u8;
    static x86_64_scheduler_fault_ready_return: u8;
    static x86_64_scheduler_fault_instruction: u8;
    static x86_64_scheduler_faulting_end: u8;
    static x86_64_scheduler_peer_start: u8;
    static x86_64_scheduler_peer_ready_return: u8;
    static x86_64_scheduler_peer_completion_return: u8;
    static x86_64_scheduler_peer_end: u8;
}

pub(super) fn run() -> ! {
    if !active_runtime.load(Ordering::Acquire).is_null()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_scheduler_state_invalid");
    }

    let kernel_cr3 = paging::read_cr3();
    // SAFETY: The timer restriction left IF clear, Threshold keeps NMI masked,
    // and no application processor exists in this bounded proof.
    let bootstrap_privilege_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        run_with_allocator(allocator, kernel_cr3, bootstrap_privilege_stack_top)
    });

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This exact root was live before the feature and every private
        // root borrowed its resident low kernel branch.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    let evidence = match result {
        Ok(Ok(evidence)) => evidence,
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    };
    emit_evidence(evidence);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_privilege_stack_top: u64,
) -> Result<scheduler_evidence, &'static str> {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || paging::read_cr3() != kernel_cr3
        || bootstrap_privilege_stack_top == 0
    {
        return Err("scheduler_setup_state_invalid");
    }
    let initial = allocator.accounting();
    if initial.free < 22
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("scheduler_initial_accounting_invalid");
    }

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let faulting_circle = circles
        .create()
        .map_err(|_| "faulting_circle_create_failed")?;
    let peer_circle = circles.create().map_err(|_| "peer_circle_create_failed")?;
    let faulting_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_scheduler_faulting_start),
        ptr::addr_of!(x86_64_scheduler_faulting_end),
    )?;
    let peer_space = prepare_task_space(
        allocator,
        kernel_cr3,
        ptr::addr_of!(x86_64_scheduler_peer_start),
        ptr::addr_of!(x86_64_scheduler_peer_end),
    )?;
    attach_task_space(&mut circles, faulting_circle)?;
    attach_task_space(&mut circles, peer_circle)?;
    let faulting_identity = create_task(
        &mut circles,
        &mut tasks,
        faulting_circle,
        1,
        faulting_space.payload_length,
    )?;
    let peer_identity = create_task(
        &mut circles,
        &mut tasks,
        peer_circle,
        2,
        peer_space.payload_length,
    )?;
    let mut scheduler =
        scheduler_table::new(scheduler_task_count).map_err(|_| "scheduler_create_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, faulting_identity)
        .map_err(|_| "faulting_task_admission_failed")?;
    scheduler
        .admit(&mut circles, &mut tasks, peer_identity)
        .map_err(|_| "peer_task_admission_failed")?;

    let ready = allocator.accounting();
    validate_ready_ownership(
        initial,
        ready,
        &circles,
        &tasks,
        &scheduler,
        &faulting_space,
        &peer_space,
    )?;

    let mut runtime = scheduler_runtime {
        circles,
        tasks,
        scheduler,
        faulting: scheduled_task {
            circle: faulting_circle,
            identity: faulting_identity,
            space: faulting_space,
            ready_seen: false,
        },
        peer: scheduled_task {
            circle: peer_circle,
            identity: peer_identity,
            space: peer_space,
            ready_seen: false,
        },
        active_role: None,
        kernel_cr3,
        bootstrap_privilege_stack_top,
        saved_kernel_stack: 0,
        original_task_switched: u64::MAX,
        trace: scheduler_trace::empty(),
    };

    let first = runtime
        .scheduler
        .start_next(&mut runtime.tasks)
        .map_err(|_| "initial_dispatch_failed")?;
    if first.task() != runtime.faulting.identity
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        return Err("initial_dispatch_invalid");
    }
    runtime.trace.dispatches[0] = first.sequence();
    let first_frame = runtime.select_role(fixture_role::faulting, false)?;
    let runtime_pointer = ptr::from_mut(&mut runtime);
    if active_runtime
        .compare_exchange(
            ptr::null_mut(),
            runtime_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return Err("scheduler_context_publication_failed");
    }

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The selected root, TSS stack, initial frame, and active runtime
    // are complete; IF is clear and only exact completion can return here.
    let status = unsafe {
        x86_64_scheduler_enter(
            first_frame,
            ptr::addr_of_mut!(runtime.saved_kernel_stack),
            ptr::addr_of_mut!(runtime.original_task_switched),
        )
    };

    active_runtime.store(ptr::null_mut(), Ordering::Release);
    runtime.trace.kernel_continuation_restored = true;
    if status != completion_status
        || runtime.original_task_switched != original_task_switched
        || paging::read_cr0() & cr0_task_switched != original_task_switched
        || paging::read_cr3() != kernel_cr3
        || interrupts::interrupts_enabled()
        || switch_active.load(Ordering::Acquire)
        || gate_active.load(Ordering::Acquire)
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_privilege_stack_top
        || runtime.trace.dispatches != [0, 1, 2, 3]
        || runtime.trace.preemptions != [0, 1]
        || runtime.trace.ticks[0] == 0
        || runtime.trace.ticks[1] <= runtime.trace.ticks[0]
        || runtime.trace.resume_count != 2
        || runtime.trace.fault_count != 1
        || runtime.trace.completion_count != 1
        || !runtime.trace.peer_continued
        || !interrupts::scheduler_interrupts_are_masked()
    {
        return Err("scheduler_completion_restore_failed");
    }
    compiler_fence(Ordering::SeqCst);
    runtime.saved_kernel_stack = 0;
    runtime.original_task_switched = 0;

    let faulting_circle_generation = runtime.faulting.circle.generation_value();
    let faulting_task_generation = runtime.faulting.identity.task().generation_value();
    let peer_circle_generation = runtime.peer.circle.generation_value();
    let peer_task_generation = runtime.peer.identity.task().generation_value();
    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            runtime.faulting.identity,
        )
        .map_err(|_| "faulting_context_withdrawal_failed")?;
    runtime
        .scheduler
        .withdraw_terminal_context(
            &mut runtime.circles,
            &mut runtime.tasks,
            runtime.peer.identity,
        )
        .map_err(|_| "peer_context_withdrawal_failed")?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        runtime.faulting.circle,
        &mut runtime.faulting.space,
    )?;
    release_task_space(
        allocator,
        &mut runtime.circles,
        runtime.peer.circle,
        &mut runtime.peer.space,
    )?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, runtime.faulting.identity)
        .map_err(|_| "faulting_task_destroy_failed")?;
    runtime
        .tasks
        .destroy(&mut runtime.circles, runtime.peer.identity)
        .map_err(|_| "peer_task_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.faulting.circle)
        .map_err(|_| "faulting_circle_destroy_failed")?;
    runtime
        .circles
        .destroy(runtime.peer.circle)
        .map_err(|_| "peer_circle_destroy_failed")?;

    let final_frames = allocator.accounting();
    let final_circles = runtime.circles.accounting();
    let final_scheduler = runtime
        .scheduler
        .snapshot(&runtime.tasks)
        .map_err(|_| "final_scheduler_invalid")?;
    if final_frames != initial
        || final_circles.circles != 0
        || final_circles.tasks != 0
        || final_circles.task_contexts != 0
        || final_circles.endpoints != 0
        || final_circles.sigils != 0
        || final_circles.address_spaces != 0
        || final_circles.mappings != 0
        || runtime.tasks.active_count() != 0
        || runtime.tasks.attached_context_count() != 0
        || final_scheduler.admitted() != 0
        || final_scheduler.runnable() != 0
        || final_scheduler.current().is_some()
    {
        return Err("scheduler_final_accounting_invalid");
    }

    let evidence = scheduler_evidence {
        initial,
        faulting_circle_generation,
        faulting_task_generation,
        peer_circle_generation,
        peer_task_generation,
        trace: runtime.trace,
    };
    retire_runtime_metadata(&mut runtime);
    Ok(evidence)
}

fn retire_runtime_metadata(runtime: &mut scheduler_runtime) {
    debug_assert!(runtime.faulting.space.frame.is_null());
    debug_assert!(runtime.peer.space.frame.is_null());
    // SAFETY: Teardown has restored the kernel root and bootstrap RSP0, and
    // evidence no longer borrows this bookkeeping. Every replacement value is
    // valid; volatile writes retire the remaining address-bearing scalars.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(runtime.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.bootstrap_privilege_stack_top), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.original_task_switched), 0);
        ptr::write_volatile(ptr::addr_of_mut!(runtime.trace), scheduler_trace::empty());
    }
}

impl scheduler_runtime {
    fn task(&self, role: fixture_role) -> &scheduled_task {
        match role {
            fixture_role::faulting => &self.faulting,
            fixture_role::peer => &self.peer,
        }
    }

    fn task_mut(&mut self, role: fixture_role) -> &mut scheduled_task {
        match role {
            fixture_role::faulting => &mut self.faulting,
            fixture_role::peer => &mut self.peer,
        }
    }

    fn role_for_identity(&self, identity: task_identity) -> Option<fixture_role> {
        if identity == self.faulting.identity {
            Some(fixture_role::faulting)
        } else if identity == self.peer.identity {
            Some(fixture_role::peer)
        } else {
            None
        }
    }

    fn current_role(&self) -> Option<fixture_role> {
        let current = self.scheduler.snapshot(&self.tasks).ok()?.current()?;
        let role = self.role_for_identity(current)?;
        (self.active_role == Some(role)).then_some(role)
    }

    fn select_role(
        &mut self,
        role: fixture_role,
        require_task_switched: bool,
    ) -> Result<*mut scheduler_frame, &'static str> {
        let task = self.task(role);
        let root = task
            .space
            .root
            .root_address()
            .map_err(|_| "selected_root_missing")?;
        let stack_top = task
            .space
            .privilege_stack_range
            .end()
            .ok_or("selected_stack_extent_invalid")?;
        let frame = task.space.frame;
        // SAFETY: The root has the exact validated mapping set and borrows the
        // live kernel branch while IF and NMI remain excluded.
        unsafe { task.space.root.activate() }.map_err(|_| "selected_root_activation_failed")?;
        // SAFETY: The task's bound four-page lease remains live and accessible
        // through the shared supervisor identity branch.
        if !unsafe { descriptors::select_privilege_stack_top(stack_top) } {
            return Err("selected_stack_activation_failed");
        }
        self.active_role = Some(role);
        if paging::entry_address(paging::read_cr3()) != root
            || unsafe { descriptors::selected_privilege_stack_top() } != stack_top
            || !self.frame_is_valid_for_role(frame, role, require_task_switched)
        {
            return Err("selected_task_state_invalid");
        }
        Ok(frame)
    }

    fn frame_is_valid_for_role(
        &self,
        frame: *const scheduler_frame,
        role: fixture_role,
        require_task_switched: bool,
    ) -> bool {
        let task = self.task(role);
        if frame != task.space.frame
            || !frame_address_matches_stack(frame, task.space.privilege_stack_range)
            || task.space.privilege_stack.frame_count() != privilege_stack_pages
            || task.space.privilege_stack.purpose() != frame_purpose::data
            || task.space.privilege_stack.range() != Some(task.space.privilege_stack_range)
            || task.space.root.validate_all_mappings().is_err()
        {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(task.identity) else {
            return false;
        };
        if snapshot.state() != task_state::running || !snapshot.context_attached() {
            return false;
        }
        let root = task.space.root.root_address().ok();
        let stack_top = task.space.privilege_stack_range.end();
        if root != Some(paging::entry_address(paging::read_cr3()))
            || stack_top != Some(unsafe { descriptors::selected_privilege_stack_top() })
            || (require_task_switched && paging::read_cr0() & cr0_task_switched == 0)
        {
            return false;
        }
        // SAFETY: Exact extent checks above prove the complete frame is live.
        let frame = unsafe { ptr::read(frame) };
        user_return_fields_are_valid(frame, snapshot.bounds())
    }
}

pub(super) fn user_interrupt_frame_is_valid(frame: *const interrupts::interrupt_frame) -> bool {
    #[cfg(all(
        feature = "dispatcher_test",
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::dispatcher_runtime::is_active() {
        return super::dispatcher_runtime::user_frame_valid(frame);
    }
    #[cfg(all(
        feature = "workspace_dev",
        not(feature = "dispatcher_test"),
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::workspace_runtime::is_active() {
        return super::workspace_runtime::user_frame_valid(frame);
    }
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        return super::semantic_surface_presentation_test::user_interrupt_frame_is_valid(frame);
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        return super::supervised_renderer_test::user_interrupt_frame_is_valid(frame);
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        return super::input_wait_test::user_interrupt_frame_is_valid(frame);
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        return super::shell_vessel_test::user_interrupt_frame_is_valid(frame);
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        return super::supervision_test::user_interrupt_frame_is_valid(frame);
    }
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return false;
    }
    // SAFETY: Publication names the unique live stack runtime until completion
    // clears the pointer. IF is clear throughout every caller of this hook.
    let runtime = unsafe { &*runtime };
    let Some(role) = runtime.current_role() else {
        return false;
    };
    runtime.frame_is_valid_for_role(frame.cast(), role, true)
}

pub(super) fn dispatch_timer_interrupt(
    frame: *mut interrupts::interrupt_frame,
    observed_at: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    #[cfg(all(
        feature = "dispatcher_test",
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::dispatcher_runtime::is_active() {
        return super::dispatcher_runtime::timer(frame, observed_at);
    }
    #[cfg(all(
        feature = "workspace_dev",
        not(feature = "dispatcher_test"),
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::workspace_runtime::is_active() {
        return Some(super::workspace_runtime::timer(frame, observed_at.tick()));
    }
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        return super::semantic_surface_presentation_test::dispatch_timer_interrupt(
            frame,
            observed_at,
        );
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        return super::supervised_renderer_test::dispatch_timer_interrupt(frame, observed_at);
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        return super::input_wait_test::dispatch_timer_interrupt(frame, observed_at);
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        return super::shell_vessel_test::dispatch_timer_interrupt(frame, observed_at);
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        return super::supervision_test::dispatch_timer_interrupt(frame, observed_at);
    }
    if switch_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    let result = dispatch_timer(frame.cast(), observed_at).ok();
    switch_active.store(false, Ordering::Release);
    result.map(|frame| frame.cast())
}

fn dispatch_timer(
    frame: *mut scheduler_frame,
    observed_at: monotonic_instant,
) -> Result<*mut scheduler_frame, &'static str> {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return Err("timer_without_scheduler");
    }
    // SAFETY: The switch latch grants unique access while IF is clear and NMI
    // is masked. The stack owner remains live until terminal teardown.
    let runtime = unsafe { &mut *runtime };
    let role = runtime.current_role().ok_or("timer_without_current_task")?;
    if !runtime.frame_is_valid_for_role(frame, role, true) {
        return Err("timer_frame_invalid");
    }
    // SAFETY: The exact live frame was validated above.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != timer_vector
        || incoming.flags & (1 << 9) == 0
        || observed_at.timebase().slot() != 0
        || observed_at.timebase().generation() != crate::time::boot_timebase_generation
    {
        return Err("timer_observation_invalid");
    }

    match runtime.trace.preemptions {
        [u64::MAX, u64::MAX] if role == fixture_role::faulting => {
            if !runtime.faulting.ready_seen
                || !mailbox_matches(&runtime.faulting.space, fixture_role::faulting, 0)
            {
                return Err("faulting_preemption_state_invalid");
            }
            write_mailbox_word(
                runtime.faulting.space.physical_pages[1],
                mailbox_arm_offset,
                fault_arm,
            )?;
            let transition = runtime
                .scheduler
                .preempt_current(&mut runtime.tasks, runtime.faulting.identity)
                .map_err(|_| "faulting_preemption_failed")?;
            if transition.task() != runtime.faulting.identity
                || transition.sequence() != 0
                || transition.dispatch().task() != runtime.peer.identity
                || transition.dispatch().sequence() != 1
                || transition.dispatch().context() != scheduler_context::initial
            {
                return Err("faulting_preemption_transition_invalid");
            }
            runtime.trace.preemptions[0] = transition.sequence();
            runtime.trace.dispatches[1] = transition.dispatch().sequence();
            runtime.trace.ticks[0] = observed_at.tick();
            runtime.select_role(fixture_role::peer, true)
        }
        [0, u64::MAX] if role == fixture_role::peer => {
            if !runtime.peer.ready_seen
                || !mailbox_matches(&runtime.peer.space, fixture_role::peer, 0)
                || observed_at.tick() <= runtime.trace.ticks[0]
            {
                return Err("peer_preemption_state_invalid");
            }
            write_mailbox_word(
                runtime.peer.space.physical_pages[1],
                mailbox_arm_offset,
                peer_arm,
            )?;
            let transition = runtime
                .scheduler
                .preempt_current(&mut runtime.tasks, runtime.peer.identity)
                .map_err(|_| "peer_preemption_failed")?;
            if transition.task() != runtime.peer.identity
                || transition.sequence() != 1
                || transition.dispatch().task() != runtime.faulting.identity
                || transition.dispatch().sequence() != 2
                || transition.dispatch().context() != scheduler_context::saved
            {
                return Err("peer_preemption_transition_invalid");
            }
            interrupts::disable_external_interrupts();
            if !interrupts::scheduler_interrupts_are_masked() {
                return Err("scheduler_timer_mask_failed");
            }
            runtime.trace.preemptions[1] = transition.sequence();
            runtime.trace.dispatches[2] = transition.dispatch().sequence();
            runtime.trace.ticks[1] = observed_at.tick();
            runtime.trace.resume_count = 1;
            runtime.select_role(fixture_role::faulting, true)
        }
        _ => Err("scheduler_preemption_budget_exceeded"),
    }
}

pub(super) fn contain_expected_user_exception(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    #[cfg(all(
        feature = "dispatcher_test",
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::dispatcher_runtime::is_active() {
        return super::dispatcher_runtime::fault(frame, snapshot);
    }
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        return super::semantic_surface_presentation_test::contain_expected_user_exception(
            frame, snapshot,
        );
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        return super::supervised_renderer_test::contain_expected_user_exception(frame, snapshot);
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        return super::input_wait_test::contain_expected_user_exception(frame, snapshot);
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        return super::shell_vessel_test::contain_expected_user_exception(frame, snapshot);
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        return super::supervision_test::contain_expected_user_exception(frame, snapshot);
    }
    if switch_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return None;
    }
    let result = contain_user_exception(frame.cast(), snapshot).ok();
    switch_active.store(false, Ordering::Release);
    result.map(|frame| frame.cast())
}

pub(super) fn scheduler_is_active() -> bool {
    #[cfg(all(
        feature = "dispatcher_test",
        not(test),
        not(any(
            feature = "fault_test",
            feature = "circle_test",
            feature = "control_test",
            feature = "rite_test",
            feature = "memory_test",
            feature = "interrupt_test",
            feature = "supervision_test",
            feature = "shell_vessel_test",
            feature = "input_wait_test",
            feature = "display_vessel_test",
            feature = "supervised_renderer_test",
            feature = "semantic_surface_transport_test",
            feature = "semantic_surface_presentation_test",
            feature = "desktop_dev"
        ))
    ))]
    if super::dispatcher_runtime::is_active() {
        return true;
    }
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        return true;
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        return true;
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        return true;
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        return true;
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        return true;
    }
    !active_runtime.load(Ordering::Acquire).is_null()
}

pub(super) fn reject_unexpected_exception() -> ! {
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        super::semantic_surface_presentation_test::reject_unexpected_exception()
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        super::supervised_renderer_test::reject_unexpected_exception()
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        super::input_wait_test::reject_unexpected_exception()
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        super::shell_vessel_test::reject_unexpected_exception()
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        super::supervision_test::reject_unexpected_exception()
    }
    fail("unexpected_exception")
}

fn contain_user_exception(
    frame: *mut scheduler_frame,
    snapshot: exceptions::exception_snapshot,
) -> Result<*mut scheduler_frame, &'static str> {
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        return Err("fault_without_scheduler");
    }
    // SAFETY: The exception gate cleared IF and the switch latch uniquely owns
    // the published runtime for this bounded transition.
    let runtime = unsafe { &mut *runtime };
    if runtime.current_role() != Some(fixture_role::faulting)
        || runtime.trace.preemptions != [0, 1]
        || runtime.trace.resume_count != 1
        || !runtime.faulting.ready_seen
        || !runtime.frame_is_valid_for_role(frame, fixture_role::faulting, true)
        || !mailbox_matches(&runtime.faulting.space, fixture_role::faulting, fault_arm)
    {
        return Err("faulting_exception_state_invalid");
    }
    let expected_instruction = relocated_user_address(
        ptr::addr_of!(x86_64_scheduler_faulting_start),
        ptr::addr_of!(x86_64_scheduler_fault_instruction),
        runtime.faulting.space.payload_length,
    )
    .ok_or("fault_instruction_address_invalid")?;
    // SAFETY: The complete frame is owned and validated above.
    let registers = unsafe { ptr::read(frame) };
    if !expected_invalid_opcode_matches(snapshot, registers, expected_instruction) {
        return Err("unexpected_user_exception");
    }
    let transition = runtime
        .scheduler
        .fault_current(&mut runtime.tasks, runtime.faulting.identity)
        .map_err(|_| "fault_containment_transition_failed")?;
    if transition.task() != runtime.faulting.identity
        || transition.dispatch().task() != runtime.peer.identity
        || transition.dispatch().sequence() != 3
        || transition.dispatch().context() != scheduler_context::saved
    {
        return Err("fault_containment_dispatch_invalid");
    }
    runtime.trace.dispatches[3] = transition.dispatch().sequence();
    runtime.trace.fault = Some(snapshot);
    runtime.trace.fault_count = 1;
    runtime.trace.resume_count = 2;
    runtime.select_role(fixture_role::peer, true)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_scheduler_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    #[cfg(all(feature = "semantic_surface_presentation_test", not(test)))]
    if super::semantic_surface_presentation_test::semantic_surface_presentation_is_active() {
        // SAFETY: The shared entry supplied the live 22-qword frame. The
        // presentation runtime validates it and owns the selected return.
        return unsafe { super::semantic_surface_presentation_test::gate_dispatch(frame) };
    }
    #[cfg(all(
        feature = "supervised_renderer_test",
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::supervised_renderer_test::supervised_renderer_is_active() {
        // SAFETY: The shared entry supplied the live 22-qword frame. The
        // renderer runtime validates it and owns the selected return.
        return unsafe { super::supervised_renderer_test::gate_dispatch(frame) };
    }
    #[cfg(all(
        feature = "input_wait_test",
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test"),
        not(test)
    ))]
    if super::input_wait_test::input_wait_is_active() {
        // SAFETY: The shared entry supplied the live 22-qword frame. The
        // input-wait runtime validates it and owns the selected return.
        return unsafe { super::input_wait_test::gate_dispatch(frame) };
    }
    #[cfg(all(
        feature = "shell_vessel_test",
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::shell_vessel_test::shell_vessel_is_active() {
        // SAFETY: The shared entry supplied the live 22-qword frame. The
        // composition runtime owns exact validation and the selected return.
        return unsafe { super::shell_vessel_test::gate_dispatch(frame) };
    }
    #[cfg(all(
        feature = "supervision_test",
        not(feature = "shell_vessel_test"),
        not(feature = "input_wait_test"),
        not(feature = "supervised_renderer_test"),
        not(feature = "semantic_surface_transport_test")
    ))]
    if super::supervision_test::supervision_is_active() {
        // SAFETY: The shared assembly entry supplied the complete live frame;
        // the active feature runtime owns its exact validation and transition.
        return unsafe { super::supervision_test::gate_dispatch(frame) };
    }
    if gate_active
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        fail("scheduler_gate_reentry");
    }
    let runtime = active_runtime.load(Ordering::Acquire);
    if runtime.is_null() {
        fail("scheduler_gate_without_runtime");
    }
    // SAFETY: The gate cleared IF, NMI remains masked, and the gate latch grants
    // unique access to the published stack runtime.
    let runtime = unsafe { &mut *runtime };
    let role = runtime
        .current_role()
        .unwrap_or_else(|| fail("scheduler_gate_without_current_task"));
    if !runtime.frame_is_valid_for_role(frame, role, true) {
        fail("scheduler_gate_frame_invalid");
    }
    // SAFETY: Exact ownership validation above covers all 22 qwords.
    let incoming = unsafe { ptr::read(frame) };
    if incoming.vector != scheduler_gate_vector || incoming.error_code != 0 {
        fail("scheduler_gate_vector_invalid");
    }

    if incoming.accumulator == operation_ready {
        handle_ready_gate(runtime, role, frame, incoming).unwrap_or_else(|reason| fail(reason));
        gate_active.store(false, Ordering::Release);
        return frame;
    }
    if incoming.accumulator == operation_complete && role == fixture_role::peer {
        finish_peer(runtime, incoming)
    }
    fail("scheduler_gate_operation_invalid")
}

fn handle_ready_gate(
    runtime: &mut scheduler_runtime,
    role: fixture_role,
    frame: *mut scheduler_frame,
    incoming: scheduler_frame,
) -> Result<(), &'static str> {
    let task = runtime.task(role);
    let template_start = match role {
        fixture_role::faulting => ptr::addr_of!(x86_64_scheduler_faulting_start),
        fixture_role::peer => ptr::addr_of!(x86_64_scheduler_peer_start),
    };
    let ready_return = match role {
        fixture_role::faulting => ptr::addr_of!(x86_64_scheduler_fault_ready_return),
        fixture_role::peer => ptr::addr_of!(x86_64_scheduler_peer_ready_return),
    };
    let expected_return =
        relocated_user_address(template_start, ready_return, task.space.payload_length)
            .ok_or("ready_return_address_invalid")?;
    if task.ready_seen
        || incoming.instruction_pointer != expected_return
        || incoming.flags != initial_user_flags
        || incoming.stack_pointer != user_stack_top
        || incoming.register_15 != user_mailbox_address
        || !ready_gate_registers_are_exact(incoming)
        || !mailbox_matches(&task.space, role, 0)
    {
        return Err("ready_gate_state_invalid");
    }
    runtime.task_mut(role).ready_seen = true;
    // SAFETY: The gate owns this exact frame; setting only IF preserves the
    // sanitized user flag contract and enables the real PIT on IRETQ.
    unsafe { ptr::addr_of_mut!((*frame).flags).write(running_user_flags) };
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

fn finish_peer(runtime: &mut scheduler_runtime, incoming: scheduler_frame) -> ! {
    let expected_return = relocated_user_address(
        ptr::addr_of!(x86_64_scheduler_peer_start),
        ptr::addr_of!(x86_64_scheduler_peer_completion_return),
        runtime.peer.space.payload_length,
    )
    .unwrap_or_else(|| fail("completion_return_address_invalid"));
    if !runtime.peer.ready_seen
        || runtime.trace.preemptions != [0, 1]
        || runtime.trace.fault_count != 1
        || runtime.trace.resume_count != 2
        || incoming.instruction_pointer != expected_return
        || incoming.flags != running_user_flags
        || incoming.stack_pointer != user_stack_top
        || incoming.accumulator != operation_complete
        || !completion_gate_registers_are_exact(incoming)
        || !mailbox_matches(&runtime.peer.space, fixture_role::peer, peer_arm)
        || !mailbox_matches(&runtime.faulting.space, fixture_role::faulting, fault_arm)
    {
        fail("peer_completion_state_invalid");
    }
    runtime
        .scheduler
        .complete_current(&mut runtime.tasks, runtime.peer.identity)
        .unwrap_or_else(|_| fail("peer_completion_transition_failed"));
    runtime.trace.completion_count = 1;
    runtime.trace.peer_continued = true;
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("completion_interrupt_mask_failed");
    }
    // SAFETY: Every task has reached a terminal state and external delivery is
    // fully masked, so the exact pre-feature root can be restored now.
    unsafe { paging::write_cr3(runtime.kernel_cr3) };
    if paging::read_cr3() != runtime.kernel_cr3 {
        fail("completion_kernel_root_restore_failed");
    }
    // SAFETY: No later privilege transition may target a task stack; restore
    // the resident bootstrap RSP0 before abandoning the peer stack.
    if !unsafe { descriptors::restore_bootstrap_privilege_stack() }
        || unsafe { descriptors::selected_privilege_stack_top() }
            != runtime.bootstrap_privilege_stack_top
    {
        fail("completion_tss_restore_failed");
    }
    let saved_kernel_stack = runtime.saved_kernel_stack;
    let original_task_switched = runtime.original_task_switched;
    if saved_kernel_stack == 0
        || !(saved_kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !matches!(original_task_switched, 0 | cr0_task_switched)
        || paging::read_cr0() & cr0_task_switched == 0
    {
        fail("completion_continuation_invalid");
    }
    runtime.active_role = None;
    gate_active.store(false, Ordering::Release);
    active_runtime.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: Exact completion exclusively consumes the saved kernel
    // continuation and abandons the still-live peer transition stack.
    unsafe {
        x86_64_scheduler_resume_kernel(
            completion_status,
            saved_kernel_stack,
            original_task_switched,
        )
    }
}

pub(super) fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    payload_start: *const u8,
    payload_end: *const u8,
) -> Result<task_space, &'static str> {
    let code = allocate_data(allocator, 1, 1, "code_frame_allocation_failed")?;
    let mailbox = allocate_data(allocator, 1, 1, "mailbox_frame_allocation_failed")?;
    let stack = allocate_data(allocator, 1, 1, "stack_frame_allocation_failed")?;
    let privilege_stack = allocate_data(
        allocator,
        privilege_stack_pages,
        privilege_stack_pages,
        "privilege_stack_allocation_failed",
    )?;
    let code_range = code.range().ok_or("code_frame_extent_missing")?;
    let mailbox_range = mailbox.range().ok_or("mailbox_frame_extent_missing")?;
    let stack_range = stack.range().ok_or("stack_frame_extent_missing")?;
    let privilege_stack_range = privilege_stack
        .range()
        .ok_or("privilege_stack_extent_missing")?;
    if code_range.length != frame_size
        || mailbox_range.length != frame_size
        || stack_range.length != frame_size
        || privilege_stack_range.length != descriptors::privilege_stack_size as u64
        || privilege_stack_range.base & (descriptors::privilege_stack_size as u64 - 1) != 0
        || [
            code_range,
            mailbox_range,
            stack_range,
            privilege_stack_range,
        ]
        .iter()
        .any(|range| !range_is_zero(*range))
    {
        return Err("task_frames_invalid");
    }
    // SAFETY: The allocator grants exclusive code storage and the checked
    // immutable template must fit wholly inside its one page.
    let payload_length = unsafe { copy_payload(payload_start, payload_end, code_range)? };
    // SAFETY: The published allocator is exclusively borrowed and clears the
    // complete four-frame page-table extent before publication.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "address_space_create_failed")?;
    root.map_owned(
        allocator,
        user_code_address,
        code,
        paging::page_permissions {
            user: true,
            writable: false,
            executable: true,
        },
    )
    .map_err(|_| "code_mapping_failed")?;
    root.map_owned(
        allocator,
        user_mailbox_address,
        mailbox,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "mailbox_mapping_failed")?;
    root.map_owned(
        allocator,
        user_stack_address,
        stack,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "stack_mapping_failed")?;
    validate_mapping(
        root.validate_mapping(user_code_address)
            .map_err(|_| "code_mapping_walk_failed")?,
        code_range.base,
        false,
        true,
    )?;
    validate_mapping(
        root.validate_mapping(user_mailbox_address)
            .map_err(|_| "mailbox_mapping_walk_failed")?,
        mailbox_range.base,
        true,
        false,
    )?;
    validate_mapping(
        root.validate_mapping(user_stack_address)
            .map_err(|_| "stack_mapping_walk_failed")?,
        stack_range.base,
        true,
        false,
    )?;
    for guard in [
        user_window_address,
        user_window_address + 0x2000,
        user_window_address + 0x4000,
        user_window_address + 0x6000,
    ] {
        if root.validate_absent(guard).is_err() {
            return Err("guard_mapping_present");
        }
    }
    if !payload_matches(payload_start, payload_length, code_range.base) {
        return Err("payload_copy_mismatch");
    }
    allocator
        .bind(&privilege_stack)
        .map_err(|_| "privilege_stack_binding_failed")?;
    let stack_top = privilege_stack_range
        .end()
        .ok_or("privilege_stack_extent_invalid")?;
    let frame_address = stack_top
        .checked_sub(context_size)
        .ok_or("initial_frame_address_invalid")?;
    if frame_address < privilege_stack_range.base
        || frame_address & (mem::align_of::<u64>() as u64 - 1) != 0
    {
        return Err("initial_frame_address_invalid");
    }
    let frame = frame_address as usize as *mut scheduler_frame;
    // SAFETY: The bound privilege-stack lease exclusively owns the complete
    // aligned frame slot, which remains inaccessible to user mappings.
    unsafe { ptr::write_volatile(frame, scheduler_frame::initial(user_code_address)) };
    compiler_fence(Ordering::SeqCst);
    Ok(task_space {
        root,
        physical_pages: [code_range.base, mailbox_range.base, stack_range.base],
        privilege_stack,
        privilege_stack_range,
        frame,
        payload_length,
    })
}

fn allocate_data(
    allocator: &mut frame_allocator<'_>,
    count: usize,
    alignment: usize,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    // SAFETY: The allocator grants the callback exclusive access to the exact
    // whole-frame extent for synchronous destructive initialization.
    unsafe {
        allocator.allocate_with(count, alignment, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

unsafe fn copy_payload(
    start: *const u8,
    end: *const u8,
    destination: physical_range,
) -> Result<usize, &'static str> {
    let length = (end as usize)
        .checked_sub(start as usize)
        .ok_or("payload_extent_invalid")?;
    if length == 0 || length > frame_size as usize || destination.length != frame_size {
        return Err("payload_extent_invalid");
    }
    let destination =
        usize::try_from(destination.base).map_err(|_| "payload_destination_invalid")? as *mut u8;
    // SAFETY: The source linker range and exclusive destination both cover the
    // exact checked byte count and cannot overlap.
    unsafe { ptr::copy_nonoverlapping(start, destination, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(length)
}

fn payload_matches(start: *const u8, length: usize, destination: u64) -> bool {
    let Ok(destination) = usize::try_from(destination) else {
        return false;
    };
    for offset in 0..length {
        // SAFETY: Preparation proved both complete ranges live for this check.
        let expected = unsafe { ptr::read(start.add(offset)) };
        let actual = unsafe { ptr::read_volatile((destination as *const u8).add(offset)) };
        if expected != actual {
            return false;
        }
    }
    true
}

fn range_is_zero(range: physical_range) -> bool {
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    let Ok(length) = usize::try_from(range.length) else {
        return false;
    };
    for offset in 0..length {
        // SAFETY: The allocator-owned identity-mapped range covers this byte.
        if unsafe { ptr::read_volatile((base as *const u8).add(offset)) } != 0 {
            return false;
        }
    }
    true
}

fn validate_mapping(
    evidence: mapping_evidence,
    physical_address: u64,
    writable: bool,
    executable: bool,
) -> Result<(), &'static str> {
    if evidence.physical_address != physical_address
        || !evidence.permissions.present
        || !evidence.permissions.user
        || evidence.permissions.writable != writable
        || evidence.permissions.executable != executable
    {
        return Err("mapping_permissions_invalid");
    }
    Ok(())
}

pub(super) fn attach_task_space(
    circles: &mut circle_table,
    circle: circle_id,
) -> Result<(), &'static str> {
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "circle_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "circle_root_attachment_rollback_failed")?;
        return Err("circle_mapping_attachment_failed");
    }
    Ok(())
}

pub(super) fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    principal: u64,
    payload_length: usize,
) -> Result<task_identity, &'static str> {
    let code_end = user_code_address
        .checked_add(payload_length as u64)
        .ok_or("task_code_extent_invalid")?;
    let bounds = task_bounds::new(
        user_code_address,
        code_end,
        user_code_address,
        user_stack_address,
        user_stack_top,
        user_stack_top,
    )
    .ok_or("task_bounds_invalid")?;
    let principal = fixture_principal_id::new(principal).ok_or("task_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(
                circle,
                principal,
                synthetic_origin_metadata::receiver_role_fixture,
                bounds,
            ),
        )
        .map_err(|_| "task_create_failed")
}

fn validate_ready_ownership(
    initial: frame_accounting,
    ready: frame_accounting,
    circles: &circle_table,
    tasks: &task_table,
    scheduler: &scheduler_table,
    faulting: &task_space,
    peer: &task_space,
) -> Result<(), &'static str> {
    if initial.free.checked_sub(22) != Some(ready.free)
        || ready.data_owned != 14
        || ready.table_owned != 8
        || ready.active_leases != 10
        || ready.bound_leases != 10
    {
        return Err("ready_frame_accounting_invalid");
    }
    let circle_accounting = circles.accounting();
    let scheduler_snapshot = scheduler
        .snapshot(tasks)
        .map_err(|_| "ready_scheduler_invalid")?;
    if circle_accounting.circles != 2
        || circle_accounting.tasks != 2
        || circle_accounting.task_contexts != 2
        || circle_accounting.endpoints != 0
        || circle_accounting.sigils != 0
        || circle_accounting.address_spaces != 2
        || circle_accounting.mappings != 6
        || tasks.active_count() != 2
        || tasks.attached_context_count() != 2
        || scheduler_snapshot.admitted() != 2
        || scheduler_snapshot.runnable() != 2
        || scheduler_snapshot.current().is_some()
        || scheduler_snapshot.next_dispatch_sequence() != Some(0)
        || scheduler_snapshot.next_preemption_sequence() != Some(0)
    {
        return Err("ready_object_accounting_invalid");
    }
    let ranges = [
        physical_range::new(faulting.physical_pages[0], frame_size),
        physical_range::new(faulting.physical_pages[1], frame_size),
        physical_range::new(faulting.physical_pages[2], frame_size),
        physical_range::new(
            faulting
                .root
                .root_address()
                .map_err(|_| "faulting_root_missing")?,
            4 * frame_size,
        ),
        faulting.privilege_stack_range,
        physical_range::new(peer.physical_pages[0], frame_size),
        physical_range::new(peer.physical_pages[1], frame_size),
        physical_range::new(peer.physical_pages[2], frame_size),
        physical_range::new(
            peer.root.root_address().map_err(|_| "peer_root_missing")?,
            4 * frame_size,
        ),
        peer.privilege_stack_range,
    ];
    for (index, range) in ranges.iter().enumerate() {
        if range.length == 0
            || range
                .end()
                .is_none_or(|end| end > paging::identity_map_limit)
            || ranges[index + 1..]
                .iter()
                .any(|other| ranges_overlap(*range, *other))
        {
            return Err("task_owned_extents_overlap");
        }
    }
    if !mailbox_is_zero(faulting) || !mailbox_is_zero(peer) {
        return Err("initial_mailbox_state_invalid");
    }
    Ok(())
}

pub(super) fn ranges_overlap(left: physical_range, right: physical_range) -> bool {
    let Some(left_end) = left.end() else {
        return true;
    };
    let Some(right_end) = right.end() else {
        return true;
    };
    left.length != 0 && right.length != 0 && left.base < right_end && right.base < left_end
}

pub(super) fn release_task_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: circle_id,
    space: &mut task_space,
) -> Result<(), &'static str> {
    let mut code = space
        .root
        .unmap_owned(allocator, user_code_address)
        .map_err(|_| "code_mapping_withdrawal_failed")?;
    let mut mailbox = space
        .root
        .unmap_owned(allocator, user_mailbox_address)
        .map_err(|_| "mailbox_mapping_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, user_stack_address)
        .map_err(|_| "stack_mapping_withdrawal_failed")?;
    // SAFETY: The kernel root is active and this private root has no mappings.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "address_space_destroy_failed")?;
    // SAFETY: Each unmap returned one exclusive, unbound data lease.
    unsafe { allocator.release_with(&mut code, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "code_frame_release_failed")?;
    // SAFETY: The same complete clearing contract applies to the mailbox.
    unsafe { allocator.release_with(&mut mailbox, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "mailbox_frame_release_failed")?;
    // SAFETY: The same complete clearing contract applies to the user stack.
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "stack_frame_release_failed")?;
    allocator
        .unbind(&space.privilege_stack)
        .map_err(|_| "privilege_stack_unbind_failed")?;
    // SAFETY: Completion abandoned this inactive task stack and restored the
    // bootstrap TSS RSP0 before its entire bound lease was withdrawn.
    unsafe {
        allocator.release_with(&mut space.privilege_stack, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "privilege_stack_release_failed")?;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "circle_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "circle_root_detachment_failed")?;
    retire_task_space_metadata(space);
    Ok(())
}

fn retire_task_space_metadata(space: &mut task_space) {
    debug_assert!(space.privilege_stack.range().is_none());
    debug_assert!(space.root.root_address().is_err());
    // SAFETY: All mapped storage, root tables, and the privilege stack have
    // been cleared and released. These valid neutral values retire the local
    // copies of their physical extents and saved-frame pointer.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(space.physical_pages), [0; 3]);
        ptr::write_volatile(
            ptr::addr_of_mut!(space.privilege_stack_range),
            physical_range::new(0, 0),
        );
        ptr::write_volatile(ptr::addr_of_mut!(space.frame), ptr::null_mut());
        ptr::write_volatile(ptr::addr_of_mut!(space.payload_length), 0);
    }
}

fn mailbox_matches(space: &task_space, role: fixture_role, arm: u64) -> bool {
    let (canary_a, canary_b) = role.canaries();
    read_mailbox_word(space.physical_pages[1], mailbox_canary_a_offset) == Some(canary_a)
        && read_mailbox_word(space.physical_pages[1], mailbox_canary_b_offset) == Some(canary_b)
        && read_mailbox_word(space.physical_pages[1], mailbox_arm_offset) == Some(arm)
}

pub(super) fn mailbox_is_zero(space: &task_space) -> bool {
    [
        mailbox_canary_a_offset,
        mailbox_canary_b_offset,
        mailbox_arm_offset,
    ]
    .into_iter()
    .all(|offset| read_mailbox_word(space.physical_pages[1], offset) == Some(0))
}

pub(super) fn read_mailbox_word(base: u64, offset: u64) -> Option<u64> {
    let address = base.checked_add(offset)?;
    if address & (mem::align_of::<u64>() as u64 - 1) != 0 {
        return None;
    }
    // SAFETY: The caller retains the allocator-owned identity-mapped mailbox
    // page and every fixed offset names one complete aligned word.
    Some(unsafe { ptr::read_volatile(address as usize as *const u64) })
}

pub(super) fn write_mailbox_word(base: u64, offset: u64, value: u64) -> Result<(), &'static str> {
    let address = base
        .checked_add(offset)
        .ok_or("mailbox_word_address_invalid")?;
    if address & (mem::align_of::<u64>() as u64 - 1) != 0 {
        return Err("mailbox_word_address_invalid");
    }
    // SAFETY: The current root shares the supervisor-only identity alias for
    // this exclusively owned mailbox page; user access is through its private
    // high mapping and cannot overlap kernel execution here.
    unsafe { ptr::write_volatile(address as usize as *mut u64, value) };
    compiler_fence(Ordering::SeqCst);
    Ok(())
}

pub(super) fn relocated_user_address(
    template_start: *const u8,
    target: *const u8,
    payload_length: usize,
) -> Option<u64> {
    let offset = (target as usize).checked_sub(template_start as usize)?;
    if offset >= payload_length || offset >= frame_size as usize {
        return None;
    }
    user_code_address.checked_add(offset as u64)
}

fn ready_gate_registers_are_exact(frame: scheduler_frame) -> bool {
    frame.accumulator == operation_ready
        && frame.base == 0
        && frame.counter == 0
        && frame.data == 0
        && frame.frame_pointer == 0
        && frame.source_index == 0
        && frame.destination_index == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
}

fn completion_gate_registers_are_exact(frame: scheduler_frame) -> bool {
    frame.accumulator == operation_complete
        && frame.base == 0
        && frame.counter == 0
        && frame.data == 0
        && frame.frame_pointer == 0
        && frame.source_index == 0
        && frame.destination_index == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
        && frame.register_15 == 0
}

pub(super) fn all_general_registers_zero(frame: scheduler_frame) -> bool {
    frame.accumulator == 0
        && frame.base == 0
        && frame.counter == 0
        && frame.data == 0
        && frame.frame_pointer == 0
        && frame.source_index == 0
        && frame.destination_index == 0
        && frame.register_8 == 0
        && frame.register_9 == 0
        && frame.register_10 == 0
        && frame.register_11 == 0
        && frame.register_12 == 0
        && frame.register_13 == 0
        && frame.register_14 == 0
        && frame.register_15 == 0
}

fn emit_evidence(evidence: scheduler_evidence) {
    let fault = evidence
        .trace
        .fault
        .unwrap_or_else(|| fail("contained_fault_evidence_missing"));
    let stack_segment = fault
        .stack_segment
        .unwrap_or_else(|| fail("contained_fault_stack_missing"));
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.ready","level":"info","policy":"fixed_round_robin","circles":2,"tasks":2,"queue_capacity":2,"preemption_budget":2,"timer_vector":32,"master_mask":254,"slave_mask":255,"privilege_stack_pages":4,"data_frames":14,"table_frames":8,"free_frame_delta":22,"ts_set":true,"fpu_state_saved":false,"nmi_masked":true,"bsp_only":true}}"#
    );
    for (role, circle_generation, task_generation) in [
        (
            fixture_role::faulting,
            evidence.faulting_circle_generation,
            evidence.faulting_task_generation,
        ),
        (
            fixture_role::peer,
            evidence.peer_circle_generation,
            evidence.peer_task_generation,
        ),
    ] {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.scheduler_test.task.ready","level":"info","role":"{}","circle_generation":{},"task_generation":{},"state":"ready","mappings":3,"privilege_stack_pages":4,"context_attached":true,"initial_if":false}}"#,
            role.label(),
            circle_generation,
            task_generation
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.dispatched","level":"info","role":"faulting_fixture","dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[0]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.preempted","level":"info","role":"faulting_fixture","preemption":{},"tick":{},"state":"suspended","frame_retained":true,"eoi_sent":true}}"#,
        evidence.trace.preemptions[0], evidence.trace.ticks[0]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.dispatched","level":"info","role":"peer_fixture","dispatch":{},"kind":"initial","state":"running","root_selected":true,"stack_selected":true,"initial_if":false}}"#,
        evidence.trace.dispatches[1]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.preempted","level":"info","role":"peer_fixture","preemption":{},"tick":{},"state":"suspended","frame_retained":true,"eoi_sent":true}}"#,
        evidence.trace.preemptions[1], evidence.trace.ticks[1]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.resumed","level":"info","role":"faulting_fixture","dispatch":{},"resume":0,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[2]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.exception","level":"info","vector":{},"kind":"invalid_opcode","error_code":"{:#018x}","instruction_pointer":"{:#018x}","stack_pointer":"{:#018x}","stack_segment":"{:#06x}","stack_switched":true,"code_segment":"{:#06x}","flags":"{:#018x}","origin":"user","disposition":"contained_test","fault_address":null}}"#,
        fault.vector,
        fault.error_code,
        fault.instruction_pointer,
        fault.stack_pointer,
        stack_segment,
        fault.code_segment,
        fault.flags
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.faulted","level":"info","role":"faulting_fixture","state":"faulted","vector":6,"error_code":0,"contained":true,"frame_retained":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.resumed","level":"info","role":"peer_fixture","dispatch":{},"resume":1,"state":"running","context_preserved":true,"frame_owned":true}}"#,
        evidence.trace.dispatches[3]
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.task.completed","level":"info","role":"peer_fixture","state":"exited","completion_gate":true,"peer_continued":true,"kernel_continuation_restored":{}}}"#,
        evidence.trace.kernel_continuation_restored
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.interrupts.masked","level":"info","master_mask":255,"slave_mask":255,"if_enabled":false}}"#
    );
    for role in [fixture_role::faulting, fixture_role::peer] {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.scheduler_test.task.withdrawn","level":"info","role":"{}","context_withdrawn":true,"root_withdrawn":true,"storage_wiped":true,"privilege_stack_wiped":true}}"#,
            role.label()
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":0,"table_owned_frames":0,"active_circles":0,"active_tasks":0,"active_mappings":0,"active_leases":0,"bound_leases":0,"scheduler_depth":0,"attached_contexts":0,"kernel_cr3_restored":true,"tss_rsp0_restored":true,"ts_restored":true}}"#,
        evidence.initial.free, evidence.initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.passed","level":"info","dispatches":4,"preemptions":2,"resumes":{},"faults":{},"completions":{},"peer_continued":{}}}"#,
        evidence.trace.resume_count,
        evidence.trace.fault_count,
        evidence.trace.completion_count,
        evidence.trace.peer_continued
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.scheduler_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_bounds() -> task_bounds {
        task_bounds::new(
            user_code_address,
            user_code_address + 0x100,
            user_code_address,
            user_stack_address,
            user_stack_top,
            user_stack_top,
        )
        .expect("fixture bounds")
    }

    fn valid_fault_snapshot(instruction_pointer: u64) -> exceptions::exception_snapshot {
        exceptions::exception_snapshot {
            accumulator: 0,
            vector: invalid_opcode_vector,
            error_code: 0,
            instruction_pointer,
            code_segment: u64::from(descriptors::user_code_selector),
            flags: running_user_flags,
            stack_pointer: user_stack_top,
            stack_segment: Some(u64::from(descriptors::user_data_selector)),
            fault_address: None,
        }
    }

    #[test]
    fn scheduler_frame_layout_matches_every_returning_entry() {
        assert_eq!(mem::size_of::<scheduler_frame>(), 22 * 8);
        assert_eq!(mem::offset_of!(scheduler_frame, vector), 15 * 8);
        assert_eq!(mem::offset_of!(scheduler_frame, code_segment), 18 * 8);
        assert_eq!(mem::offset_of!(scheduler_frame, stack_segment), 21 * 8);
    }

    #[test]
    fn synthesized_frame_starts_sanitized_and_interrupts_disabled() {
        let frame = scheduler_frame::initial(user_code_address);
        assert_eq!(frame.instruction_pointer, user_code_address);
        assert_eq!(frame.flags, initial_user_flags);
        assert_eq!(frame.code_segment, 0x33);
        assert_eq!(frame.stack_segment, 0x2b);
        assert_eq!(frame.stack_pointer, user_stack_top);
        assert!(all_general_registers_zero(frame));
    }

    #[test]
    fn overlap_check_is_half_open_and_overflow_conservative() {
        assert!(!ranges_overlap(
            physical_range::new(0x1000, 0x1000),
            physical_range::new(0x2000, 0x1000)
        ));
        assert!(ranges_overlap(
            physical_range::new(0x1000, 0x1001),
            physical_range::new(0x2000, 0x1000)
        ));
        assert!(ranges_overlap(
            physical_range::new(u64::MAX, 2),
            physical_range::new(0, 1)
        ));
    }

    #[test]
    fn frame_pointer_must_name_the_exact_owned_stack_tail() {
        let stack = physical_range::new(0x20_000, descriptors::privilege_stack_size as u64);
        let frame_address = stack.end().expect("stack end") - context_size;
        assert!(frame_address_matches_stack(
            frame_address as usize as *const scheduler_frame,
            stack
        ));
        assert!(!frame_address_matches_stack(ptr::null(), stack));
        assert!(!frame_address_matches_stack(
            (frame_address + 1) as usize as *const scheduler_frame,
            stack
        ));
        assert!(!frame_address_matches_stack(
            (frame_address - 8) as usize as *const scheduler_frame,
            stack
        ));
        assert!(!frame_address_matches_stack(
            (u64::MAX - 7) as usize as *const scheduler_frame,
            stack
        ));
    }

    #[test]
    fn user_return_field_gate_rejects_each_mutated_class() {
        let bounds = fixture_bounds();
        let frame = scheduler_frame::initial(user_code_address);
        assert!(user_return_fields_are_valid(frame, bounds));

        for mutation in [
            scheduler_frame {
                error_code: 1,
                ..frame
            },
            scheduler_frame {
                code_segment: u64::from(descriptors::kernel_code_selector),
                ..frame
            },
            scheduler_frame {
                code_segment: frame.code_segment | (1 << 32),
                ..frame
            },
            scheduler_frame {
                stack_segment: 0x10,
                ..frame
            },
            scheduler_frame {
                stack_pointer: user_stack_top - 8,
                ..frame
            },
            scheduler_frame { flags: 0, ..frame },
            scheduler_frame {
                flags: frame.flags | (1 << 12),
                ..frame
            },
            scheduler_frame {
                flags: frame.flags | (1 << 10),
                ..frame
            },
            scheduler_frame {
                flags: frame.flags | (1 << 14),
                ..frame
            },
            scheduler_frame {
                instruction_pointer: user_code_address - 1,
                ..frame
            },
            scheduler_frame {
                instruction_pointer: user_code_address + 0x100,
                ..frame
            },
        ] {
            assert!(!user_return_fields_are_valid(mutation, bounds));
        }
    }

    #[test]
    fn user_return_field_gate_accepts_the_task_specific_stack_top() {
        let bounds = fixture_bounds();
        let expected_stack_top = user_stack_top - 8;
        let frame = scheduler_frame {
            stack_pointer: expected_stack_top,
            ..scheduler_frame::initial(user_code_address)
        };

        assert!(!user_return_fields_are_valid(frame, bounds));
        assert!(user_return_fields_are_valid_with_stack_top(
            frame,
            bounds,
            expected_stack_top
        ));
    }

    #[test]
    fn user_return_field_gate_accepts_a_live_stack_frame_within_bounds() {
        let bounds = fixture_bounds();
        let frame = scheduler_frame {
            stack_pointer: user_stack_top - 56,
            ..scheduler_frame::initial(user_code_address)
        };

        assert!(!user_return_fields_are_valid(frame, bounds));
        assert!(user_return_fields_are_valid_within_stack(frame, bounds));
    }

    #[test]
    fn gate_register_contract_requires_the_exact_mailbox_register() {
        let mailbox = 0x0000_6000_0008_0000;
        let frame = scheduler_frame {
            register_15: mailbox,
            ..scheduler_frame::initial(user_code_address)
        };

        assert!(gate_registers_are_canonical(frame, mailbox));
        assert!(!gate_registers_are_canonical(
            scheduler_frame {
                register_15: mailbox + 8,
                ..frame
            },
            mailbox
        ));
        assert!(!gate_registers_are_canonical(
            scheduler_frame {
                register_14: 1,
                ..frame
            },
            mailbox
        ));
    }

    #[test]
    fn invalid_opcode_predicate_rejects_every_observed_field_mutation() {
        let instruction_pointer = user_code_address + 0x80;
        let registers = scheduler_frame {
            vector: invalid_opcode_vector,
            flags: running_user_flags,
            ..scheduler_frame::initial(instruction_pointer)
        };
        let snapshot = valid_fault_snapshot(instruction_pointer);
        assert!(expected_invalid_opcode_matches(
            snapshot,
            registers,
            instruction_pointer
        ));
        assert!(expected_invalid_opcode_matches(
            exceptions::exception_snapshot {
                flags: running_user_flags | resume_flag,
                ..snapshot
            },
            scheduler_frame {
                flags: running_user_flags | resume_flag,
                ..registers
            },
            instruction_pointer
        ));

        for mutation in [
            exceptions::exception_snapshot {
                accumulator: 1,
                ..snapshot
            },
            exceptions::exception_snapshot {
                vector: invalid_opcode_vector + 1,
                ..snapshot
            },
            exceptions::exception_snapshot {
                error_code: 1,
                ..snapshot
            },
            exceptions::exception_snapshot {
                instruction_pointer: instruction_pointer + 1,
                ..snapshot
            },
            exceptions::exception_snapshot {
                code_segment: u64::from(descriptors::kernel_code_selector),
                ..snapshot
            },
            exceptions::exception_snapshot {
                flags: initial_user_flags,
                ..snapshot
            },
            exceptions::exception_snapshot {
                stack_pointer: user_stack_top - 8,
                ..snapshot
            },
            exceptions::exception_snapshot {
                stack_segment: None,
                ..snapshot
            },
            exceptions::exception_snapshot {
                stack_segment: Some(0x10),
                ..snapshot
            },
            exceptions::exception_snapshot {
                fault_address: Some(0),
                ..snapshot
            },
        ] {
            assert!(!expected_invalid_opcode_matches(
                mutation,
                registers,
                instruction_pointer
            ));
        }

        for mutation in [
            scheduler_frame {
                accumulator: 1,
                ..registers
            },
            scheduler_frame {
                base: 1,
                ..registers
            },
            scheduler_frame {
                counter: 1,
                ..registers
            },
            scheduler_frame {
                data: 1,
                ..registers
            },
            scheduler_frame {
                frame_pointer: 1,
                ..registers
            },
            scheduler_frame {
                source_index: 1,
                ..registers
            },
            scheduler_frame {
                destination_index: 1,
                ..registers
            },
            scheduler_frame {
                register_8: 1,
                ..registers
            },
            scheduler_frame {
                register_9: 1,
                ..registers
            },
            scheduler_frame {
                register_10: 1,
                ..registers
            },
            scheduler_frame {
                register_11: 1,
                ..registers
            },
            scheduler_frame {
                register_12: 1,
                ..registers
            },
            scheduler_frame {
                register_13: 1,
                ..registers
            },
            scheduler_frame {
                register_14: 1,
                ..registers
            },
            scheduler_frame {
                register_15: 1,
                ..registers
            },
            scheduler_frame {
                vector: invalid_opcode_vector + 1,
                ..registers
            },
            scheduler_frame {
                error_code: 1,
                ..registers
            },
            scheduler_frame {
                instruction_pointer: instruction_pointer + 1,
                ..registers
            },
            scheduler_frame {
                code_segment: u64::from(descriptors::kernel_code_selector),
                ..registers
            },
            scheduler_frame {
                flags: initial_user_flags,
                ..registers
            },
            scheduler_frame {
                stack_pointer: user_stack_top - 8,
                ..registers
            },
            scheduler_frame {
                stack_segment: 0x10,
                ..registers
            },
        ] {
            assert!(!expected_invalid_opcode_matches(
                snapshot,
                mutation,
                instruction_pointer
            ));
        }
    }
}
