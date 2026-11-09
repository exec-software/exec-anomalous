//! Boot a Ny vessel and let it speak the rite ABI.
//!
//! The vessel's code is not assembled into this kernel. It is a Ny object
//! compiled from `vessels/anomalous-ny-vessel`, entered through `__ny_entry`,
//! linked against the freestanding runtime the Ny SDK ships, and carried here
//! by `cargo xtask ny-vessel-inspect` as an inspected payload. Nothing in that
//! object knows this ABI exists: the Anomalous-owned half of the vessel is the
//! only part that names a mailbox or an interrupt vector, which is what ADR
//! 0019 asks for while the rite ABI is unstable.
//!
//! The division is memory. This fixture installs a region, the vessel's Ny
//! half composes its message into it, and the vessel's Anomalous half reads
//! the message back out and submits it as one `console_write`, then exits with
//! what Ny returned. A Ny object publishes `__ny_entry` and nothing else, so
//! the region is the only channel it has, and it is enough.
//!
//! What this proves is that a Ny vessel boots and completes rites under the
//! ordinary dispatcher. It is not a general Ny runtime, a loader for arbitrary
//! objects, or a claim about any Ny feature the vessel does not use.
use anomalous_abi::{
    id_128, rite_call, rite_operation, rite_payload_kind, rite_reply, rite_rights, rite_status,
};
use core::arch::asm;
use core::fmt::Write;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering, compiler_fence};

use super::address_space::{address_space, mapping_evidence};
use super::rite_transport::{
    call_size, clear_and_verify_bytes, copy_admitted_body, decode_exit_payload, read_call,
    reply_size, rite_scratch, scratch_is_zero, transport_payload_limit, write_reply,
};
use super::{descriptors, paging};
use crate::circle::{circle_child_kind, circle_mapping_capacity, circle_table};
use crate::console::{screen_console, serial_port};
use crate::identity::{circle_id, fixture_principal_id, task_identity};
use crate::memory;
use crate::memory::physical::{frame_allocator, frame_lease, frame_purpose, physical_range};
use crate::rite::{rite_admission, rite_channel, rite_decision};
use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_state, task_table};

const page_size: u64 = 4096;

/// The vessel's memory map, in the same user window the rite fixtures use. It
/// has to be above the 512 GiB boundary, because the first page-table entry is
/// this kernel's own branch and `address_space` refuses a user mapping there.
/// Reaching it is why the object is emitted for the large code model: the
/// small model materializes every address as a 32-bit absolute and cannot be
/// linked this high at all. The linker script in
/// `vessels/anomalous-ny-vessel` states the same addresses, and the payload
/// stage refuses an image that disagrees.
const user_window_address: u64 = 0x0000_6000_0000_0000;
const code_address: u64 = user_window_address + 0x1000;
const writable_address: u64 = user_window_address + 0x2000;
const writable_pages: u64 = 3;
const arena_address: u64 = user_window_address + 0x3000;
const mailbox_address: u64 = user_window_address + 0x4000;
const stack_address: u64 = user_window_address + 0x6000;
const stack_top: u64 = user_window_address + 0x7000;

const rite_gate_vector: u64 = 0xf3;
const sanitized_flags: u64 = 0x2;
const cr0_task_switched: u64 = 1 << 3;
const status_completed: u64 = 0x40;
const call_count: usize = 2;

/// What the vessel composes and sends. Measured by running the vessel's
/// pre-gate sequence against the same object and archive on the build host,
/// and asserted here so a change in what Ny produces fails rather than being
/// reported as whatever arrived.
const expected_console_text: &[u8] = b"Anomalous vessel speaks through a rite";
const expected_exit_code: i32 = 42;
const console_trace_word: u64 = 0x4e59_5645_5353_4c31;
const exit_trace_word: u64 = 0x4e59_5645_5353_4c32;

/// The payload header the build stage writes ahead of the image.
const header_magic: u64 = 0x4e59_5f56_4553_5331;
const header_words: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum gate_phase {
    console_write,
    task_exit,
    idle,
}

impl gate_phase {
    const fn index(self) -> Option<usize> {
        match self {
            Self::console_write => Some(0),
            Self::task_exit => Some(1),
            Self::idle => None,
        }
    }

    const fn next(self) -> Option<Self> {
        match self {
            Self::console_write => Some(Self::task_exit),
            Self::task_exit => Some(Self::idle),
            Self::idle => None,
        }
    }

    const fn sequence(self) -> Option<u64> {
        match self {
            Self::console_write => Some(1),
            Self::task_exit => Some(2),
            Self::idle => None,
        }
    }

    const fn trace_word(self) -> Option<u64> {
        match self {
            Self::console_write => Some(console_trace_word),
            Self::task_exit => Some(exit_trace_word),
            Self::idle => None,
        }
    }

    const fn operation(self) -> Option<rite_operation> {
        match self {
            Self::console_write => Some(rite_operation::console_write),
            Self::task_exit => Some(rite_operation::task_exit),
            Self::idle => None,
        }
    }
}

const fn phase_from_index(index: usize) -> Option<gate_phase> {
    match index {
        0 => Some(gate_phase::console_write),
        1 => Some(gate_phase::task_exit),
        _ => None,
    }
}

/// Where the vessel's own code expects to resume after each submission. The
/// build stage states these, because a toolchain-produced image cannot be
/// described by this kernel's linker symbols.
#[derive(Clone, Copy)]
struct vessel_header {
    image_length: u64,
    entry: u64,
    console_return: u64,
    exit_return: u64,
    failed: u64,
}

struct vessel_observation {
    sequence: u64,
    status: rite_status,
}

impl vessel_observation {
    const fn empty() -> Self {
        Self {
            sequence: 0,
            status: rite_status::faulted,
        }
    }
}

struct vessel_context {
    circles: circle_table,
    tasks: task_table,
    channel: Option<rite_channel>,
    screen: screen_console,
    current_task: Option<task_identity>,
    phase: gate_phase,
    header: vessel_header,
    mailbox_alias: u64,
    active_root: u64,
    transition_stack_top: u64,
    scratch: rite_scratch,
    console_copy: [u8; expected_console_text.len()],
    observations: [vessel_observation; call_count],
    observation_count: usize,
    body_copy_count: usize,
    body_bytes_copied: usize,
    reply_copy_count: usize,
    console_bytes_committed: usize,
    exit_code: Option<i32>,
    terminal_reply_copied: bool,
}

struct task_space {
    root: address_space,
    code_page: u64,
    writable_base: u64,
    stack_page: u64,
    payload_length: usize,
}

impl task_space {
    /// The mailbox is the third page of the vessel's writable extent, so its
    /// physical alias is that extent's base plus two pages.
    const fn mailbox_alias(&self) -> u64 {
        self.writable_base + 2 * page_size
    }

    const fn arena_alias(&self) -> u64 {
        self.writable_base + page_size
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct rite_gate_frame {
    accumulator: u64,
    base: u64,
    counter: u64,
    data: u64,
    frame_pointer: u64,
    source_index: u64,
    destination_index: u64,
    register_8: u64,
    register_9: u64,
    register_10: u64,
    register_11: u64,
    register_12: u64,
    register_13: u64,
    register_14: u64,
    register_15: u64,
    vector: u64,
    error_code: u64,
    instruction_pointer: u64,
    code_segment: u64,
    flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

const _: () = {
    assert!(code_address == user_window_address + page_size);
    assert!(writable_address == code_address + page_size);
    assert!(stack_address == writable_address + (writable_pages + 1) * page_size);
    assert!(stack_top == stack_address + page_size);
    assert!(call_size == 40);
    assert!(reply_size == 40);
    assert!(mem::size_of::<rite_gate_frame>() == 22 * mem::size_of::<u64>());
    assert!(arena_address == writable_address + page_size);
    assert!(mailbox_address == writable_address + 2 * page_size);
    assert!(expected_console_text.len() < transport_payload_limit as usize);
};

static active_context: AtomicPtr<vessel_context> = AtomicPtr::new(ptr::null_mut());
static gate_active: AtomicBool = AtomicBool::new(false);
static mut saved_kernel_stack: u64 = 0;
static mut saved_task_switched: u64 = 0;

unsafe extern "C" {
    fn x86_64_enter_ny_vessel_task(
        instruction_pointer: u64,
        stack_pointer: u64,
        data_selector: u64,
        code_selector: u64,
        saved_stack: *mut u64,
    ) -> u64;
    fn x86_64_resume_ny_vessel_task(
        status: u64,
        saved_stack: u64,
        original_task_switched: u64,
    ) -> !;

    static x86_64_ny_vessel_header_start: u8;
    static x86_64_ny_vessel_header_end: u8;
    static x86_64_ny_vessel_payload_start: u8;
    static x86_64_ny_vessel_payload_end: u8;
}

pub(super) fn run(mut screen: screen_console) -> ! {
    screen.initialize();
    let kernel_cr3 = paging::read_cr3();
    let result = memory::with_frame_allocator(move |allocator| {
        run_with_allocator(allocator, kernel_cr3, screen)
    });

    if paging::read_cr3() != kernel_cr3 {
        // SAFETY: This is the exact root captured before the bounded proof, and
        // every feature-owned root borrows its still-live supervisor branch.
        unsafe { paging::write_cr3(kernel_cr3) };
    }
    if paging::read_cr3() != kernel_cr3 {
        fail("kernel_root_restore_failed");
    }
    match result {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => fail(reason),
        Err(error) => fail(error.reason()),
    }

    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.passed","level":"info","calls":2,"commits":2,"terminal":true}}"#
    );
    super::qemu_exit_success();
    super::halt_forever()
}

fn run_with_allocator(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    screen: screen_console,
) -> Result<(), &'static str> {
    if !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
        || !paging::protections_are_enabled()
        || !maskable_interrupts_are_disabled()
    {
        return Err("initial_ny_vessel_state_invalid");
    }
    let initial = allocator.accounting();
    if initial.free == 0
        || initial.data_owned != 0
        || initial.table_owned != 0
        || initial.active_leases != 0
        || initial.bound_leases != 0
    {
        return Err("initial_accounting_invalid");
    }

    let header = read_header()?;
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.ready","level":"info","arch":"x86_64","vector":243,"grant":{},"image_bytes":{},"entry":{},"region_pages":1}}"#,
        rite_rights::shell_minimum,
        header.image_length,
        header.entry
    );

    let mut circles = circle_table::new();
    let mut tasks = task_table::new();
    let circle = circles
        .create()
        .map_err(|_| "ny_vessel_circle_create_failed")?;
    let mut space = prepare_task_space(allocator, kernel_cr3, &header)?;
    attach_task_space(&mut circles, circle)?;
    let task = create_task(&mut circles, &mut tasks, circle, space.payload_length)?;
    let channel = rite_channel::try_new(task, rite_rights::shell_minimum)
        .map_err(|_| "ny_vessel_channel_create_failed")?;

    let task_snapshot = tasks
        .snapshot(task)
        .map_err(|_| "ny_vessel_task_snapshot_failed")?;
    let circle_snapshot = circles
        .snapshot(circle)
        .map_err(|_| "ny_vessel_circle_snapshot_failed")?;
    let children = circle_snapshot.children();
    if task_snapshot.identity() != task
        || task_snapshot.state() != task_state::ready
        || task_snapshot.circle() != circle
        || task_snapshot.bounds().entry() != code_address
        || circle_snapshot.identity() != circle
        || children.tasks() != 1
        || children.address_spaces() != 1
        || children.mappings() != circle_mapping_capacity
    {
        return Err("ny_vessel_task_ready_state_invalid");
    }

    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.task.entered","level":"info","origin":"user","mappings":3,"code_permissions":"user_rx","writable_permissions":"user_rw_nx","stack_permissions":"user_rw_nx","guards_absent":true}}"#
    );

    let mut context = vessel_context {
        circles,
        tasks,
        channel: Some(channel),
        screen,
        current_task: None,
        phase: gate_phase::console_write,
        header,
        mailbox_alias: space.mailbox_alias(),
        active_root: 0,
        transition_stack_top: 0,
        scratch: [0; page_size as usize],
        console_copy: [0; expected_console_text.len()],
        observations: [vessel_observation::empty(), vessel_observation::empty()],
        observation_count: 0,
        body_copy_count: 0,
        body_bytes_copied: 0,
        reply_copy_count: 0,
        console_bytes_committed: 0,
        exit_code: None,
        terminal_reply_copied: false,
    };

    let arena_alias = space.arena_alias();
    execute_task(&mut context, &space, task, kernel_cr3)?;

    let exited = context
        .tasks
        .snapshot(task)
        .map_err(|_| "ny_vessel_task_final_snapshot_failed")?;
    // The console body is counted once and the exit body once, so two copies
    // of exactly the message and the exit payload crossed the boundary.
    let expected_body_bytes = expected_console_text.len() + mem::size_of::<u64>();
    if exited.state() != task_state::exited
        || context.phase != gate_phase::idle
        || context.observation_count != call_count
        || context.body_copy_count != call_count
        || context.body_bytes_copied != expected_body_bytes
        || context.reply_copy_count != call_count - 1
        || context.console_bytes_committed != expected_console_text.len()
        || context.console_copy != expected_console_text
        || context.exit_code != Some(expected_exit_code)
        || context.terminal_reply_copied
        || !scratch_is_zero(&context.scratch)
    {
        return Err("ny_vessel_completion_evidence_invalid");
    }
    validate_observations(&context.observations)?;

    // The vessel composed its message in the region this fixture installed, so
    // the bytes are still there to be compared against what the rite carried.
    // A vessel that had sent something it did not compose fails here.
    if !region_holds_message(arena_alias) {
        return Err("ny_vessel_region_message_absent");
    }

    context
        .channel
        .take()
        .ok_or("ny_vessel_channel_missing_at_retirement")?;
    if !clear_and_verify_bytes(&mut context.console_copy)
        || !clear_and_verify_bytes(&mut context.scratch)
    {
        return Err("ny_vessel_private_storage_clear_failed");
    }

    release_task_space(allocator, &mut context.circles, circle, &mut space)?;
    context
        .tasks
        .destroy(&mut context.circles, task)
        .map_err(|_| "ny_vessel_task_destroy_failed")?;
    context
        .circles
        .destroy(circle)
        .map_err(|_| "ny_vessel_circle_destroy_failed")?;

    let final_accounting = allocator.accounting();
    let circle_accounting = context.circles.accounting();
    if final_accounting != initial
        || circle_accounting.circles != 0
        || circle_accounting.tasks != 0
        || circle_accounting.address_spaces != 0
        || circle_accounting.mappings != 0
        || context.tasks.active_count() != 0
        || context.channel.is_some()
        || context.current_task.is_some()
        || context.mailbox_alias != 0
        || context.active_root != 0
        || context.transition_stack_top != 0
        || !active_context.load(Ordering::Acquire).is_null()
        || gate_active.load(Ordering::Acquire)
    {
        return Err("final_ny_vessel_state_mismatch");
    }

    emit_outcomes(&mut serial, &context)?;
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.released","level":"info","free_frames":{},"initial_free_frames":{},"data_owned_frames":{},"table_owned_frames":{},"active_circles":{},"active_tasks":{},"active_mappings":{},"channel_retired":true,"region_wiped":true,"transition_stack_wiped":true}}"#,
        final_accounting.free,
        initial.free,
        final_accounting.data_owned,
        final_accounting.table_owned,
        circle_accounting.circles,
        context.tasks.active_count(),
        circle_accounting.mappings
    );
    Ok(())
}

fn emit_outcomes(serial: &mut serial_port, context: &vessel_context) -> Result<(), &'static str> {
    let console = &context.observations[0];
    // The committed bytes are stated as their length and their source, not
    // echoed: the vessel's text already reached the console through the
    // ordinary commit path.
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.console.committed","level":"info","sequence":{},"status":"{}","bytes":{},"composed_by":"ny","from_region":true}}"#,
        console.sequence,
        status_label(console.status)?,
        context.console_bytes_committed
    );
    let exit = &context.observations[1];
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.exited","level":"info","sequence":{},"status":"{}","code":{},"returned_from_ny":true}}"#,
        exit.sequence,
        status_label(exit.status)?,
        context.exit_code.ok_or("ny_vessel_exit_code_missing")?
    );
    Ok(())
}

fn status_label(status: rite_status) -> Result<&'static str, &'static str> {
    match status {
        rite_status::ok => Ok("ok"),
        rite_status::would_block => Ok("would_block"),
        rite_status::denied => Ok("denied"),
        rite_status::bad_operation => Ok("bad_operation"),
        rite_status::bad_payload => Ok("bad_payload"),
        rite_status::limit_exceeded => Ok("limit_exceeded"),
        rite_status::interrupted => Ok("interrupted"),
        rite_status::faulted => Err("ny_vessel_status_faulted"),
        // The status is a u16 newtype, so an unknown value is representable.
        // A reply this fixture cannot name is a reply it must not report.
        _ => Err("ny_vessel_status_unknown"),
    }
}

fn validate_observations(
    observations: &[vessel_observation; call_count],
) -> Result<(), &'static str> {
    for (index, observation) in observations.iter().enumerate() {
        let phase = phase_from_index(index).ok_or("ny_vessel_observation_index_invalid")?;
        if observation.sequence != phase.sequence().ok_or("ny_vessel_sequence_missing")?
            || observation.status != rite_status::ok
        {
            return Err("ny_vessel_observation_mismatch");
        }
    }
    Ok(())
}

/// Reads the payload header the build stage wrote, and refuses anything whose
/// stated addresses do not lie inside the one code page this fixture maps.
fn read_header() -> Result<vessel_header, &'static str> {
    let start = ptr::addr_of!(x86_64_ny_vessel_header_start);
    let end = ptr::addr_of!(x86_64_ny_vessel_header_end);
    let length = (end as usize)
        .checked_sub(start as usize)
        .ok_or("ny_vessel_header_extent_invalid")?;
    if length != header_words * mem::size_of::<u64>()
        || !(start as usize).is_multiple_of(mem::align_of::<u64>())
    {
        return Err("ny_vessel_header_extent_invalid");
    }
    let mut words = [0u64; header_words];
    for (index, word) in words.iter_mut().enumerate() {
        // SAFETY: The checked immutable header extent covers these words and
        // lives in this kernel's read-only data for the whole proof.
        *word = unsafe { ptr::read_volatile((start as *const u64).add(index)) };
    }
    if words[0] != header_magic {
        return Err("ny_vessel_header_magic_invalid");
    }
    let header = vessel_header {
        image_length: words[1],
        entry: words[2],
        console_return: words[3],
        exit_return: words[4],
        failed: words[5],
    };
    let payload_start = ptr::addr_of!(x86_64_ny_vessel_payload_start);
    let payload_end = ptr::addr_of!(x86_64_ny_vessel_payload_end);
    let carried = (payload_end as usize)
        .checked_sub(payload_start as usize)
        .ok_or("ny_vessel_payload_extent_invalid")?;
    let code_end = code_address
        .checked_add(header.image_length)
        .ok_or("ny_vessel_image_extent_invalid")?;
    if header.image_length == 0
        || header.image_length > page_size
        || carried as u64 != header.image_length
        || header.entry != code_address
        || ![header.console_return, header.exit_return, header.failed]
            .iter()
            .all(|address| (code_address..code_end).contains(address))
    {
        return Err("ny_vessel_header_addresses_invalid");
    }
    Ok(header)
}

fn create_task(
    circles: &mut circle_table,
    tasks: &mut task_table,
    circle: circle_id,
    code_length: usize,
) -> Result<task_identity, &'static str> {
    let code_length = u64::try_from(code_length).map_err(|_| "ny_vessel_code_length_invalid")?;
    let code_end = code_address
        .checked_add(code_length)
        .filter(|end| *end <= code_address + page_size)
        .ok_or("ny_vessel_code_length_invalid")?;
    let bounds = task_bounds::new(
        code_address,
        code_end,
        code_address,
        stack_address,
        stack_top,
        stack_top,
    )
    .ok_or("ny_vessel_task_bounds_invalid")?;
    let principal = fixture_principal_id::new(1).ok_or("ny_vessel_principal_invalid")?;
    tasks
        .create(
            circles,
            task_binding::new(
                circle,
                principal,
                synthetic_origin_metadata::human_role_fixture,
                bounds,
            ),
        )
        .map_err(|_| "ny_vessel_task_create_failed")
}

fn prepare_task_space(
    allocator: &mut frame_allocator<'_>,
    kernel_cr3: u64,
    header: &vessel_header,
) -> Result<task_space, &'static str> {
    let code = allocate_pages(allocator, 1, "ny_vessel_code_frame_allocation_failed")?;
    // State, region and mailbox are one extent: they are contiguous, share
    // permissions, and a circle admits three mappings in total, which code and
    // stack would otherwise exhaust. The mailbox being inside the vessel's own
    // writable memory is not a weakening -- the dispatcher reads it through
    // this kernel's physical alias and copies admitted bodies rather than
    // trusting it, and a vessel may write anywhere in its own pages regardless.
    let writable = allocate_pages(
        allocator,
        writable_pages,
        "ny_vessel_writable_frame_allocation_failed",
    )?;
    let stack = allocate_pages(allocator, 1, "ny_vessel_stack_frame_allocation_failed")?;
    let code_range = code.range().ok_or("ny_vessel_code_frame_extent_missing")?;
    let writable_range = writable
        .range()
        .ok_or("ny_vessel_writable_frame_extent_missing")?;
    let stack_range = stack
        .range()
        .ok_or("ny_vessel_stack_frame_extent_missing")?;
    if code_range == writable_range
        || code_range == stack_range
        || writable_range == stack_range
        || !range_is_zero(code_range, page_size)
        || !range_is_zero(writable_range, writable_pages * page_size)
        || !range_is_zero(stack_range, page_size)
    {
        return Err("ny_vessel_private_frames_not_distinct_and_clear");
    }
    let payload_start = ptr::addr_of!(x86_64_ny_vessel_payload_start);
    // SAFETY: The allocator grants the exclusive one-page code lease and the
    // header-checked immutable payload fits wholly in it.
    let payload_length = unsafe {
        copy_payload(
            payload_start,
            usize::try_from(header.image_length).map_err(|_| "ny_vessel_image_extent_invalid")?,
            code_range,
        )?
    };

    // SAFETY: The published allocator serializes the proof, and the callback
    // clears every exclusive table extent before publication.
    let mut root = unsafe {
        address_space::create(allocator, kernel_cr3, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| "ny_vessel_address_space_create_failed")?;
    root.map_owned(
        allocator,
        code_address,
        code,
        paging::page_permissions {
            user: true,
            writable: false,
            executable: true,
        },
    )
    .map_err(|_| "ny_vessel_code_mapping_failed")?;
    root.map_owned_extent(
        allocator,
        writable_address,
        writable,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "ny_vessel_writable_mapping_failed")?;
    root.map_owned(
        allocator,
        stack_address,
        stack,
        paging::page_permissions {
            user: true,
            writable: true,
            executable: false,
        },
    )
    .map_err(|_| "ny_vessel_stack_mapping_failed")?;
    root.validate_all_mappings()
        .map_err(|_| "ny_vessel_address_space_validation_failed")?;
    validate_mapping(
        root.validate_mapping(code_address)
            .map_err(|_| "ny_vessel_code_mapping_walk_failed")?,
        code_range.base,
        false,
        true,
    )?;
    for page in 0..writable_pages {
        validate_mapping(
            root.validate_mapping(writable_address + page * page_size)
                .map_err(|_| "ny_vessel_writable_mapping_walk_failed")?,
            writable_range.base + page * page_size,
            true,
            false,
        )?;
    }
    validate_mapping(
        root.validate_mapping(stack_address)
            .map_err(|_| "ny_vessel_stack_mapping_walk_failed")?,
        stack_range.base,
        true,
        false,
    )?;
    // The gaps, not the neighbours: code is followed immediately by the
    // writable extent, and the extent is followed by the stack's guard. The
    // window base below the code and the page above the stack complete it.
    for guard in [
        user_window_address,
        writable_address + writable_pages * page_size,
        stack_top,
    ] {
        if root.validate_absent(guard).is_err() {
            return Err("ny_vessel_guard_mapping_present");
        }
    }
    if !payload_matches(payload_start, payload_length, code_range.base) {
        return Err("ny_vessel_payload_copy_mismatch");
    }
    Ok(task_space {
        root,
        code_page: code_range.base,
        writable_base: writable_range.base,
        stack_page: stack_range.base,
        payload_length,
    })
}

fn attach_task_space(circles: &mut circle_table, circle: circle_id) -> Result<(), &'static str> {
    circles
        .attach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "ny_vessel_root_attachment_failed")?;
    if circles
        .attach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .is_err()
    {
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .map_err(|_| "ny_vessel_root_attachment_rollback_failed")?;
        return Err("ny_vessel_mapping_attachment_failed");
    }
    Ok(())
}

fn allocate_pages<'allocator>(
    allocator: &mut frame_allocator<'allocator>,
    pages: u64,
    reason: &'static str,
) -> Result<frame_lease, &'static str> {
    let pages = usize::try_from(pages).map_err(|_| reason)?;
    // SAFETY: The allocator is the sole authority for the callback range and
    // the fixed clearer consumes and verifies the complete extent now.
    unsafe {
        allocator.allocate_with(pages, 1, frame_purpose::data, |range| {
            memory::clear_and_verify(range)
        })
    }
    .map_err(|_| reason)
}

unsafe fn copy_payload(
    start: *const u8,
    length: usize,
    destination: physical_range,
) -> Result<usize, &'static str> {
    if length == 0 || length > page_size as usize || destination.length != page_size {
        return Err("ny_vessel_payload_extent_invalid");
    }
    let destination = usize::try_from(destination.base)
        .map_err(|_| "ny_vessel_payload_destination_invalid")? as *mut u8;
    // SAFETY: The caller supplies an exclusive page and the header-checked
    // immutable payload range fits without overlap.
    unsafe { ptr::copy_nonoverlapping(start, destination, length) };
    compiler_fence(Ordering::SeqCst);
    Ok(length)
}

fn payload_matches(start: *const u8, length: usize, destination: u64) -> bool {
    let Ok(destination) = usize::try_from(destination) else {
        return false;
    };
    for offset in 0..length {
        // SAFETY: Both ranges cover the checked payload length.
        let expected = unsafe { ptr::read(start.add(offset)) };
        let actual = unsafe { ptr::read_volatile((destination as *const u8).add(offset)) };
        if actual != expected {
            return false;
        }
    }
    true
}

/// Reads the vessel's region through this kernel's alias and requires the
/// message to be there, NUL-terminated, as the vessel's first allocation.
fn region_holds_message(arena_alias: u64) -> bool {
    let Ok(base) = usize::try_from(arena_alias) else {
        return false;
    };
    for (offset, expected) in expected_console_text.iter().enumerate() {
        // SAFETY: The region page is identity mapped and this fixture retains
        // its exclusive lease until release.
        let actual = unsafe { ptr::read_volatile((base as *const u8).add(offset)) };
        if actual != *expected {
            return false;
        }
    }
    // SAFETY: The terminator lies inside the same exclusively held page.
    unsafe { ptr::read_volatile((base as *const u8).add(expected_console_text.len())) == 0 }
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
        return Err("ny_vessel_mapping_permissions_invalid");
    }
    Ok(())
}

fn execute_task(
    context: &mut vessel_context,
    space: &task_space,
    task: task_identity,
    kernel_cr3: u64,
) -> Result<(), &'static str> {
    // SAFETY: This single-BSP feature exclusively owns both continuation cells.
    let saved_stack_clear = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) == 0 };
    // SAFETY: The same exclusive interval protects the saved TS cell.
    let saved_task_switched_clear =
        unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) == 0 };
    // SAFETY: IF is clear, NMI is masked, and no privilege transition can race
    // this read of the packed TSS RSP0 field.
    let transition_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    if context.current_task.is_some()
        || context.mailbox_alias == 0
        || context.channel.is_none()
        || context.phase != gate_phase::console_write
        || !active_context.load(Ordering::Acquire).is_null()
        || space.payload_length == 0
        || !saved_stack_clear
        || !saved_task_switched_clear
        || transition_stack_top == 0
        || transition_stack_top & 0x0f != 0
    {
        return Err("ny_vessel_task_entry_state_invalid");
    }

    context
        .tasks
        .start(task)
        .map_err(|_| "ny_vessel_task_start_failed")?;
    context.current_task = Some(task);
    context.active_root = space
        .root
        .root_address()
        .map_err(|_| "ny_vessel_task_root_extent_missing")?;
    context.transition_stack_top = transition_stack_top;

    let original_task_switched = paging::read_cr0() & cr0_task_switched;
    // SAFETY: The bootstrap processor is the only writer and the gate cannot
    // run until after context publication and IRETQ.
    unsafe {
        ptr::write_volatile(
            ptr::addr_of_mut!(saved_task_switched),
            original_task_switched,
        )
    };
    // SAFETY: The root has exactly three validated mappings and borrows the
    // live kernel branch; maskable interrupts and NMI remain excluded.
    unsafe { space.root.activate() }.map_err(|_| "ny_vessel_root_activation_failed")?;
    let context_pointer = ptr::from_mut(context);
    if active_context
        .compare_exchange(
            ptr::null_mut(),
            context_pointer,
            Ordering::Release,
            Ordering::Relaxed,
        )
        .is_err()
    {
        // SAFETY: User entry did not occur, so restoring the original root is
        // immediate and cannot race another execution context.
        unsafe { paging::write_cr3(kernel_cr3) };
        return Err("ny_vessel_context_publication_failed");
    }

    // SAFETY: Task identity, root, selectors, the vessel's own mailbox page,
    // TSS RSP0, and the feature-local gate were validated before this one BSP
    // enters CPL3.
    let status = unsafe {
        x86_64_enter_ny_vessel_task(
            code_address,
            stack_top,
            u64::from(descriptors::user_data_selector),
            u64::from(descriptors::user_code_selector),
            ptr::addr_of_mut!(saved_kernel_stack),
        )
    };

    active_context.store(ptr::null_mut(), Ordering::Release);
    let task_switched_restored = paging::read_cr0() & cr0_task_switched == original_task_switched;
    // SAFETY: The nonlocal completion restored the original Rust stack. The
    // exact kernel root must be active before withdrawing any owned mapping.
    unsafe { paging::write_cr3(kernel_cr3) };
    let kernel_root_restored = paging::read_cr3() == kernel_cr3;
    context.current_task = None;
    context.mailbox_alias = 0;
    context.active_root = 0;
    let transition_stack_wiped = descriptors::wipe_privilege_stack();
    context.transition_stack_top = 0;
    // SAFETY: Completion consumed both single-use continuation values.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(saved_kernel_stack), 0);
        ptr::write_volatile(ptr::addr_of_mut!(saved_task_switched), 0);
    }
    compiler_fence(Ordering::SeqCst);
    if status != status_completed
        || !task_switched_restored
        || !kernel_root_restored
        || !transition_stack_wiped
        || gate_active.load(Ordering::Acquire)
    {
        return Err("ny_vessel_task_completion_restore_failed");
    }
    Ok(())
}

fn release_task_space(
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    circle: circle_id,
    space: &mut task_space,
) -> Result<(), &'static str> {
    let mut code = space
        .root
        .unmap_owned(allocator, code_address)
        .map_err(|_| "ny_vessel_code_mapping_withdrawal_failed")?;
    let mut writable = space
        .root
        .unmap_owned(allocator, writable_address)
        .map_err(|_| "ny_vessel_writable_mapping_withdrawal_failed")?;
    let mut stack = space
        .root
        .unmap_owned(allocator, stack_address)
        .map_err(|_| "ny_vessel_stack_mapping_withdrawal_failed")?;
    // SAFETY: The root is inactive, owns no mappings, and its table frames are
    // exclusive to this allocator-authorized address-space object.
    unsafe {
        space
            .root
            .destroy(allocator, |range| memory::clear_and_verify(range))
    }
    .map_err(|_| "ny_vessel_address_space_destroy_failed")?;
    // SAFETY: Unmapping returned three exclusive, unbound data leases. The
    // region is cleared with everything else, so the message the vessel
    // composed does not outlive the proof.
    unsafe { allocator.release_with(&mut code, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "ny_vessel_code_frame_release_failed")?;
    // SAFETY: The writable extent's lease is inactive and exclusively held.
    unsafe { allocator.release_with(&mut writable, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "ny_vessel_writable_frame_release_failed")?;
    // SAFETY: The stack lease is inactive and exclusively held.
    unsafe { allocator.release_with(&mut stack, |range| memory::clear_and_verify(range)) }
        .map_err(|_| "ny_vessel_stack_frame_release_failed")?;
    space.code_page = 0;
    space.writable_base = 0;
    space.stack_page = 0;
    circles
        .detach_children(circle, circle_child_kind::mapping, circle_mapping_capacity)
        .map_err(|_| "ny_vessel_mapping_detachment_failed")?;
    circles
        .detach_child(circle, circle_child_kind::address_space)
        .map_err(|_| "ny_vessel_root_detachment_failed")?;
    Ok(())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn x86_64_ny_vessel_gate_dispatch(frame: *mut rite_gate_frame) {
    if frame.is_null()
        || !(frame as usize).is_multiple_of(mem::align_of::<u64>())
        || gate_active
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
    {
        fail("ny_vessel_gate_reentry_or_frame_invalid");
    }
    let context_pointer = active_context.load(Ordering::Acquire);
    if context_pointer.is_null() {
        fail("ny_vessel_gate_without_active_context");
    }
    // SAFETY: Publication names the unique live kernel-stack context. IF is
    // clear, NMI is masked, and the latch provides exclusive mutable access.
    let context = unsafe { &mut *context_pointer };
    // SAFETY: Assembly supplies exactly 22 naturally aligned qwords whose
    // lifetime covers this dispatch.
    let frame = unsafe { &mut *frame };
    if !valid_gate_frame(context, frame) {
        fail("ny_vessel_gate_frame_validation_failed");
    }

    let reply_copies_before = context.reply_copy_count;
    match dispatch_gate(context) {
        Ok(gate_action::return_to_user(reply)) => {
            // SAFETY: The active task space retains exclusive ownership of its
            // live identity-mapped mailbox alias through this gate dispatch.
            if unsafe { write_reply(context.mailbox_alias, reply) }.is_err() {
                fail("ny_vessel_reply_copy_failed");
            }
            context.reply_copy_count += 1;
            compiler_fence(Ordering::SeqCst);
            gate_active.store(false, Ordering::Release);
        }
        Ok(gate_action::complete) => {
            context.terminal_reply_copied = context.reply_copy_count != reply_copies_before;
            if context.terminal_reply_copied {
                fail("ny_vessel_terminal_reply_was_copied");
            }
            // SAFETY: Both values were written once immediately before IRETQ
            // and remain private until this exact terminal completion.
            let saved_stack = unsafe { ptr::read_volatile(ptr::addr_of!(saved_kernel_stack)) };
            // SAFETY: The same single-BSP publication contract protects TS.
            let original_task_switched =
                unsafe { ptr::read_volatile(ptr::addr_of!(saved_task_switched)) };
            if saved_stack == 0
                || saved_stack & (mem::align_of::<u64>() as u64 - 1) != 0
                || !matches!(original_task_switched, 0 | cr0_task_switched)
                || paging::read_cr0() & cr0_task_switched == 0
            {
                fail("ny_vessel_completion_continuation_invalid");
            }
            gate_active.store(false, Ordering::Release);
            active_context.store(ptr::null_mut(), Ordering::Release);
            // SAFETY: The terminal task is exited and its success reply was not
            // copied. This single-use trampoline abandons the user/TSS frames,
            // restores CR0.TS, and resumes the saved kernel stack.
            unsafe {
                x86_64_resume_ny_vessel_task(status_completed, saved_stack, original_task_switched)
            }
        }
        Err(reason) => fail(reason),
    }
}

enum gate_action {
    return_to_user(rite_reply),
    complete,
}

fn valid_gate_frame(context: &vessel_context, frame: &rite_gate_frame) -> bool {
    let Some(task) = context.current_task else {
        return false;
    };
    let Ok(snapshot) = context.tasks.snapshot(task) else {
        return false;
    };
    let Some(frame_end) = (frame as *const rite_gate_frame as usize as u64)
        .checked_add(mem::size_of::<rite_gate_frame>() as u64)
    else {
        return false;
    };
    // SAFETY: IF is clear and NMI remains masked for this single-BSP proof.
    let selected_stack_top = unsafe { descriptors::selected_privilege_stack_top() };
    let bounds = snapshot.bounds();
    snapshot.identity() == task
        && snapshot.state() == task_state::running
        && frame.vector == rite_gate_vector
        && frame.error_code == 0
        && frame.code_segment == u64::from(descriptors::user_code_selector)
        && frame.stack_segment == u64::from(descriptors::user_data_selector)
        && frame.flags == sanitized_flags
        && bounds.contains_code(frame.instruction_pointer)
        && expected_gate_instruction_pointer(context) == Some(frame.instruction_pointer)
        && bounds.contains_stack_pointer(frame.stack_pointer)
        && frame.accumulator == 0
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
        && frame.register_15 == mailbox_address
        && context.mailbox_alias != 0
        && context.mailbox_alias & (page_size - 1) == 0
        && context
            .mailbox_alias
            .checked_add(page_size)
            .is_some_and(|end| end <= memory::address_limit)
        && context.active_root != 0
        && paging::entry_address(paging::read_cr3()) == context.active_root
        && context.transition_stack_top == selected_stack_top
        && frame_end == selected_stack_top
        && maskable_interrupts_are_disabled()
}

/// The vessel's own resume address for the submission in flight, as the build
/// stage stated it. A gate taken from anywhere else in the image is a vessel
/// that is not doing what was inspected.
fn expected_gate_instruction_pointer(context: &vessel_context) -> Option<u64> {
    match context.phase {
        gate_phase::console_write => Some(context.header.console_return),
        gate_phase::task_exit => Some(context.header.exit_return),
        gate_phase::idle => None,
    }
}

fn dispatch_gate(context: &mut vessel_context) -> Result<gate_action, &'static str> {
    let phase = context.phase;
    let index = phase.index().ok_or("ny_vessel_gate_idle_phase")?;
    if context.observation_count != index {
        return Err("ny_vessel_observation_order_invalid");
    }
    // SAFETY: The active task space retains exclusive ownership of its live,
    // identity-mapped mailbox alias through this gate dispatch.
    let call = unsafe { read_call(context.mailbox_alias) }?;
    validate_vessel_call(phase, &call)?;

    let before_copies = context.body_copy_count;
    let completion = {
        let channel = context
            .channel
            .as_mut()
            .ok_or("ny_vessel_channel_not_live")?;
        let decision = channel
            .assess(
                &context.tasks,
                context.current_task.ok_or("ny_vessel_task_missing")?,
                call,
            )
            .map_err(|_| "ny_vessel_assessment_failed")?;
        match decision {
            // Every call this vessel makes is one its grant covers, so a
            // rejection means the vessel or the grant is not what was
            // inspected, not that a negative case is being exercised here.
            rite_decision::rejected(_) => return Err("ny_vessel_call_rejected"),
            rite_decision::admitted(admission) => complete_admitted(
                phase,
                admission,
                &mut context.tasks,
                &mut context.screen,
                context.mailbox_alias,
                &mut context.scratch,
                &mut context.console_copy,
                &mut context.body_copy_count,
                &mut context.body_bytes_copied,
                &mut context.console_bytes_committed,
                &mut context.exit_code,
            )?,
        }
    };
    let (reply, terminal) = completion;
    if context.body_copy_count != index + 1
        || context.body_copy_count < before_copies
        || !scratch_is_zero(&context.scratch)
    {
        return Err("ny_vessel_body_copy_order_invalid");
    }
    validate_reply(phase, reply)?;

    context.observations[index] = vessel_observation {
        sequence: reply.sequence,
        status: reply.status,
    };
    context.observation_count += 1;

    if terminal {
        if phase != gate_phase::task_exit
            || context
                .tasks
                .snapshot(context.current_task.ok_or("ny_vessel_task_missing")?)
                .map_err(|_| "ny_vessel_terminal_snapshot_failed")?
                .state()
                != task_state::exited
        {
            return Err("ny_vessel_terminal_state_invalid");
        }
        context.phase = gate_phase::idle;
        Ok(gate_action::complete)
    } else {
        context.phase = phase
            .next()
            .ok_or("ny_vessel_phase_advanced_past_terminal")?;
        Ok(gate_action::return_to_user(reply))
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_admitted(
    phase: gate_phase,
    admission: rite_admission<'_>,
    tasks: &mut task_table,
    screen: &mut screen_console,
    mailbox: u64,
    scratch: &mut rite_scratch,
    console_copy: &mut [u8; expected_console_text.len()],
    body_copy_count: &mut usize,
    body_bytes_copied: &mut usize,
    console_bytes_committed: &mut usize,
    exit_code: &mut Option<i32>,
) -> Result<(rite_reply, bool), &'static str> {
    let vessel = admission.vessel();
    match phase {
        gate_phase::idle => Err("ny_vessel_unexpected_admission"),
        gate_phase::console_write => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for the vessel's mailbox page.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let length = usize::try_from(admission.payload_size())
                .map_err(|_| "ny_vessel_console_length_invalid")?;
            let text = core::str::from_utf8(&scratch[..length])
                .map_err(|_| "ny_vessel_console_utf8_invalid")?;
            if text.as_bytes() != expected_console_text
                || !text.bytes().all(|byte| (b' '..=b'~').contains(&byte))
                || length != console_copy.len()
            {
                return Err("ny_vessel_console_body_invalid");
            }
            let reply = admission
                .finish_ok(tasks, |_, exact_vessel| {
                    if exact_vessel != vessel {
                        fail("ny_vessel_console_identity_changed");
                    }
                    console_copy.copy_from_slice(text.as_bytes());
                    if screen.write_str(text).is_err() {
                        fail("ny_vessel_console_commit_failed");
                    }
                    *console_bytes_committed = length;
                })
                .map_err(|_| "ny_vessel_console_completion_failed")?;
            if !clear_and_verify_bytes(scratch) {
                return Err("ny_vessel_console_scratch_clear_failed");
            }
            Ok((reply, false))
        }
        gate_phase::task_exit => {
            // SAFETY: `mailbox` is the active task space's exclusively owned,
            // identity-mapped alias for the vessel's mailbox page.
            unsafe {
                copy_admitted_body(
                    mailbox,
                    mailbox_address,
                    admission.payload_size(),
                    scratch,
                    body_copy_count,
                    body_bytes_copied,
                )
            }?;
            let payload = decode_exit_payload(scratch)?;
            // The code is Ny's own return value, carried through the vessel's
            // private cell because no register survives a submission.
            if !payload.has_supported_body() || payload.code != expected_exit_code {
                return Err("ny_vessel_exit_body_invalid");
            }
            let reply = admission
                .finish_ok(tasks, |tasks, exact_vessel| {
                    if exact_vessel != vessel || tasks.exit(exact_vessel).is_err() {
                        fail("ny_vessel_task_exit_commit_failed");
                    }
                })
                .map_err(|_| "ny_vessel_task_exit_completion_failed")?;
            *exit_code = Some(payload.code);
            if !clear_and_verify_bytes(scratch) {
                return Err("ny_vessel_terminal_scratch_clear_failed");
            }
            Ok((reply, true))
        }
    }
}

fn validate_vessel_call(phase: gate_phase, call: &rite_call) -> Result<(), &'static str> {
    let trace = phase.trace_word().ok_or("ny_vessel_trace_missing")?;
    let mut expected = [0u8; 16];
    expected[..8].copy_from_slice(&trace.to_le_bytes());
    expected[8..].copy_from_slice(&trace.to_le_bytes());
    let expected_kind = match phase {
        gate_phase::console_write => rite_payload_kind::utf8_text,
        gate_phase::task_exit => rite_payload_kind::exit_status,
        gate_phase::idle => return Err("ny_vessel_idle_call"),
    };
    let expected_size = match phase {
        gate_phase::console_write => expected_console_text.len() as u32,
        gate_phase::task_exit => mem::size_of::<u64>() as u32,
        gate_phase::idle => return Err("ny_vessel_idle_call"),
    };
    if call.header_size as usize != call_size
        || call.operation != phase.operation().ok_or("ny_vessel_operation_missing")?
        || call.payload_kind != expected_kind
        || call.reserved != 0
        || call.payload_size != expected_size
        || call.payload_limit != expected_size
        || call.trace_id != id_128::from_bytes(expected)
    {
        return Err("ny_vessel_call_invalid");
    }
    Ok(())
}

fn validate_reply(phase: gate_phase, reply: rite_reply) -> Result<(), &'static str> {
    let trace = phase.trace_word().ok_or("ny_vessel_trace_missing")?;
    let mut expected = [0u8; 16];
    expected[..8].copy_from_slice(&trace.to_le_bytes());
    expected[8..].copy_from_slice(&trace.to_le_bytes());
    if !reply.has_supported_header()
        || reply.header_size as usize != reply_size
        || reply.status != rite_status::ok
        || reply.payload_kind != rite_payload_kind::none
        || reply.payload_size != 0
        || reply.reserved != 0
        || reply.reserved_2 != 0
        || reply.sequence != phase.sequence().ok_or("ny_vessel_sequence_missing")?
        || reply.trace_id != id_128::from_bytes(expected)
    {
        return Err("ny_vessel_reply_invalid");
    }
    Ok(())
}

fn maskable_interrupts_are_disabled() -> bool {
    let flags: u64;
    // SAFETY: Reading RFLAGS is non-mutating and this proof runs at CPL0.
    unsafe {
        asm!(
            "pushfq",
            "pop {}",
            out(reg) flags,
            options(preserves_flags)
        );
    }
    flags & (1 << 9) == 0
}

fn range_is_zero(range: physical_range, expected_length: u64) -> bool {
    let Some(end) = range.end() else {
        return false;
    };
    if range.length != expected_length
        || range.base & (page_size - 1) != 0
        || end > memory::address_limit
    {
        return false;
    }
    let Ok(base) = usize::try_from(range.base) else {
        return false;
    };
    for offset in 0..expected_length as usize {
        // SAFETY: The allocator extent is identity mapped and remains
        // exclusively borrowed throughout this validation.
        if unsafe { ptr::read_volatile((base as *const u8).add(offset)) } != 0 {
            return false;
        }
    }
    true
}

fn fail(reason: &'static str) -> ! {
    let mut serial = serial_port::com1();
    serial.initialize();
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.ny_vessel.failed","level":"fatal","reason":"{reason}"}}"#
    );
    super::qemu_exit_failure();
    super::halt_forever()
}
