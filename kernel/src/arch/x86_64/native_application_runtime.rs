//! Experimental native application/service boundary over retained task owners.
//! Two copied channels carry opaque packets. Desktop and editor semantics stay
//! in the independently executing userspace client and service. Presentation is
//! deliberately deferred until the bootstrap root is restored.

// A program-bearing launch replaces this module's profile wholesale: its own
// two-task runtime, its display stage and its framebuffer baseline are not the
// proof that runs, while its gate, timer and fault delegation still are. That
// leaves this module's unreachable half genuinely unused rather than wrongly
// gated, and saying so once here is better than gating twenty items in a file
// three other profiles share. The program profile re-denies both lints for
// itself, so nothing is hidden in the code that does run.
#![cfg_attr(feature = "native_program_test", allow(dead_code, unused_imports))]

use core::arch::asm;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use super::application_cpu::{x86_64_application_enter, x86_64_application_resume_kernel};
use super::owned_execution::{owned_execution, tls_bases_are_zero, user_code_base, user_stack_top};
use super::scheduler_test::{scheduler_frame, user_return_fields_are_valid_within_stack};
use super::{application_cpu, application_fault, descriptors, exceptions, interrupts, paging};
use crate::application_channel::{
    application_channel, application_channel_binding, application_channel_error as channel_error,
    application_channel_phase as channel_phase, application_packet_capacity,
};
use crate::boot_omen::framebuffer_info;
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::display::framebuffer_presenter;
use crate::identity::{endpoint_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_accounting, frame_allocator};
use crate::scheduler::{scheduler_context, scheduler_table};
use crate::task::{task_state, task_table};
use crate::time::monotonic_instant;

const gate_vector: u64 = 0xf4;
const completion_status: u64 = 0x4150;
const stage_width: usize = 384;
const stage_height: usize = 24;
const stage_row_bytes: usize = stage_width * 3;
const stage_size: usize = stage_row_bytes * stage_height;
const code_pages: u64 = 8;
const owned_frames: usize = 50;
const max_calls: u64 = 10_000;

struct display_storage(UnsafeCell<[u8; stage_size]>);
// SAFETY: Only the sole BSP's active runtime writes under its IF-clear dispatch
// latch. Bootstrap reads after active is withdrawn, then clears before reentry.
unsafe impl Sync for display_storage {}
static display_stage: display_storage = display_storage(UnsafeCell::new([0; stage_size]));
static active: AtomicPtr<runtime> = AtomicPtr::new(ptr::null_mut());
static dispatch_active: AtomicBool = AtomicBool::new(false);

// The ELF comparison repeats five owned sessions. Keep the large suspended
// runtime in explicitly reserved supervisor BSS instead of consuming bootstrap
// stack headroom with a second set of aggregate move temporaries.
#[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
struct retained_runtime_storage(UnsafeCell<core::mem::MaybeUninit<runtime>>);
#[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
// SAFETY: Sole BSP, IF clear; storage_busy covers construction through complete
// teardown. Only the published active pointer lends it to serialized handlers.
unsafe impl Sync for retained_runtime_storage {}
#[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
static retained_runtime: retained_runtime_storage =
    retained_runtime_storage(UnsafeCell::new(core::mem::MaybeUninit::uninit()));
#[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
static storage_busy: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn x86_64_native_application_gate_entry();
    static x86_64_native_application_payload_start: u8;
    static x86_64_native_application_payload_end: u8;
    static x86_64_native_application_user_fault_instruction: u8;
    static kernel_rodata_start: u8;
    static kernel_rodata_end: u8;
}
macro_rules! address {
    ($symbol:ident) => {{
        let result: u64;
        // SAFETY: Materialize linker metadata without dereferencing a symbol.
        unsafe { asm!("lea {out}, [rip + {symbol}]", out = out(reg) result,
            symbol = sym $symbol, options(nomem, nostack, preserves_flags)); }
        result
    }};
}
#[cfg(feature = "elf_application_test")]
#[path = "elf_application_profile.rs"]
mod elf_profile;
#[cfg(all(
    feature = "linux_application_test",
    not(feature = "veil_application_test")
))]
#[path = "linux_application_calls.rs"]
mod linux_calls;
#[cfg(feature = "linux_application_test")]
#[path = "linux_application_profile.rs"]
mod linux_profile;

#[cfg(feature = "veil_application_test")]
#[cfg_attr(
    all(feature = "shared_veil_test", not(feature = "linux_tls_test")),
    path = "shared_veil_runtime.rs"
)]
#[cfg_attr(
    any(feature = "linux_tls_test", not(feature = "shared_veil_test")),
    path = "veil_application_runtime.rs"
)]
mod veil_profile;

#[cfg(all(
    feature = "native_runtime_test",
    any(
        not(feature = "elf_application_test"),
        all(
            any(feature = "elf_document_test", feature = "native_relaunch_test"),
            not(feature = "linux_application_test")
        )
    )
))]
#[cfg_attr(
    all(
        feature = "native_durable_failure_test",
        not(any(feature = "native_relaunch_test", feature = "elf_document_test"))
    ),
    path = "native_durable_failure_profile.rs"
)]
#[cfg_attr(
    all(
        feature = "native_interactive_editor_test",
        not(feature = "native_durable_failure_test"),
        not(any(feature = "native_relaunch_test", feature = "elf_document_test"))
    ),
    path = "native_interactive_profile.rs"
)]
#[cfg_attr(
    all(
        feature = "native_durable_test",
        not(feature = "native_durable_failure_test"),
        not(feature = "native_interactive_editor_test"),
        not(any(feature = "native_relaunch_test", feature = "elf_document_test"))
    ),
    path = "native_durable_profile.rs"
)]
#[cfg_attr(feature = "native_relaunch_test", path = "native_relaunch_profile.rs")]
#[cfg_attr(
    all(
        not(feature = "native_relaunch_test"),
        any(not(feature = "native_durable_test"), feature = "elf_document_test"),
        feature = "native_document_test"
    ),
    path = "native_document_profile.rs"
)]
// A program-bearing launch replaces the whole profile rather than widening
// this one. It is a single task with its own geometry, so ADR 0094 attaches it
// at the one place a profile is chosen and every delegation below is unchanged.
#[cfg_attr(feature = "native_program_test", path = "native_program_profile.rs")]
#[cfg_attr(
    not(any(
        feature = "native_document_test",
        feature = "native_relaunch_test",
        feature = "native_program_test"
    )),
    path = "native_runtime_profile.rs"
)]
mod native_runtime_profile;

#[derive(Clone, Copy)]
struct artifact {
    source: u64,
    length: u64,
    fault: u64,
    #[cfg(feature = "elf_application_test")]
    data: (u64, u64),
    #[cfg(feature = "elf_application_test")]
    loader: bool,
    #[cfg(feature = "linux_application_test")]
    linux: bool,
}
impl artifact {
    #[cfg_attr(
        all(
            feature = "native_interactive_editor_test",
            not(any(
                feature = "native_durable_failure_test",
                feature = "native_relaunch_test",
                feature = "elf_application_test"
            ))
        ),
        allow(dead_code)
    )]
    fn inspect() -> Self {
        let source = address!(x86_64_native_application_payload_start);
        let length = address!(x86_64_native_application_payload_end)
            .checked_sub(source)
            .unwrap_or_else(|| fail("payload_extent"));
        let fault = address!(x86_64_native_application_user_fault_instruction);
        if source < address!(kernel_rodata_start)
            || length == 0
            || length > code_pages * 4096
            || source
                .checked_add(length)
                .is_none_or(|end| end > address!(kernel_rodata_end))
            || fault < user_code_base
            || fault
                .checked_add(2)
                .is_none_or(|end| end > user_code_base + length)
        {
            fail("artifact_geometry");
        }
        // SAFETY: Complete two-byte marker lies in inspected immutable rodata.
        let opcode =
            unsafe { ptr::read_unaligned((source + fault - user_code_base) as *const [u8; 2]) };
        if opcode != [0x0f, 0x0b] {
            fail("artifact_fault_opcode");
        }
        Self {
            source,
            length,
            fault,
            #[cfg(feature = "elf_application_test")]
            data: (0, 0),
            #[cfg(feature = "elf_application_test")]
            loader: false,
            #[cfg(feature = "linux_application_test")]
            linux: false,
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct evidence {
    calls: u64,
    #[cfg(feature = "linux_application_test")]
    linux_calls: [u64; 6],
    switches: u64,
    timer_ticks: u64,
    sends: [u64; 2],
    deliveries: [u64; 2],
    acknowledgements: [u64; 2],
    denials: u64,
    bad_copies: u64,
    invalid: u64,
    closed: u64,
    discarded: u64,
    exits: u64,
    faults: u64,
    rows: u64,
    surface_hashes: [u64; 7],
    #[cfg(feature = "elf_application_test")]
    accepted_plan: Option<anomalous_elf_adapter::load_plan>,
}
struct runtime {
    circles: circle_table,
    tasks: task_table,
    scheduler: scheduler_table,
    identities: [task_identity; 2],
    execution: owned_execution,
    cpu: application_cpu::cpu_session,
    channels: [application_channel; 2],
    last_ack: [u64; 2],
    image: artifact,
    scenario: usize,
    selected: usize,
    started: [bool; 2],
    terminal: [bool; 2],
    failure_armed: bool,
    evidence: evidence,
    last_tick: u64,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    kernel_stack: u64,
    original_ts: u64,
}

pub(super) fn install_gate() -> bool {
    // SAFETY: Exclusive one-shot exception initialization.
    unsafe {
        super::idt::install_gate(
            gate_vector as usize,
            x86_64_native_application_gate_entry as *const () as u64,
            descriptors::kernel_code_selector,
            0,
            super::idt::user_interrupt_gate_attributes,
        )
    }
}

#[cfg(all(
    feature = "elf_application_test",
    not(feature = "linux_application_test"),
    not(any(feature = "elf_document_test", feature = "native_relaunch_test"))
))]
pub(super) fn run() -> ! {
    elf_profile::run()
}

#[cfg(all(
    feature = "linux_application_test",
    not(feature = "veil_application_test")
))]
pub(super) fn run() {
    linux_profile::run()
}

#[cfg(feature = "veil_application_test")]
pub(super) fn run() {
    veil_profile::run()
}

#[cfg(all(
    feature = "native_runtime_test",
    any(
        not(feature = "elf_application_test"),
        all(
            any(feature = "elf_document_test", feature = "native_relaunch_test"),
            not(feature = "linux_application_test")
        )
    )
))]
pub(super) fn run() -> ! {
    native_runtime_profile::run()
}

#[cfg(not(any(feature = "elf_application_test", feature = "native_runtime_test")))]
pub(super) fn run() -> ! {
    if is_active()
        || dispatch_active.load(Ordering::Acquire)
        || !interrupts::restrict_to_scheduler_timer()
    {
        fail("initial_state");
    }
    let image = artifact::inspect();
    // SAFETY: Sole BSP consumes the registered aperture before creating its only
    // presenter. That presenter is never used while a user root is selected.
    let info = unsafe { crate::console::withdraw_interrupt_failure_surface() }
        .unwrap_or_else(|| fail("framebuffer_unavailable"));
    // SAFETY: The consumed console registration grants this sole live aperture.
    let mut presenter = unsafe { framebuffer_presenter::try_new(info) }
        .unwrap_or_else(|_| fail("framebuffer_geometry"));
    if presenter.width() != 1024 || presenter.height() != 768 {
        fail("display_profile");
    }
    let kernel_cr3 = paging::read_cr3();
    // SAFETY: Descriptor initialization is complete; sole BSP reads its TSS.
    let bootstrap_stack = unsafe { descriptors::selected_privilege_stack_top() };
    let initial_outside = framebuffer_hash(info, true);
    let (initial, vendor, evidence) = memory::with_frame_allocator(|allocator| {
        let initial = allocator.accounting();
        let (vendor, normal) = run_owned(allocator, image, kernel_cr3, bootstrap_stack, 0, initial);
        if normal.rows != stage_height as u64 || framebuffer_hash(info, true) != initial_outside {
            fail("presentation_preflight");
        }
        // SAFETY: Both tasks retired and bootstrap resumed; sole owner retains
        // immutable copied service rows until the verified framebuffer commit.
        let pixels = unsafe { &*display_stage.0.get() };
        let patch = presenter
            .prepare_rgb8_region(32, 32, stage_width, stage_height, pixels)
            .unwrap_or_else(|_| fail("display_patch"));
        if !patch.commit_and_verify() || framebuffer_hash(info, true) != initial_outside {
            fail("framebuffer_readback");
        }
        clear_stage();
        let presented = framebuffer_hash(info, false);
        let (fault_vendor, faulted) =
            run_owned(allocator, image, kernel_cr3, bootstrap_stack, 1, initial);
        if fault_vendor != vendor || framebuffer_hash(info, false) != presented {
            fail("fault_changed_presentation");
        }
        clear_stage();
        (initial, vendor, [normal, faulted])
    })
    .unwrap_or_else(|error| fail(error.reason()));
    interrupts::disable_external_interrupts();
    if !interrupts::scheduler_interrupts_are_masked() {
        fail("final_interrupt_state");
    }
    emit_evidence(initial, vendor, evidence);
    super::qemu_exit_success();
    super::halt_forever()
}

#[cfg(not(feature = "veil_application_test"))]
#[cfg_attr(
    all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ),
    allow(dead_code)
)]
fn run_owned(
    allocator: &mut frame_allocator<'_>,
    image: artifact,
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
) -> (&'static str, evidence) {
    run_pair(
        allocator,
        [image; 2],
        kernel_cr3,
        bootstrap_stack,
        scenario,
        initial,
    )
}

/// Construct directly into retained storage before acquiring mapping ownership.
/// Keeping this outlined prevents inactive construction temporaries from sharing
/// the suspended dispatcher frame. Failures retain every initialized field and
/// keep the storage latch held; no cleanup is delegated to Drop or replacement.
#[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
#[inline(never)]
fn initialize_pair(
    allocator: &mut frame_allocator<'_>,
    images: [artifact; 2],
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
) -> &'static mut runtime {
    if storage_busy.swap(true, Ordering::AcqRel) {
        fail("elf_storage_busy");
    }
    let pointer = retained_runtime.0.get().cast::<runtime>();
    let image = images[0];
    // SAFETY: Exclusive uninitialized or fully retired storage. Each field is
    // initialized separately, and no whole runtime reference exists before the
    // identities and both channels are valid. Owners never move after prepare.
    unsafe {
        ptr::addr_of_mut!((*pointer).circles).write(circle_table::new());
        ptr::addr_of_mut!((*pointer).tasks).write(task_table::new());
        ptr::addr_of_mut!((*pointer).scheduler)
            .write(scheduler_table::new(2).unwrap_or_else(|_| fail("scheduler_create")));
        ptr::addr_of_mut!((*pointer).execution).write(owned_execution::new());
        ptr::addr_of_mut!((*pointer).cpu).write(application_cpu::cpu_session::new());
        ptr::addr_of_mut!((*pointer).last_ack).write([0; 2]);
        ptr::addr_of_mut!((*pointer).image).write(image);
        ptr::addr_of_mut!((*pointer).scenario).write(scenario);
        ptr::addr_of_mut!((*pointer).selected).write(0);
        ptr::addr_of_mut!((*pointer).started).write([false; 2]);
        ptr::addr_of_mut!((*pointer).terminal).write([false; 2]);
        ptr::addr_of_mut!((*pointer).failure_armed).write(false);
        ptr::addr_of_mut!((*pointer).evidence).write(evidence::default());
        ptr::addr_of_mut!((*pointer).last_tick).write(0);
        ptr::addr_of_mut!((*pointer).kernel_cr3).write(kernel_cr3);
        ptr::addr_of_mut!((*pointer).bootstrap_stack).write(bootstrap_stack);
        ptr::addr_of_mut!((*pointer).kernel_stack).write(0);
        ptr::addr_of_mut!((*pointer).original_ts).write(paging::read_cr0() & 8);
        let execution = &mut *ptr::addr_of_mut!((*pointer).execution);
        let circles = &mut *ptr::addr_of_mut!((*pointer).circles);
        let tasks = &mut *ptr::addr_of_mut!((*pointer).tasks);
        let scheduler = &mut *ptr::addr_of_mut!((*pointer).scheduler);
        #[cfg(feature = "elf_application_test")]
        let data = images.map(|i| i.data);
        #[cfg(not(feature = "elf_application_test"))]
        let data = [(0, 0); 2];
        #[cfg(not(feature = "linux_application_test"))]
        execution
            .prepare_segments(
                allocator,
                circles,
                images.map(|i| (i.source, i.length)),
                data,
                code_pages,
            )
            .unwrap_or_else(|reason| fail(reason));
        #[cfg(feature = "linux_application_test")]
        execution
            .prepare_segments_with_initial_stack(
                allocator,
                circles,
                images.map(|i| (i.source, i.length)),
                data,
                code_pages,
                images.map(|i| {
                    if i.linux && !i.loader {
                        user_stack_top - 256
                    } else {
                        user_stack_top
                    }
                }),
            )
            .unwrap_or_else(|reason| fail(reason));
        for (index, _image) in images.into_iter().enumerate() {
            #[cfg(feature = "elf_application_test")]
            let image = _image;
            for offset in (0..8192).step_by(8) {
                #[cfg(feature = "elf_application_test")]
                let expected = elf_profile::data_word(image, offset, 0);
                #[cfg(not(feature = "elf_application_test"))]
                let expected = 0;
                if execution.read_data(index, offset) != Some(expected)
                    || execution.read_stack(index, offset) != Some(0)
                {
                    fail("initial_storage");
                }
            }
            #[cfg(feature = "linux_application_test")]
            if image.linux && !image.loader {
                linux_profile::populate_stack(execution, index, scenario);
                continue;
            }
            execution
                .write_data(index, 0, index as u64)
                .unwrap_or_else(|reason| fail(reason));
            execution
                .write_data(index, 8, scenario as u64)
                .unwrap_or_else(|reason| fail(reason));
        }
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, tasks, circles, 401 + index as u64)
                .unwrap_or_else(|reason| fail(reason))
        });
        ptr::addr_of_mut!((*pointer).identities).write(identities);
        for id in identities {
            scheduler
                .admit(circles, tasks, id)
                .unwrap_or_else(|_| fail("admit"));
        }
        for index in 0..2 {
            let endpoint = endpoint_id::try_from_parts(index, scenario as u64 + 1)
                .unwrap_or_else(|| fail("endpoint_identity"));
            let binding = application_channel_binding::try_new(
                endpoint,
                identities[index],
                identities[1 - index],
            )
            .unwrap_or_else(|_| fail("channel_binding"));
            let channel = ptr::addr_of_mut!((*pointer).channels)
                .cast::<application_channel>()
                .add(index);
            channel.write(
                application_channel::try_new(binding).unwrap_or_else(|_| fail("channel_create")),
            );
            #[cfg(feature = "elf_application_test")]
            if image.loader {
                (*channel)
                    .close(binding)
                    .unwrap_or_else(|_| fail("loader_channel_close"));
            }
        }
        execution
            .activate(allocator)
            .unwrap_or_else(|reason| fail(reason));
        &mut *pointer
    }
}

fn run_pair(
    allocator: &mut frame_allocator<'_>,
    images: [artifact; 2],
    kernel_cr3: u64,
    bootstrap_stack: u64,
    scenario: usize,
    initial: frame_accounting,
) -> (&'static str, evidence) {
    #[cfg(any(feature = "elf_application_test", not(feature = "native_runtime_test")))]
    let image = images[0];
    if interrupts::interrupts_enabled()
        || !paging::protections_are_enabled()
        || !tls_bases_are_zero()
        || allocator.accounting() != initial
        || initial.free < owned_frames
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
        || !stage_is_clear()
    {
        fail("setup_state");
    }
    #[cfg(not(any(feature = "elf_application_test", feature = "native_runtime_test")))]
    let mut state = {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut execution = owned_execution::new();
        // SAFETY: The aggregate retains every acquired lease through bootstrap cleanup.
        unsafe {
            execution.prepare_with_code_pages(
                allocator,
                &mut circles,
                images.map(|i| (i.source, i.length)),
                code_pages,
            )
        }
        .unwrap_or_else(|reason| fail(reason));
        for index in 0..2 {
            for offset in (0..8192).step_by(8) {
                if execution.read_data(index, offset) != Some(0)
                    || execution.read_stack(index, offset) != Some(0)
                {
                    fail("initial_storage");
                }
            }
            execution
                .write_data(index, 0, index as u64)
                .unwrap_or_else(|reason| fail(reason));
            execution
                .write_data(index, 8, scenario as u64)
                .unwrap_or_else(|reason| fail(reason));
        }
        let identities = core::array::from_fn(|index| {
            execution
                .commit(index, &mut tasks, &mut circles, 401 + index as u64)
                .unwrap_or_else(|reason| fail(reason))
        });
        let mut scheduler = scheduler_table::new(2).unwrap_or_else(|_| fail("scheduler_create"));
        for id in identities {
            scheduler
                .admit(&mut circles, &mut tasks, id)
                .unwrap_or_else(|_| fail("admit"));
        }
        let channels = core::array::from_fn(|index| {
            let endpoint = endpoint_id::try_from_parts(index, scenario as u64 + 1)
                .unwrap_or_else(|| fail("endpoint_identity"));
            let binding = application_channel_binding::try_new(
                endpoint,
                identities[index],
                identities[1 - index],
            )
            .unwrap_or_else(|_| fail("channel_binding"));
            application_channel::try_new(binding).unwrap_or_else(|_| fail("channel_create"))
        });
        // SAFETY: IF/NMI remain excluded and all prepared leases stay owned.
        unsafe { execution.activate(allocator) }.unwrap_or_else(|reason| fail(reason));
        let accounting = allocator.accounting();
        if accounting.free.checked_add(owned_frames) != Some(initial.free)
            || accounting.data_owned != 32
            || accounting.table_owned != 18
            || accounting.active_leases != 10
            || accounting.bound_leases != 10
            || execution.root(0).ok() == execution.root(1).ok()
            || execution.transition_stack_top(0).ok() == execution.transition_stack_top(1).ok()
        {
            fail("live_ownership");
        }
        runtime {
            circles,
            tasks,
            scheduler,
            identities,
            execution,
            cpu: application_cpu::cpu_session::new(),
            channels,
            last_ack: [0; 2],
            image,
            scenario,
            selected: 0,
            started: [false; 2],
            terminal: [false; 2],
            failure_armed: false,
            evidence: evidence::default(),
            last_tick: 0,
            kernel_cr3,
            bootstrap_stack,
            kernel_stack: 0,
            original_ts: paging::read_cr0() & 8,
        }
    };
    #[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
    let state = initialize_pair(allocator, images, kernel_cr3, bootstrap_stack, scenario);
    let identities = state.identities;
    #[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
    {
        let accounting = allocator.accounting();
        if accounting.free.checked_add(owned_frames) != Some(initial.free)
            || accounting.data_owned != 32
            || accounting.table_owned != 18
            || accounting.active_leases != 10
            || accounting.bound_leases != 10
            || state.execution.root(0).ok() == state.execution.root(1).ok()
            || state.execution.transition_stack_top(0).ok()
                == state.execution.transition_stack_top(1).ok()
        {
            fail("live_ownership");
        }
    }
    // SAFETY: CPU images stay at their final stable retained address.
    unsafe {
        #[cfg(feature = "linux_application_test")]
        if image.linux && !image.loader {
            state.cpu.prepare_linux()
        } else {
            state.cpu.prepare()
        }
        #[cfg(not(feature = "linux_application_test"))]
        state.cpu.prepare()
    }
    .unwrap_or_else(|reason| fail(reason));
    #[cfg(feature = "linux_application_test")]
    if image.linux && !image.loader {
        if !state
            .cpu
            .task_image(0)
            .is_some_and(|i| i.is_initial_clean())
        {
            fail("linux_initial_cpu");
        }
        state.started[0] = true;
    }
    let first = state
        .scheduler
        .start_next(&mut state.tasks)
        .unwrap_or_else(|_| fail("start"));
    if first.task() != identities[0] || first.context() != scheduler_context::initial {
        fail("first_dispatch");
    }
    let frame = state.select(0);
    #[cfg(not(any(feature = "elf_application_test", feature = "native_runtime_test")))]
    let pointer = ptr::from_mut(&mut state);
    #[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
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
        fail("publication");
    }
    // SAFETY: Only handlers borrow the runtime while this continuation is suspended.
    let status = unsafe {
        if !(*pointer).cpu.restore_task(0) {
            fail("initial_cpu_restore");
        }
        x86_64_application_enter(frame, ptr::addr_of_mut!((*pointer).kernel_stack))
    };
    // SAFETY: The saved continuation resumed on the sole BSP; TSS remains live.
    if status != completion_status
        || is_active()
        || dispatch_active.load(Ordering::Acquire)
        || paging::read_cr3() != kernel_cr3
        || paging::read_cr0() & 8 != state.original_ts
        || unsafe { descriptors::selected_privilege_stack_top() } != bootstrap_stack
        || interrupts::interrupts_enabled()
        || !tls_bases_are_zero()
        || !state.cpu.storage_is_cleared()
        || state.terminal != [true; 2]
        || state.started != [true; 2]
        || state
            .channels
            .iter()
            .any(|c| c.phase() != channel_phase::closed || !c.storage_is_clear())
    {
        fail("continuation_restore");
    }
    #[cfg(feature = "linux_application_test")]
    if image.linux
        && !image.loader
        && (!state.cpu.linux_entry_is_restored() || !state.cpu.linux_entry_storage_is_cleared())
    {
        fail("linux_msr_restore");
    }
    let is_loader = false;
    #[cfg(feature = "elf_application_test")]
    let is_loader = image.loader || is_loader;
    #[cfg(feature = "elf_application_test")]
    if is_loader {
        elf_profile::verify_loader(image, state.evidence);
    }
    if !is_loader && scenario == 0 {
        if state.evidence.sends != [7, 6]
            || state.evidence.deliveries != [7, 6]
            || state.evidence.acknowledgements != [7, 6]
            || state.evidence.exits != 2
            || state.evidence.faults != 0
            || state.evidence.discarded != 0
            || state.evidence.rows != stage_height as u64
        {
            fail("normal_evidence");
        }
    } else if !is_loader
        && (state.evidence.sends != [1, 0]
            || state.evidence.deliveries != [0, 0]
            || state.evidence.faults != 1
            || state.evidence.exits != 1
            || state.evidence.discarded != 1
            || state.evidence.closed == 0
            || state.evidence.rows != 0)
    {
        fail("revocation_evidence");
    }
    if !is_loader && (state.evidence.denials < 2 || state.evidence.bad_copies == 0) {
        fail("rejection_evidence");
    }
    for (index, identity) in identities.into_iter().enumerate() {
        let expected = if !is_loader && scenario == 1 && index == 0 {
            task_state::faulted
        } else {
            task_state::exited
        };
        if !state
            .tasks
            .snapshot(identity)
            .is_ok_and(|s| s.state() == expected)
        {
            fail("terminal_state");
        }
        state
            .scheduler
            .withdraw_terminal_context(&mut state.circles, &mut state.tasks, identity)
            .unwrap_or_else(|_| fail("context_withdrawal"));
    }
    // SAFETY: Bootstrap is restored, every task terminal and context withdrawn;
    // the aggregate retains the alias and all leases until explicit release.
    unsafe {
        state
            .execution
            .finish(allocator, &mut state.tasks, &mut state.circles)
    }
    .unwrap_or_else(|reason| fail(reason));
    let snapshot = state
        .scheduler
        .snapshot(&state.tasks)
        .unwrap_or_else(|_| fail("final_scheduler"));
    if allocator.accounting() != initial
        || state.circles.accounting() != circle_table::new().accounting()
        || state.tasks.active_count() != 0
        || state.tasks.attached_context_count() != 0
        || snapshot.admitted() != 0
        || snapshot.runnable() != 0
        || snapshot.current().is_some()
        || identities
            .iter()
            .any(|id| state.tasks.validate(*id).is_ok())
    {
        fail("final_accounting");
    }
    let result = (state.cpu.cpu_vendor(), state.evidence);
    #[cfg(any(feature = "elf_application_test", feature = "native_runtime_test"))]
    {
        // Every owner was explicitly finished and all handler pointers withdrawn.
        // No fallible resource cleanup is delegated to storage reuse or Drop.
        storage_busy.store(false, Ordering::Release);
    }
    result
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

fn channel_status(error: channel_error) -> u64 {
    match error {
        channel_error::wrong_binding
        | channel_error::wrong_producer
        | channel_error::wrong_service => 2,
        channel_error::closed => 5,
        channel_error::backpressure | channel_error::empty | channel_error::already_delivered => 1,
        channel_error::copy_failed => 3,
        _ => 4,
    }
}
pub(super) fn is_active() -> bool {
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    if native_runtime_profile::is_active() {
        return true;
    }

    #[cfg(feature = "veil_application_test")]
    if veil_profile::is_active() {
        return true;
    }
    !active.load(Ordering::Acquire).is_null()
}
pub(super) fn user_frame_valid(frame: *const interrupts::interrupt_frame) -> bool {
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    if native_runtime_profile::is_active() {
        return native_runtime_profile::user_frame_valid(frame);
    }

    #[cfg(feature = "veil_application_test")]
    if veil_profile::is_active() {
        return veil_profile::user_frame_valid(frame);
    }
    let pointer = active.load(Ordering::Acquire);
    // SAFETY: Published state outlives the IF-clear sole-BSP interrupt entry.
    !pointer.is_null() && unsafe { (&*pointer).frame_valid(frame.cast()) }
}
fn enter_dispatch() -> *mut runtime {
    if dispatch_active.swap(true, Ordering::AcqRel) {
        fail("reentry");
    }
    let pointer = active.load(Ordering::Acquire);
    if pointer.is_null() || interrupts::interrupts_enabled() {
        fail("inactive_dispatch");
    }
    pointer
}
pub(super) fn timer(
    frame: *mut interrupts::interrupt_frame,
    observed: monotonic_instant,
) -> Option<*mut interrupts::interrupt_frame> {
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    if native_runtime_profile::is_active() {
        return native_runtime_profile::timer(frame, observed);
    }

    #[cfg(feature = "veil_application_test")]
    if veil_profile::is_active() {
        return veil_profile::timer(frame, observed);
    }
    // SAFETY: enter_dispatch acquires the sole-BSP IF-clear nonreentrant borrow.
    let state = unsafe { &mut *enter_dispatch() };
    // SAFETY: The stable session owns both aligned CPU images under this latch.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_valid(frame.cast())
        || state.failure_armed
        || observed.timebase().slot() != 0
        || observed.timebase().generation() != crate::time::boot_timebase_generation
        || observed.tick() <= state.last_tick
        || state.evidence.timer_ticks >= max_calls
    {
        fail("timer_frame");
    }
    state.last_tick = observed.tick();
    state.evidence.timer_ticks += 1;
    // SAFETY: Selected image belongs to this stable, exclusively borrowed session.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("timer_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    Some(frame)
}
pub(super) fn fault(
    frame: *mut exceptions::exception_frame,
    snapshot: exceptions::exception_snapshot,
) -> Option<*mut exceptions::exception_frame> {
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    if native_runtime_profile::is_active() {
        return native_runtime_profile::fault(frame, snapshot);
    }

    #[cfg(feature = "veil_application_test")]
    if veil_profile::is_active() {
        return veil_profile::fault(frame, snapshot);
    }
    // SAFETY: enter_dispatch acquires the sole-BSP IF-clear nonreentrant borrow.
    let state = unsafe { &mut *enter_dispatch() };
    // SAFETY: The stable session owns both aligned CPU images under this latch.
    if !unsafe { state.cpu.save_current(state.selected) }
        || !state.frame_owned(frame.cast())
        || !application_fault::classify_user_fault(snapshot)
        || !fault_is_armed(state, snapshot)
        || state.scenario != 1
        || state.selected != 0
        || snapshot.vector != 6
        || snapshot.error_code != 0
        || snapshot.instruction_pointer != state.image.fault
    {
        fail("unexpected_fault");
    }
    state.close_channels();
    let transition = state
        .scheduler
        .fault_current(&mut state.tasks, state.identities[0])
        .unwrap_or_else(|_| fail("fault_transition"));
    if transition.dispatch().task() != state.identities[1] {
        fail("fault_peer");
    }
    state.terminal[0] = true;
    state.evidence.faults += 1;
    state.failure_armed = false;
    let selected = state.select(1);
    // SAFETY: Selected image belongs to this stable, exclusively borrowed session.
    if !unsafe { state.cpu.restore_task(1) } {
        fail("fault_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    Some(selected.cast())
}
fn fault_is_armed(state: &runtime, snapshot: exceptions::exception_snapshot) -> bool {
    #[cfg(feature = "linux_application_test")]
    if state.image.linux && !state.image.loader {
        return snapshot.flags & 0x202 == 0x202
            && state.evidence.sends == [1, 0]
            && state.evidence.deliveries == [0, 0]
            && state.channels[0].phase() == channel_phase::queued;
    }
    state.failure_armed && snapshot.flags & 0x200 == 0
}
pub(super) fn resume_kernel_after_fault() -> ! {
    #[cfg(all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ))]
    if native_runtime_profile::is_active() {
        native_runtime_profile::resume_kernel_after_fault();
    }
    fail("unexpected_last_fault")
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_native_application_gate_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    #[cfg(all(
        feature = "native_runtime_test",
        any(
            not(feature = "elf_application_test"),
            all(
                any(feature = "elf_document_test", feature = "native_relaunch_test"),
                not(feature = "linux_application_test")
            )
        )
    ))]
    if native_runtime_profile::is_active() {
        return native_runtime_profile::gate(frame);
    }

    #[cfg(feature = "veil_application_test")]
    if veil_profile::is_active() {
        return veil_profile::gate(frame);
    }
    // SAFETY: enter_dispatch acquires the sole-BSP IF-clear nonreentrant borrow.
    let state = unsafe { &mut *enter_dispatch() };
    // SAFETY: The stable session owns both aligned CPU images under this latch.
    if !unsafe { state.cpu.save_current(state.selected) } || !state.frame_valid(frame) {
        fail("gate_frame");
    }
    // SAFETY: frame_valid checked complete retained-frame ownership above.
    let value = unsafe { ptr::read(frame) };
    #[cfg(feature = "linux_application_test")]
    if state.image.linux && !state.image.loader && state.selected == 0 {
        fail("linux_used_native_gate");
    }
    if value.vector != gate_vector || value.error_code != 0 || state.evidence.calls >= max_calls {
        fail("gate_vector");
    }
    if value.accumulator != 0 && !state.started[state.selected] {
        fail("call_before_startup");
    }
    state.evidence.calls += 1;
    let mut selected = frame;
    let mut reply = (0, 0, 0);
    #[cfg(feature = "elf_application_test")]
    if state.image.loader && !matches!(value.accumulator, 0 | 5 | 9) {
        fail("loader_operation");
    }
    match value.accumulator {
        #[cfg(feature = "elf_application_test")]
        9 => {
            reply.0 = elf_profile::offer_plan(state, value);
            match reply.0 {
                2 => state.evidence.denials += 1,
                3 => state.evidence.bad_copies += 1,
                4 => state.evidence.invalid += 1,
                _ => {}
            }
        }
        0 => {
            if state.started[state.selected]
                || value.flags != 2
                || value.stack_pointer != user_stack_top
                || !state
                    .cpu
                    .task_image(state.selected)
                    .is_some_and(|i| i.is_initial_clean())
            {
                fail("startup");
            }
            state.started[state.selected] = true;
            // SAFETY: The validated current frame remains exclusively owned.
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
            // Cooperative yield uses the existing saved-frame transition. PIT
            // returns resume the same current task and are counted separately.
            if !state.terminal[1 - state.selected] {
                let transition = state
                    .scheduler
                    .preempt_current(&mut state.tasks, state.identities[state.selected])
                    .unwrap_or_else(|_| fail("yield_transition"));
                if transition.dispatch().task() != state.identities[1 - state.selected] {
                    fail("yield_peer");
                }
                // SAFETY: Save the reply in the retained outgoing frame before
                // selecting the peer root; both stacks remain mapped.
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
                fail("user_failure_status");
            }
            let index = state.selected;
            state.close_channels();
            state.terminal[index] = true;
            state.evidence.exits += 1;
            if state.terminal[1 - index] {
                state
                    .scheduler
                    .complete_current(&mut state.tasks, state.identities[index])
                    .unwrap_or_else(|_| fail("complete_last"));
                state.finish();
            }
            let transition = state
                .scheduler
                .complete_current_and_dispatch(&mut state.tasks, state.identities[index])
                .unwrap_or_else(|_| fail("complete_peer"));
            if transition.dispatch().task() != state.identities[1 - index] {
                fail("complete_wrong_peer");
            }
            selected = state.select(1 - index);
        }
        7 => {
            if state.selected != 1 {
                reply.0 = 2;
                state.evidence.denials += 1;
            } else if state.scenario != 0
                || !state.terminal[0]
                || state.last_ack != [7, 6]
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
                    let start = state.evidence.rows as usize * stage_row_bytes;
                    // SAFETY: Sole serialized service grant and complete checked
                    // copy; each row can be staged once, with no user alias.
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
            if state.scenario != 1
                || state.selected != 0
                || state.failure_armed
                || value.instruction_pointer != state.image.fault
                || state.evidence.sends != [1, 0]
                || state.evidence.deliveries != [0, 0]
            {
                fail("fault_arm");
            }
            state.failure_armed = true;
            // SAFETY: Clear IF only in the owned return frame at the exact UD2.
            unsafe {
                (*frame).flags = value.flags & !0x200;
            }
        }
        _ => {
            reply.0 = 4;
            state.evidence.invalid += 1;
        }
    }
    if selected == frame {
        // SAFETY: The selected current frame passed full ownership validation.
        unsafe {
            (*frame).accumulator = reply.0;
            (*frame).counter = reply.1;
            (*frame).data = reply.2;
        }
    }
    // SAFETY: Selected image belongs to this stable, exclusively borrowed session.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("gate_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    selected
}

#[cfg(all(
    feature = "linux_application_test",
    not(feature = "veil_application_test")
))]
#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_linux_syscall_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    // SAFETY: Assembly switched to the retained supervisor stack, masked flags,
    // and built the complete frame before taking this exclusive runtime borrow.
    let state = unsafe { &mut *enter_dispatch() };
    if !state.image.linux
        || state.image.loader
        || state.selected != 0
        || !state.started[0]
        || state.evidence.calls >= max_calls
        || !unsafe { state.cpu.save_current(0) }
        || !state.frame_valid(frame)
    {
        fail("linux_syscall_frame");
    }
    // SAFETY: Complete frame ownership and return geometry checked above.
    let mut value = unsafe { ptr::read(frame) };
    if !super::linux_syscall::entry_frame_matches_hardware(value) {
        fail("linux_hardware_frame");
    }
    // Linux x86-64 dispatch sign-extends its int syscall number from EAX.
    value.accumulator = (value.accumulator as u32 as i32 as i64) as u64;
    state.evidence.calls += 1;
    let bucket = match value.accumulator {
        0 => 0,
        1 => 1,
        7 => 2,
        24 => 3,
        60 => 4,
        _ => 5,
    };
    state.evidence.linux_calls[bucket] += 1;
    let mut selected = frame;
    let reply = if let Some(reply) = state.linux_io(value) {
        reply
    } else {
        match value.accumulator {
            24 => {
                // Only RAX changes: RCX/R11 retain their architectural clobbers
                // and every other register (including argument registers) survives.
                unsafe {
                    (*frame).accumulator = 0;
                }
                if !state.terminal[1] {
                    let transition = state
                        .scheduler
                        .preempt_current(&mut state.tasks, state.identities[0])
                        .unwrap_or_else(|_| fail("linux_yield_transition"));
                    if transition.dispatch().task() != state.identities[1] {
                        fail("linux_yield_peer");
                    }
                    selected = state.select(1);
                    state.evidence.switches += 1;
                }
                0
            }
            60 => {
                if value.destination_index as u8 != 0 {
                    fail("linux_user_failure_status");
                }
                state.close_channels();
                state.terminal[0] = true;
                state.evidence.exits += 1;
                if state.terminal[1] {
                    state
                        .scheduler
                        .complete_current(&mut state.tasks, state.identities[0])
                        .unwrap_or_else(|_| fail("linux_complete_last"));
                    state.finish();
                }
                let transition = state
                    .scheduler
                    .complete_current_and_dispatch(&mut state.tasks, state.identities[0])
                    .unwrap_or_else(|_| fail("linux_complete_peer"));
                if transition.dispatch().task() != state.identities[1] {
                    fail("linux_complete_wrong_peer");
                }
                selected = state.select(1);
                0
            }
            _ => -38,
        }
    };
    match reply {
        -9 => state.evidence.denials += 1,
        -14 => state.evidence.bad_copies += 1,
        -22 | -38 | -75 | -90 => state.evidence.invalid += 1,
        -19 => state.evidence.closed += 1,
        _ => {}
    }
    if selected == frame {
        // SAFETY: Validated current frame, exclusively retained until IRETQ.
        unsafe {
            (*frame).accumulator = reply as u64;
        }
    }
    // SAFETY: Selected CPU image belongs to this live retained session.
    if !unsafe { state.cpu.restore_task(state.selected) } {
        fail("linux_cpu_restore");
    }
    dispatch_active.store(false, Ordering::Release);
    selected
}

fn packet_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}
fn stage_is_clear() -> bool {
    // SAFETY: Bootstrap only, with no active runtime or aliases to this storage.
    unsafe {
        (&*display_stage.0.get())
            .iter()
            .all(|v| ptr::read_volatile(v) == 0)
    }
}
#[cfg(not(all(
    feature = "native_interactive_editor_test",
    not(any(
        feature = "native_durable_failure_test",
        feature = "native_relaunch_test",
        feature = "elf_application_test"
    ))
)))]
fn clear_stage() {
    // SAFETY: Native tasks have retired; sole bootstrap owner clears all bytes.
    unsafe {
        for byte in &mut *display_stage.0.get() {
            ptr::write_volatile(byte, 0);
        }
    }
    if !stage_is_clear() {
        fail("stage_clear");
    }
}
#[cfg_attr(
    all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ),
    allow(dead_code)
)]
fn framebuffer_hash(info: framebuffer_info, outside_patch: bool) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes_per_pixel = info.bits_per_pixel as usize / 8;
    for row in 0..info.height {
        for offset in 0..info.pitch {
            if outside_patch
                && (32..32 + stage_height).contains(&row)
                && (32 * bytes_per_pixel..(32 + stage_width) * bytes_per_pixel).contains(&offset)
            {
                continue;
            }
            // SAFETY: Only called under bootstrap after presenter construction
            // validated the complete unique framebuffer aperture and geometry.
            let byte = unsafe {
                ptr::read_volatile((info.address as *const u8).add(row * info.pitch + offset))
            };
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}
#[cfg_attr(
    all(
        feature = "native_interactive_editor_test",
        not(any(
            feature = "native_durable_failure_test",
            feature = "native_relaunch_test",
            feature = "elf_application_test"
        ))
    ),
    allow(dead_code)
)]
fn emit_evidence(initial: frame_accounting, vendor: &str, records: [evidence; 2]) {
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_application_test.ready","level":"info","tasks":2,"scenarios":2,"cpu_profile":"x87_sse2","cpu_vendor":"{}","source_language":"rust","code_pages":8,"data_frames":32,"table_frames":18,"channels":2}}"#,
        vendor
    );
    for (scenario, e) in records.iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_application_test.scenario","level":"info","scenario":{},"calls":{},"cooperative_switches":{},"timer_ticks":{},"surface_sends":{},"input_sends":{},"surface_deliveries":{},"input_deliveries":{},"surface_acks":{},"input_acks":{},"denials":{},"bad_copies":{},"invalid":{},"closed":{},"discarded":{},"exits":{},"faults":{},"rows":{}}}"#,
            scenario,
            e.calls,
            e.switches,
            e.timer_ticks,
            e.sends[0],
            e.sends[1],
            e.deliveries[0],
            e.deliveries[1],
            e.acknowledgements[0],
            e.acknowledgements[1],
            e.denials,
            e.bad_copies,
            e.invalid,
            e.closed,
            e.discarded,
            e.exits,
            e.faults,
            e.rows
        );
    }
    for (index, hash) in records[0].surface_hashes.iter().enumerate() {
        let _ = writeln!(
            serial,
            r#"{{"schema":1,"event":"kernel.native_application_test.surface","level":"info","sequence":{},"public_fixture_hash":{}}}"#,
            index + 1,
            hash
        );
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_application_test.presented","level":"info","x":32,"y":32,"width":384,"height":24,"frame_commits":1,"framebuffer_verified":true,"outside_unchanged":true,"fault_preserved_frame":true,"stage_cleared":true}}"#
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_application_test.released","level":"info","free_frames":{},"resources_restored":true,"channel_storage_cleared":true,"cpu_state_cleared":true,"kernel_cpu_restored":true,"bootstrap_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_application_test.passed","level":"info","scenarios":2,"tasks":4,"exits":3,"faults":1,"surfaces":7,"inputs":6}}"#
    );
}
fn fail(reason: &'static str) -> ! {
    interrupts::disable_external_interrupts();
    let mut serial = serial_port::com1();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.native_application_test.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}

#[cfg(feature = "veil_application_test")]
#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_linux_syscall_dispatch(
    frame: *mut scheduler_frame,
) -> *mut scheduler_frame {
    veil_profile::syscall(frame)
}

#[cfg(all(
    feature = "native_interactive_editor_test",
    not(any(
        feature = "native_durable_failure_test",
        feature = "native_relaunch_test",
        feature = "elf_application_test"
    ))
))]
pub(super) fn interactive_input_active() -> bool {
    native_runtime_profile::is_active()
}
#[cfg(all(
    feature = "native_interactive_editor_test",
    not(any(
        feature = "native_durable_failure_test",
        feature = "native_relaunch_test",
        feature = "elf_application_test"
    ))
))]
pub(super) fn resume_interactive_input(
    frame: *mut interrupts::interrupt_frame,
) -> *mut interrupts::interrupt_frame {
    native_runtime_profile::resume_input_interrupt(frame)
}
