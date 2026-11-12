//! Shared one-task delivery lifecycle for ADR 0101 and native admission ADR 0103.
//!
//! ANPG describes the launch; the controller separately binds the console to
//! its exact live task. Requests use bounded owned user-copy, complete text
//! preflight, and a monotonic success sequence before any serial publication.
//! Edition bytes are copied from a retained sealed source, never executed.
//! The optional manifest specialization admits packages in userspace.
//! Edition interpretation remains a separate qualification.
//!
//! The parent module lowers `dead_code` and `unused_imports` under this
//! feature, because a program build leaves its two-task half unreachable.
//! Re-denied here so the lowered level never reaches the code that does run.
#![deny(dead_code, unused_imports)]

#[cfg(not(feature = "native_manifest_test"))]
use super::super::arena_profile::program_arena_bases;
#[cfg(feature = "native_manifest_test")]
#[path = "native_manifest_profile.rs"]
mod manifest;
use super::super::owned_execution::{owned_execution_group, user_data_base};
use super::super::{linux_loader_source, program_console, program_edition};
use super::*;
use anomalous_native_runtime::{
    maximum_read_bytes, maximum_write_bytes, program_descriptor, program_startup_size,
    program_status, service,
};
#[cfg(feature = "native_manifest_test")]
use manifest::{arena_bytes, expected_data_frames, program_code_pages, scenario};

/// Where the descriptor sits in the task's private RW data. The vessel's
/// linker script pins the same address, and startup admission compares the
/// registers against it.
const descriptor_offset: u64 = 4096;
const descriptor_address: u64 = user_data_base + descriptor_offset;

/// The arena length this profile expects to be given.
///
/// Taken from the ABI's own stated minimum rather than from `arena_profile`.
/// Comparing the mapping against the profile that produced it would prove
/// only that the two agree, and they can drift together while it passes; the
/// contract `program_descriptor` enforces is the independent authority.
#[cfg(not(feature = "native_manifest_test"))]
const arena_bytes: u64 = anomalous_native_runtime::minimum_arena_bytes;
#[cfg(not(feature = "native_manifest_test"))]
const program_code_pages: u64 = 2;
#[cfg(not(feature = "native_manifest_test"))]
const edition_length: u64 = 8195;
#[cfg(not(feature = "native_manifest_test"))]
const _: usize = 3;
#[cfg(not(feature = "native_manifest_test"))]
const edition_source_fnv: u64 = 0x84ca7df1efd6b401;
#[cfg(not(feature = "native_manifest_test"))]
const edition_source_name: &[u8] = b"opt/anomalous-program-edition";

/// The arena offset and word count the vessel writes its grant record at.
/// Offset zero: a program's first act is to state what it was given.
#[cfg(not(feature = "native_manifest_test"))]
const proof_offset: u64 = 0;
const proof_words: usize = 6;

/// Frame accounting for one program task, derived rather than measured.
///
/// `data_pages` for this geometry is 2 code + 2 data + 16 arena + 2 stack =
/// 22, and `prepare_profile` then allocates one `transition_pages` stack of
/// 4, so 26 data frames. Nine table frames is the resident count the host
/// fixture brackets: the user plan needs eight and `resident_plan` adds one.
/// Six leases is one per mapped region plus the table lease plus the
/// transition stack.
///
/// These are expectations, not receipts. The run below is what measures them,
/// and it fails closed on any of them rather than reporting what it found.
#[cfg(not(feature = "native_manifest_test"))]
const expected_data_frames: usize = 26;
const expected_table_frames: usize = 9;
const expected_leases: usize = 6;
const program_frames: usize = expected_data_frames + expected_table_frames;

/// What a single run grants.
///
/// Two runs, one per admitted base, because the base must not be baked into
/// the launch. The console pairing is proved in both directions: granted with
/// a handle, and absent with none. `program_descriptor::try_new` refuses
/// either one without the other, and the vessel checks the pairing again on
/// the side of the boundary that would be lied to.
#[derive(Clone, Copy)]
struct scenario_profile {
    base: u64,
    console_handle: u64,
    granted_services: u64,
    edition_handle: u64,
    edition_length: u64,
    arguments: &'static [u8],
    source_fnv: u64,
    source_name: &'static [u8],
}
#[cfg(not(feature = "native_manifest_test"))]
const scenarios: [scenario_profile; 2] = [
    scenario_profile {
        base: program_arena_bases[0],
        console_handle: 0x62,
        granted_services: service::console_write | service::file_read,
        edition_handle: 0x41,
        edition_length,
        arguments: b"hello\0--once",
        source_fnv: edition_source_fnv,
        source_name: edition_source_name,
    },
    scenario_profile {
        base: program_arena_bases[1],
        console_handle: 0,
        granted_services: 0,
        edition_handle: 0x9f,
        edition_length: 64,
        arguments: b"",
        source_fnv: 0,
        source_name: b"",
    },
];

#[cfg(not(feature = "native_manifest_test"))]
const scenarios_count: usize = scenarios.len();
#[cfg(not(feature = "native_manifest_test"))]
fn scenario(index: usize) -> scenario_profile {
    *scenarios
        .get(index)
        .unwrap_or_else(|| fail("program_scenario"))
}
#[cfg(not(feature = "native_manifest_test"))]
const event_prefix: &str = "kernel.native_program_test";
#[cfg(feature = "native_manifest_test")]
const event_prefix: &str = "kernel.native_manifest_test";

static active: AtomicPtr<program> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);
static storage_busy: AtomicBool = AtomicBool::new(false);
struct retained_storage(UnsafeCell<core::mem::MaybeUninit<program>>);
// SAFETY: Sole BSP, IF clear. The latch covers initialization through teardown.
unsafe impl Sync for retained_storage {}
static storage: retained_storage =
    retained_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));

#[cfg(not(feature = "native_manifest_test"))]
unsafe extern "C" {
    static x86_64_native_program_payload_start: u8;
    static x86_64_native_program_payload_end: u8;
    static x86_64_native_program_user_fault_instruction: u8;
}

#[cfg(feature = "native_manifest_test")]
unsafe extern "C" {
    #[link_name = "x86_64_native_manifest_payload_start"]
    static x86_64_native_program_payload_start: u8;
    #[link_name = "x86_64_native_manifest_payload_end"]
    static x86_64_native_program_payload_end: u8;
    #[link_name = "x86_64_native_manifest_user_fault_instruction"]
    static x86_64_native_program_user_fault_instruction: u8;
}

#[derive(Clone, Copy)]
struct program_image {
    source: u64,
    length: u64,
}
impl program_image {
    fn inspect() -> Self {
        let source = address!(x86_64_native_program_payload_start);
        let length = address!(x86_64_native_program_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("program_payload_extent"));
        let fault = address!(x86_64_native_program_user_fault_instruction);
        // The selected named profile supplies an independent exact code bound.
        // Refuse an oversized payload before acquiring or copying any mapping.
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > program_code_pages * paging::page_size
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || fault < user_code_base
            || fault
                .checked_add(2)
                .is_none_or(|end| end > user_code_base + length)
        {
            fail("program_artifact_geometry");
        }
        // SAFETY: Inspected complete two-byte marker in immutable resident payload.
        if unsafe { ptr::read_unaligned((source + fault - user_code_base) as *const [u8; 2]) }
            != [0x0f, 0x0b]
        {
            fail("program_artifact_fault_marker");
        }
        Self { source, length }
    }
}

struct program {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identity: task_identity,
    execution: owned_execution_group<1>,
    cpu: application_cpu::cpu_session_group<1>,
    image: program_image,
    /// Six controller-owned startup facts. The delivery fixture restates them;
    /// the manifest fixture instead returns a separate bounded diagnostic.
    granted: [u64; proof_words],
    arena_base: u64,
    scenario: usize,
    zeroed_bytes: u64,
    calls: u64,
    timer_ticks: u64,
    invalid: u64,
    exits: u64,
    started: bool,
    terminal: bool,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
    console: program_console::state,
    console_scratch: [u8; maximum_write_bytes],
    console_attempts: u64,
    console_rejected: u64,
    edition: program_edition::state,
    edition_scratch: [u8; maximum_read_bytes],
    source: Option<linux_loader_source::source_controller>,
    live_accounting: Option<frame_accounting>,
    edition_attempts: u64,
    edition_rejected: u64,
    edition_source_reads: u64,
    allocator_service_borrows: u64,
    allocator_nested_denials: u64,
}

#[derive(Clone, Copy)]
struct program_record {
    calls: u64,
    timer_ticks: u64,
    invalid: u64,
    exits: u64,
    zeroed_bytes: u64,
    granted: [u64; proof_words],
    stated: [u64; proof_words],
    #[cfg(feature = "native_manifest_test")]
    report: [u8; anomalous_native_runtime::manifest_report::report_bytes],
    console_attempts: u64,
    console_rejected: u64,
    console_writes: u64,
    console_bytes: u64,
    edition_attempts: u64,
    edition_rejected: u64,
    edition_reads: u64,
    edition_read_bytes: u64,
    edition_source_reads: u64,
    allocator_service_borrows: u64,
    allocator_nested_denials: u64,
}

impl program {
    fn frame_owned(&self, frame: *const scheduler_frame) -> bool {
        // SAFETY: The sole BSP reads its initialized TSS with interrupts excluded.
        self.execution.frame_owned(0, frame)
            && self.execution.root(0).ok() == Some(paging::read_cr3())
            && self.execution.transition_stack_top(0).ok()
                == Some(unsafe { descriptors::selected_privilege_stack_top() })
            && self.cpu.controls_are_active()
            && self
                .scheduler
                .snapshot(&self.tasks)
                .is_ok_and(|s| s.current() == Some(self.identity))
            && self
                .tasks
                .snapshot(self.identity)
                .is_ok_and(|s| s.state() == task_state::running && s.context_attached())
    }
    fn frame_valid(&self, frame: *const scheduler_frame) -> bool {
        if !self.frame_owned(frame) {
            return false;
        }
        let Ok(snapshot) = self.tasks.snapshot(self.identity) else {
            return false;
        };
        // SAFETY: frame_owned validated the complete aligned retained frame.
        let value = unsafe { ptr::read(frame) };
        user_return_fields_are_valid_within_stack(value, snapshot.bounds())
            && self
                .execution
                .admission(0)
                .is_ok_and(|a| a.accepts_return(value.instruction_pointer, value.stack_pointer))
            && value.flags & !0x25_0fd7 == 0
            && (!self.started || value.flags & 0x202 == 0x202)
    }
    /// The grant record the task wrote, read back through the kernel's own
    /// alias. Nothing in userspace is trusted to report it.
    #[cfg(not(feature = "native_manifest_test"))]
    fn stated_grant(&self) -> [u64; proof_words] {
        core::array::from_fn(|word| {
            self.execution
                .read_arena(0, proof_offset + word as u64 * 8)
                .unwrap_or_else(|| fail("program_proof_readback"))
        })
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
            fail("program_bootstrap_restore");
        }
        let stack = self.kernel_stack;
        dispatch_active.store(false, Ordering::Release);
        active.store(ptr::null_mut(), Ordering::Release);
        // SAFETY: stack is the exact suspended enter continuation; bootstrap root
        // and CPU are restored and no handler can access the withdrawn program.
        unsafe { x86_64_application_resume_kernel(completion_status, stack) }
    }
}

// Return constructor temporaries before any mapping preparation or activation.
#[inline(never)]
unsafe fn initialize_storage(
    pointer: *mut program,
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    arena_base: u64,
    granted: [u64; proof_words],
) {
    // SAFETY: Caller owns the sole storage latch over uninitialized or fully
    // retired BSS. No whole-program reference exists until the identity is
    // written after retained preparation. No live owner is moved.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new_native_arenas());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(1).unwrap_or_else(|_| fail("program_scheduler")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution_group::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session_group::new());
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).granted).write(granted);
        ptr::addr_of_mut!((*pointer).arena_base).write(arena_base);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(0);
        ptr::addr_of_mut!((*pointer).calls).write(0);
        ptr::addr_of_mut!((*pointer).timer_ticks).write(0);
        ptr::addr_of_mut!((*pointer).invalid).write(0);
        ptr::addr_of_mut!((*pointer).exits).write(0);
        ptr::addr_of_mut!((*pointer).started).write(false);
        ptr::addr_of_mut!((*pointer).terminal).write(false);
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
        ptr::addr_of_mut!((*pointer).console_attempts).write(0);
        ptr::addr_of_mut!((*pointer).console_rejected).write(0);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).console_scratch), 0, 1);
        ptr::addr_of_mut!((*pointer).source).write(None);
        ptr::addr_of_mut!((*pointer).live_accounting).write(None);
        ptr::addr_of_mut!((*pointer).allocator_service_borrows).write(0);
        ptr::addr_of_mut!((*pointer).allocator_nested_denials).write(0);
        ptr::addr_of_mut!((*pointer).edition_attempts).write(0);
        ptr::addr_of_mut!((*pointer).edition_rejected).write(0);
        ptr::addr_of_mut!((*pointer).edition_source_reads).write(0);
        ptr::write_bytes(ptr::addr_of_mut!((*pointer).edition_scratch), 0, 1);
    }
}

#[inline(never)]
fn initialize(
    allocator: &mut frame_allocator,
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
) -> &'static mut program {
    let selected = self::scenario(scenario);
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("program_storage_busy");
    }
    let pointer = storage.0.get().cast::<program>();
    // Built before anything is acquired, so an unsatisfiable launch costs
    // nothing. The descriptor is the contract; its own validation decides
    // whether this geometry and these grants are admissible at all.
    let descriptor = program_descriptor::try_new(
        selected.base,
        arena_bytes,
        selected.edition_handle,
        selected.edition_length,
        selected.console_handle,
        selected.granted_services,
        selected.arguments,
    )
    .unwrap_or_else(|_| fail("program_descriptor"));
    let granted = [
        descriptor.arena_length(),
        descriptor.edition_length(),
        descriptor.granted_services(),
        descriptor.arguments().len() as u64,
        u64::from(descriptor.console().is_some()),
        descriptor.edition_handle(),
    ];
    // SAFETY: Exclusive BSS latch, IF clear and NMI masked. Initialize each
    // field before borrowing it; the whole program remains unborrowed until
    // complete.
    unsafe {
        initialize_storage(
            pointer,
            image,
            scenario,
            kernel_cr3,
            bootstrap_stack,
            selected.base,
            granted,
        );
        // Provision before installing the execution alias: both use slot 288.
        // The retained BSS field owns partial imports through explicit cleanup.
        if selected.granted_services & service::file_read != 0 {
            let before = allocator.accounting();
            let source = &mut *ptr::addr_of_mut!((*pointer).source);
            *source = Some(
                linux_loader_source::source_controller::new_configured(
                    0x4544_0001,
                    selected.edition_length,
                    selected.source_fnv,
                    selected.source_name,
                    selected.edition_length,
                )
                .unwrap_or_else(|_| fail("program_source_configuration")),
            );
            let source = source
                .as_mut()
                .unwrap_or_else(|| fail("program_source_owner"));
            if let Err(error) = source.import(allocator) {
                let reason = match error {
                    linux_loader_source::source_load_error::missing_entry => {
                        "program_source_missing"
                    }
                    linux_loader_source::source_load_error::size => "program_source_size",
                    linux_loader_source::source_load_error::hash => "program_source_hash",
                    _ => error.as_str(),
                };
                source
                    .finish(allocator)
                    .unwrap_or_else(|_| fail("program_source_cleanup"));
                if !source.is_released() || allocator.accounting() != before {
                    fail("program_source_rejection_cleanup");
                }
                let mut serial = serial_port::com1();
                let _ = writeln!(
                    serial,
                    r#"{{"schema":1,"event":"{}.source_rejected","level":"info","reason":"{}","source_released":true,"resources_restored":true}}"#,
                    event_prefix, reason
                );
                fail(reason);
            }
            if !source.is_ready()
                || source.length() != selected.edition_length
                || source.observed_fnv64() != selected.source_fnv
                || source.chunks() != selected.edition_length.div_ceil(4096)
            {
                fail("program_source_import_evidence");
            }
        }
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        #[cfg(not(feature = "native_manifest_test"))]
        execution
            .prepare_program_arena(
                allocator,
                circles,
                (image.source, image.length),
                selected.base,
            )
            .unwrap_or_else(|e| fail(e));
        #[cfg(feature = "native_manifest_test")]
        execution
            .prepare_manifest_arena(
                allocator,
                circles,
                (image.source, image.length),
                selected.base,
            )
            .unwrap_or_else(|e| fail(e));
        // The arena the launch promised must be the arena the task got. A
        // native application owner at this same base would report 32 KiB.
        if execution.arena_extent(0) != Some((selected.base, arena_bytes)) {
            fail("program_arena_extent");
        }
        for offset in (0..8192).step_by(8) {
            if execution.read_data(0, offset) != Some(0) {
                fail("program_initial_storage");
            }
        }
        let stack_bytes = if cfg!(feature = "native_manifest_test") {
            32768
        } else {
            8192
        };
        for offset in (0..stack_bytes).step_by(8) {
            if execution.read_stack(0, offset) != Some(0) {
                fail("program_initial_stack");
            }
        }
        let mut zeroed_bytes = 0;
        for offset in (0..arena_bytes).step_by(8) {
            if execution.read_arena(0, offset) != Some(0) {
                fail("program_initial_arena");
            }
            zeroed_bytes += 8;
        }
        ptr::addr_of_mut!((*pointer).zeroed_bytes).write(zeroed_bytes);
        let bytes = descriptor.encode();
        let mut readback = [0; program_startup_size];
        for (word, bytes) in bytes.chunks_exact(8).enumerate() {
            let offset = descriptor_offset + word as u64 * 8;
            execution
                .write_data(0, offset, u64::from_le_bytes(bytes.try_into().unwrap()))
                .unwrap_or_else(|e| fail(e));
            readback[word * 8..word * 8 + 8].copy_from_slice(
                &execution
                    .read_data(0, offset)
                    .unwrap_or_else(|| fail("program_descriptor_copy"))
                    .to_le_bytes(),
            );
        }
        // Decoded from what was actually written, not from what was intended:
        // the task will decode these same bytes and nothing else.
        if program_descriptor::decode(&readback) != Ok(descriptor) {
            fail("program_descriptor_readback");
        }
        let frame = execution.frame(0).unwrap_or_else(|e| fail(e));
        if !execution.frame_owned(0, frame)
            || (*frame).flags != 2
            || (*frame).stack_pointer != user_stack_top
        {
            fail("program_initial_frame");
        }
        (*frame).destination_index = descriptor_address;
        (*frame).source_index = program_startup_size as u64;
        let identity = execution
            .commit(0, tasks, circles, 701 + scenario as u64)
            .unwrap_or_else(|e| fail(e));
        ptr::addr_of_mut!((*pointer).identity).write(identity);
        // Authority comes from the controller's retained launch selection,
        // never from a descriptor that userspace may later rewrite.
        let console = program_console::state::new(
            identity,
            (selected.console_handle != 0).then_some(selected.console_handle),
            selected.base,
            arena_bytes,
        )
        .unwrap_or_else(|_| fail("program_console_binding"));
        ptr::addr_of_mut!((*pointer).console).write(console);
        let edition = program_edition::state::new(
            identity,
            (selected.granted_services & service::file_read != 0)
                .then_some(selected.edition_handle),
            selected.edition_length,
            selected.base,
            arena_bytes,
        )
        .unwrap_or_else(|_| fail("program_edition_binding"));
        ptr::addr_of_mut!((*pointer).edition).write(edition);
        &mut *pointer
    }
}

#[inline(never)]
fn run_program(
    image: program_image,
    scenario: usize,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> (&'static str, program_record) {
    // Preparation returns before selection/publication or any guest transfer.
    let pointer = memory::with_frame_allocator(|allocator| {
        if super::is_active()
            || is_active()
            || interrupts::interrupts_enabled()
            || paging::read_cr3() != kernel_cr3
            || !tls_bases_are_zero()
            || allocator.accounting() != initial
        {
            fail("program_setup");
        }
        let state = initialize(allocator, image, scenario, kernel_cr3, bootstrap_stack);
        state
            .scheduler
            .admit(&mut state.circles, &mut state.tasks, state.identity)
            .unwrap_or_else(|_| fail("program_admit"));
        // SAFETY: The mapping owner remains at its final retained BSS location.
        unsafe { state.execution.activate(allocator) }.unwrap_or_else(|e| fail(e));
        let accounting = allocator.accounting();
        let selected = self::scenario(scenario);
        let source_leases = usize::from(selected.granted_services & service::file_read != 0);
        let source_frames = if source_leases != 0 {
            selected.edition_length.div_ceil(4096) as usize
        } else {
            0
        };
        if accounting.free.checked_add(program_frames + source_frames) != Some(initial.free)
            || accounting.data_owned != expected_data_frames + source_frames
            || accounting.table_owned != expected_table_frames
            || accounting.active_leases != expected_leases + source_leases
            || accounting.bound_leases != expected_leases + source_leases
            || state.zeroed_bytes != arena_bytes
        {
            fail("program_live_ownership");
        }
        state.live_accounting = Some(accounting);
        ptr::from_mut(state)
    })
    .unwrap_or_else(|e| fail(e.reason()));
    let frame = {
        // SAFETY: Stable initialized BSS, not yet published to handlers.
        let state = unsafe { &mut *pointer };
        // SAFETY: Stable sole-BSP CPU owner and unchanged native entry profile.
        unsafe { state.cpu.prepare() }.unwrap_or_else(|e| fail(e));
        let first = state
            .scheduler
            .start_next(&mut state.tasks)
            .unwrap_or_else(|_| fail("program_start"));
        if first.task() != state.identity || first.context() != scheduler_context::initial {
            fail("program_first");
        }
        // SAFETY: Serialized dispatch retains the root, stack and alias.
        unsafe { state.execution.select(0) }.unwrap_or_else(|reason| fail(reason));
        let frame = state
            .execution
            .frame(0)
            .unwrap_or_else(|reason| fail(reason));
        if !state.frame_valid(frame) {
            fail("program_selected_frame");
        }
        if active
            .compare_exchange(
                ptr::null_mut(),
                pointer,
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_err()
        {
            fail("program_publication");
        }
        frame
    };
    let entry_rsp: u64;
    // SAFETY: Read the current bootstrap continuation without touching memory.
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) entry_rsp, options(nomem, nostack, preserves_flags));
    }
    // No mutable program reference or allocator borrow crosses this transfer.
    // SAFETY: Serialized handlers alone borrow this stable owner until resume.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("program_initial_cpu");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    let resumed_rsp: u64;
    // SAFETY: Inspect the restored bootstrap continuation before reborrowing BSS.
    unsafe {
        core::arch::asm!("mov {}, rsp", out(reg) resumed_rsp, options(nomem, nostack, preserves_flags));
    }
    if entry_rsp != resumed_rsp
        || status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || interrupts::interrupts_enabled()
        // SAFETY: Initialized sole-BSP TSS, bootstrap dispatch withdrawn.
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
    {
        fail("program_resume_ownership");
    }
    // SAFETY: Terminal dispatch withdrew all handler access and restored the
    // bootstrap continuation before this fresh exclusive borrow.
    let state = unsafe { &mut *pointer };
    if paging::read_cr0() & 8 != state.original_ts
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || !state.terminal
        || !state.started
    {
        fail("program_continuation");
    }
    #[cfg(not(feature = "native_manifest_test"))]
    let stated = {
        let (attempts, rejected, writes, bytes) = if scenario == 0 {
            (16, 14, 2, 60)
        } else {
            (2, 2, 0, 0)
        };
        let (edition_attempts, edition_rejected, edition_reads, edition_bytes, source_reads) =
            if scenario == 0 {
                (21, 14, 7, 8203, 5)
            } else {
                (3, 3, 0, 0, 0)
            };
        if state.calls != attempts + edition_attempts + 2
            || state.exits != 1
            || state.invalid != 0
            || state.console_attempts != attempts
            || state.console_rejected != rejected
            || state.console.written() != writes
            || state.console.bytes_written() != bytes
            || !state.console.is_closed()
            || state.console_scratch.iter().any(|byte| *byte != 0)
            || state.edition_attempts != edition_attempts
            || state.edition_rejected != edition_rejected
            || state.edition.reads() != edition_reads
            || state.edition.bytes_read() != edition_bytes
            || state.edition_source_reads != source_reads
            || state.allocator_service_borrows != if scenario == 0 { 2 } else { 0 }
            || state.allocator_nested_denials != if scenario == 0 { 1 } else { 0 }
            || !state.edition.is_closed()
            || state.edition_scratch.iter().any(|byte| *byte != 0)
        {
            fail("program_evidence");
        }
        let stated = state.stated_grant();
        if stated != state.granted {
            fail("program_grant_mismatch");
        }
        stated
    };
    #[cfg(feature = "native_manifest_test")]
    let report = manifest::validate_report(state);
    #[cfg(feature = "native_manifest_test")]
    let stated = state.granted;
    if !state
        .tasks
        .snapshot(state.identity)
        .is_ok_and(|s| s.state() == task_state::exited)
    {
        fail("program_terminal");
    }
    memory::with_frame_allocator(|allocator| {
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, state.identity)
            .unwrap_or_else(|_| fail("program_withdraw"));
        // SAFETY: Bootstrap restored, the task is terminal and its context is
        // withdrawn; every mapping, arena and alias lease stays retained until
        // this explicit teardown.
        unsafe {
            state
                .execution
                .finish(allocator, &mut state.tasks, &mut state.circles)
        }
        .unwrap_or_else(|e| fail(e));
        if let Some(source) = state.source.as_mut() {
            // SAFETY: All consumers and their aliases retired above; bootstrap root
            // is exclusive. Retain the owner until source cleanup succeeds.
            unsafe { source.finish(allocator) }.unwrap_or_else(|_| fail("program_source_cleanup"));
            if !source.is_released() {
                fail("program_source_not_released");
            }
        }
        let snapshot = state
            .scheduler
            .snapshot(&state.tasks)
            .unwrap_or_else(|_| fail("program_final_scheduler"));
        if allocator.accounting() != initial
            || state.circles.accounting() != circle_table::new().accounting()
            || state.tasks.active_count() != 0
            || state.tasks.attached_context_count() != 0
            || snapshot.admitted() != 0
            || snapshot.runnable() != 0
            || snapshot.current().is_some()
            || state.tasks.validate(state.identity).is_ok()
        {
            fail("program_final_accounting");
        }
    })
    .unwrap_or_else(|e| fail(e.reason()));
    let record = program_record {
        calls: state.calls,
        timer_ticks: state.timer_ticks,
        invalid: state.invalid,
        exits: state.exits,
        zeroed_bytes: state.zeroed_bytes,
        granted: state.granted,
        stated,
        #[cfg(feature = "native_manifest_test")]
        report,
        console_attempts: state.console_attempts,
        console_rejected: state.console_rejected,
        console_writes: state.console.written(),
        console_bytes: state.console.bytes_written(),
        edition_attempts: state.edition_attempts,
        edition_rejected: state.edition_rejected,
        edition_reads: state.edition.reads(),
        edition_read_bytes: state.edition.bytes_read(),
        edition_source_reads: state.edition_source_reads,
        allocator_service_borrows: state.allocator_service_borrows,
        allocator_nested_denials: state.allocator_nested_denials,
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
fn enter_dispatch() -> &'static mut program {
    if dispatch_active.swap(true, Ordering::AcqRel) || interrupts::interrupts_enabled() {
        fail("program_reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() {
        fail("program_inactive");
    }
    // SAFETY: Sole BSP, IF clear, nonreentrant handler owns the retained program.
    unsafe { &mut *pointer }
}
fn leave_dispatch(state: &mut program, frame: *mut scheduler_frame) -> *mut scheduler_frame {
    // SAFETY: The selected eager image belongs to this stable exclusive owner.
    if !unsafe { state.cpu.restore_task(0) } {
        fail("program_cpu_restore");
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
    if !unsafe { state.cpu.save_current(0) }
        || !state.frame_valid(frame.cast())
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.timer_ticks >= max_calls
    {
        fail("program_timer");
    }
    state.last_tick = observed.tick();
    state.timer_ticks += 1;
    Some(leave_dispatch(state, frame.cast()).cast())
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    let state = enter_dispatch();
    // No fault is expected and none is armed. The vessel's panic handler ends
    // in `ud2`, so a fault here is a failed proof and is reported as one
    // rather than contained: there is nothing to contain it on behalf of.
    let _ = (state, frame, snapshot);
    fail("program_unexpected_fault")
}
// One exclusive current-task interval covers preflight, source read and copyout.
// Neither the source reader nor the physical alias escapes an operation.
struct edition_access<'a> {
    execution: &'a mut owned_execution_group<1>,
    source: Option<&'a linux_loader_source::source_controller>,
    tasks: &'a task_table,
    circles: &'a circle_table,
    scheduler: &'a scheduler_table,
    identity: task_identity,
    source_reads: &'a mut u64,
    live_accounting: frame_accounting,
    allocator_service_borrows: &'a mut u64,
    allocator_nested_denials: &'a mut u64,
}
impl program_edition::read_io for edition_access<'_> {
    fn preflight(&mut self, address: u64, length: usize) -> Result<(), ()> {
        self.execution
            .preflight_to_user(
                self.tasks,
                self.circles,
                self.scheduler,
                self.identity,
                address,
                length,
            )
            .map_err(|_| ())?;
        if *self.allocator_service_borrows == 0 {
            memory::with_frame_allocator(|allocator| {
                if allocator.accounting() != self.live_accounting {
                    fail("program_allocator_changed");
                }
                let mut nested_entered = false;
                let nested = memory::with_frame_allocator(|_| {
                    nested_entered = true;
                });
                if nested_entered || nested != Err(memory::memory_error::allocator_busy) {
                    fail("program_allocator_nested");
                }
                *self.allocator_nested_denials += 1;
            })
            .unwrap_or_else(|e| fail(e.reason()));
            *self.allocator_service_borrows += 1;
            memory::with_frame_allocator(|allocator| {
                if allocator.accounting() != self.live_accounting {
                    fail("program_allocator_changed");
                }
            })
            .unwrap_or_else(|e| fail(e.reason()));
            *self.allocator_service_borrows += 1;
        }
        Ok(())
    }
    fn read(&mut self, offset: u64, destination: &mut [u8]) -> Result<usize, ()> {
        let reader = self.source.ok_or(())?.reader().map_err(|_| ())?;
        *self.source_reads += 1;
        // SAFETY: Live gate authenticated the exact task/root/frame; IF is clear
        // and the retained immutable source outlives this scoped scratch read.
        unsafe { self.execution.read_source(&reader, offset, destination) }.map_err(|_| ())?;
        Ok(destination.len())
    }
    fn publish(&mut self, address: u64, bytes: &[u8]) -> Result<(), ()> {
        // Complete revalidation precedes the first store. Recoverable errors
        // change nothing; an unexpected hardware copy fault cannot return.
        self.execution
            .copy_to_user(
                self.tasks,
                self.circles,
                self.scheduler,
                self.identity,
                address,
                bytes,
            )
            .map_err(|_| ())
    }
}

pub(super) fn gate(frame: *mut scheduler_frame) -> *mut scheduler_frame {
    let state = enter_dispatch();
    // SAFETY: Stable session exclusively owns the current aligned CPU image.
    if !unsafe { state.cpu.save_current(0) } || !state.frame_valid(frame) {
        fail("program_gate_frame");
    }
    // SAFETY: Complete retained frame ownership validated above.
    let value = unsafe { ptr::read(frame) };
    if value.vector != gate_vector
        || value.error_code != 0
        || state.calls >= max_calls
        || (value.accumulator != 0 && !state.started)
    {
        fail("program_gate_vector");
    }
    state.calls += 1;
    let mut reply = (program_status::ok, 0, 0);
    match value.accumulator {
        0 => {
            if state.started
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || value.destination_index != descriptor_address
                || value.source_index != program_startup_size as u64
                || !state
                    .cpu
                    .task_image(0)
                    .is_some_and(|image| image.is_initial_clean())
            {
                fail("program_startup");
            }
            state.started = true;
            // SAFETY: The exact startup frame remains exclusively owned. RDI/RSI
            // continue to carry the descriptor span; they are never reply fields.
            unsafe {
                (*frame).flags = 0x202;
            }
        }
        1 => {
            state.console_attempts += 1;
            let request = program_console::write_request {
                caller: state.identity,
                handle: value.destination_index,
                address: value.source_index,
                length: value.data,
                sequence: value.counter,
            };
            let identity = state.identity;
            let execution = &mut state.execution;
            let tasks = &state.tasks;
            let circles = &state.circles;
            let scheduler = &state.scheduler;
            let result = state.console.write(
                request,
                &mut state.console_scratch,
                |address, destination| {
                    execution
                        .copy_from_user(tasks, circles, scheduler, identity, address, destination)
                        .map_err(|_| ())
                },
                |text| {
                    // The complete UTF-8 request has been copied and validated.
                    // The current serial sink commits synchronously; a failure
                    // must never return and allow a duplicate retry.
                    let mut serial = serial_port::com1();
                    serial
                        .write_str(text)
                        .unwrap_or_else(|_| fail("program_console_publish"));
                },
            );
            if result.status != program_status::ok {
                state.console_rejected += 1;
            }
            if state.console_scratch.iter().any(|byte| *byte != 0) {
                fail("program_console_scratch");
            }
            reply = (result.status, result.sequence, result.length);
        }
        2 => {
            state.edition_attempts += 1;
            let request = program_edition::read_request {
                caller: state.identity,
                handle: value.destination_index,
                address: value.source_index,
                length: value.data,
                sequence: value.counter,
                offset: value.register_8,
            };
            let mut access = edition_access {
                execution: &mut state.execution,
                source: state.source.as_ref(),
                tasks: &state.tasks,
                circles: &state.circles,
                scheduler: &state.scheduler,
                identity: state.identity,
                source_reads: &mut state.edition_source_reads,
                live_accounting: state
                    .live_accounting
                    .unwrap_or_else(|| fail("program_accounting_missing")),
                allocator_service_borrows: &mut state.allocator_service_borrows,
                allocator_nested_denials: &mut state.allocator_nested_denials,
            };
            let result = state
                .edition
                .read(request, &mut state.edition_scratch, &mut access);
            // scheduler_frame is repr(C), all 22 words are initialized u64s,
            // and the validated frame stays exclusively retained in this gate.
            const _: () = assert!(core::mem::size_of::<scheduler_frame>() == 22 * 8);
            // SAFETY: The exact authenticated saved frame and its initialized
            // entry snapshot contain no padding; neither slice escapes.
            let unchanged = unsafe {
                core::slice::from_raw_parts(frame.cast::<u8>(), 22 * 8)
                    == core::slice::from_raw_parts(ptr::from_ref(&value).cast::<u8>(), 22 * 8)
            };
            if !unchanged {
                fail("program_edition_frame_changed");
            }

            if result.status != program_status::ok {
                state.edition_rejected += 1;
            }
            if state.edition_scratch.iter().any(|byte| *byte != 0) {
                fail("program_edition_scratch");
            }
            reply = (result.status, result.sequence, result.length);
        }
        5 => {
            if value.destination_index != 0 {
                fail("program_user_status");
            }
            state.console.close();
            state.edition.close();
            state.terminal = true;
            state.exits += 1;
            state
                .scheduler
                .complete_current(&mut state.tasks, state.identity)
                .unwrap_or_else(|_| fail("program_complete"));
            state.finish();
        }
        // This experimental program profile exposes only startup, console
        // output, bounded edition reads, and exit. Other operations carry no implicit service grant.
        _ => {
            reply = (program_status::invalid, 0, 0);
            state.invalid += 1;
        }
    }
    // SAFETY: The current exact frame remains exclusively retained until IRETQ.
    unsafe {
        (*frame).accumulator = reply.0;
        (*frame).counter = reply.1;
        (*frame).data = reply.2;
    }
    leave_dispatch(state, frame)
}

pub(super) fn run() -> ! {
    if super::is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("program_initial_state");
    }
    let image = program_image::inspect();
    // Withdrawn for the same reason the native runtime profile withdraws it:
    // the interrupt failure surface paints through the bootstrap root, and a
    // user root is selected for the whole of this proof. Nothing presents
    // here, so the aperture is consumed and never handed on. Required rather
    // than skipped when absent, so a missing aperture is reported as one.
    // SAFETY: Sole bootstrap owner consumes the unique framebuffer aperture.
    let _aperture = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("program_framebuffer_unavailable"));
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Initialized sole-BSP descriptors remain exclusively owned.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let initial = memory::with_frame_allocator(|allocator| allocator.accounting())
        .unwrap_or_else(|e| fail(e.reason()));
    #[cfg(feature = "native_manifest_test")]
    manifest::run(image, kernel_cr3, bootstrap_stack, initial);
    #[cfg(not(feature = "native_manifest_test"))]
    run_delivery(image, kernel_cr3, bootstrap_stack, initial)
}
#[cfg(not(feature = "native_manifest_test"))]
fn run_delivery(
    image: program_image,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    initial: frame_accounting,
) -> ! {
    let mut records: [Option<program_record>; scenarios_count] = [None; scenarios_count];
    let mut vendor = "";
    for (scenario, slot) in records.iter_mut().enumerate() {
        let (current_vendor, record) =
            run_program(image, scenario, kernel_cr3, bootstrap_stack, initial);
        if scenario == 0 {
            vendor = current_vendor;
        } else if current_vendor != vendor {
            fail("program_cpu_comparison");
        }
        *slot = Some(record);
    }
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("program_final_interrupts");
    }
    let records = records.map(|record| record.unwrap_or_else(|| fail("program_record_missing")));
    emit_program(vendor, initial, records);
    super::super::qemu_exit_success();
    super::super::halt_forever()
}
#[cfg(not(feature = "native_manifest_test"))]
fn emit_program(
    vendor: &str,
    initial: frame_accounting,
    records: [program_record; scenarios_count],
) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_program_test.ready","level":"info","vendor":"{}","tasks":1,"scenarios":{},"code_pages":{},"descriptor_bytes":{},"arena_bytes":{},"mappings":4,"data_frames":{},"table_frames":{},"leases":{},"free_frames":{},"edition_mapped":false,"console_transport":true,"edition_transport":true,"source_frames":3,"source_leases":1}}"#,
        vendor,
        scenarios.len(),
        program_code_pages,
        program_startup_size,
        arena_bytes,
        expected_data_frames,
        expected_table_frames,
        expected_leases,
        initial.free,
    );
    let mut zeroed_bytes = 0;
    for (scenario, record) in records.into_iter().enumerate() {
        zeroed_bytes += record.zeroed_bytes;
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_program_test.scenario","level":"info","scenario":{},"arena_base":{},"zeroed_bytes":{},"calls":{},"timer_ticks":{},"invalid":{},"exits":{},"granted_services":{},"console_granted":{},"console_transport":{},"argument_bytes":{},"edition_bytes":{},"grant_restated":{},"console_attempts":{},"console_writes":{},"console_rejected":{},"console_bytes":{},"console_last_sequence":{},"edition_granted":{},"edition_attempts":{},"edition_reads":{},"edition_rejected":{},"edition_read_bytes":{},"edition_last_sequence":{},"edition_source_reads":{},"allocator_service_borrows":{},"allocator_nested_denials":{}}}"#,
            scenario,
            scenarios[scenario].base,
            record.zeroed_bytes,
            record.calls,
            record.timer_ticks,
            record.invalid,
            record.exits,
            record.granted[2],
            record.granted[4],
            record.granted[4] == 1,
            record.granted[3],
            record.granted[1],
            record.stated == record.granted,
            record.console_attempts,
            record.console_writes,
            record.console_rejected,
            record.console_bytes,
            record.console_writes,
            u64::from(record.granted[2] & service::file_read != 0),
            record.edition_attempts,
            record.edition_reads,
            record.edition_rejected,
            record.edition_read_bytes,
            record.edition_reads,
            record.edition_source_reads,
            record.allocator_service_borrows,
            record.allocator_nested_denials,
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_program_test.passed","level":"info","program_tasks":{},"descriptor_bytes":{},"arena_bytes":{},"zeroed_bytes":{},"grants_restated":true,"editions_executed":0,"console_writes":2,"console_rejected":16,"console_bytes":60,"console_verified":true,"edition_reads":7,"edition_rejected":17,"edition_read_bytes":8203,"edition_verified":true,"edition_frame_preserved":true,"source_released":true,"allocator_available":true,"resources_restored":true,"authority_revoked":true,"source_language":"rust"}}"#,
        scenarios.len(),
        program_startup_size,
        arena_bytes,
        zeroed_bytes
    );
}
