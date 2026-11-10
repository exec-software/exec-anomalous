//! Supervised unchanged interpreter file-service replies and dependent containment.
//! ELF interpretation and Linux operation policy remain outside this module.
use super::application_cpu::{self, x86_64_application_enter, x86_64_application_resume_kernel};
use super::linux_loader_plan::native_plan;
use super::linux_loader_reply::{caller_read, caller_read_capacity, frame_words, returned_frame};
use super::linux_loader_source::source_controller;
use super::owned_execution::{
    loader_mapping_context, loader_mapping_observation, owned_execution_group, tls_bases_are_zero,
    user_stack_top,
};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
use super::{descriptors, exceptions, interrupts, linux_syscall, paging, task_tls};
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::task_identity;
use crate::memory::{
    self,
    physical::{frame_accounting, frame_allocator},
};
use crate::scheduler::{
    scheduler_call_authorization, scheduler_context, scheduler_table,
    scheduler_veil_wait_authorization,
};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;
use anomalous_veil_protocol::source_read::{source_binding, source_request};
use anomalous_veil_protocol::{native_status, source_mapping::source_mapping_request};
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use core::{arch::asm, cell::UnsafeCell, fmt::Write, ptr};

const boundary_capacity: usize = 32;
const service_report_address: u64 = 0x8010d0;
static next_service_incarnation: AtomicU64 = AtomicU64::new(1);
const completion_status: u64 = 0x4c45;
const max_entries: u64 = 10000;
const file_binding_offset: usize = 136;
const file_report_address: u64 = 0x8000b0;
const file_source_name: &[u8] = b"opt/anomalous/linux-loader-file-source";
const file_source_limit: u64 = 8 * 1024 * 1024;
const fnv_offset: u64 = 0xcbf29ce484222325;
const fnv_prime: u64 = 0x100000001b3;
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained<T>(UnsafeCell<core::mem::MaybeUninit<T>>);
// SAFETY: Sole BSP, IF/NMI excluded; storage is exclusively reserved until
// terminal cleanup. No owner moves after any hardware state is installed.
unsafe impl<T> Sync for retained<T> {}
static storage: retained<runtime> = retained(UnsafeCell::new(core::mem::MaybeUninit::uninit()));
static plan_storage: retained<native_plan> =
    retained(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

unsafe extern "C" {
    fn x86_64_native_application_gate_entry();
    static x86_64_linux_loader_entry_plan_start: u8;
    static x86_64_linux_loader_entry_plan_end: u8;
    static x86_64_linux_loader_entry_payload_start: u8;
    static x86_64_linux_loader_entry_payload_end: u8;
    static x86_64_linux_loader_entry_data_start: u8;
    static x86_64_linux_loader_entry_data_end: u8;
    static x86_64_linux_loader_entry_stage_fault: u8;
    static x86_64_linux_loader_entry_unknown_fault: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
}
macro_rules! address {
    ($symbol:ident) => {{
        let result: u64;
        // SAFETY: Materialize a linker address without dereferencing it.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) result,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        result
    }};
}

#[derive(Clone, Copy, Default)]
struct boundary {
    sequence: u64,
    number: u64,
    arguments: [u64; 6],
    instruction_pointer: u64,
    stack_pointer: u64,
    rcx: u64,
    r11: u64,
    fs_base: u64,
    result_bits: u64,
}
// A transcript work budget for this removable proof, not a pathname limit.
const copy_journal_capacity: usize = 256;
#[derive(Clone, Copy)]
struct copy_observation {
    attempts: u64,
    successes: u64,
    rejections: u64,
    sequence: u64,
    address: u64,
    length: usize,
    bytes: [u8; copy_journal_capacity],
}
impl copy_observation {
    fn clear(&mut self) {
        for byte in &mut self.bytes {
            // SAFETY: Exclusive retained diagnostic journal, not caller memory.
            unsafe { ptr::write_volatile(byte, 0) };
        }
        self.attempts = 0;
        self.successes = 0;
        self.rejections = 0;
        self.sequence = 0;
        self.address = 0;
        self.length = 0;
    }
    fn is_clear(&self) -> bool {
        self.attempts == 0
            && self.successes == 0
            && self.rejections == 0
            && self.sequence == 0
            && self.address == 0
            && self.length == 0
            && self.bytes.iter().all(|byte| {
                // SAFETY: Initialized retained bytes; witness the volatile wipe.
                unsafe { ptr::read_volatile(byte) == 0 }
            })
    }
}
// Retain only bounded counters and the guest's consumed-byte report. In
// particular, these observations never enlarge the by-value run_observation.
struct file_copy_observation {
    attempts: u64,
    successes: u64,
    rejections: u64,
    delivered_bytes: u64,
    delivered_fnv64: u64,
    ordered: bool,
    report: [u8; 32],
}
// These proof journals never move through bootstrap return values.
struct delivery_observation {
    probe_address: u64,
    probe_length: usize,
    sequence: u64,
    address: u64,
    length: usize,
    source_offset: u64,
    source_reads: u64,
    source_bytes: u64,
    stage_rejections: u64,
    completion_rejections: u64,
    committed: bool,
    staged: bool,
    bytes: [u8; caller_read_capacity],
    before: [u8; caller_read_capacity],
    after: [u8; caller_read_capacity],
}
impl delivery_observation {
    fn is_pristine(&self) -> bool {
        self.probe_address == 0
            && self.probe_length == 0
            && self.sequence == 0
            && self.address == 0
            && self.length == 0
            && self.source_offset == 0
            && self.source_reads == 0
            && self.source_bytes == 0
            && self.stage_rejections == 0
            && self.completion_rejections == 0
            && !self.committed
            && !self.staged
            && self.bytes.iter().all(|byte| *byte == 0)
            && self.before.iter().all(|byte| *byte == 0)
            && self.after.iter().all(|byte| *byte == 0)
    }
}
#[path = "linux_loader_entry_runtime/mapping_lifetime.rs"]
mod mapping_lifetime;
use mapping_lifetime::lifetime_observation;

struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 3],
    execution: owned_execution_group<3>,
    cpu: application_cpu::cpu_session_group<3>,
    tls: task_tls::session_group<3>,
    source: source_controller,
    file_source: Option<source_controller>,
    mapping_lifetime: lifetime_observation,
    file_binding: source_binding,
    file_copy: file_copy_observation,
    file_proof_complete: bool,
    scenario: u64,
    incarnation: u64,
    last_committed: u64,
    context_queries: u64,
    allocator_live_accounting: frame_accounting,
    allocator_service_borrows: u64,
    allocator_nested_denials: u64,
    mapping_preparations: u64,
    mapping_rejections: u64,
    mapping_sequence: u64,
    mapping_observation: loader_mapping_observation,
    mapping_readback: [u8; 4096],
    // Four bounded copyouts per proof lifetime; never moved through the stack.
    deliveries: [delivery_observation; 4],
    delivery_index: usize,
    stage_bytes: [u8; caller_read_capacity],
    service_report: [u8; 80],
    fault_vector: u64,
    fault_ip: u64,
    fault_sequence: u64,
    expected_fault: bool,
    native_buffer_cleared: bool,
    selected: usize,
    started: [bool; 3],
    terminal: [bool; 3],
    pending: Option<scheduler_call_authorization>,
    waiting: Option<scheduler_veil_wait_authorization>,
    request: [u8; 80],
    sequence: u64,
    next_sequence: u64,
    caller_returns: u64,
    calls: u64,
    raw_calls: u64,
    entries: u64,
    returns: u64,
    last_tick: u64,
    boundaries: [boundary; boundary_capacity],
    reply_frames: [[[u64; 22]; 2]; boundary_capacity],
    path_copies: [copy_observation; boundary_capacity],
    terminal_frames: [[u64; 22]; 2],
    copy: copy_observation,
    copy_scratch: [u8; caller_read_capacity],
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

fn additional_controls_clear() -> bool {
    let cr4: u64;
    // SAFETY: Read-only privileged control inspection at CPL0.
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
    }
    // This profile owns legacy FP/FS only. CET, user/supervisor protection keys,
    // FSGSBASE, XSAVE and five-level paging are not admitted implicitly by CPUID.
    cr4 & ((1 << 12) | (1 << 16) | (1 << 18) | (1 << 22) | (1 << 23) | (1 << 24)) == 0
}

fn linked_span(start: u64, end: u64, limit: u64) -> &'static [u8] {
    if start < address!(kernel_rodata_start)
        || end > address!(kernel_rodata_end)
        || end <= start
        || end - start > limit
    {
        fail("loader_linked_span");
    }
    // SAFETY: Complete immutable linked rodata, retained for the kernel lifetime.
    unsafe { core::slice::from_raw_parts(start as *const u8, (end - start) as usize) }
}

impl runtime {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        self.selected < 3
            && self.execution.frame_owned(self.selected, frame)
            && self.execution.root(self.selected).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(self.selected).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self.cpu.controls_are_active()
            && additional_controls_clear()
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
        // SAFETY: Complete retained frame authenticated above.
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
        // SAFETY: Prior CPU image saved; all retained roots/stacks remain owned.
        unsafe { self.execution.select(index) }.unwrap_or_else(|e| fail(e));
        let stack = self
            .execution
            .transition_stack_top(index)
            .unwrap_or_else(|e| fail(e));
        if !unsafe { self.cpu.select_linux_entry(stack, index == 0) }
            || !self.cpu.linux_entry_matches(stack, index == 0)
        {
            fail("loader_entry_selection");
        }
        self.selected = index;
        let frame = self.execution.frame(index).unwrap_or_else(|e| fail(e));
        if !self.frame_valid(frame) {
            fail("loader_selected_frame");
        }
        frame
    }
    fn capture(&mut self, frame: *const scheduler_frame) {
        if !self.frame_valid(frame) {
            fail("loader_capture_frame");
        }
        // SAFETY: Exact current task/root/TSS/frame authenticated above. Assembly
        // already captured FS and cleared all kernel segment bases.
        unsafe {
            self.tls
                .consume_entry(self.identities[self.selected], frame)
        }
        .unwrap_or_else(|e| fail(e.reason()));
        if !self.tls.kernel_is_clean() || !unsafe { self.cpu.save_current(self.selected) } {
            fail("loader_capture_cpu");
        }
        self.entries += 1;
        if self.entries >= max_entries {
            fail("loader_entry_budget");
        }
    }
    fn stage(&mut self, frame: *const scheduler_frame) {
        if !self.frame_valid(frame) {
            fail("loader_return_frame");
        }
        // SAFETY: Destination retained root/TSS/frame and task identity agree.
        unsafe { self.tls.stage_return(self.identities[self.selected], frame) }
            .unwrap_or_else(|e| fail(e.reason()));
        if !unsafe { self.cpu.restore_task(self.selected) } {
            fail("loader_return_cpu");
        }
        self.returns += 1;
    }
    fn authenticated_request(&self, sequence: u64) -> bool {
        self.selected == 2
            && sequence != 0
            && sequence == self.sequence
            && self.pending.as_ref().is_some_and(|authority| {
                authority.caller() == self.identities[0]
                    && authority.veil() == self.identities[2]
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
    fn scratch_is_clear(&self) -> bool {
        self.copy_scratch.iter().all(|byte| {
            // SAFETY: Initialized exclusively retained scratch; witness wipe.
            unsafe { ptr::read_volatile(byte) == 0 }
        })
    }
    fn clear_scratch(&mut self) {
        for byte in &mut self.copy_scratch {
            // SAFETY: Exclusive initialized BSS scratch, including error paths.
            unsafe { ptr::write_volatile(byte, 0) };
        }
    }
    fn read_file_source(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(self.sequence) || !self.scratch_is_clear() {
            fail("loader_file_read_phase");
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("loader_file_read_frame");
        }
        // SAFETY: The sealed blocked caller and retained supervisor frame were
        // authenticated above. No task, mapping or pending authority can change
        // during this IF-clear source/copy interval.
        let before = frame_words(unsafe { ptr::read(frame) });
        if self.reply_frames[self.raw_calls as usize - 1][0] != before {
            fail("loader_file_read_pending_frame");
        }
        let request = self.request;
        let sequence = self.sequence;
        let pending_sequence = self.pending.as_ref().unwrap().sequence();
        if !self.file_proof_complete {
            self.file_copy.attempts += 1;
        }
        let reply = self.copy_file_source(value);
        self.clear_scratch();
        if !self.scratch_is_clear()
            || !self.authenticated_request(sequence)
            || self.sequence != sequence
            || self.request != request
            || self.waiting.is_some()
            || self.pending.as_ref().unwrap().sequence() != pending_sequence
            || frame_words(unsafe { ptr::read(frame) }) != before
        {
            fail("loader_file_read_effect");
        }
        if !self.file_proof_complete {
            if reply.0 == 0 {
                self.file_copy.successes += 1;
            } else {
                self.file_copy.rejections += 1;
            }
        }
        reply
    }
    fn copy_file_source(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        let read = match source_request::decode(
            self.sequence,
            self.file_binding,
            [
                value.destination_index,
                value.source_index,
                value.data,
                value.register_10,
                value.register_8,
                value.register_9,
            ],
        ) {
            Ok(read) => read,
            Err(status) => return (status as u64, 0, 0),
        };
        // Admit a journal before source I/O, but commit its selection only after
        // the private copy succeeds. Source reads and service-produced copyouts
        // share sequence-based admission without decoding foreign operations.
        let mut delivery_index = self.delivery_index;
        if self.file_proof_complete {
            let Some(candidate) = self.delivery_candidate() else {
                return (4, 0, 0);
            };
            delivery_index = candidate;
            if self.deliveries[delivery_index].source_reads != 0
                || self.deliveries[delivery_index].probe_length != 0
                || self.deliveries[delivery_index].length != 0
            {
                return (4, 0, 0);
            }
        }
        let Some(delivered_bytes) = self
            .file_copy
            .delivered_bytes
            .checked_add(read.length as u64)
        else {
            return (4, 0, 0);
        };
        // Preflight the entire current veil destination before any source I/O.
        // A later copy repeats these checks; this is not durable permission.
        if self
            .execution
            .preflight_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                read.destination,
                read.length,
            )
            .is_err()
        {
            return (3, 0, 0);
        }
        let Some(source) = self.file_source.as_ref() else {
            return (4, 0, 0);
        };
        let reader = source.reader().unwrap_or_else(|e| fail(e.as_str()));
        // SAFETY: Sole BSP, IF/NMI excluded. This sealed independent source
        // remains retained; the execution owner's alias is already live. The
        // initialized supervisor BSS scratch is disjoint from every source and
        // task lease, and is wiped before any return, including read failures.
        if unsafe {
            self.execution.read_loader_source(
                &reader,
                read.offset,
                &mut self.copy_scratch[..read.length],
            )
        }
        .is_err()
        {
            return (3, 0, 0);
        }
        if self
            .execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                read.destination,
                &self.copy_scratch[..read.length],
            )
            .is_err()
        {
            return (3, 0, 0);
        }
        if self.file_proof_complete {
            self.delivery_index = delivery_index;
            self.deliveries[delivery_index].sequence = self.sequence;
            self.deliveries[self.delivery_index].source_reads = 1;
            self.deliveries[self.delivery_index].source_bytes = read.length as u64;
            self.deliveries[self.delivery_index].source_offset = read.offset;
            return (0, read.length as u64, 0);
        }
        let expected_length = if self.file_copy.delivered_bytes == 0 {
            self.file_binding.length().min(17)
        } else {
            self.file_binding
                .length()
                .saturating_sub(self.file_copy.delivered_bytes)
                .min(caller_read_capacity as u64)
        };
        self.file_copy.ordered &=
            read.offset == self.file_copy.delivered_bytes && read.length as u64 == expected_length;
        for byte in &self.copy_scratch[..read.length] {
            self.file_copy.delivered_fnv64 =
                (self.file_copy.delivered_fnv64 ^ u64::from(*byte)).wrapping_mul(fnv_prime);
        }
        self.file_copy.delivered_bytes = delivered_bytes;
        (0, read.length as u64, 0)
    }
    fn capture_file_report(&mut self) {
        if !self.authenticated_request(self.sequence)
            || self.file_copy.report != [0; 32]
            || !self.scratch_is_clear()
        {
            fail("loader_file_report_state");
        }
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                file_report_address,
                &mut self.file_copy.report,
            )
            .unwrap_or_else(|_| fail("loader_file_report_copy"));
        let word = |offset| {
            u64::from_le_bytes(
                self.file_copy.report[offset..offset + 8]
                    .try_into()
                    .unwrap(),
            )
        };
        let chunks =
            1 + (self.file_binding.length() - self.file_binding.length().min(17)).div_ceil(4096);
        if &self.file_copy.report[..8] != b"ANFSR001"
            || word(8) != self.file_binding.length()
            || word(16) != self.file_binding.fnv64()
            || word(24) != chunks
            || !self.file_copy.ordered
            || self.file_copy.delivered_bytes != self.file_binding.length()
            || self.file_copy.delivered_fnv64 != self.file_binding.fnv64()
            || self.file_copy.successes != chunks
            || self.file_copy.rejections != 10
            || self.file_copy.attempts != chunks + 10
        {
            fail("loader_file_report_evidence");
        }
    }
    fn read_caller(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if self.raw_calls != self.caller_returns + 1
            || !self.authenticated_request(self.sequence)
            || !self.scratch_is_clear()
        {
            fail("loader_copy_phase");
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("loader_copy_frame");
        }
        // SAFETY: Exact retained blocked frame authenticated above. Copy APIs
        // cannot retire or replace either task or mapping in this IF-clear span.
        let before = frame_words(unsafe { ptr::read(frame) });
        if self.reply_frames[self.raw_calls as usize - 1][0] != before {
            fail("loader_copy_pending_frame");
        }
        let request = self.request;
        let sequence = self.sequence;
        self.copy.attempts += 1;
        let reply = self.copy_caller_bytes(value);
        self.clear_scratch();
        if !self.scratch_is_clear()
            || !self.authenticated_request(sequence)
            || self.sequence != sequence
            || self.request != request
            || self.waiting.is_some()
            || frame_words(unsafe { ptr::read(frame) }) != before
        {
            fail("loader_copy_effect");
        }
        if reply.0 == 0 {
            self.copy.successes += 1;
        } else {
            self.copy.rejections += 1;
        }
        reply
    }
    fn copy_caller_bytes(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        let Ok(read) = caller_read::decode(self.sequence, value) else {
            return (4, 0, 0);
        };
        if !self.authenticated_request(read.sequence)
            || read.length > copy_journal_capacity - self.copy.length
            || (self.copy.length != 0
                && (self.copy.sequence != read.sequence
                    || self.copy.address.checked_add(self.copy.length as u64) != Some(read.source)))
        {
            return (4, 0, 0);
        }
        let authority = self.pending.as_ref().unwrap();
        let result = self
            .execution
            .copy_from_suspended_caller(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                authority,
                read.source,
                &mut self.copy_scratch[..read.length],
            )
            .and_then(|_| {
                self.execution.copy_to_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[2],
                    read.destination,
                    &self.copy_scratch[..read.length],
                )
            });
        if result.is_err() {
            return (3, 0, 0);
        }
        if self.copy.length == 0 {
            self.copy.sequence = read.sequence;
            self.copy.address = read.source;
        }
        let end = self.copy.length + read.length;
        self.copy.bytes[self.copy.length..end].copy_from_slice(&self.copy_scratch[..read.length]);
        self.copy.length = end;
        (0, read.length as u64, 0)
    }
    fn retire_tls(&mut self, index: usize) {
        self.tls
            .retire(self.identities[index])
            .unwrap_or_else(|e| fail(e.reason()));
        self.terminal[index] = true;
    }
    fn stage_clear(&mut self) {
        for byte in &mut self.stage_bytes {
            // SAFETY: Sole retained supervisor stage, no live transport borrow.
            unsafe { ptr::write_volatile(byte, 0) };
        }
        if self.deliveries[self.delivery_index].staged {
            self.deliveries[self.delivery_index].staged = false;
        }
    }
    fn clear_native_buffer(&mut self, expect_clear: bool) -> bool {
        let read = self
            .execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                0x8000d0,
                &mut self.copy_scratch,
            )
            .is_ok();
        let previously_clear = read && self.scratch_is_clear();
        self.clear_scratch();
        let wrote = self
            .execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                0x8000d0,
                &self.copy_scratch,
            )
            .is_ok();
        let verified = wrote
            && self
                .execution
                .copy_from_user(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.identities[2],
                    0x8000d0,
                    &mut self.copy_scratch,
                )
                .is_ok();
        self.native_buffer_cleared = verified && self.scratch_is_clear();
        self.clear_scratch();
        self.native_buffer_cleared && (!expect_clear || previously_clear)
    }
    fn capture_service_report(&mut self) {
        if !self.authenticated_request(self.sequence) || self.service_report != [0; 80] {
            fail("loader_service_report_state");
        }
        self.execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                service_report_address,
                &mut self.service_report,
            )
            .unwrap_or_else(|_| fail("loader_service_report_copy"));
        if &self.service_report[..8] != b"ANFSPR01"
            || u64::from_le_bytes(self.service_report[8..16].try_into().unwrap()) != self.scenario
        {
            fail("loader_service_report_header");
        }
    }
    #[inline(never)]
    fn probe_allocator_borrow(&mut self) {
        if !self.authenticated_request(self.sequence)
            || interrupts::interrupts_enabled()
            || self.waiting.is_some()
            || self.allocator_service_borrows != 0
            || self.allocator_nested_denials != 0
        {
            fail("loader_allocator_probe_phase");
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("loader_allocator_probe_frame");
        }
        // SAFETY: The sealed blocked caller retains this complete supervisor
        // frame. IF/NMI and dispatch exclusion cover the entire synchronous
        // probe; no allocator reference or callback pointer may escape it.
        let before = frame_words(unsafe { ptr::read(frame) });
        if self.reply_frames[self.raw_calls as usize - 1][0] != before {
            fail("loader_allocator_probe_pending_frame");
        }
        let request = self.request;
        let sequence = self.sequence;
        let pending_sequence = self.pending.as_ref().unwrap().sequence();
        let maximum_extent = memory::with_frame_allocator(|allocator| {
            if allocator.accounting() != self.allocator_live_accounting {
                fail("loader_allocator_probe_accounting");
            }
            let mut nested_entered = false;
            let nested = memory::with_frame_allocator(|_| {
                nested_entered = true;
            });
            if nested_entered || nested != Err(memory::memory_error::allocator_busy) {
                fail("loader_allocator_probe_nested");
            }
            self.allocator_nested_denials += 1;
            allocator.maximum_physical_extent()
        })
        .unwrap_or_else(|e| fail(e.reason()));
        self.allocator_service_borrows += 1;
        let reacquired_extent =
            memory::with_frame_allocator(|allocator| allocator.maximum_physical_extent())
                .unwrap_or_else(|e| fail(e.reason()));
        self.allocator_service_borrows += 1;
        if maximum_extent == 0
            || reacquired_extent != maximum_extent
            || !self.authenticated_request(sequence)
            || self.sequence != sequence
            || self.request != request
            || self.waiting.is_some()
            || self.pending.as_ref().unwrap().sequence() != pending_sequence
            || frame_words(unsafe { ptr::read(frame) }) != before
        {
            fail("loader_allocator_probe_effect");
        }
    }
    #[inline(never)]
    fn probe_source_mapping(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(self.sequence)
            || self.waiting.is_some()
            || interrupts::interrupts_enabled()
            || !self.file_proof_complete
            || !self.scratch_is_clear()
            || self.mapping_readback.iter().any(|byte| *byte != 0)
        {
            fail("loader_mapping_probe_phase");
        }
        let frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, frame) {
            fail("loader_mapping_probe_frame");
        }
        // SAFETY: The sealed caller remains blocked on this retained complete
        // supervisor frame throughout the synchronous IF/NMI-excluded probe.
        let before = frame_words(unsafe { ptr::read(frame) });
        if self.reply_frames[self.raw_calls as usize - 1][0] != before {
            fail("loader_mapping_probe_pending_frame");
        }
        let sequence = self.sequence;
        let pending = self.pending.as_ref().unwrap().correlation_words();
        let invocation = self.request;
        let request = source_mapping_request::decode(
            sequence,
            self.file_binding,
            [
                value.destination_index,
                value.source_index,
                value.data,
                value.register_10,
                value.register_8,
                value.register_9,
            ],
        );
        let result = match request {
            Err(status) => Err(status),
            Ok(_) if self.mapping_preparations != 0 || self.execution.has_loader_mapping() => {
                Err(native_status::invalid)
            }
            Ok(request) => {
                let reader = self
                    .file_source
                    .as_ref()
                    .unwrap_or_else(|| fail("loader_source_moved"))
                    .reader()
                    .unwrap_or_else(|e| fail(e.as_str()));
                memory::with_frame_allocator(|allocator| {
                    if allocator.accounting() != self.allocator_live_accounting {
                        fail("loader_mapping_probe_initial_accounting");
                    }
                    // SAFETY: Genuine reader and exact sealed caller stay
                    // borrowed. The existing execution alias and disjoint BSS
                    // buffers remain live; no guest transfer occurs here.
                    unsafe {
                        self.execution.probe_loader_mapping(
                            allocator,
                            loader_mapping_context {
                                tasks: &self.tasks,
                                circles: &self.circles,
                                scheduler: &self.scheduler,
                                authorization: self.pending.as_ref().unwrap(),
                                binding: self.file_binding,
                            },
                            request,
                            &reader,
                            (&mut self.copy_scratch, &mut self.mapping_readback),
                        )
                    }
                })
                .unwrap_or_else(|e| fail(e.reason()))
            }
        };
        if !self.authenticated_request(sequence)
            || self.sequence != sequence
            || self.pending.as_ref().unwrap().correlation_words() != pending
            || self.request != invocation
            || self.waiting.is_some()
            || frame_words(unsafe { ptr::read(frame) }) != before
            || !self.scratch_is_clear()
            || self.mapping_readback.iter().any(|byte| *byte != 0)
        {
            fail("loader_mapping_probe_effect");
        }
        match result {
            Ok(observation) => {
                self.mapping_preparations = 1;
                self.mapping_sequence = sequence;
                self.mapping_observation = observation;
                (
                    0,
                    observation.population.populated_pages,
                    observation.virtual_pages,
                )
            }
            Err(status) => {
                self.mapping_rejections = self
                    .mapping_rejections
                    .checked_add(1)
                    .unwrap_or_else(|| fail("loader_mapping_rejections_exhausted"));
                (status as u64, 0, 0)
            }
        }
    }
    fn context(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(value.destination_index)
            || value.data != 80
            || value.register_10 != 0
            || value.register_8 != 0
            || value.register_9 != 0
        {
            return (4, 0, 0);
        }
        if self
            .execution
            .preflight_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                value.source_index,
                80,
            )
            .is_err()
        {
            return (3, 0, 0);
        }
        if self.allocator_service_borrows == 0 {
            self.probe_allocator_borrow();
        } else if self.allocator_service_borrows != 2 || self.allocator_nested_denials != 1 {
            fail("loader_allocator_probe_counts");
        }
        if !self.file_proof_complete && self.raw_calls == 4 {
            self.capture_file_report();
            self.file_proof_complete = true;
        }
        let caller = self.identities[0];
        let veil = self.identities[2];
        let words = [
            u64::from_le_bytes(*b"ANFCTX01"),
            self.sequence,
            caller.task().slot() as u64 + 1,
            caller.task().generation_value(),
            caller.circle().slot() as u64 + 1,
            caller.circle().generation_value(),
            veil.task().slot() as u64 + 1,
            veil.task().generation_value(),
            self.incarnation,
            self.last_committed,
        ];
        let mut bytes = [0; 80];
        for (slot, word) in bytes.chunks_exact_mut(8).zip(words) {
            slot.copy_from_slice(&word.to_le_bytes());
        }
        if self
            .execution
            .copy_to_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                veil,
                value.source_index,
                &bytes,
            )
            .is_err()
        {
            return (3, 0, 0);
        }
        self.context_queries += 1;
        (0, self.sequence, 80)
    }
    fn delivery_candidate(&self) -> Option<usize> {
        if !self.file_proof_complete || self.sequence == 0 {
            return None;
        }
        let mut index = self.delivery_index;
        let delivery = &self.deliveries[index];
        if delivery.sequence == self.sequence {
            return (!delivery.committed && !delivery.staged).then_some(index);
        }
        if delivery.sequence != 0 {
            if !delivery.committed
                || delivery.staged
                || delivery.sequence >= self.sequence
                || self.stage_bytes.iter().any(|byte| *byte != 0)
                || index + 1 >= self.deliveries.len()
            {
                return None;
            }
            index += 1;
        }
        self.deliveries[index].is_pristine().then_some(index)
    }
    fn stage_copyout(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(self.sequence) || !self.scratch_is_clear() {
            fail("loader_stage_observation_authority");
        }
        let request = self.request;
        let sequence = self.sequence;
        let sealed_sequence = self.pending.as_ref().unwrap().sequence();
        let Some(candidate) = self.delivery_candidate() else {
            return (4, 0, 0);
        };
        if self.deliveries[candidate].probe_length == 0 {
            // Diagnostic admission uses only the actual sealed current caller,
            // never the intentionally invalid sequence supplied by the probe.
            // Retain the witness before the first attempted stage, not after.
            let Some(length) = usize::try_from(value.register_10)
                .ok()
                .filter(|length| (1..=caller_read_capacity).contains(length))
            else {
                return (4, 0, 0);
            };
            if self
                .execution
                .preflight_to_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.pending.as_ref().unwrap(),
                    value.data,
                    length,
                )
                .is_err()
            {
                return (3, 0, 0);
            }
            if self
                .execution
                .copy_from_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.pending.as_ref().unwrap(),
                    value.data,
                    &mut self.copy_scratch[..length],
                )
                .is_err()
            {
                self.clear_scratch();
                return (3, 0, 0);
            }
            // No journal changes until the complete witness has been copied.
            // In particular, a bad first destination preserves the earlier
            // committed journal and does not consume another observation slot.
            let delivery = &mut self.deliveries[candidate];
            delivery.before[..length].copy_from_slice(&self.copy_scratch[..length]);
            delivery.sequence = sequence;
            delivery.probe_address = value.data;
            delivery.probe_length = length;
            self.delivery_index = candidate;
            self.clear_scratch();
        }
        let result = self.prepare_copyout(value);
        let length = self.deliveries[self.delivery_index].probe_length;
        self.execution
            .copy_from_suspended_caller(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.pending.as_ref().unwrap(),
                self.deliveries[self.delivery_index].probe_address,
                &mut self.copy_scratch[..length],
            )
            .unwrap_or_else(|_| fail("loader_stage_probe_after"));
        if self.copy_scratch[..length] != self.deliveries[self.delivery_index].before[..length]
            || self.request != request
            || self.sequence != sequence
            || !self.authenticated_request(sequence)
            || self.waiting.is_some()
            || self.pending.as_ref().unwrap().sequence() != sealed_sequence
            || frame_words(unsafe {
                ptr::read(self.execution.frame(0).unwrap_or_else(|e| fail(e)))
            }) != self.reply_frames[self.raw_calls as usize - 1][0]
            || (result.0 != 0
                && (self.deliveries[self.delivery_index].staged
                    || self.stage_bytes.iter().any(|b| *b != 0)))
        {
            fail("loader_stage_probe_effect");
        }
        self.clear_scratch();
        if result.0 != 0 {
            self.deliveries[self.delivery_index].stage_rejections += 1;
        }
        result
    }
    fn prepare_copyout(&mut self, value: scheduler_frame) -> (u64, u64, u64) {
        if !self.authenticated_request(value.destination_index)
            || !self.file_proof_complete
            || self.deliveries[self.delivery_index].sequence != self.sequence
            || self.deliveries[self.delivery_index].committed
            || self.deliveries[self.delivery_index].staged
            || self.deliveries[self.delivery_index].length != 0
            || value.source_index != 3
            || !(1..=caller_read_capacity as u64).contains(&value.register_10)
            || value.register_9 != 0
        {
            return (4, 0, 0);
        }
        let length = value.register_10 as usize;
        let authority = self.pending.as_ref().unwrap();
        if self
            .execution
            .preflight_to_suspended_caller(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                authority,
                value.data,
                length,
            )
            .is_err()
        {
            return (3, 0, 0);
        }
        if self
            .execution
            .copy_from_user(
                &self.tasks,
                &self.circles,
                &self.scheduler,
                self.identities[2],
                value.register_8,
                &mut self.stage_bytes[..length],
            )
            .is_err()
        {
            self.stage_clear();
            return (3, 0, 0);
        }
        if value.data != self.deliveries[self.delivery_index].probe_address
            || length != self.deliveries[self.delivery_index].probe_length
        {
            self.stage_clear();
            return (4, 0, 0);
        }
        let delivery = &mut self.deliveries[self.delivery_index];
        delivery.sequence = self.sequence;
        delivery.address = value.data;
        delivery.length = length;
        delivery.bytes[..length].copy_from_slice(&self.stage_bytes[..length]);
        delivery.after[..length].copy_from_slice(&delivery.before[..length]);
        delivery.staged = true;
        (0, self.sequence, length as u64)
    }
    fn completion_rejected(&mut self, frame: *mut scheduler_frame) -> *mut scheduler_frame {
        if !self.authenticated_request(self.sequence) {
            fail("loader_rejected_authority");
        }
        let caller = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if frame_words(unsafe { ptr::read(caller) })
            != self.reply_frames[self.raw_calls as usize - 1][0]
        {
            fail("loader_rejected_frame");
        }
        if self.deliveries[self.delivery_index].staged {
            let length = self.deliveries[self.delivery_index].length;
            if self.stage_bytes[..length] != self.deliveries[self.delivery_index].bytes[..length] {
                fail("loader_rejected_stage");
            }
            self.execution
                .copy_from_suspended_caller(
                    &self.tasks,
                    &self.circles,
                    &self.scheduler,
                    self.pending.as_ref().unwrap(),
                    self.deliveries[self.delivery_index].address,
                    &mut self.copy_scratch[..length],
                )
                .unwrap_or_else(|_| fail("loader_rejected_destination"));
            if self.copy_scratch[..length] != self.deliveries[self.delivery_index].before[..length]
            {
                fail("loader_rejected_effect");
            }
            self.clear_scratch();
        }
        let delivery = &mut self.deliveries[self.delivery_index];
        if delivery.sequence == self.sequence && !delivery.committed {
            delivery.completion_rejections += 1;
        }
        if self.mapping_preparations != 0 {
            self.mapping_lifetime.reject_resume();
        }
        unsafe {
            (*frame).accumulator = 4;
            (*frame).data = 0;
            (*frame).counter = 0;
        }
        frame
    }
    fn archive_pending(&mut self) {
        let index = self.raw_calls as usize - 1;
        if !self.path_copies[index].is_clear() {
            fail("loader_copy_archive");
        }
        self.path_copies[index] = self.copy;
        self.copy.clear();
    }
    fn complete_boundary(
        &mut self,
        frame: *mut scheduler_frame,
        value: scheduler_frame,
    ) -> *mut scheduler_frame {
        if !self.authenticated_request(value.destination_index)
            || !matches!(value.data, 0 | 2)
            || value.register_8 != 0
            || value.register_9 != 0
            || value.register_10
                != if self.deliveries[self.delivery_index].staged {
                    self.sequence
                } else {
                    0
                }
            || (value.data == 2 && self.deliveries[self.delivery_index].staged)
        {
            return self.completion_rejected(frame);
        }
        let caller_frame = self.execution.frame(0).unwrap_or_else(|e| fail(e));
        if !self.execution.frame_owned(0, caller_frame) || !self.frame_valid(frame) {
            fail("loader_reply_frames");
        }
        let before = unsafe { ptr::read(caller_frame) };
        let index = self.raw_calls as usize - 1;
        if frame_words(before) != self.reply_frames[index][0] {
            fail("loader_reply_pending_frame");
        }
        // An admitted runtime mapping has one exact reply at its creation
        // invocation. The later invocation may only retire this bounded proof;
        // it cannot reuse the creation handle to grant another continuation.
        let mapping_resume = if value.data == 0
            && (self.mapping_preparations != 0 || self.execution.has_loader_mapping())
        {
            let context = super::owned_execution::loader_mapping_lifetime::lifetime_context {
                tasks: &self.tasks,
                scheduler: &self.scheduler,
                authorization: self.pending.as_ref().unwrap(),
                binding: self.file_binding,
                sequence: self.sequence,
                incarnation: self.incarnation,
            };
            let prepared = memory::with_frame_allocator(|allocator| {
                // SAFETY: The sealed pending creation request, exact saved
                // caller and independent current veil root remain unchanged.
                // No allocator/access borrow crosses the userspace return.
                unsafe {
                    self.execution.prepare_loader_mapping_resume(
                        allocator,
                        context,
                        &self.circles,
                        value.source_index,
                    )
                }
            })
            .unwrap_or_else(|e| fail(e.reason()));
            match prepared {
                Ok(ticket) => Some(ticket),
                Err(_) => return self.completion_rejected(frame),
            }
        } else {
            if self.execution.has_loader_mapping()
                && (!self.execution.loader_mapping_resumed()
                    || self.mapping_sequence.checked_add(1) != Some(self.sequence))
            {
                return self.completion_rejected(frame);
            }
            None
        };
        if value.data == 2 {
            if !self.file_proof_complete || self.scenario != 0 {
                fail("loader_terminal_phase");
            }
            self.capture_service_report();
            if !self.clear_native_buffer(true) {
                fail("loader_native_buffer_retained");
            }
            self.terminal_frames = [frame_words(before); 2];
        }
        let old_sequence = self.sequence;
        let length = if self.deliveries[self.delivery_index].staged {
            self.deliveries[self.delivery_index].length
        } else {
            0
        };
        let prepared = self
            .scheduler
            .prepare_call_reply(&mut self.tasks, &mut self.pending, &mut self.waiting)
            .unwrap_or_else(|_| fail("loader_reply_prepare"));
        self.execution
            .validate_suspended_caller(
                prepared.tasks(),
                &self.circles,
                prepared.scheduler(),
                prepared.authorization(),
            )
            .unwrap_or_else(|_| fail("loader_reply_caller"));
        if prepared.transition().task() != self.identities[0]
            || prepared.transition().context() != scheduler_context::saved
        {
            fail("loader_reply_task");
        }
        if length != 0 {
            self.execution
                .copy_to_suspended_caller_observed(
                    prepared.tasks(),
                    &self.circles,
                    prepared.scheduler(),
                    prepared.authorization(),
                    self.deliveries[self.delivery_index].address,
                    &self.stage_bytes[..length],
                    &mut self.deliveries[self.delivery_index].after[..length],
                )
                .unwrap_or_else(|_| fail("loader_copyout_commit"));
        }
        // The complete destination readback uses the same admission. No new
        // fallible authority operation follows its first write.
        prepared.commit();
        self.boundaries[index].result_bits = value.source_index;
        self.sequence = 0;
        self.request.fill(0);
        self.archive_pending();
        if value.data == 0 {
            let expected = returned_frame(before, value.source_index);
            unsafe {
                (*caller_frame).accumulator = value.source_index;
                (*frame).accumulator = 0;
                (*frame).data = old_sequence;
                (*frame).counter = length as u64;
                self.reply_frames[index][1] = frame_words(ptr::read(caller_frame));
            }
            if self.reply_frames[index][1] != frame_words(expected) {
                fail("loader_reply_frame_effect");
            }
            if let Some(ticket) = mapping_resume {
                self.execution
                    .commit_loader_mapping_resume(ticket)
                    .unwrap_or_else(|_| fail("loader_mapping_resume_commit"));
                self.mapping_lifetime.committed_resume();
            }
            if length != 0 {
                self.deliveries[self.delivery_index].committed = true;
            }
            self.stage_clear();
            self.last_committed = old_sequence;
            self.caller_returns += 1;
            return self.select(0);
        }
        self.scheduler
            .retire_waiting_veil(&mut self.tasks, &mut self.waiting)
            .unwrap_or_else(|_| fail("loader_retire_veil"));
        self.retire_tls(2);
        let next = self
            .scheduler
            .complete_current_and_dispatch(&mut self.tasks, self.identities[0])
            .unwrap_or_else(|_| fail("loader_retire_caller"));
        self.retire_tls(0);
        if next.dispatch().task() != self.identities[1] {
            fail("loader_retire_peer");
        }
        self.select(1)
    }
    fn finish(&mut self) -> u64 {
        self.scheduler
            .complete_current(&mut self.tasks, self.identities[1])
            .unwrap_or_else(|_| fail("loader_finish_peer"));
        self.retire_tls(1);
        // SAFETY: Terminal tasks retain mappings; no user state is live in CPU.
        unsafe { self.execution.restore_bootstrap() }.unwrap_or_else(|e| fail(e));
        if paging::read_cr3() != self.kernel_cr3
            || unsafe { descriptors::selected_privilege_stack_top() } != self.bootstrap_stack
            || self.kernel_stack == 0
            || !unsafe { self.cpu.finish() }
        {
            fail("loader_bootstrap_restore");
        }
        let stack = self.kernel_stack;
        active.store(ptr::null_mut(), Ordering::Release);
        dispatch_active.store(false, Ordering::Release);
        // End this mutable method borrow before the dispatcher performs the
        // nonlocal return to the exact saved bootstrap continuation.
        stack
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator<'_>,
    plan: &native_plan,
    scenario: u64,
) -> &'static mut runtime {
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("loader_storage_busy");
    }
    let pointer = storage.0.get().cast::<runtime>();
    let native_rx = linked_span(
        address!(x86_64_linux_loader_entry_payload_start),
        address!(x86_64_linux_loader_entry_payload_end),
        32768,
    );
    let native_rw = linked_span(
        address!(x86_64_linux_loader_entry_data_start),
        address!(x86_64_linux_loader_entry_data_end),
        8192,
    );
    let file_binding = source_binding::decode(
        native_rw
            .get(file_binding_offset..file_binding_offset + 32)
            .unwrap_or_else(|| fail("loader_file_binding_extent")),
    )
    .unwrap_or_else(|_| fail("loader_file_binding"));
    // SAFETY: Exclusive BSS; every field is initialized before a runtime
    // reference exists. Address-sensitive CPU/launch owners stay fixed after
    // activation; ready source ownership may move without any reader or alias.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new_loader_runtime());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler).write(scheduler_table::new(3).unwrap());
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution_group::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session_group::new());
        ptr::addr_of_mut!((*pointer).tls).write(task_tls::session_group::new());
        ptr::addr_of_mut!((*pointer).source).write(
            source_controller::new(0x4c450001, plan.source_length, plan.source_fnv64)
                .unwrap_or_else(|e| fail(e.as_str())),
        );
        ptr::addr_of_mut!((*pointer).file_source).write(Some(
            source_controller::new_configured(
                file_binding.grant(),
                file_binding.length(),
                file_binding.fnv64(),
                file_source_name,
                file_source_limit,
            )
            .unwrap_or_else(|e| fail(e.as_str())),
        ));
        ptr::addr_of_mut!((*pointer).mapping_lifetime).write(lifetime_observation::default());
        ptr::addr_of_mut!((*pointer).file_binding).write(file_binding);
        ptr::addr_of_mut!((*pointer).file_copy).write(file_copy_observation {
            attempts: 0,
            successes: 0,
            rejections: 0,
            delivered_bytes: 0,
            delivered_fnv64: fnv_offset,
            ordered: true,
            report: [0; 32],
        });
        ptr::addr_of_mut!((*pointer).file_proof_complete).write(false);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).incarnation).write(
            next_service_incarnation
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    v.checked_add(1).filter(|_| v != 0)
                })
                .unwrap_or_else(|_| fail("loader_service_incarnation_exhausted")),
        );
        ptr::addr_of_mut!((*pointer).last_committed).write(0);
        ptr::addr_of_mut!((*pointer).context_queries).write(0);
        // All accounting fields are usize. Initialize inert BSS before forming
        // any runtime reference; preparation replaces this with the live value.
        ptr::write_bytes(
            ptr::addr_of_mut!((*pointer).allocator_live_accounting),
            0,
            1,
        );
        ptr::addr_of_mut!((*pointer).allocator_service_borrows).write(0);
        ptr::addr_of_mut!((*pointer).allocator_nested_denials).write(0);
        ptr::addr_of_mut!((*pointer).mapping_preparations).write(0);
        ptr::addr_of_mut!((*pointer).mapping_rejections).write(0);
        ptr::addr_of_mut!((*pointer).mapping_sequence).write(0);
        ptr::addr_of_mut!((*pointer).mapping_observation)
            .write(loader_mapping_observation::default());
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).mapping_readback), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).deliveries), 0, 1);
        ptr::addr_of_mut!((*pointer).delivery_index).write(0);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).stage_bytes), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).service_report), 0, 1);
        ptr::addr_of_mut!((*pointer).fault_vector).write(0);
        ptr::addr_of_mut!((*pointer).fault_ip).write(0);
        ptr::addr_of_mut!((*pointer).fault_sequence).write(0);
        ptr::addr_of_mut!((*pointer).expected_fault).write(false);
        ptr::addr_of_mut!((*pointer).native_buffer_cleared).write(false);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([true, false, false]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; 3]);
        ptr::addr_of_mut!((*pointer).pending).write(None);
        ptr::addr_of_mut!((*pointer).waiting).write(None);
        ptr::addr_of_mut!((*pointer).request).write([0; 80]);
        ptr::addr_of_mut!((*pointer).sequence).write(0);
        ptr::addr_of_mut!((*pointer).next_sequence).write(1);
        ptr::addr_of_mut!((*pointer).caller_returns).write(0);
        ptr::addr_of_mut!((*pointer).calls).write(0);
        ptr::addr_of_mut!((*pointer).raw_calls).write(0);
        ptr::addr_of_mut!((*pointer).entries).write(0);
        ptr::addr_of_mut!((*pointer).returns).write(0);
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).boundaries), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).reply_frames), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).path_copies), 0, 1);
        ptr::addr_of_mut!((*pointer).terminal_frames).write([[0; 22]; 2]);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).copy), 0, 1);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).copy_scratch), 0, 1);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(paging::read_cr3());
        ptr::addr_of_mut!((*pointer).bootstrap_stack)
            .write(descriptors::selected_privilege_stack_top());
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
        let cpu = &mut *ptr::addr_of_mut!((*pointer).cpu);
        cpu.prepare_linux().unwrap_or_else(|e| fail(e));
        let source = &mut *ptr::addr_of_mut!((*pointer).source);
        let file_source = (&mut *ptr::addr_of_mut!((*pointer).file_source))
            .as_mut()
            .unwrap();
        if let Err(error) = source.import(allocator) {
            source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            file_source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            if !cpu.finish() {
                fail("loader_import_cpu_restore");
            }
            fail(error.as_str());
        }
        if !source.is_ready()
            || source.length() != plan.source_length
            || source.observed_fnv64() != plan.source_fnv64
            || source.chunks() != plan.source_length.div_ceil(4096)
        {
            fail("loader_source_binding");
        }
        if let Err(error) = file_source.import(allocator) {
            file_source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            if !cpu.finish() {
                fail("loader_file_import_cpu_restore");
            }
            fail(error.as_str());
        }
        if !file_source.is_ready()
            || file_source.length() != file_binding.length()
            || file_source.observed_fnv64() != file_binding.fnv64()
            || file_source.chunks() != file_binding.length().div_ceil(4096)
        {
            fail("loader_file_source_binding");
        }
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        let image = plan
            .image(source.reader().unwrap_or_else(|e| fail(e.as_str())))
            .unwrap_or_else(|_| fail("loader_source_plan"));
        let native = (
            (native_rx.as_ptr() as u64, native_rx.len() as u64),
            (native_rw.as_ptr() as u64, native_rw.len() as u64),
        );
        if let Err(error) = execution.prepare_loader_entry(allocator, circles, image, [native; 2]) {
            execution
                .finish(allocator, tasks, circles)
                .unwrap_or_else(|e| fail(e));
            file_source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            source
                .finish(allocator)
                .unwrap_or_else(|e| fail(e.as_str()));
            if !cpu.finish() {
                fail("loader_prepare_cpu_restore");
            }
            fail(error);
        }
        for index in 1..3 {
            execution
                .write_data(index, 0, index as u64)
                .unwrap_or_else(|e| fail(e));
        }
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 601 + index as u64)
                .unwrap_or_else(|e| fail(e))
        });
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        let frame = execution.frame(0).unwrap_or_else(|e| fail(e));
        (*frame).flags = 0x202;
        (*frame).data = 0;
        (*frame).frame_pointer = 0;
        &mut *pointer
    }
}

#[inline(never)]
fn prepare_lifetime(
    allocator: &mut frame_allocator<'_>,
    plan: &native_plan,
    initial: frame_accounting,
    scenario: u64,
) {
    let state = initialize(allocator, plan, scenario);
    for id in state.identities {
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, id)
            .unwrap_or_else(|_| fail("loader_admit"));
    }
    // SAFETY: All retained owners are initialized at their final BSS addresses.
    unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
    let source_pages =
        (plan.source_length.div_ceil(4096) + state.file_binding.length().div_ceil(4096)) as usize;
    let live = allocator.accounting();
    // What this proof owns while it is live, pinned exactly. The data figure
    // tracks what the two installed images map, so it moves when the host's
    // packages do; the rest are structural. The measured values are reported
    // before failing, because a pin that only refuses makes the next reviewer
    // rediscover them from a QEMU run before they can judge the change.
    let expected_free_consumed = 145 + source_pages;
    let expected_data_owned = initial.data_owned + 117 + source_pages;
    if live.free.checked_add(expected_free_consumed) != Some(initial.free)
        || live.data_owned != expected_data_owned
        || live.table_owned != initial.table_owned + 28
        || live.active_leases != initial.active_leases + 23
        || live.bound_leases != initial.bound_leases + 23
    {
        let _ = writeln!(
            serial_port::com1(),
            r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.ownership","level":"error","source_pages":{},"free_consumed":{},"free_consumed_pinned":{},"data_owned":{},"data_owned_pinned":{},"table_owned":{},"table_owned_pinned":{},"active_leases":{},"active_leases_pinned":{},"bound_leases":{},"bound_leases_pinned":{}}}"#,
            source_pages,
            initial.free.saturating_sub(live.free),
            expected_free_consumed,
            live.data_owned.saturating_sub(initial.data_owned),
            expected_data_owned.saturating_sub(initial.data_owned),
            live.table_owned.saturating_sub(initial.table_owned),
            28,
            live.active_leases.saturating_sub(initial.active_leases),
            23,
            live.bound_leases.saturating_sub(initial.bound_leases),
            23
        );
        fail("loader_live_ownership");
    }
    state.allocator_live_accounting = live;
    unsafe { state.tls.prepare(state.identities) }.unwrap_or_else(|e| fail(e.reason()));
    if (0..3).any(|index| {
        !state
            .cpu
            .task_image(index)
            .is_some_and(|i| i.is_initial_clean())
    }) {
        fail("loader_initial_cpu_images");
    }
}

#[inline(never)]
fn enter_lifetime() -> bool {
    if !storage_busy.load(Ordering::Acquire)
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || interrupts::interrupts_enabled()
    {
        fail("loader_enter_phase");
    }
    let pointer = storage.0.get().cast::<runtime>();
    let frame = {
        // SAFETY: Preparation returned with stable BSS owners and IF clear.
        // This exclusive borrow ends before guest dispatch can borrow the BSS.
        let state = unsafe { &mut *pointer };
        let first = state
            .scheduler
            .start_next(&mut state.tasks)
            .unwrap_or_else(|_| fail("loader_start"));
        if first.task() != state.identities[0] || first.context() != scheduler_context::initial {
            fail("loader_first_task");
        }
        let frame = state.select(0);
        if active
            .compare_exchange(
                ptr::null_mut(),
                pointer,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            fail("loader_publication");
        }
        // SAFETY: Rearm the existing PIT cadence only once, after the first
        // preparation. Later cleaned lifetimes retain the running cadence;
        // all fresh owners and CPU state are ready before IRET enables IRQs.
        if state.scenario == 0 && !unsafe { interrupts::rearm_multi_region_timer_before_entry() } {
            fail("loader_timer_rearm");
        }
        state.stage(frame);
        frame
    };
    let entry_rsp: u64;
    // SAFETY: Observe the original bootstrap continuation stack without access.
    unsafe {
        asm!("mov {}, rsp", out(reg) entry_rsp, options(nomem, nostack, preserves_flags));
    }
    // SAFETY: Only raw pointers to retained BSS/frame storage cross entry. No
    // runtime reference or allocator borrow survives guest/nonlocal execution.
    let status =
        unsafe { x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack)) };
    let resumed_rsp: u64;
    // SAFETY: Read the resumed stack before any teardown allocator acquisition.
    unsafe {
        asm!("mov {}, rsp", out(reg) resumed_rsp, options(nomem, nostack, preserves_flags));
    }
    // SAFETY: Guest dispatch has returned to this retained bootstrap frame.
    // Validate the complete continuation before acquiring the allocator again.
    let state = unsafe { &mut *pointer };
    if status != completion_status
        || resumed_rsp != entry_rsp
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != state.kernel_cr3
        || paging::read_cr0() & 8 != state.original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != state.bootstrap_stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !additional_controls_clear()
        || !state.cpu.storage_is_cleared()
        || !state.cpu.linux_entry_is_restored()
        || !state.cpu.linux_entry_storage_is_cleared()
    {
        fail("loader_bootstrap_continuation");
    }
    // SAFETY: Terminal bootstrap continuation retains all task mappings.
    unsafe { state.tls.finish() }.unwrap_or_else(|e| fail(e.reason()));
    let scenario = state.scenario;
    let mapping_scenario = matches!(scenario, 0 | 7);
    let qualified = !(state.raw_calls != state.caller_returns + 1
        || state.raw_calls > boundary_capacity as u64
        || state.next_sequence != state.raw_calls + 1
        || !state.file_proof_complete
        || state.allocator_service_borrows != 2
        || state.allocator_nested_denials != 1
        || state.mapping_preparations != u64::from(mapping_scenario)
        || state.mapping_rejections != if mapping_scenario { 9 } else { 0 }
        || state.mapping_sequence != if mapping_scenario { 11 } else { 0 }
        || state.mapping_readback.iter().any(|byte| *byte != 0)
        || !state.mapping_lifetime.qualified(scenario)
        || state.execution.has_loader_mapping() != mapping_scenario
        || state.execution.loader_mapping_resumed() != mapping_scenario
        || state.delivery_index
            != if matches!(scenario, 1 | 2) {
                0
            } else if matches!(scenario, 3 | 4) {
                1
            } else if matches!(scenario, 5 | 6) {
                2
            } else {
                3
            }
        || state.stage_bytes.iter().any(|byte| *byte != 0)
        || state.deliveries[..=state.delivery_index]
            .iter()
            .enumerate()
            .any(|(index, delivery)| {
                delivery.staged
                    || delivery.source_reads > 1
                    || delivery.source_bytes
                        != if delivery.source_reads == 1 {
                            delivery.length as u64
                        } else {
                            0
                        }
                    || (delivery.source_reads == 0 && delivery.source_offset != 0)
                    || delivery.length == 0
                    || delivery.stage_rejections != 8
                    || delivery.completion_rejections != 4
                    || delivery.committed
                        != !(index == state.delivery_index && matches!(scenario, 1 | 3 | 5))
            })
        || state.fault_vector != if scenario == 0 { 0 } else { 6 }
        || state.expected_fault != (scenario != 0)
        || !state.native_buffer_cleared
        || state.started != [true; 3]);
    if !state.tls.storage_is_cleared()
        || state.terminal != [true; 3]
        || state.pending.is_some()
        || state.waiting.is_some()
        || state.sequence != 0
        || state.request.iter().any(|b| *b != 0)
        || !state.copy.is_clear()
        || !state.scratch_is_clear()
        || state.entries != state.returns
    {
        fail("loader_continuation");
    }
    qualified
}

#[inline(never)]
fn finish_lifetime(allocator: &mut frame_allocator<'_>, initial: frame_accounting) {
    // SAFETY: enter_lifetime returned only after bootstrap root/stack, hardware
    // state and dispatch withdrawal were verified. This is a fresh allocator
    // borrow and all retained BSS owners remain at their original addresses.
    let state = unsafe { &mut *storage.0.get().cast::<runtime>() };
    // SAFETY: Bootstrap continuation proves all tasks and translations are
    // quiescent before runtime leases or the execution alias may be released.
    if state.execution.has_loader_mapping() {
        let observation = unsafe {
            state.execution.release_loader_mapping_storage(
                allocator,
                &mut state.circles,
                &mut state.file_source,
            )
        };
        state.mapping_lifetime.terminal_release(observation);
    }
    for id in state.identities {
        if !state
            .tasks
            .snapshot(id)
            .is_ok_and(|s| matches!(s.state(), task_state::faulted | task_state::exited))
        {
            fail("loader_terminal_state");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, id)
            .unwrap_or_else(|_| fail("loader_withdraw"));
    }
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|e| fail(e));
    unsafe {
        state
            .file_source
            .as_mut()
            .unwrap_or_else(|| fail("loader_source_missing"))
            .finish(allocator)
    }
    .unwrap_or_else(|e| fail(e.as_str()));
    unsafe { state.source.finish(allocator) }.unwrap_or_else(|e| fail(e.as_str()));
    let scheduler = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("loader_final_scheduler"));
    if allocator.accounting() != initial
        || !state.source.is_released()
        || !state
            .file_source
            .as_ref()
            .is_some_and(source_controller::is_released)
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || scheduler.admitted() != 0
        || scheduler.runnable() != 0
        || scheduler.current().is_some()
    {
        fail("loader_final_ownership");
    }
    storage_busy.store(false, Ordering::Release);
}

pub(super) fn install_gate() -> bool {
    // SAFETY: Exclusive one-shot exception initialization.
    unsafe {
        super::idt::install_gate(
            0xf4,
            x86_64_native_application_gate_entry as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}
pub(super) fn is_active() -> bool {
    !active.load(Ordering::Acquire).is_null()
}
pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    let pointer = active.load(Ordering::Acquire);
    // SAFETY: Published storage outlives every IF-clear handler.
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}
fn enter_dispatch() -> &'static mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("loader_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("loader_inactive");
    }
    // SAFETY: Exclusive serialized handler borrows the stable retained state.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut runtime, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    state.stage(frame);
    dispatch_active.store(false, Ordering::Release);
    frame
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    let state = enter_dispatch();
    state.capture(frame.cast());
    if observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
    {
        fail("loader_timer");
    }
    state.last_tick = observed.tick();
    Some(leave_dispatch(state, frame.cast()).cast())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_linux_syscall_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = enter_dispatch();
    state.capture(frame);
    if state.selected != 0
        || state.sequence != 0
        || state.pending.is_some()
        || state.raw_calls >= boundary_capacity as u64
        || state.raw_calls != state.caller_returns
        || !state.copy.is_clear()
        || !state.scratch_is_clear()
    {
        fail("loader_raw_state");
    }
    // SAFETY: capture authenticated the complete frame and saved its CPU state.
    let value = unsafe { ptr::read(frame) };
    if !linux_syscall::entry_frame_matches_hardware(value) {
        fail("loader_raw_hardware");
    }
    let site = value
        .instruction_pointer
        .checked_sub(2)
        .unwrap_or_else(|| fail("loader_syscall_site"));
    let mut bytes = [0; 2];
    state
        .execution
        .copy_from_user(
            &state.tasks,
            &state.circles,
            &state.scheduler,
            state.identities[0],
            site,
            &mut bytes,
        )
        .unwrap_or_else(|_| fail("loader_syscall_copy"));
    if bytes != [0x0f, 0x05] {
        fail("loader_syscall_bytes");
    }
    let arguments = [
        value.destination_index,
        value.source_index,
        value.data,
        value.register_10,
        value.register_8,
        value.register_9,
    ];
    let sequence = state.next_sequence;
    let next_sequence = sequence
        .checked_add(1)
        .filter(|_| sequence != 0)
        .unwrap_or_else(|| fail("loader_request_exhausted"));
    state.reply_frames[state.raw_calls as usize][0] = frame_words(value);
    state.boundaries[state.raw_calls as usize] = boundary {
        sequence,
        number: value.accumulator,
        arguments,
        instruction_pointer: value.instruction_pointer,
        stack_pointer: value.stack_pointer,
        rcx: value.counter,
        r11: value.register_11,
        fs_base: state
            .tls
            .get_fs(state.identities[0])
            .unwrap_or_else(|e| fail(e.reason())),
        result_bits: 0,
    };
    // Fresh numeric correlation accompanies, but never replaces, the sealed
    // scheduler owner and exact task generations. No Linux policy is chosen.
    let request = anomalous_veil_protocol::invocation::new(sequence, value.accumulator, arguments)
        .unwrap_or_else(|_| fail("loader_request_geometry"))
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
        .unwrap_or_else(|_| fail("loader_handoff"));
    if prepared.transition().task() != state.identities[2]
        || prepared.transition().context()
            != if state.raw_calls == 0 {
                scheduler_context::initial
            } else {
                scheduler_context::saved
            }
    {
        fail("loader_handoff_task");
    }
    prepared.commit();
    state.sequence = sequence;
    state.next_sequence = next_sequence;
    state.request = request;
    state.raw_calls += 1;
    if state.execution.has_loader_mapping() {
        if !state.execution.loader_mapping_resumed()
            || state.last_committed != state.mapping_sequence
        {
            fail("loader_mapping_next_authority");
        }
        state.mapping_lifetime.observe_next(sequence);
    }
    let selected = state.select(2);
    leave_dispatch(state, selected)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_native_application_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    let state = enter_dispatch();
    state.capture(frame);
    if state.selected == 0 {
        fail("loader_native_gate_from_linux");
    }
    // SAFETY: Authenticated retained frame, no caller-selected kernel pointer.
    let value = unsafe { ptr::read(frame) };
    if value.vector != 0xf4 || value.error_code != 0 {
        fail("loader_gate_vector");
    }
    state.calls += 1;
    let mut reply = (0, 0, 0);
    let selected = match value.accumulator {
        0 => {
            if state.started[state.selected]
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || !state
                    .cpu
                    .task_image(state.selected)
                    .is_some_and(|i| i.is_initial_clean())
            {
                fail("loader_native_startup");
            }
            state.started[state.selected] = true;
            unsafe {
                (*frame).flags = 0x202;
            }
            reply.1 = state.selected as u64;
            reply.2 = state.scenario;
            frame
        }
        16 if state.selected == 2 && state.started[2] => {
            if !state.authenticated_request(state.sequence)
                || value.source_index != 80
                || value.data != 0
                || value.register_10 != 0
                || value.register_8 != 0
                || value.register_9 != 0
            {
                fail("loader_request_arguments");
            }
            state
                .execution
                .copy_to_user(
                    &state.tasks,
                    &state.circles,
                    &state.scheduler,
                    state.identities[2],
                    value.destination_index,
                    &state.request,
                )
                .unwrap_or_else(|_| fail("loader_request_copy"));
            reply = (0, state.sequence, 80);
            frame
        }
        20 if state.selected == 2 && state.started[2] => {
            let selected = state.complete_boundary(frame, value);
            // Rejected completion already wrote its inert status; successful
            // completion selected the caller and retained a delayed receipt.
            return leave_dispatch(state, selected);
        }
        19 if state.selected == 2 && state.started[2] => {
            reply = state.stage_copyout(value);
            frame
        }
        24 if state.selected == 2 && state.started[2] => {
            reply = state.context(value);
            frame
        }
        18 if state.selected == 2 && state.started[2] => {
            reply = state.read_caller(value);
            frame
        }
        23 if state.selected == 2 && state.started[2] => {
            reply = state.read_file_source(value);
            frame
        }
        25 if state.selected == 2 && state.started[2] => {
            reply = state.probe_source_mapping(value);
            frame
        }
        26..=29 if state.selected == 2 && state.started[2] => {
            reply = state.mapping_lifetime_call(value);
            frame
        }
        5 if state.selected == 1 && state.started[1] && state.terminal[0] && state.terminal[2] => {
            if [
                value.destination_index,
                value.source_index,
                value.data,
                value.register_10,
                value.register_8,
                value.register_9,
            ] != [0; 6]
            {
                fail("loader_peer_exit");
            }
            let stack = state.finish();
            // SAFETY: The final state borrow ended when finish returned. CPU,
            // root and TSS are restored, dispatch is withdrawn, and this raw
            // frame entry retains only the copied bootstrap stack for resume.
            unsafe { x86_64_application_resume_kernel(completion_status, stack) }
        }
        _ => fail("loader_native_operation"),
    };
    if selected == frame {
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
    if !state.frame_owned(frame.cast()) || !super::application_fault::classify_user_fault(snapshot)
    {
        fail("loader_fault_provenance");
    }
    // Faulted RIP/RSP need not satisfy return admission. Only exact retained
    // hardware provenance authorizes terminal capture, never another IRET.
    unsafe {
        state
            .tls
            .consume_entry(state.identities[state.selected], frame)
    }
    .unwrap_or_else(|e| fail(e.reason()));
    if !state.tls.kernel_is_clean() || !unsafe { state.cpu.save_current(state.selected) } {
        fail("loader_fault_cpu");
    }
    state.entries += 1;
    state.fault_vector = snapshot.vector;
    state.fault_ip = snapshot.instruction_pointer;
    state.fault_sequence = state.sequence;
    if state.selected == 2 && state.authenticated_request(state.sequence) {
        let pending_sequence = state.sequence;
        let pending_correlation = state.pending.as_ref().unwrap().correlation_words();
        let pending_request = state.request;
        let caller = state.execution.frame(0).unwrap_or_else(|e| fail(e));
        let before = frame_words(unsafe { ptr::read(caller) });
        if before != state.reply_frames[state.raw_calls as usize - 1][0] {
            fail("loader_fault_caller_frame");
        }
        state.terminal_frames = [before; 2];
        let report_valid = state
            .execution
            .copy_from_user(
                &state.tasks,
                &state.circles,
                &state.scheduler,
                state.identities[2],
                service_report_address,
                &mut state.service_report,
            )
            .is_ok()
            && &state.service_report[..8] == b"ANFSPR01"
            && u64::from_le_bytes(state.service_report[8..16].try_into().unwrap())
                == state.scenario;
        let mut instruction = [0; 2];
        let instruction_read = state
            .execution
            .copy_from_user(
                &state.tasks,
                &state.circles,
                &state.scheduler,
                state.identities[2],
                snapshot.instruction_pointer,
                &mut instruction,
            )
            .is_ok();
        let report = |index: usize| {
            u64::from_le_bytes(
                state.service_report[index * 8..index * 8 + 8]
                    .try_into()
                    .unwrap(),
            )
        };
        state.expected_fault = report_valid
            && instruction_read
            && snapshot.vector == 6
            && snapshot.error_code == 0
            && instruction == [0x0f, 0x0b]
            && state.deliveries[state.delivery_index].completion_rejections == 4
            && match state.scenario {
                1 | 3 | 5 => {
                    state.deliveries[state.delivery_index].staged
                        && !state.deliveries[state.delivery_index].committed
                        && snapshot.instruction_pointer
                            == address!(x86_64_linux_loader_entry_stage_fault)
                        && state.sequence == state.deliveries[state.delivery_index].sequence
                        && report(2) == state.sequence
                        && report(8)
                            == if state.scenario == 1 {
                                0
                            } else {
                                state.deliveries[0].length as u64
                            }
                        && report(9) == 0
                }
                2 | 4 | 6 => {
                    !state.deliveries[state.delivery_index].staged
                        && state.deliveries[state.delivery_index].committed
                        && snapshot.instruction_pointer
                            == address!(x86_64_linux_loader_entry_unknown_fault)
                        && state.sequence == state.deliveries[state.delivery_index].sequence + 1
                        && report(2) == state.deliveries[state.delivery_index].sequence
                        && report(3) == state.deliveries[state.delivery_index].sequence
                        && report(5) == 0
                        && report(8) == u64::MAX
                        && report(9) == 1
                }
                7 => {
                    // This fault is expected only after the actual retained
                    // mapping reply and a newly sealed invocation. A fault at
                    // any earlier phase, from Linux, or at another instruction
                    // still performs containment but cannot qualify this run.
                    snapshot.instruction_pointer
                        == address!(x86_64_linux_loader_entry_unknown_fault)
                        && state.sequence == 12
                        && state.raw_calls == 12
                        && state.caller_returns == 11
                        && state.last_committed == 11
                        && state.mapping_sequence == 11
                        && state.execution.has_loader_mapping()
                        && state.execution.loader_mapping_resumed()
                        && state.mapping_lifetime.qualified(7)
                        && state.delivery_index == 3
                        && state
                            .deliveries
                            .iter()
                            .all(|delivery| delivery.committed && !delivery.staged)
                        && report(2) == 12
                        && report(3) == 11
                        && report(4) == 10
                        && report(5) == 1
                        && report(6) == 3
                        && report(7) == 1
                        && report(8) == state.deliveries[0].length as u64
                        && report(9) == 0
                }
                _ => false,
            };
        if state.deliveries[state.delivery_index].staged {
            let length = state.deliveries[state.delivery_index].length;
            if state
                .execution
                .copy_from_suspended_caller(
                    &state.tasks,
                    &state.circles,
                    &state.scheduler,
                    state.pending.as_ref().unwrap(),
                    state.deliveries[state.delivery_index].address,
                    &mut state.deliveries[state.delivery_index].after[..length],
                )
                .is_err()
                || state.deliveries[state.delivery_index].before[..length]
                    != state.deliveries[state.delivery_index].after[..length]
            {
                state.expected_fault = false;
            }
        }
        let buffer_valid = state.clear_native_buffer(matches!(state.scenario, 2 | 4 | 6 | 7));
        state.expected_fault &= buffer_valid;
        // All diagnostic reads and native-buffer cleanup must preserve the
        // sealed request and the caller's complete saved frame until dependent
        // containment consumes that same authority. No return frame is minted.
        if state.scenario == 7
            && (!state.authenticated_request(pending_sequence)
                || state.sequence != pending_sequence
                || state.pending.as_ref().unwrap().correlation_words() != pending_correlation
                || state.request != pending_request
                || frame_words(unsafe { ptr::read(caller) }) != before)
        {
            fail("loader_post_resume_fault_effect");
        }
        state.archive_pending();
        state.stage_clear();
        state.clear_scratch();
        state.request.fill(0);
        state.sequence = 0;
        let transition = state
            .scheduler
            .prepare_call_fault(&mut state.tasks, &mut state.pending)
            .unwrap_or_else(|_| fail("loader_pair_fault"))
            .commit();
        state.retire_tls(0);
        state.retire_tls(2);
        if transition.task() != state.identities[1] {
            fail("loader_fault_peer");
        }
    } else if state.selected == 0 && state.pending.is_none() && state.waiting.is_some() {
        // Unexpected caller failure still retires its blocked dependency and
        // preserves the peer. Final qualification rejects this unplanned run.
        state
            .scheduler
            .retire_waiting_veil(&mut state.tasks, &mut state.waiting)
            .unwrap_or_else(|_| fail("loader_caller_fault_veil"));
        state.retire_tls(2);
        let transition = state
            .scheduler
            .fault_current(&mut state.tasks, state.identities[0])
            .unwrap_or_else(|_| fail("loader_caller_fault"));
        state.retire_tls(0);
        state.stage_clear();
        state.clear_scratch();
        state.request.fill(0);
        state.sequence = 0;
        if transition.dispatch().task() != state.identities[1] {
            fail("loader_caller_fault_peer");
        }
    } else if state.selected == 0
        && state.pending.is_none()
        && state.waiting.is_none()
        && state.raw_calls == 0
        && !state.started[2]
    {
        let peer = state
            .scheduler
            .fault_current(&mut state.tasks, state.identities[0])
            .unwrap_or_else(|_| fail("loader_initial_caller_fault"));
        state.retire_tls(0);
        if peer.dispatch().task() != state.identities[1] {
            fail("loader_initial_fault_peer");
        }
        let veil = state
            .scheduler
            .preempt_current(&mut state.tasks, state.identities[1])
            .unwrap_or_else(|_| fail("loader_initial_fault_veil_select"));
        if veil.dispatch().task() != state.identities[2] {
            fail("loader_initial_fault_veil");
        }
        let peer = state
            .scheduler
            .fault_current(&mut state.tasks, state.identities[2])
            .unwrap_or_else(|_| fail("loader_initial_fault_veil_retire"));
        state.retire_tls(2);
        if peer.dispatch().task() != state.identities[1] {
            fail("loader_initial_fault_final_peer");
        }
        // No native user continuation ran. Owner teardown clears all user RAM;
        // no per-buffer observed wipe receipt is claimed for this failed run.
    } else {
        fail("loader_unexpected_fault_relation");
    }
    let selected = state.select(1);
    Some(leave_dispatch(state, selected).cast())
}
pub(super) fn resume_kernel_after_fault() -> ! {
    fail("loader_unexpected_fault_resume")
}

pub(super) fn run() -> ! {
    // Boot enters with IF set. The scheduler restriction owns the transition
    // into this proof's exclusive IF-clear preparation interval.
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !additional_controls_clear()
    {
        fail("loader_initial_state");
    }
    // SAFETY: Sole BSP, IF clear. Withdraw the bootstrap failure renderer before
    // private user roots become active; this proof publishes only serial data.
    unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("loader_framebuffer_withdraw"));
    let bytes = linked_span(
        address!(x86_64_linux_loader_entry_plan_start),
        address!(x86_64_linux_loader_entry_plan_end),
        super::linux_loader_plan::encoded_size as u64,
    );
    // SAFETY: Sole one-shot bootstrap initialization before owner publication.
    let plan = unsafe {
        plan_storage
            .0
            .get()
            .cast::<native_plan>()
            .write(native_plan::decode(bytes).unwrap_or_else(|_| fail("loader_plan")));
        &*plan_storage.0.get().cast::<native_plan>()
    };
    for scenario in 0..8 {
        let initial = memory::with_frame_allocator(|allocator| {
            let initial = allocator.accounting();
            prepare_lifetime(allocator, plan, initial, scenario);
            initial
        })
        .unwrap_or_else(|e| fail(e.reason()));
        let qualified = enter_lifetime();
        memory::with_frame_allocator(|allocator| finish_lifetime(allocator, initial))
            .unwrap_or_else(|e| fail(e.reason()));
        emit_observation(plan, qualified);
        if !qualified {
            fail("loader_unqualified_run_after_cleanup");
        }
    }
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("loader_final_interrupts");
    }
    super::qemu_exit_success();
    super::halt_forever()
}

fn emit_hex(serial: &mut serial_port, bytes: &[u8]) {
    for byte in bytes {
        let _ = write!(serial, "{byte:02x}");
    }
}
fn emit_frame(serial: &mut serial_port, words: &[u64; 22]) {
    for word in words {
        emit_hex(serial, &word.to_le_bytes());
    }
}
fn emit_observation(plan: &native_plan, qualified: bool) {
    // SAFETY: Each fully cleaned lifetime has withdrawn active dispatch. Emit
    // from retained BSS before the next initialization can overwrite journals.
    // Failed qualification emits diagnostic observations too, but can never
    // emit the passed event; the caller stops before starting another lifetime.
    let state = unsafe { &mut *storage.0.get().cast::<runtime>() };
    let scenario = state.scenario;
    let binding = &state.file_binding;
    // Reported from the plan this proof actually mapped rather than as a
    // literal. It follows the installed images, so a host whose packages have
    // moved changes it, and a number written here by hand would have to be
    // re-pinned in step with the one in the build stage.
    let user_pages: u64 = plan.regions.iter().map(|region| region.page_count).sum();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.ready","level":"info","scenario":{},"incarnation":{},"caller_task":{},"caller_generation":{},"veil_task":{},"veil_generation":{},"tasks":3,"regions":9,"user_pages":{},"source_bytes":{},"source_fnv64":{},"file_source_bytes":{},"file_source_fnv64":{},"file_source_pages":{},"initial_rip":{},"initial_rsp":{},"main_entry":{},"main_phdr":{},"interpreter_bias":{},"break_start":{},"source_kind":"privileged_fw_cfg","policy":"userspace","cpu":"{}"}}"#,
        scenario,
        state.incarnation,
        state.identities[0].task().slot() + 1,
        state.identities[0].task().generation_value(),
        state.identities[2].task().slot() + 1,
        state.identities[2].task().generation_value(),
        user_pages,
        plan.source_length,
        plan.source_fnv64,
        binding.length(),
        binding.fnv64(),
        binding.length().div_ceil(4096),
        plan.entry,
        plan.stack_pointer,
        plan.reported_main_entry,
        plan.reported_main_phdr,
        plan.reported_interpreter_bias,
        plan.reported_break,
        state.cpu.cpu_vendor()
    );
    for index in 0..state.raw_calls as usize {
        let boundary = &state.boundaries[index];
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.boundary","level":"info","scenario":{},"sequence":{},"number":{},"arg0":{},"arg1":{},"arg2":{},"arg3":{},"arg4":{},"arg5":{},"instruction_pointer":{},"syscall_instruction":{},"stack_pointer":{},"rcx":{},"r11":{},"fs_base":{},"result_bits":{},"returned_to_caller":{},"linux_exit_observed":false,"syscall_bytes_verified":true}}"#,
            scenario,
            boundary.sequence,
            boundary.number,
            boundary.arguments[0],
            boundary.arguments[1],
            boundary.arguments[2],
            boundary.arguments[3],
            boundary.arguments[4],
            boundary.arguments[5],
            boundary.instruction_pointer,
            boundary.instruction_pointer - 2,
            boundary.stack_pointer,
            boundary.rcx,
            boundary.r11,
            boundary.fs_base,
            boundary.result_bits,
            index < state.caller_returns as usize
        );
        if index < state.caller_returns as usize {
            let copy = &state.path_copies[index];
            let _ = write!(
                serial,
                r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.reply","level":"info","scenario":{},"sequence":{},"result_bits":{},"frame_before_hex":""#,
                scenario, boundary.sequence, boundary.result_bits
            );
            emit_frame(&mut serial, &state.reply_frames[index][0]);
            let _ = write!(serial, r#"","frame_after_hex":""#);
            emit_frame(&mut serial, &state.reply_frames[index][1]);
            let _ = write!(
                serial,
                r#"","copy_sequence":{},"copy_address":{},"copy_bytes":{},"copy_hex":""#,
                copy.sequence, copy.address, copy.length
            );
            emit_hex(&mut serial, &copy.bytes[..copy.length]);
            let _ = writeln!(serial, r#"","committed":true}}"#);
        }
    }
    if let Some(terminal) = state
        .raw_calls
        .checked_sub(1)
        .and_then(|index| state.path_copies.get(index as usize))
    {
        let _ = write!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.terminal_copy","level":"info","scenario":{},"sequence":{},"frame_before_hex":""#,
            scenario, state.raw_calls
        );
        emit_frame(&mut serial, &state.terminal_frames[0]);
        let _ = write!(serial, r#"","frame_after_hex":""#);
        emit_frame(&mut serial, &state.terminal_frames[1]);
        let _ = write!(
            serial,
            r#"","copy_sequence":{},"copy_address":{},"copy_bytes":{},"copy_hex":""#,
            terminal.sequence, terminal.address, terminal.length
        );
        emit_hex(&mut serial, &terminal.bytes[..terminal.length]);
        let _ = writeln!(
            serial,
            r#"","copy_attempts":{},"copy_successes":{},"copy_rejections":{},"pending_preserved":true}}"#,
            terminal.attempts, terminal.successes, terminal.rejections
        );
    }
    let file = &state.file_copy;
    let word = |offset| u64::from_le_bytes(file.report[offset..offset + 8].try_into().unwrap());
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.file_source","level":"info","scenario":{},"sequence":4,"grant":{},"source_bytes":{},"source_fnv64":{},"source_pages":{},"attempts":{},"successes":{},"rejections":{},"delivered_bytes":{},"delivered_fnv64":{},"guest_bytes":{},"guest_fnv64":{},"guest_chunks":{},"pending_preserved":true,"scratch_cleared":true,"source_cleared":true}}"#,
        scenario,
        binding.grant(),
        binding.length(),
        binding.fnv64(),
        binding.length().div_ceil(4096),
        file.attempts,
        file.successes,
        file.rejections,
        file.delivered_bytes,
        file.delivered_fnv64,
        word(8),
        word(16),
        word(24)
    );
    for delivery in &state.deliveries[..=state.delivery_index] {
        let length = delivery.length;
        let _ = write!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.file_delivery","level":"info","scenario":{},"sequence":{},"address":{},"length":{},"source_offset":{},"stage_token":{},"stage_bytes_hex":""#,
            scenario,
            delivery.sequence,
            delivery.address,
            length,
            delivery.source_offset,
            delivery.sequence
        );
        emit_hex(&mut serial, &delivery.bytes[..length]);
        let _ = write!(serial, r#"","before_hex":""#);
        emit_hex(&mut serial, &delivery.before[..length]);
        let _ = write!(serial, r#"","after_hex":""#);
        emit_hex(&mut serial, &delivery.after[..length]);
        let _ = writeln!(
            serial,
            r#"","committed":{},"stage_rejections":{},"completion_rejections":{},"source_reads":{},"source_bytes":{},"rejections_preserved":true,"scratch_cleared":true,"stage_cleared":true}}"#,
            delivery.committed,
            delivery.stage_rejections,
            delivery.completion_rejections,
            delivery.source_reads,
            delivery.source_bytes
        );
    }
    let word = |index: usize| {
        u64::from_le_bytes(
            state.service_report[index * 8..index * 8 + 8]
                .try_into()
                .unwrap(),
        )
    };
    let _ = write!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.file_service","level":"info","scenario":{},"active_request_sequence":{},"acknowledged_sequence":{},"service_last_sequence":{},"open_count":{},"fd":{},"handle_generation":{},"offset":{},"fenced":{},"report_hex":""#,
        scenario,
        word(2),
        word(3),
        word(4),
        word(5),
        word(6),
        word(7),
        word(8),
        word(9) == 1
    );
    emit_hex(&mut serial, &state.service_report);
    let _ = writeln!(serial, r#""}}"#);
    let mapping = state.mapping_observation;
    let population = mapping.population;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.mapping_cancel","level":"info","scenario":{},"sequence":{},"preparations":{},"rejections":{},"base":{},"byte_length":{},"virtual_pages":{},"source_bytes":{},"source_fnv64":{},"destination_fnv64":{},"zero_bytes":{},"populated_pages":{},"absent_pages":{},"table_frames":{},"caller_tree_fnv64":{},"cleanup_retries":{},"unpublished":true,"cancelled":{},"accounting_restored":true,"caller_preserved":true,"scratch_cleared":true}}"#,
        scenario,
        state.mapping_sequence,
        state.mapping_preparations,
        state.mapping_rejections,
        mapping.virtual_address,
        mapping.byte_length,
        mapping.virtual_pages,
        population.source_bytes,
        population.source_fnv64,
        population.destination_fnv64,
        population.zero_bytes,
        population.populated_pages,
        population.absent_pages,
        population.table_frames,
        mapping.table_fingerprint,
        mapping.cleanup_retries,
        state.mapping_preparations == 1,
    );
    mapping_lifetime::emit(state, &mut serial);
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.containment","level":"info","scenario":{},"fault_vector":{},"fault_instruction_pointer":{},"pending_sequence":{},"last_committed_sequence":{},"expected_fault":{},"peer_survived":true}}"#,
        scenario,
        state.fault_vector,
        state.fault_ip,
        if scenario == 0 {
            state.raw_calls
        } else {
            state.fault_sequence
        },
        state.last_committed,
        state.expected_fault
    );
    let attempts = state
        .path_copies
        .iter()
        .map(|copy| copy.attempts)
        .sum::<u64>();
    let successes = state
        .path_copies
        .iter()
        .map(|copy| copy.successes)
        .sum::<u64>();
    let rejections = state
        .path_copies
        .iter()
        .map(|copy| copy.rejections)
        .sum::<u64>();
    for copy in &mut state.path_copies {
        copy.clear();
        if !copy.is_clear() {
            fail("loader_copy_retirement");
        }
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.{}","level":"{}","scenario":{},"raw_calls":{},"caller_returns":{},"native_calls":{},"retired_tasks":3,"copy_attempts":{},"copy_successes":{},"copy_rejections":{},"context_queries":{},"allocator_service_borrows":{},"allocator_nested_denials":{},"copy_scratch_cleared":true,"copy_journal_cleared":true,"stage_cleared":{},"native_buffer_cleared":{},"source_cleared":true,"tls_cleared":true,"cpu_restored":true,"resources_restored":true,"request_cleared":true}}"#,
        if qualified { "passed" } else { "unqualified" },
        if qualified { "info" } else { "error" },
        scenario,
        state.raw_calls,
        state.caller_returns,
        state.calls,
        attempts,
        successes,
        rejections,
        state.context_queries,
        state.allocator_service_borrows,
        state.allocator_nested_denials,
        !state.deliveries[state.delivery_index].staged
            && state.stage_bytes.iter().all(|byte| *byte == 0),
        state.native_buffer_cleared
    );
    if !qualified {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.qualification_failed","level":"error","scenario":{},"next_sequence":{},"file_proof_complete":{},"delivery_staged":{},"native_buffer_cleared":{},"caller_started":{},"peer_started":{},"veil_started":{},"expected_fault":{},"fault_vector":{},"fault_instruction_pointer":{},"peer_survived":true,"resources_restored":true}}"#,
            scenario,
            state.next_sequence,
            state.file_proof_complete,
            state.deliveries[state.delivery_index].staged,
            state.native_buffer_cleared,
            state.started[0],
            state.started[1],
            state.started[2],
            state.expected_fault,
            state.fault_vector,
            state.fault_ip
        );
    }
}
pub(super) fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let _ = writeln!(
        serial_port::com1(),
        r#"{{"schema":1,"event":"kernel.linux_loader_entry_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
