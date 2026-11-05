//! Sustained dispatch over retained prepared-launch owners and low user roots.
//!
//! Two x87/SSE2 tasks execute inside nested calls across real PIT switches.
//! Every mapping, high-alias transition stack and activation registration stays
//! owned through terminal restoration. This feature does not define an app ABI.

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
use crate::identity::{fixture_principal_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};
use crate::time::monotonic_instant;

const switch_limit: u64 = 64;
const survivor_limit: u64 = 16;
const gate_vector: u64 = 0xf2;
const task_switched: u64 = 8;
const completion_status: u64 = 0xa991;
const fault_vectors: [u64; 4] = [0, 6, 13, 14];
const canary_a: u64 = 0x61a7_c0de_f17e_0001;
const canary_b: u64 = 0x9ee7_c0de_ba5e_0002;
const mailbox_arm: u64 = 16;
const mailbox_progress: u64 = 24;
const nested_stack_pointer: u64 = user_stack_top - 24;
const stack_canary_offset: u64 = 2 * 4096 - 56;

static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_application_profile_gate_entry();
    static x86_64_application_profile_faulting_start: u8;
    static x86_64_application_profile_faulting_end: u8;
    static x86_64_application_profile_faulting_ready_return: u8;
    static x86_64_application_profile_faulting_initial_return: u8;
    static x86_64_application_profile_peer_initial_return: u8;
    static x86_64_application_profile_peer_start: u8;
    static x86_64_application_profile_peer_end: u8;
    static x86_64_application_profile_peer_ready_return: u8;
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
    initialized: bool,
    ready: bool,
    quanta: u64,
    progress: u64,
}

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    admitted: [admitted_task; 2],
    execution: owned_execution,
    cpu: application_cpu::cpu_session,
    stale: task_identity,
    scenario: usize,
    terminal_pending: bool,
    copies_verified: bool,
    selected: usize,
    switches: u64,
    survivor_ticks: u64,
    last_tick: u64,
    faulted: bool,
    last_faulted: bool,
    peer_progress_at_fault: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

pub(super) fn install_gate() -> bool {
    // SAFETY: One-shot fault-path initialization owns the IDT with IF clear.
    unsafe {
        super::idt::install_gate(
            gate_vector as usize,
            x86_64_application_profile_gate_entry as *const () as u64,
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
        fail("application_profile_initial_state");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Timer restriction leaves IF clear on the sole BSP; NMI is masked.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let result = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let mut vendor = "";
        for scenario in 0..fault_vectors.len() {
            let observed = run_owned(allocator, kernel_cr3, bootstrap_stack, scenario, initial);
            if scenario == 0 {
                vendor = observed;
            } else if observed != vendor {
                fail("application_profile_vendor_changed");
            }
        }
        (initial, vendor)
    });
    let (initial, vendor) = match result {
        Ok(result) => result,
        Err(error) => fail(error.reason()),
    };
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("application_profile_final_interrupt_state");
    }
    emit_evidence(initial, vendor);
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_owned(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
) -> &'static str {
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
        fail("application_profile_setup_state");
    }
    let payloads = [
        (
            symbol_address!(x86_64_application_profile_faulting_start),
            symbol_address!(x86_64_application_profile_faulting_end),
        ),
        (
            symbol_address!(x86_64_application_profile_peer_start),
            symbol_address!(x86_64_application_profile_peer_end),
        ),
    ]
    .map(|(start, end)| {
        let length = end
            .checked_sub(start)
            .filter(|length| *length > 0 && *length <= 4096)
            .unwrap_or_else(|| fail("application_profile_payload_extent"));
        (start, length)
    });
    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    // Retire a real table identity before the actual launches reuse its slots.
    // The copy proof uses this stale identity, not a fabricated generation.
    let stale = retire_unstarted(&mut circles, &mut tasks);
    let mut execution = owned_execution::new();
    // SAFETY: Sole BSP, IF clear, NMI masked. Every error halts in this owning
    // scope; no acquired resource or cleanup obligation is returned or dropped.
    unsafe { execution.prepare(allocator, &mut circles, payloads) }
        .unwrap_or_else(|reason| fail(reason));
    if tasks.active_count() != 0 {
        fail("application_profile_preparation_published_task");
    }
    let identities = core::array::from_fn::<_, 2, _>(|index| {
        execution
            .commit(index, &mut tasks, &mut circles, index as u64 + 201)
            .unwrap_or_else(|reason| fail(reason))
    });
    if tasks.validate(stale).is_ok() || circles.validate(stale.circle()).is_ok() {
        fail("application_profile_stale_identity_live");
    }
    for index in 0..2 {
        execution
            .write_data(index, 32, fault_vectors[scenario])
            .unwrap_or_else(|reason| fail(reason));
    }
    let mut scheduler =
        scheduler_table::new(2).unwrap_or_else(|_| fail("application_profile_scheduler"));
    for identity in identities {
        scheduler
            .admit(&mut circles, &mut tasks, identity)
            .unwrap_or_else(|_| fail("application_profile_admit"));
    }
    // SAFETY: All committed owners and high-alias stacks remain retained.
    unsafe { execution.activate(allocator) }.unwrap_or_else(|reason| fail(reason));
    let mut state = runtime {
        circles,
        tasks,
        scheduler,
        admitted: core::array::from_fn(|index| admitted_task {
            identity: identities[index],
            payload_start: payloads[index].0,
            payload_length: payloads[index].1,
            initialized: false,
            ready: false,
            quanta: 0,
            progress: 0,
        }),
        execution,
        cpu: application_cpu::cpu_session::new(),
        stale,
        scenario,
        terminal_pending: false,
        copies_verified: false,
        selected: 0,
        switches: 0,
        survivor_ticks: 0,
        last_tick: 0,
        faulted: false,
        last_faulted: false,
        peer_progress_at_fault: 0,
        kernel_cr3,
        bootstrap_stack,
        kernel_stack: 0,
        original_ts: paging::read_cr0() & task_switched,
    };
    validate_ready(&state, initial, allocator.accounting()).unwrap_or_else(|reason| fail(reason));
    // SAFETY: Initialize in its final stable runtime location; the supervisor
    // bootstrap residency keeps all aligned images live throughout scheduling.
    unsafe { state.cpu.prepare() }.unwrap_or_else(|reason| fail(reason));
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("application_profile_initial_dispatch"));
    if first.task() != identities[0]
        || first.sequence() != 0
        || first.context() != scheduler_context::initial
    {
        fail("application_profile_initial_selection");
    }
    let frame = state.select(0).unwrap_or_else(|reason| fail(reason));
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
        fail("application_profile_publication");
    }
    // SAFETY: The nonlocal return consumes exactly this retained bootstrap
    // continuation. CPU state is restored before the entry's ordinary IRETQ.
    let status = unsafe {
        if !(*runtime_pointer).cpu.restore_task(0) {
            fail("application_profile_initial_cpu_restore");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*runtime_pointer).kernel_stack))
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
        || state.terminal_pending
        || state.switches != switch_limit
        || state.survivor_ticks != survivor_limit
        || !state.faulted
        || !state.last_faulted
        || !state.copies_verified
        || state
            .admitted
            .iter()
            .any(|task| !task.initialized || !task.ready || task.quanta != 32 || task.progress <= 1)
        || state.admitted[1].progress <= state.peer_progress_at_fault
    {
        fail("application_profile_continuation_restore");
    }
    for task in &state.admitted {
        if !state
            .tasks
            .snapshot(task.identity)
            .is_ok_and(|snapshot| snapshot.state() == task_state::faulted)
        {
            fail("application_profile_terminal_task");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, task.identity)
            .unwrap_or_else(|_| fail("application_profile_withdraw_context"));
    }
    // SAFETY: Both tasks faulted, contexts withdrawn, bootstrap continuation
    // restored. All failure paths retain state and its unreleased ownership.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|reason| fail(reason));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("application_profile_final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
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
        fail("application_profile_final_accounting");
    }
    // SAFETY: All native consumers and storage have been retired. Wipe the
    // consumed continuation's address-bearing metadata before this scope ends.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_cr3), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.bootstrap_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(state.original_ts), 0);
    }
    state.cpu.cpu_vendor()
}

fn retire_unstarted(circles: &mut circle_table, tasks: &mut task_table) -> task_identity {
    let circle = circles
        .create()
        .unwrap_or_else(|_| fail("application_profile_stale_circle"));
    let bounds = task_bounds::new(
        user_code_base,
        user_code_base + 1,
        user_code_base,
        user_stack_top - 8192,
        user_stack_top,
        user_stack_top,
    )
    .unwrap_or_else(|| fail("application_profile_stale_bounds"));
    let principal = fixture_principal_id::new(200)
        .unwrap_or_else(|| fail("application_profile_stale_principal"));
    let identity = tasks
        .create(
            circles,
            task_binding::new(
                circle,
                principal,
                synthetic_origin_metadata::machine_role_fixture,
                bounds,
            ),
        )
        .unwrap_or_else(|_| fail("application_profile_stale_task"));
    tasks
        .cancel_unstarted(identity)
        .unwrap_or_else(|_| fail("application_profile_stale_cancel"));
    tasks
        .destroy(circles, identity)
        .unwrap_or_else(|_| fail("application_profile_stale_destroy"));
    circles
        .destroy(circle)
        .unwrap_or_else(|_| fail("application_profile_stale_circle_destroy"));
    identity
}

fn validate_ready(
    state: &runtime,
    initial: frame_accounting,
    ready: frame_accounting,
) -> Result<(), &'static str> {
    let circles = state.circles.accounting();
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .map_err(|_| "application_profile_ready_scheduler")?;
    if initial.free.checked_sub(36) != Some(ready.free)
        || ready.data_owned != 18
        || ready.table_owned != 18
        || ready.active_leases != 10
        || ready.bound_leases != 10
        || circles.circles != 2
        || circles.tasks != 2
        || circles.task_contexts != 2
        || circles.address_spaces != 2
        || circles.mappings != 6
        || snapshot.admitted() != 2
        || snapshot.runnable() != 2
        || snapshot.current().is_some()
    {
        return Err("application_profile_ready_accounting");
    }
    if state.execution.root(0)? == state.execution.root(1)?
        || state.execution.frame(0)? == state.execution.frame(1)?
        || state.execution.transition_stack_top(0)? == state.execution.transition_stack_top(1)?
    {
        return Err("application_profile_aliased_owner");
    }
    for index in 0..2 {
        let admission = state.execution.admission(index)?;
        if admission.bounds().code_start() != user_code_base
            || admission.bounds().initial_stack_pointer() != user_stack_top
            || !state
                .tasks
                .snapshot(state.admitted[index].identity)
                .is_ok_and(|task| task.bounds() == admission.bounds())
        {
            return Err("application_profile_ready_admission");
        }
    }
    Ok(())
}

impl runtime {
    fn cpu_canaries_match(&self, index: usize) -> bool {
        let Some(image) = self.cpu.task_image(index) else {
            return false;
        };
        let bytes = image.bytes();
        if image.control_word() != 0x037f + ((index as u16) << 10)
            || image.mxcsr() != 0x1f80 + ((index as u32) << 13)
            || u16::from_le_bytes([bytes[2], bytes[3]]) != 0x3800
            || bytes[4] != 0x80
            || u64::from_le_bytes(bytes[32..40].try_into().unwrap()) != 0x8000_0000_0000_0000
            || u16::from_le_bytes([bytes[40], bytes[41]]) != 0x3fff + index as u16
        {
            return false;
        }
        (1..8).all(|slot| {
            bytes[32 + slot * 16..42 + slot * 16]
                .iter()
                .all(|byte| *byte == 0)
        }) && (0..16).all(|register| {
            let start = 160 + register * 16;
            let low = u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap());
            let high = u64::from_le_bytes(bytes[start + 8..start + 16].try_into().unwrap());
            low == 0x3141_5926_0000_0000 + ((index as u64) << 16) + register as u64
                && high == 0x2718_2818_0000_0000 + ((index as u64) << 16) + register as u64
        })
    }

    fn prove_copies(&mut self) -> Result<(), &'static str> {
        if self.selected != 0 || self.copies_verified {
            return Err("application_profile_copy_state");
        }
        let identity = self.admitted[0].identity;
        let peer = self.admitted[1].identity;
        let data_address = user_data_base + 4093;
        let stack_address = user_stack_top - 8192 + 4095;
        let data: [u8; 19] = core::array::from_fn(|index| 0x41 + index as u8);
        let stack: [u8; 23] = core::array::from_fn(|index| 0x81 + index as u8);
        // Four successful operations span odd offsets across actual page
        // boundaries in separately admitted data and stack mappings.
        self.execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                identity,
                data_address,
                &data,
            )
            .map_err(|_| "application_profile_copy_data_write")?;
        let mut data_read = [0; 19];
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                identity,
                data_address,
                &mut data_read,
            )
            .map_err(|_| "application_profile_copy_data_read")?;
        self.execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                identity,
                stack_address,
                &stack,
            )
            .map_err(|_| "application_profile_copy_stack_write")?;
        let mut stack_read = [0; 23];
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                identity,
                stack_address,
                &mut stack_read,
            )
            .map_err(|_| "application_profile_copy_stack_read")?;
        if data_read != data || stack_read != stack {
            return Err("application_profile_copy_roundtrip");
        }
        for index in 0..2 {
            for offset in 4088u64..4120 {
                let word = self
                    .execution
                    .read_stack(index, offset & !7)
                    .ok_or("application_profile_copy_stack_readback")?;
                let byte = word.to_le_bytes()[(offset & 7) as usize];
                let expected = if index == 0 && (4095..4118).contains(&offset) {
                    stack[(offset - 4095) as usize]
                } else {
                    0
                };
                if byte != expected {
                    return Err("application_profile_copy_stack_destination_changed");
                }
            }
        }

        // Six rejected reads must preserve the entire destination, including
        // bytes outside the requested subslice and the zero-length case.
        for (requested, address, length) in [
            (peer, data_address, 19),
            (self.stale, data_address, 19),
            (identity, u64::MAX - 3, 19),
            (identity, user_data_base + 8191, 19),
            (identity, user_stack_top - 8193, 19),
            (identity, data_address, 0),
        ] {
            let mut destination = [0xc5; 23];
            if self
                .execution
                .copy_from_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    requested,
                    address,
                    &mut destination[1..1 + length],
                )
                .is_ok()
                || destination != [0xc5; 23]
            {
                return Err("application_profile_copy_rejected_read");
            }
        }
        // Four rejected writes: executable permissions, wrong current task,
        // retired identity, and an extent crossing into an unmapped page.
        for (requested, address) in [
            (identity, user_code_base),
            (peer, data_address),
            (self.stale, data_address),
            (identity, user_data_base + 8191),
        ] {
            if self
                .execution
                .copy_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    requested,
                    address,
                    &[0xee; 19],
                )
                .is_ok()
            {
                return Err("application_profile_copy_rejected_write");
            }
        }
        // Privileged readback of both owners catches mutation even if an
        // erroneous write selected the wrong private physical mapping.
        for index in 0..2 {
            for offset in 4088u64..4120 {
                let word = self
                    .execution
                    .read_data(index, offset & !7)
                    .ok_or("application_profile_copy_private_readback")?;
                let byte = word.to_le_bytes()[(offset & 7) as usize];
                let expected = if index == 0 && (4093..4112).contains(&offset) {
                    data[(offset - 4093) as usize]
                } else {
                    0
                };
                if byte != expected {
                    return Err("application_profile_copy_destination_changed");
                }
            }
            if self.execution.read_data(index, 8184) != Some(0) {
                return Err("application_profile_copy_tail_changed");
            }
        }
        // Fifth successful operation reads RX bytes after the rejected write.
        // Compare against immutable admitted linked source, not a user claim.
        let mut code = [0; 19];
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                identity,
                user_code_base,
                &mut code,
            )
            .map_err(|_| "application_profile_copy_code_read")?;
        for (offset, byte) in code.iter().enumerate() {
            // SAFETY: Preparation admitted the complete linked-rodata payload;
            // these exact bytes remain immutable and supervisor resident.
            let expected =
                unsafe { ptr::read((self.admitted[0].payload_start as *const u8).add(offset)) };
            if *byte != expected {
                return Err("application_profile_copy_code_changed");
            }
        }
        Ok(())
    }

    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        let Some(task) = self.admitted.get(self.selected) else {
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
                .is_ok_and(|s| s.current() == Some(task.identity))
            && self
                .tasks
                .snapshot(task.identity)
                .is_ok_and(|s| s.state() == task_state::running && s.context_attached())
    }

    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        let task = &self.admitted[self.selected];
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
            && (!task.ready
                || frame.flags & 0x202 == 0x202
                || (self.terminal_fault_armed() && frame.flags & 0x202 == 0x2))
    }

    fn terminal_fault_armed(&self) -> bool {
        self.switches == switch_limit
            && self.admitted[self.selected].ready
            && self.execution.read_data(self.selected, mailbox_arm) == Some(1)
            && ((self.selected == 0 && !self.faulted && self.survivor_ticks == 0)
                || (self.selected == 1
                    && self.faulted
                    && !self.last_faulted
                    && self.survivor_ticks == survivor_limit))
    }

    fn select(&mut self, index: usize) -> Result<*mut scheduler_frame, &'static str> {
        // SAFETY: The retained support owner validates both registered roots,
        // the current supervisor stack and the selected transition stack. IF
        // and NMI are excluded; the new root retains the live Rust stack too.
        unsafe { self.execution.select(index) }?;
        self.selected = index;
        let frame = self.execution.frame(index)?;
        if !self.frame_valid(frame) {
            return Err("application_profile_selected_frame");
        }
        Ok(frame)
    }

    fn sample(&mut self, frame: scheduler_frame, require_stack: bool) -> Result<(), &'static str> {
        let task = &mut self.admitted[self.selected];
        let a = canary_a + self.selected as u64;
        let b = canary_b + self.selected as u64;
        let progress = self
            .execution
            .read_data(self.selected, mailbox_progress)
            .ok_or("application_profile_progress_read")?;
        if frame.register_12 != a
            || frame.register_13 != b
            || frame.register_15 != user_data_base
            || (require_stack && frame.stack_pointer != nested_stack_pointer)
            || self.execution.read_data(self.selected, 0) != Some(a)
            || self.execution.read_data(self.selected, 8) != Some(b)
            || self
                .execution
                .read_stack(self.selected, stack_canary_offset)
                != Some(a)
            || progress < task.progress
            || frame.register_14 < progress
            || frame.register_14 - progress > 1
        {
            return Err("application_profile_private_state");
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
    !state.is_null() && unsafe { (&*state).frame_valid(frame.cast()) }
}

fn enter_dispatch() -> *mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) {
        fail("application_profile_reentry");
    }
    let state = active.load(Ordering::Acquire);
    if state.is_null() || interrupts::interrupts_enabled() {
        fail("application_profile_inactive");
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
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame.cast())
        || !state.cpu_canaries_match(state.selected)
    {
        fail("application_profile_timer_frame");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if value.vector != 0x20
        || !state.admitted[state.selected].ready
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("application_profile_timer_observation");
    }
    state.last_tick = observed.tick();
    state
        .sample(value, true)
        .unwrap_or_else(|reason| fail(reason));
    let selected = if !state.faulted {
        if state.switches >= switch_limit {
            fail("application_profile_fault_deadline");
        }
        state.admitted[state.selected].quanta += 1;
        let identity = state.admitted[state.selected].identity;
        let next = 1 - state.selected;
        let transition = state
            .scheduler
            .preempt_current(&mut state.tasks, identity)
            .unwrap_or_else(|_| fail("application_profile_preemption"));
        if transition.task() != identity
            || transition.sequence() != state.switches
            || transition.dispatch().task() != state.admitted[next].identity
            || transition.dispatch().sequence() != state.switches + 1
            || transition.dispatch().context()
                != if state.switches == 0 {
                    scheduler_context::initial
                } else {
                    scheduler_context::saved
                }
        {
            fail("application_profile_preemption_identity");
        }
        state.switches += 1;
        if state.switches == switch_limit {
            if next != 0
                || state
                    .admitted
                    .iter()
                    .any(|task| task.quanta != 32 || task.progress <= 1)
            {
                fail("application_profile_sustained_progress");
            }
            state
                .execution
                .write_data(0, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        state.select(next).unwrap_or_else(|reason| fail(reason))
    } else {
        if state.selected != 1 || state.survivor_ticks >= survivor_limit {
            fail("application_profile_exit_deadline");
        }
        let snapshot = state
            .scheduler
            .snapshot(&state.tasks)
            .unwrap_or_else(|_| fail("application_profile_survivor_scheduler"));
        if snapshot.runnable() != 0
            || snapshot.next_dispatch_sequence() != Some(66)
            || snapshot.next_preemption_sequence() != Some(64)
        {
            fail("application_profile_survivor_dispatch_changed");
        }
        state.survivor_ticks += 1;
        if state.survivor_ticks == survivor_limit {
            if state.admitted[1].progress <= state.peer_progress_at_fault {
                fail("application_profile_survivor_no_progress");
            }
            state
                .execution
                .write_data(1, mailbox_arm, 1)
                .unwrap_or_else(|reason| fail(reason));
        }
        frame.cast()
    };
    if state.terminal_fault_armed() {
        // The private fixture is now committed to an immediate terminal fault.
        // Exclude a PIT arrival between the peer's deliberate invalid RSP and
        // UD2, or before the first fault has dispatched its surviving peer.
        // Ordinary user execution and the survivor's saved frame keep IF set.
        unsafe {
            let flags = ptr::addr_of!((*selected).flags).read();
            ptr::addr_of_mut!((*selected).flags).write(flags & !(1 << 9));
        }
        if !state.frame_valid(selected) {
            fail("application_profile_terminal_return_frame");
        }
    }
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("application_profile_timer_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}

pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    // SAFETY: Exception entry supplied the frame; the retained runtime/latch
    // owns CPU images and both privilege stacks. Terminal frames never return.
    let state = unsafe { &mut *enter_dispatch() };
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || !state.cpu_canaries_match(state.selected)
    {
        fail("application_profile_unattributable_fault");
    }
    let value = unsafe { ptr::read(frame.cast::<scheduler_frame>()) };
    if value.vector != snapshot.vector
        || value.error_code != snapshot.error_code
        || value.instruction_pointer != snapshot.instruction_pointer
        || value.code_segment != snapshot.code_segment
        || value.flags != snapshot.flags
        || value.stack_pointer != snapshot.stack_pointer
        || Some(value.stack_segment) != snapshot.stack_segment
        || state.switches != switch_limit
        || value.flags & (1 << 9) != 0
        || !state.terminal_fault_armed()
        || state.execution.read_data(state.selected, mailbox_arm) != Some(1)
    {
        fail("application_profile_fault_provenance");
    }
    // This checks private fixture progress, not RIP/RSP return admissibility.
    // A terminal user fault is accepted even when its RIP/RSP cannot be resumed.
    state
        .sample(value, false)
        .unwrap_or_else(|reason| fail(reason));
    if state.selected == 0 && !state.faulted {
        if snapshot.vector != fault_vectors[state.scenario]
            || (snapshot.vector == 14 && snapshot.instruction_pointer != user_data_base + 8192)
        {
            fail("application_profile_fault_scenario");
        }
        let transition = state
            .scheduler
            .fault_current(&mut state.tasks, state.admitted[0].identity)
            .unwrap_or_else(|_| fail("application_profile_fault_transition"));
        if transition.task() != state.admitted[0].identity
            || transition.dispatch().task() != state.admitted[1].identity
            || transition.dispatch().sequence() != 65
            || transition.dispatch().context() != scheduler_context::saved
        {
            fail("application_profile_fault_dispatch");
        }
        state.faulted = true;
        state.peer_progress_at_fault = state.admitted[1].progress;
        let selected = state.select(1).unwrap_or_else(|reason| fail(reason));
        if !unsafe { state.cpu.restore_task(1) } {
            fail("application_profile_fault_cpu_restore");
        }
        dispatch_active.store(false, Ordering::Release);
        return Some(selected.cast());
    }
    if state.selected != 1
        || !state.faulted
        || state.last_faulted
        || snapshot.vector != 6
        || snapshot.stack_pointer != 0
        || state.survivor_ticks != survivor_limit
    {
        fail("application_profile_last_fault_state");
    }
    state
        .scheduler
        .fault_last_current(&mut state.tasks, state.admitted[1].identity)
        .unwrap_or_else(|_| fail("application_profile_last_fault_transition"));
    state.last_faulted = true;
    state.terminal_pending = true;
    // Keep the runtime latch held. The common exception handler first clears
    // its own exception latch, then invokes the single-use bootstrap return.
    None
}

pub(super) fn resume_kernel_after_fault() -> ! {
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null()
        || !dispatch_active.load(Ordering::Acquire)
        || interrupts::interrupts_enabled()
    {
        fail("application_profile_terminal_without_owner");
    }
    let state = unsafe { &mut *pointer };
    if !state.terminal_pending
        || !state.faulted
        || !state.last_faulted
        || state.scheduler.snapshot(&state.tasks).is_err()
        || state
            .scheduler
            .snapshot(&state.tasks)
            .is_ok_and(|s| s.current().is_some() || s.runnable() != 0)
        || state.admitted.iter().any(|task| {
            !state
                .tasks
                .snapshot(task.identity)
                .is_ok_and(|snapshot| snapshot.state() == task_state::faulted)
        })
    {
        fail("application_profile_terminal_predicate");
    }
    // SAFETY: Both tasks terminal. The live high-alias privilege stack remains
    // owned while bootstrap root/TSS and then exact kernel CPU state restore.
    unsafe { state.execution.restore_bootstrap() }.unwrap_or_else(|reason| fail(reason));
    if paging::read_cr3() != state.kernel_cr3
        || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
        || state.kernel_stack == 0
        || !(state.kernel_stack as usize).is_multiple_of(mem::align_of::<u64>())
        || !unsafe { state.cpu.finish() }
        || !state.cpu.storage_is_cleared()
    {
        fail("application_profile_restore_continuation");
    }
    state.terminal_pending = false;
    let kernel_stack = state.kernel_stack;
    dispatch_active.store(false, Ordering::Release);
    active.store(ptr::null_mut(), Ordering::Release);
    // SAFETY: Consume the single-use continuation only after common exception
    // bookkeeping and the CPU session are closed. Reclamation happens later.
    unsafe { x86_64_application_resume_kernel(completion_status, kernel_stack) }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_application_profile_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = unsafe { &mut *enter_dispatch() };
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame) {
        fail("application_profile_gate_frame");
    }
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector || value.error_code != 0 || value.flags != 2 {
        fail("application_profile_gate_vector");
    }
    state
        .sample(value, true)
        .unwrap_or_else(|reason| fail(reason));
    let index = state.selected;
    if value.accumulator == 1 {
        let marker = if index == 0 {
            symbol_address!(x86_64_application_profile_faulting_initial_return)
        } else {
            symbol_address!(x86_64_application_profile_peer_initial_return)
        };
        if state.admitted[index].initialized
            || state.admitted[index].ready
            || state.admitted[index].progress != 1
            || Some(value.instruction_pointer)
                != relocated_user_address(&state.admitted[index], marker)
            || !state
                .cpu
                .task_image(index)
                .is_some_and(|image| image.is_initial_clean())
        {
            fail("application_profile_initial_state");
        }
        if index == 0 {
            state.prove_copies().unwrap_or_else(|reason| fail(reason));
            state.copies_verified = true;
        }
        state.admitted[index].initialized = true;
    } else if value.accumulator == 2 {
        let marker = if index == 0 {
            symbol_address!(x86_64_application_profile_faulting_ready_return)
        } else {
            symbol_address!(x86_64_application_profile_peer_ready_return)
        };
        if !state.admitted[index].initialized
            || state.admitted[index].ready
            || state.admitted[index].progress != 1
            || Some(value.instruction_pointer)
                != relocated_user_address(&state.admitted[index], marker)
            || !state.cpu_canaries_match(index)
            || state.execution.read_data(index, mailbox_arm) != Some(0)
        {
            fail("application_profile_ready_gate");
        }
        state.admitted[index].ready = true;
        unsafe { ptr::addr_of_mut!((*frame).flags).write(0x202) };
        compiler_fence(Ordering::SeqCst);
    } else {
        fail("application_profile_gate_operation");
    }
    if !unsafe { state.cpu.restore_task(index) } {
        fail("application_profile_gate_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    frame
}

fn relocated_user_address(task: &admitted_task, symbol: u64) -> Option<u64> {
    let offset = symbol.checked_sub(task.payload_start)?;
    (offset < task.payload_length).then_some(user_code_base.checked_add(offset)?)
}

fn emit_evidence(initial: frame_accounting, vendor: &str) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.application_profile_test.ready","level":"info","tasks":2,"scenarios":4,"quantum_switches":64,"survivor_ticks":16,"data_frames":18,"table_frames":18,"active_leases":10,"bound_leases":10,"cpu_profile":"x87_sse2","cpu_vendor":"{}","tls":false}}"#,
        vendor
    );
    for (scenario, vector) in fault_vectors.iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.application_profile_test.scenario","level":"info","scenario":{},"first_fault_vector":{},"last_fault_vector":6,"timer_ticks":80,"dispatches":66,"preemptions":64,"task_0_quanta":32,"task_1_quanta":32,"initial_state_clean":true,"all_xmm_preserved":true,"x87_preserved":true,"mxcsr_preserved":true,"peer_continued":true,"last_fault_returned":true}}"#,
            scenario, vector
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.application_profile_test.copy.verified","level":"info","successful_operations":20,"rejected_operations":40,"odd_cross_page":true,"destination_unchanged":true,"code_unchanged":true,"identity_bound":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.application_profile_test.released","level":"info","free_frames":{},"resources_restored":true,"storage_cleared":true,"cpu_state_cleared":true,"kernel_cpu_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.application_profile_test.passed","level":"info","scenarios":4,"tasks":8,"timer_ticks":320,"dispatches":264,"preemptions":256,"faults":8,"exits":0}}"#
    );
}

fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.application_profile_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
