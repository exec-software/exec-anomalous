//! Native producer-to-consumer proof over retained, sealed source owners.
//! These fixtures are neither ELF inputs nor a stable userspace upload ABI.

use core::fmt::Write;

use super::owned_execution::owned_execution;
use super::owned_source::{owned_source, source_budget, source_error, source_memory};
use super::physical_access::bootstrap_physical_access;
use super::scheduler_test::scheduler_frame;
use crate::circle::circle_table;
use crate::console::serial_port;
use crate::identity::task_identity;
use crate::memory::physical::{frame_accounting, frame_allocator, frame_error, physical_range};
use crate::scheduler::scheduler_table;
use crate::task::task_table;

unsafe extern "C" {
    static x86_64_owned_multi_region_faulting_start: u8;
    static x86_64_owned_source_faulting_request_0_return: u8;
    static x86_64_owned_source_faulting_request_1_return: u8;
    static x86_64_owned_source_faulting_request_2_return: u8;
    static x86_64_owned_source_faulting_request_3_return: u8;
    static x86_64_owned_source_faulting_request_4_return: u8;
    static x86_64_owned_source_faulting_request_5_return: u8;
    static x86_64_owned_source_faulting_request_6_return: u8;
    static x86_64_owned_source_faulting_request_7_return: u8;
    static x86_64_owned_source_faulting_request_8_return: u8;
    static x86_64_owned_source_faulting_request_9_return: u8;
    static x86_64_owned_multi_region_peer_start: u8;
    static x86_64_owned_source_peer_request_0_return: u8;
    static x86_64_owned_source_peer_request_1_return: u8;
    static x86_64_owned_source_peer_request_2_return: u8;
    static x86_64_owned_source_peer_request_3_return: u8;
    static x86_64_owned_source_peer_request_4_return: u8;
    static x86_64_owned_source_peer_request_5_return: u8;
    static x86_64_owned_source_peer_request_6_return: u8;
    static x86_64_owned_source_peer_request_7_return: u8;
    static x86_64_owned_source_peer_request_8_return: u8;
    static x86_64_owned_source_peer_request_9_return: u8;
}

// Numeric linker markers never confer source or copy authority. They qualify
// the exact proof call site after the real current-task admission succeeds.
fn request_site(selected: usize, step: u8) -> Option<u64> {
    macro_rules! address {
        ($symbol:ident) => {{
            let result: u64;
            // SAFETY: Materialize an immutable linked marker, without reading it.
            unsafe { core::arch::asm!("lea {out}, [rip + {symbol}]",
                out = out(reg) result, symbol = sym $symbol,
                options(nomem, nostack, preserves_flags)); }
            result
        }};
    }
    let start = match selected {
        0 => address!(x86_64_owned_multi_region_faulting_start),
        1 => address!(x86_64_owned_multi_region_peer_start),
        _ => return None,
    };
    let site = match (selected, step) {
        (0, 0) => address!(x86_64_owned_source_faulting_request_0_return),
        (0, 1) => address!(x86_64_owned_source_faulting_request_1_return),
        (0, 2) => address!(x86_64_owned_source_faulting_request_2_return),
        (0, 3) => address!(x86_64_owned_source_faulting_request_3_return),
        (0, 4) => address!(x86_64_owned_source_faulting_request_4_return),
        (0, 5) => address!(x86_64_owned_source_faulting_request_5_return),
        (0, 6) => address!(x86_64_owned_source_faulting_request_6_return),
        (0, 7) => address!(x86_64_owned_source_faulting_request_7_return),
        (0, 8) => address!(x86_64_owned_source_faulting_request_8_return),
        (0, 9) => address!(x86_64_owned_source_faulting_request_9_return),
        (1, 0) => address!(x86_64_owned_source_peer_request_0_return),
        (1, 1) => address!(x86_64_owned_source_peer_request_1_return),
        (1, 2) => address!(x86_64_owned_source_peer_request_2_return),
        (1, 3) => address!(x86_64_owned_source_peer_request_3_return),
        (1, 4) => address!(x86_64_owned_source_peer_request_4_return),
        (1, 5) => address!(x86_64_owned_source_peer_request_5_return),
        (1, 6) => address!(x86_64_owned_source_peer_request_6_return),
        (1, 7) => address!(x86_64_owned_source_peer_request_7_return),
        (1, 8) => address!(x86_64_owned_source_peer_request_8_return),
        (1, 9) => address!(x86_64_owned_source_peer_request_9_return),
        _ => return None,
    };
    0x60001000_u64.checked_add(site.checked_sub(start)?)
}

pub(super) const snapshot_length: u64 = 7048;
pub(super) const pattern_address: u64 = 0x6003b003;
pub(super) const pattern_length: usize = 5000;
pub(super) const mailbox_phase: u64 = 32;
pub(super) const mailbox_verified: u64 = 40;

pub(super) static pattern: [u8; pattern_length] = {
    let mut bytes = [0; pattern_length];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = (index as u8).wrapping_mul(17).wrapping_add(3);
        index += 1;
    }
    bytes
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum source_phase {
    producer,
    consumer,
}

impl source_phase {
    pub(super) const fn word(self) -> u64 {
        match self {
            Self::producer => 0,
            Self::consumer => 1,
        }
    }
}

/// This outer owner outlives both dispatch frames and their physical aliases.
/// Only the currently published runtime may borrow it while userspace runs.
pub(super) struct source_set {
    owners: [Option<owned_source>; 2],
    steps: [u8; 2],
    producer_verified: u8,
    consumer_verified: u8,
}

impl source_set {
    pub(super) const fn new() -> Self {
        Self {
            owners: [None, None],
            steps: [0; 2],
            producer_verified: 0,
            consumer_verified: 0,
        }
    }

    /// # Safety
    /// Committed producer identities, exclusive bootstrap ownership and the
    /// execution owner's live physical alias must remain valid throughout.
    pub(super) unsafe fn allocate(
        &mut self,
        execution: &mut owned_execution,
        allocator: &mut frame_allocator<'_>,
        identities: [task_identity; 2],
    ) -> Result<(), &'static str> {
        if self.owners.iter().any(Option::is_some) {
            return Err("owned_source_allocate_state");
        }
        for (index, identity) in identities.into_iter().enumerate() {
            self.owners[index] = Some(
                owned_source::new(
                    identity,
                    0x5100 + index as u64,
                    snapshot_length,
                    source_budget {
                        max_bytes: 8192,
                        max_pages: 2,
                    },
                )
                .map_err(|_| "owned_source_binding")?,
            );
            let owner = self.owners[index].as_mut().ok_or("owned_source_missing")?;
            // SAFETY: The caller retains both the source and physical owner on
            // every error; no user instruction or callback runs during setup.
            unsafe { execution.source_allocate(owner, allocator) }?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn request(
        &mut self,
        phase: source_phase,
        selected: usize,
        execution: &mut owned_execution,
        tasks: &task_table,
        circles: &circle_table,
        scheduler: &scheduler_table,
        identity: task_identity,
        frame: scheduler_frame,
    ) -> Result<u64, &'static str> {
        if selected >= 2 {
            return Err("owned_source_request_phase");
        }
        // A fresh task table can reuse numeric task slots. Phase admission is
        // therefore mandatory: copied uploader/grant metadata cannot reopen a
        // sealed source from the subsequent consumer lifetime.
        if phase == source_phase::consumer {
            return Ok(1);
        }
        let step = self.steps[selected];
        let grant = 0x5100 + selected as u64;
        let (operation, expected_grant, address, length, offset, rejected) = match step {
            0 => (11, grant, 0, 0, 0, true),
            1 => (10, grant + 1, 0x40002000, 1024, 0, true),
            2 => (10, grant, 0x40002000, 1024, 0, false),
            3 => (10, grant, 0x40002000, 1024, 0, true),
            4 => (10, grant, 0x60001000, 1024, 1024, false),
            5 => (10, grant, 0x6003efff, 2, 2048, true),
            6 => (10, grant, pattern_address, 4096, 2048, false),
            7 => (10, grant, pattern_address + 4096, 904, 6144, false),
            8 => (11, grant, 0, 0, 0, false),
            9 => (10, grant, 0x40002000, 1024, 0, true),
            _ => return Err("owned_source_request_order"),
        };
        if Some(frame.instruction_pointer) != request_site(selected, step)
            || frame.accumulator != operation
            || frame.destination_index != expected_grant
            || frame.source_index != address
            || frame.data != length
            || frame.register_8 != offset
        {
            return Err("owned_source_request_operands");
        }
        let owner = self.owners[selected]
            .as_mut()
            .ok_or("owned_source_missing")?;
        let before = owner.received();
        let result = if operation == 10 {
            execution.source_append(
                owner,
                tasks,
                circles,
                scheduler,
                identity,
                expected_grant,
                offset,
                address,
                length as usize,
            )
        } else {
            execution.source_seal(owner, tasks, circles, scheduler, identity, expected_grant)
        };
        if result.is_err() != rejected
            || (rejected && owner.received() != before)
            || (!rejected && operation == 10 && owner.received() != before + length)
            || (step >= 8 && !owner.is_sealed())
        {
            return Err("owned_source_request_outcome");
        }
        self.steps[selected] += 1;
        Ok(u64::from(rejected))
    }

    pub(super) fn ready(
        &mut self,
        phase: source_phase,
        selected: usize,
        execution: &mut owned_execution,
    ) -> Result<(), &'static str> {
        let owner = self
            .owners
            .get(selected)
            .and_then(Option::as_ref)
            .ok_or("owned_source_missing")?;
        if !owner.is_sealed()
            || owner.received() != snapshot_length
            || self.steps[selected] != 10
            || execution.multi_read(selected, mailbox_phase) != Some(phase.word())
            || execution.multi_read(selected, mailbox_verified) != Some(1)
        {
            return Err("owned_source_ready_state");
        }
        // Independently inspect the retained snapshot after userspace has
        // overwritten its old pages. This small transition-stack buffer never
        // holds a pointer into either source storage or the producer mapping.
        let mut observed = [0_u8; 64];
        let mut offset = 0;
        while offset < pattern_length {
            let length = observed.len().min(pattern_length - offset);
            execution.source_observe(owner, 2048 + offset as u64, &mut observed[..length])?;
            if observed[..length] != pattern[offset..offset + length] {
                return Err("owned_source_pattern_readback");
            }
            offset += length;
        }
        let mask = 1 << selected;
        let verified = match phase {
            source_phase::producer => &mut self.producer_verified,
            source_phase::consumer => &mut self.consumer_verified,
        };
        if *verified & mask != 0 {
            return Err("owned_source_repeated_ready");
        }
        *verified |= mask;
        Ok(())
    }

    pub(super) fn complete(&self, phase: source_phase) -> bool {
        self.steps == [10, 10]
            && self.producer_verified == 3
            && (phase == source_phase::producer || self.consumer_verified == 3)
            && self
                .owners
                .iter()
                .all(|owner| owner.as_ref().is_some_and(owned_source::is_sealed))
    }
}

pub(super) fn retained_accounting(mut initial: frame_accounting) -> Option<frame_accounting> {
    initial.free = initial.free.checked_sub(4)?;
    initial.data_owned = initial.data_owned.checked_add(4)?;
    initial.active_leases = initial.active_leases.checked_add(2)?;
    initial.bound_leases = initial.bound_leases.checked_add(2)?;
    Some(initial)
}

/// # Safety
/// Exclusive bootstrap preparation; the immutable source borrow persists until
/// population finishes and no slice into physical storage is ever constructed.
#[inline(never)]
pub(super) unsafe fn prepare_consumer(
    execution: &mut owned_execution,
    allocator: &mut frame_allocator<'_>,
    circles: &mut circle_table,
    sources: &source_set,
    main_lengths: [u64; 2],
    interpreter_lengths: [u64; 2],
) -> Result<(), &'static str> {
    use super::mapping_plan::{mapping_budget, mapping_request, reserved_request};
    use super::owned_source_plan::{launch_input, owned_source_image, source_span};
    use crate::task::{executable_interval, executable_interval_set};
    if !sources.complete(source_phase::producer) {
        return Err("owned_source_consumer_premature");
    }
    let requests = [
        (0x40000000, 2, false, false),
        (0x40002000, 4, false, true),
        (0x40006000, 1, false, false),
        (0x40007000, 2, true, false),
        (0x60000000, 1, false, false),
        (0x60001000, 47, false, true),
        (0x60030000, 11, false, false),
        (0x6003b000, 4, true, false),
        (0x7fff00000000, 16, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: super::paging::page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    let reservations = [(0, 512), (0x7ffefffff000, 1), (0x7fff00010000, 1)].map(
        |(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        },
    );
    let spans = [
        source_span {
            destination: 0x40002000,
            source_offset: 0,
            length: 1024,
        },
        source_span {
            destination: 0x60001000,
            source_offset: 1024,
            length: 1024,
        },
        source_span {
            destination: pattern_address,
            source_offset: 2048,
            length: pattern_length as u64,
        },
    ];
    let make_image = |index: usize| {
        let executable = executable_interval_set::new(&[
            executable_interval {
                start: 0x40002037,
                end: 0x40002000 + main_lengths[index],
            },
            executable_interval {
                start: 0x60001020,
                end: 0x60001000 + interpreter_lengths[index],
            },
        ])
        .map_err(|_| "owned_source_consumer_intervals")?;
        let reader = sources.owners[index]
            .as_ref()
            .ok_or("owned_source_missing")?
            .reader()
            .map_err(|_| "owned_source_reader")?;
        owned_source_image::new(
            launch_input {
                regions: &requests,
                reservations: &reservations,
                budget: mapping_budget {
                    data_pages: 88,
                    table_frames: 32,
                },
                spans: &spans,
                executable,
                entry: 0x60001020,
                stack_region: 8,
                stack_pointer: 0x7fff00010000,
            },
            reader,
        )
        .map_err(|_| "owned_source_consumer_plan")
    };
    let images = [make_image(0)?, make_image(1)?];
    // SAFETY: Forward the retained ownership and exclusive bootstrap contract.
    unsafe { execution.prepare_owned_sources(allocator, circles, images) }
}

struct refuse_clear<'a> {
    access: &'a mut bootstrap_physical_access,
}

// SAFETY: Reads/writes delegate to the same checked physical owner. The sole
// injected clear fails without modifying storage or claiming verified zeroing.
unsafe impl source_memory for refuse_clear<'_> {
    unsafe fn clear_and_verify(&mut self, _range: physical_range) -> bool {
        false
    }
    unsafe fn write(&mut self, range: physical_range, bytes: &[u8]) -> bool {
        unsafe { source_memory::write(self.access, range, bytes) }
    }
    unsafe fn read(&mut self, range: physical_range, bytes: &mut [u8]) -> bool {
        unsafe { source_memory::read(self.access, range, bytes) }
    }
}

/// # Safety
/// Both dispatch lifetimes have finished completely. The sole BSP holds IF
/// clear, NMI masked and the unchanged Threshold root with its alias removed.
#[inline(never)]
pub(super) unsafe fn cleanup(
    sources: &mut source_set,
    allocator: &mut frame_allocator<'_>,
    initial: frame_accounting,
    abort: fn(&'static str) -> !,
) -> Result<(), &'static str> {
    if !sources.complete(source_phase::consumer)
        || retained_accounting(initial) != Some(allocator.accounting())
    {
        return Err("owned_source_cleanup_state");
    }
    let mut access = bootstrap_physical_access::new();
    let result = (|| {
        // SAFETY: Exclusive restored bootstrap hierarchy; retain on every failure.
        unsafe { access.install() }.map_err(|_| "owned_source_cleanup_alias")?;
        for owner in &mut sources.owners {
            let source = owner.as_mut().ok_or("owned_source_missing")?;
            let before = allocator.accounting();
            let range = source.retained_range();
            // SAFETY: No reader or user root survives; deliberately retain failure.
            let failed = unsafe {
                source.cleanup(
                    allocator,
                    &mut refuse_clear {
                        access: &mut access,
                    },
                )
            };
            let mut expected = before;
            expected.bound_leases = expected
                .bound_leases
                .checked_sub(1)
                .ok_or("owned_source_cleanup_accounting")?;
            if failed != Err(source_error::frame(frame_error::clearing_failed))
                || allocator.accounting() != expected
                || source.retained_range() != range
                || source.reader().is_ok()
                || source.is_released()
            {
                return Err("owned_source_cleanup_retention");
            }
            // SAFETY: Retry the same retained unbound lease through the live alias.
            unsafe { source.cleanup(allocator, &mut access) }
                .map_err(|_| "owned_source_cleanup_retry")?;
            if !source.is_released() || source.retained_range().is_some() {
                return Err("owned_source_cleanup_owner");
            }
        }
        // SAFETY: Every source lease is cleared/released and no borrower exists.
        unsafe { access.remove() }.map_err(|_| "owned_source_cleanup_alias_remove")?;
        if allocator.accounting() != initial {
            return Err("owned_source_cleanup_restore");
        }
        Ok(())
    })();
    if let Err(reason) = result {
        // Retain the live alias owner in this scope if an unexpected failure
        // prevents complete retirement; no caller may continue past it.
        abort(reason);
    }
    Ok(())
}

pub(super) fn emit_evidence(initial: frame_accounting) {
    let mut serial = serial_port::com1();
    for record in [
        r#"{"schema":1,"event":"kernel.owned_source_test.rollback","level":"info","unpublished_cancellations":2,"stack_clear_failures":2,"cleanup_retries":2,"resources_restored":true}"#,
        r#"{"schema":1,"event":"kernel.owned_source_test.producer","level":"info","tasks":2,"sources":2,"source_bytes":7048,"chunks_per_source":4,"denials_per_task":5,"uploaded_bytes":14096,"original_bytes_overwritten":10000,"original_zero_verified":true}"#,
        r#"{"schema":1,"event":"kernel.owned_source_test.retained","level":"info","data_frames":4,"active_leases":2,"bound_leases":2,"producer_resources_released":true,"producer_identities_retired":true,"alias_removed":true}"#,
        r#"{"schema":1,"event":"kernel.owned_source_test.consumer","level":"info","tasks":2,"regions_per_task":9,"executable_regions":2,"initialized_readback":true,"pattern_bytes_per_task":5000,"pattern_verified":true,"sealed_only":true}"#,
        r#"{"schema":1,"event":"kernel.owned_source_test.progress","level":"info","lifetimes":2,"preemptions":128,"task_quanta_per_lifetime":32,"canaries_intact":true,"noninitial_stacks":true,"cross_region_gates":4}"#,
        r#"{"schema":1,"event":"kernel.owned_source_test.fault_contained","level":"info","faults":2,"vector":6,"survivor_ticks_per_lifetime":16,"peers_continued":true}"#,
    ] {
        let _ = writeln!(serial, "{record}");
    }
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_source_test.released","level":"info","free_frames":{},"resources_restored":true,"source_clear_failures":2,"source_cleanup_retries":2,"storage_cleared":true,"kernel_continuation_restored":true,"bootstrap_restored":true,"tss_restored":true,"ts_restored":true,"alias_removed":true}}"#,
        initial.free
    );
    let _ = writeln!(
        serial,
        r#"{{"schema":1,"event":"kernel.owned_source_test.passed","level":"info","timer_ticks":160,"dispatches":132,"preemptions":128,"faults":2,"exits":2,"sources":2}}"#
    );
}
