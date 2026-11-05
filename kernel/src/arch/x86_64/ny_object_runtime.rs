//! Removable execution proof for one inspected Ny object and its narrow shim.
//!
//! This owns the same private roots and eager legacy CPU state as the preceding
//! application proof. It does not admit arbitrary objects or define a Ny target.

use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::application_cpu::{x86_64_application_enter, x86_64_application_resume_kernel};
use super::owned_execution::{
    owned_execution, tls_bases_are_zero, user_code_base, user_data_base, user_stack_top,
};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
use super::{application_cpu, application_fault, descriptors, exceptions, interrupts, paging};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::task_identity;
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

const gate_vector: u64 = 0xf3;
const task_switched: u64 = 8;
const completion_status: u64 = 0x4e59;
const header_bytes: usize = 128;
const startup_magic: u64 = 0x4e59_5f53_5441_5254;
const expected_result: u64 = 50_000_000;
const timer_limit: u64 = 6000;
const abi_message: &[u8] = b"Ny native runtime error: incompatible nyruntime ABI (expected 1)\n\0";
const context_message: &[u8] = b"Ny Anomalous probe: injected context_end failure\n\0";

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_ny_object_gate_entry();
    static x86_64_ny_object_payload_start: u8;
    static x86_64_ny_object_payload_end: u8;
    static x86_64_ny_object_data_start: u8;
    static x86_64_ny_object_data_end: u8;
    static x86_64_ny_object_user_compiled_start: u8;
    static x86_64_ny_object_user_compiled_end: u8;
    static x86_64_ny_object_user_fail_ud2: u8;
    static x86_64_ny_object_user_start_return: u8;
    static x86_64_ny_object_user_exit_return: u8;
    static x86_64_ny_object_user_bss_start: u8;
    static x86_64_ny_object_user_bss_end: u8;
    static x86_64_ny_object_user_abi_message_start: u8;
    static x86_64_ny_object_user_abi_message_length: u8;
    static x86_64_ny_object_user_context_message_start: u8;
    static x86_64_ny_object_user_context_message_length: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
}

macro_rules! symbol_address {
    ($symbol:ident) => {{
        let address: u64;
        // SAFETY: Materialize a linker marker, including absolute ELF metadata,
        // as a number. Absolute markers are never dereferenced as Rust objects.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) address,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        address
    }};
}

#[derive(Clone, Copy)]
struct artifact {
    source: u64,
    length: u64,
    data_source: u64,
    data_length: u64,
    compiled_start: u64,
    compiled_end: u64,
    fail_instruction: u64,
    start_return: u64,
    exit_return: u64,
    bss_start: u64,
    bss_end: u64,
    messages: [u64; 2],
}

impl artifact {
    fn inspect() -> Self {
        let source = symbol_address!(x86_64_ny_object_payload_start);
        let data_source = symbol_address!(x86_64_ny_object_data_start);
        let image = Self {
            source,
            length: symbol_address!(x86_64_ny_object_payload_end)
                .checked_sub(source)
                .unwrap_or_else(|| fail("ny_object_payload_extent")),
            data_source,
            data_length: symbol_address!(x86_64_ny_object_data_end)
                .checked_sub(data_source)
                .unwrap_or_else(|| fail("ny_object_data_extent")),
            compiled_start: symbol_address!(x86_64_ny_object_user_compiled_start),
            compiled_end: symbol_address!(x86_64_ny_object_user_compiled_end),
            fail_instruction: symbol_address!(x86_64_ny_object_user_fail_ud2),
            start_return: symbol_address!(x86_64_ny_object_user_start_return),
            exit_return: symbol_address!(x86_64_ny_object_user_exit_return),
            bss_start: symbol_address!(x86_64_ny_object_user_bss_start),
            bss_end: symbol_address!(x86_64_ny_object_user_bss_end),
            messages: [
                symbol_address!(x86_64_ny_object_user_abi_message_start),
                symbol_address!(x86_64_ny_object_user_context_message_start),
            ],
        };
        let rodata = symbol_address!(kernel_rodata_start)..symbol_address!(kernel_rodata_end);
        if image.length == 0
            || image.length > 4096
            || image.data_length != header_bytes as u64
            || !rodata.contains(&source)
            || source
                .checked_add(image.length)
                .is_none_or(|end| end > rodata.end)
            || !rodata.contains(&data_source)
            || data_source
                .checked_add(image.data_length)
                .is_none_or(|end| end > rodata.end)
            || image.compiled_start < user_code_base
            || image.compiled_start >= image.compiled_end
            || image.compiled_end > user_code_base + image.length
            || [
                image.fail_instruction,
                image.start_return,
                image.exit_return,
            ]
            .iter()
            .any(|address| !(user_code_base..user_code_base + image.length).contains(address))
            || image.bss_start < user_data_base + image.data_length
            || image.bss_start >= image.bss_end
            || image.bss_end > user_data_base + 8192
            || symbol_address!(x86_64_ny_object_user_abi_message_length) != abi_message.len() as u64
            || symbol_address!(x86_64_ny_object_user_context_message_length)
                != context_message.len() as u64
        {
            fail("ny_object_artifact_geometry");
        }
        for (address, bytes) in image
            .messages
            .into_iter()
            .zip([abi_message, context_message])
        {
            if address < user_code_base
                || address
                    .checked_add(bytes.len() as u64)
                    .is_none_or(|end| end > user_code_base + image.length)
            {
                fail("ny_object_message_extent");
            }
            for (offset, expected) in bytes.iter().enumerate() {
                // SAFETY: Validated marker offsets select immutable bytes inside
                // the complete linked payload in supervisor rodata.
                let actual = unsafe {
                    ptr::read((source + address - user_code_base + offset as u64) as *const u8)
                };
                if actual != *expected {
                    fail("ny_object_message_source");
                }
            }
        }
        image
    }
}

#[derive(Clone, Copy)]
struct scenario_evidence {
    timer_ticks: u64,
    body_ticks: [u64; 2],
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 2],
    execution: owned_execution,
    cpu: application_cpu::cpu_session,
    image: artifact,
    scenario: usize,
    selected: usize,
    started: [bool; 2],
    failure_armed: bool,
    faulted: bool,
    exited: bool,
    timer_ticks: u64,
    body_ticks: [u64; 2],
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot exception initialization exclusively owns the IDT.
    unsafe {
        super::idt::install_gate(
            gate_vector as usize,
            x86_64_ny_object_gate_entry as *const () as u64,
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
        fail("ny_object_initial_state");
    }
    let image = artifact::inspect();
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Sole BSP, IF clear, NMI masked, initialized TSS.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let mut evidence = [scenario_evidence {
            timer_ticks: 0,
            body_ticks: [0; 2],
        }; 2];
        let mut vendor = "";
        for (scenario, item) in evidence.iter_mut().enumerate() {
            let (observed, completed) = run_owned(
                allocator,
                image,
                kernel_cr3,
                bootstrap_stack,
                scenario,
                initial,
            );
            if scenario == 0 {
                vendor = observed;
            } else if vendor != observed {
                fail("ny_object_vendor_changed");
            }
            *item = completed;
        }
        (initial, vendor, evidence)
    });
    let (initial, vendor, evidence) = result.unwrap_or_else(|error| fail(error.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("ny_object_final_interrupt_state");
    }
    emit_evidence(initial, vendor, evidence);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_owned(
    allocator: &mut frame_allocator<'_>,
    image: artifact,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
) -> (&'static str, scenario_evidence) {
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || bootstrap_stack == 0
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || initial.free < 36
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        fail("ny_object_setup_state");
    }
    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let mut execution = owned_execution::new();
    // SAFETY: The aggregate stays retained on every error in this owning scope.
    // Both payloads are the same inspected supervisor-resident immutable image.
    unsafe { execution.prepare(allocator, &mut circles, [(image.source, image.length); 2]) }
        .unwrap_or_else(|reason| fail(reason));
    for index in 0..2 {
        for offset in (0..image.data_length).step_by(8) {
            let mut word = [0; 8];
            for (byte, destination) in word.iter_mut().enumerate() {
                // SAFETY: The complete initialized 128-byte image was bounded
                // inside immutable linked supervisor rodata before allocation.
                *destination =
                    unsafe { ptr::read((image.data_source + offset + byte as u64) as *const u8) };
            }
            execution
                .write_data(index, offset, u64::from_le_bytes(word))
                .unwrap_or_else(|reason| fail(reason));
        }
        execution
            .write_data(index, 0, index as u64)
            .unwrap_or_else(|reason| fail(reason));
        execution
            .write_data(index, 8, scenario as u64)
            .unwrap_or_else(|reason| fail(reason));
        for offset in (0..header_bytes as u64).step_by(8) {
            let expected = match offset {
                0 => index as u64,
                8 => scenario as u64,
                80 => startup_magic,
                _ => 0,
            };
            if execution.read_data(index, offset) != Some(expected) {
                fail("ny_object_initialized_data");
            }
        }
        for address in image.bss_start..image.bss_end {
            let offset = address - user_data_base;
            let word = execution
                .read_data(index, offset & !7)
                .unwrap_or_else(|| fail("ny_object_bss_extent"));
            if word.to_le_bytes()[(offset & 7) as usize] != 0 {
                fail("ny_object_bss_initial_state");
            }
        }
        if (0..64)
            .step_by(8)
            .any(|offset| execution.read_stack(index, offset) != Some(0))
        {
            fail("ny_object_stack_initial_state");
        }
    }
    if tasks.active_count() != 0 {
        fail("ny_object_prepare_published_task");
    }
    let identities = core::array::from_fn(|index| {
        execution
            .commit(index, &mut tasks, &mut circles, index as u64 + 301)
            .unwrap_or_else(|reason| fail(reason))
    });
    let mut scheduler = scheduler_table::new(2).unwrap_or_else(|_| fail("ny_object_scheduler"));
    for identity in identities {
        scheduler
            .admit(&mut circles, &mut tasks, identity)
            .unwrap_or_else(|_| fail("ny_object_admit"));
    }
    // SAFETY: Both committed owners and transition stacks stay retained.
    unsafe { execution.activate(allocator) }.unwrap_or_else(|reason| fail(reason));
    let accounting = allocator.accounting();
    if accounting.free.checked_add(36) != Some(initial.free)
        || accounting.data_owned != 18
        || accounting.table_owned != 18
        || accounting.active_leases != 10
        || accounting.bound_leases != 10
        || execution.root(0).ok() == execution.root(1).ok()
        || execution.frame(0).ok() == execution.frame(1).ok()
        || execution.transition_stack_top(0).ok() == execution.transition_stack_top(1).ok()
    {
        fail("ny_object_live_ownership");
    }
    let mut state = runtime {
        circles,
        tasks,
        scheduler,
        identities,
        execution,
        cpu: application_cpu::cpu_session::new(),
        image,
        scenario,
        selected: 0,
        started: [false; 2],
        failure_armed: false,
        faulted: false,
        exited: false,
        timer_ticks: 0,
        body_ticks: [0; 2],
        last_tick: 0,
        kernel_cr3,
        bootstrap_stack,
        kernel_stack: 0,
        original_ts: paging::read_cr0() & task_switched,
    };
    // SAFETY: Prepare CPU storage only at this stable address. Bootstrap
    // residency retains the whole runtime while the kernel stack is suspended.
    unsafe { state.cpu.prepare() }.unwrap_or_else(|reason| fail(reason));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("ny_object_start"));
    if first.task() != identities[0]
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        fail("ny_object_first_dispatch");
    }
    let frame = state.select(0);
    let pointer = ptr::from_mut(&mut state);
    if active
        .compare_exchange(
            ptr::null_mut(),
            pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        fail("ny_object_publication");
    }
    // SAFETY: Raw access ends before entry; handlers receive the exclusive
    // runtime borrow only while this bootstrap continuation is suspended.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("ny_object_initial_cpu_restore");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    if status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || paging::read_cr0() & task_switched != state.original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || !state.faulted
        || !state.exited
        || state.started != [true; 2]
        || !(1..=timer_limit).contains(&state.timer_ticks)
        || state.body_ticks[1] == 0
        || (scenario == 0 && state.body_ticks[0] != 0)
        || (scenario == 1 && state.body_ticks[0] == 0)
        || state.body_ticks[0] + state.body_ticks[1] > state.timer_ticks
    {
        fail("ny_object_continuation_restore");
    }
    for (index, identity) in identities.into_iter().enumerate() {
        let terminal = if index == 0 {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|snapshot| snapshot.state() == terminal)
        {
            fail("ny_object_terminal_task");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("ny_object_context_withdrawal"));
    }
    // SAFETY: Bootstrap continuation restored; all contexts withdrawn. Every
    // cleanup error halts with the full aggregate and its obligations retained.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|reason| fail(reason));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("ny_object_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || snapshot.next_dispatch_sequence() != Some(2)
        || snapshot.next_preemption_sequence() != Some(0)
        || identities
            .iter()
            .any(|identity| state.tasks.validate(*identity).is_ok())
    {
        fail("ny_object_final_accounting");
    }
    let evidence = scenario_evidence {
        timer_ticks: state.timer_ticks,
        body_ticks: state.body_ticks,
    };
    // SAFETY: All native consumers retired; erase consumed continuation metadata.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.bootstrap_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.original_ts), 0);
    }
    (state.cpu.cpu_vendor(), evidence)
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        let Some(identity) = self.identities.get(self.selected) else {
            return false;
        };
        self.execution.frame_owned(self.selected, frame)
            && self.execution.root(self.selected).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(self.selected).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self.cpu.controls_are_active()
            && self
                .scheduler
                .snapshot(&self.tasks)
                .is_ok_and(|snapshot| snapshot.current() == Some(*identity))
            && self.tasks.snapshot(*identity).is_ok_and(|snapshot| {
                snapshot.state() == task_state::running && snapshot.context_attached()
            })
    }
    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(self.identities[self.selected]) else {
            return false;
        };
        // SAFETY: Exact complete retained frame, validated before this read.
        let value = unsafe { ptr::read(frame) };
        user_return_fields_are_valid_within_stack(value, snapshot.bounds())
            && self
                .execution
                .admission(self.selected)
                .is_ok_and(|admission| {
                    admission.accepts_return(value.instruction_pointer, value.stack_pointer)
                })
            && value.flags & !0x0000_0000_0025_0fd7 == 0
            && (!self.started[self.selected] || value.flags & 0x202 == 0x202)
    }
    fn select(&mut self, index: usize) -> *mut scheduler_frame {
        // SAFETY: Sole BSP and both retained roots share the handler's owned
        // high-alias stack. The caller holds the nonreentrant dispatch latch.
        unsafe { self.execution.select(index) }.unwrap_or_else(|reason| fail(reason));
        self.selected = index;
        let frame = self
            .execution
            .frame(index)
            .unwrap_or_else(|reason| fail(reason));
        if !self.frame_valid(frame) {
            fail("ny_object_selected_frame");
        }
        frame
    }
    fn header(&mut self) -> [u64; 16] {
        let mut bytes = [0; header_bytes];
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[self.selected],
                user_data_base,
                &mut bytes,
            )
            .unwrap_or_else(|_| fail("ny_object_header_copy"));
        let words = core::array::from_fn(|index| {
            u64::from_le_bytes(bytes[index * 8..index * 8 + 8].try_into().unwrap())
        });
        if words[0] != self.selected as u64
            || words[1] != self.scenario as u64
            || words[2] != 1
            || words[8] != 1
            || words[9] != 1
            || words[10] != startup_magic
            || words[11..].iter().any(|value| *value != 0)
        {
            fail("ny_object_header_contract");
        }
        words
    }
    fn stack_guard_is_clear(&self) -> bool {
        (0..64)
            .step_by(8)
            .all(|offset| self.execution.read_stack(self.selected, offset) == Some(0))
    }
}

pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}

pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let pointer = active.load(Ordering::Acquire);
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}

fn enter_dispatch() -> *mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) {
        fail("ny_object_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() || interrupts::interrupts_enabled() {
        fail("ny_object_inactive_dispatch");
    }
    // The sole BSP, IF-clear/NMI-masked latch grants one exclusive runtime
    // borrow until the final user restore or single-use bootstrap return.
    pointer
}

pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    // SAFETY: The live runtime and every CPU image remain on the retained
    // bootstrap stack. Save the user before processing any interrupt state.
    let state = unsafe { &mut *enter_dispatch() };
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame.cast()) {
        fail("ny_object_timer_frame");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if value.vector != 0x20
        || !state.started[state.selected]
        || state.failure_armed
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.timer_ticks >= timer_limit
    {
        fail("ny_object_timer_observation");
    }
    state.last_tick = observed.tick();
    state.timer_ticks += 1;
    // Compiled code may use every ordinary register and any admitted stack
    // depth. Count actual object instructions, not a startup/shim wait loop.
    if (state.image.compiled_start..state.image.compiled_end).contains(&value.instruction_pointer)
        && state.execution.read_data(state.selected, 32) == Some(1)
        && state.execution.read_data(state.selected, 40) == Some(0)
    {
        state.body_ticks[state.selected] += 1;
    }
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("ny_object_timer_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    Some(frame)
}

pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    // SAFETY: The dispatch latch retains the complete frame, roots, and images.
    let state = unsafe { &mut *enter_dispatch() };
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
    {
        fail("ny_object_unattributable_fault");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if state.selected != 0
        || !state.failure_armed
        || state.faulted
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.instruction_pointer != state.image.fail_instruction
        || snapshot.flags & 0x200 != 0
        || value.vector != snapshot.vector
        || value.error_code != snapshot.error_code
        || value.instruction_pointer != snapshot.instruction_pointer
        || value.code_segment != snapshot.code_segment
        || value.flags != snapshot.flags
        || value.stack_pointer != snapshot.stack_pointer
        || Some(value.stack_segment) != snapshot.stack_segment
    {
        fail("ny_object_expected_failure");
    }
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.identities[0])
        .unwrap_or_else(|_| fail("ny_object_fault_transition"));
    if transition.task() != state.identities[0]
        || transition.dispatch().task() != state.identities[1]
        || transition.dispatch().sequence() != 1
        || transition.dispatch().context() != scheduler_context::initial
    {
        fail("ny_object_peer_dispatch");
    }
    state.failure_armed = false;
    state.faulted = true;
    let selected = state.select(1);
    if !unsafe { state.cpu.restore_task(1) } {
        fail("ny_object_peer_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

pub(super) fn resume_kernel_after_fault() -> ! {
    fail("ny_object_unexpected_terminal_fault")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_ny_object_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = unsafe { &mut *enter_dispatch() };
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame) {
        fail("ny_object_gate_frame");
    }
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector || value.error_code != 0 {
        fail("ny_object_gate_vector");
    }
    let words = state.header();
    if !state.stack_guard_is_clear() {
        fail("ny_object_stack_guard");
    }
    match value.accumulator {
        1 => {
            if state.started[state.selected]
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || value.instruction_pointer != state.image.start_return
                || words[3..8].iter().any(|word| *word != 0)
                || !state
                    .cpu
                    .task_image(state.selected)
                    .is_some_and(|image| image.is_initial_clean())
            {
                fail("ny_object_start_gate");
            }
            let mut bss = [0; 64];
            let length = usize::try_from(state.image.bss_end - state.image.bss_start)
                .unwrap_or_else(|_| fail("ny_object_bss_size"));
            for offset in (0..length).step_by(bss.len()) {
                let count = (length - offset).min(bss.len());
                state
                    .execution
                    .copy_from_user(
                        &state.tasks,
                        &state.circles,
                        &state.scheduler,
                        state.identities[state.selected],
                        state.image.bss_start + offset as u64,
                        &mut bss[..count],
                    )
                    .unwrap_or_else(|_| fail("ny_object_bss_copy"));
                if bss[..count].iter().any(|byte| *byte != 0) {
                    fail("ny_object_bss_startup");
                }
            }
            state.started[state.selected] = true;
            unsafe { ptr::addr_of_mut!((*frame).flags).write(0x202) };
            compiler_fence(Ordering::SeqCst);
        }
        2 => {
            let expected = if state.scenario == 0 {
                abi_message
            } else {
                context_message
            };
            let count = state.scenario as u64;
            if state.selected != 0
                || !state.started[0]
                || state.failure_armed
                || state.faulted
                || words[3] != 1
                || words[4] != count
                || words[5] != count
                || words[6] != 1
                || words[7]
                    != if state.scenario == 0 {
                        0
                    } else {
                        expected_result
                    }
                || value.instruction_pointer != state.image.fail_instruction
                || value.destination_index != state.image.messages[state.scenario]
                || (state.scenario == 0 && state.body_ticks[0] != 0)
                || (state.scenario == 1 && state.body_ticks[0] == 0)
            {
                fail("ny_object_failure_gate");
            }
            let mut message = [0; 128];
            state
                .execution
                .copy_from_user(
                    &state.tasks,
                    &state.circles,
                    &state.scheduler,
                    state.identities[0],
                    value.destination_index,
                    &mut message[..expected.len()],
                )
                .unwrap_or_else(|_| fail("ny_object_failure_message_copy"));
            if &message[..expected.len()] != expected {
                fail("ny_object_failure_message");
            }
            state.failure_armed = true;
            // Return once to the exact inspected UD2 with interrupts disabled;
            // no timer can observe the armed failure interval.
            unsafe { ptr::addr_of_mut!((*frame).flags).write(value.flags & !0x200) };
            compiler_fence(Ordering::SeqCst);
        }
        3 => {
            if state.selected != 1
                || !state.started[1]
                || !state.faulted
                || state.exited
                || value.instruction_pointer != state.image.exit_return
                || value.destination_index != user_data_base
                || value.source_index != header_bytes as u64
                || words[3] != 1
                || words[4] != 1
                || words[5] != 1
                || words[6] != 0
                || words[7] != expected_result
                || state.body_ticks[1] == 0
            {
                fail("ny_object_exit_gate");
            }
            state
                .scheduler
                .complete_current(&mut state.tasks, state.identities[1])
                .unwrap_or_else(|_| fail("ny_object_exit_transition"));
            state.exited = true;
            // SAFETY: Both tasks terminal; the live transition stack remains
            // owned until this nonlocal return and later bootstrap reclamation.
            unsafe { state.execution.restore_bootstrap() }.unwrap_or_else(|reason| fail(reason));
            if paging::read_cr3() != state.kernel_cr3
                || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
                || state.kernel_stack == 0
                || !(state.kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
                || !unsafe { state.cpu.finish() }
                || !state.cpu.storage_is_cleared()
            {
                fail("ny_object_restore_continuation");
            }
            let kernel_stack = state.kernel_stack;
            dispatch_active.store(false, Ordering::Release);
            active.store(ptr::null_mut(), Ordering::Release);
            // SAFETY: Consume the retained single-use continuation only after
            // restoring root/TSS and exact original kernel CPU state.
            unsafe { x86_64_application_resume_kernel(completion_status, kernel_stack) }
        }
        _ => fail("ny_object_unsupported_operation"),
    }
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("ny_object_gate_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}

fn emit_evidence(initial: frame_accounting, vendor: &str, evidence: [scenario_evidence; 2]) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_object_test.ready","level":"info","tasks":2,"scenarios":2,"cpu_profile":"x87_sse2","cpu_vendor":"{}","source_language":"ny","runtime_shim":"bounded_object","data_frames":18,"table_frames":18,"active_leases":10,"bound_leases":10}}"#,
        vendor
    );
    for (scenario, item) in evidence.iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.ny_object_test.scenario","level":"info","scenario":{},"task_0_fault_vector":6,"task_1_result":50000000,"task_0_begin":{},"task_0_end":{},"task_1_begin":1,"task_1_end":1,"timer_ticks":{},"task_0_body_ticks":{},"task_1_body_ticks":{},"dispatches":2,"preemptions":0,"faults":1,"exits":1,"initial_state_clean":true,"initialized_data":true,"bss_clean":true,"stack_aligned":true,"checked_result_copy":true,"failure_message_verified":true}}"#,
            scenario, scenario, scenario, item.timer_ticks, item.body_ticks[0], item.body_ticks[1]
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_object_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"cpu_state_cleared":true,"kernel_cpu_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_object_test.passed","level":"info","scenarios":2,"tasks":4,"dispatches":4,"preemptions":0,"faults":2,"exits":2,"timer_ticks":{}}}"#,
        evidence[0].timer_ticks + evidence[1].timer_ticks
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_object_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
