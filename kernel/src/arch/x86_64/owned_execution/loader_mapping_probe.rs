//! Synchronous private mapping preparation through an authenticated live alias.
//! No candidate is published and no owning value may leave this stack unreleased.

use super::*;
use crate::arch::x86_64::dispatcher_runtime::fail;
use crate::arch::x86_64::owned_source::{sealed_source_reader, source_origin};
use crate::arch::x86_64::runtime_mapping::{
    runtime_budget, runtime_mapping, runtime_phase, runtime_plan, runtime_request,
};
use crate::arch::x86_64::runtime_source_mapping::{populate_and_verify, population_observation};
use anomalous_veil_protocol::native_status;
use anomalous_veil_protocol::source_mapping::{page_limit, source_mapping_request};

impl owned_execution_group<3> {
    /// Prepare and cancel a detached source mapping without publishing any root.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked. The genuine source and execution owners,
    /// allocator and disjoint initialized kernel scratch remain live throughout
    /// this synchronous call. No external references to candidate resources are
    /// published. Every returning path has completed explicit candidate cleanup;
    /// a cleanup failure halts with the owner and reader still in this frame.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn probe_loader_mapping(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        context: loader_mapping_context<'_>,
        request: source_mapping_request,
        reader: &sealed_source_reader<'_>,
        scratch: (&mut [u8; 4096], &mut [u8; 4096]),
    ) -> Result<loader_mapping_observation, native_status> {
        if !self.prepared
            || !self.selecting
            || self.finishing
            || self.released
            || super::super::interrupts::interrupts_enabled()
            || !self.access.is_current()
            || !self.current_stack_is_owned()
            || reader.grant() != context.binding.grant()
            || reader.length() != context.binding.length()
            || !matches!(reader.origin(), source_origin::provisioned(grant)
                if grant.get() == context.binding.grant())
            || request.grant() != context.binding.grant()
            || scratch
                .0
                .iter()
                .chain(scratch.1.iter())
                .any(|byte| *byte != 0)
        {
            return Err(native_status::denied);
        }
        let (caller, current_root) = self
            .suspended_caller_index(
                context.tasks,
                context.circles,
                context.scheduler,
                context.authorization,
            )
            .map_err(|_| native_status::denied)?;
        let caller_root = self.root(caller).map_err(|_| native_status::denied)?;
        let tables = self.launches[caller]
            .as_ref()
            .ok_or(native_status::denied)?
            .mapping()
            .table_range()
            .ok_or(native_status::denied)?;
        let before_tree =
            table_fingerprint(&mut self.access, tables).ok_or(native_status::bad_copy)?;
        let before = allocator.accounting();
        let plan = runtime_plan::new(
            self.launches[caller].as_ref().unwrap().mapping().plan(),
            runtime_request {
                virtual_address: request.base(),
                byte_length: request.byte_length(),
                populated_byte_length: request.populated_byte_length(),
                permissions: page_permissions {
                    user: true,
                    writable: false,
                    executable: false,
                },
            },
            runtime_budget {
                virtual_pages: page_limit,
                data_pages: page_limit,
                table_frames: 32,
            },
        )
        .map_err(|_| native_status::invalid)?;
        let geometry = plan.geometry();
        let mut candidate = runtime_mapping::new(plan, reader);
        let mut population = population_observation::default();
        // SAFETY: The genuine reader, sealed suspended caller, exclusive
        // allocator and existing privileged alias remain held. The callback
        // writes only candidate data leases and verifies every intended byte.
        let prepared = unsafe {
            candidate.prepare(allocator, &mut self.access, |mapping, source, memory| {
                populate_and_verify(
                    mapping,
                    source,
                    geometry,
                    memory,
                    scratch.0,
                    scratch.1,
                    &mut population,
                )
            })
        };
        let live = allocator.accounting();
        let counts_match = prepared.is_ok()
            && candidate.phase() == runtime_phase::ready
            && candidate.mapping().retained_leases() == 2
            && live.unavailable == before.unavailable
            && live.data_owned == before.data_owned + population.populated_pages as usize
            && live.table_owned == before.table_owned + population.table_frames as usize
            && live.free.checked_add(
                population.populated_pages as usize + population.table_frames as usize,
            ) == Some(before.free)
            && live.active_leases == before.active_leases + 2
            && live.bound_leases == before.bound_leases + 2;
        // No fallible return may bypass this cleanup: failed preparation owns
        // every partial lease. One explicit retry uses the same retained owner.
        let mut cleanup_retries = 0;
        if unsafe { candidate.cleanup(allocator, &mut self.access) }.is_err() {
            cleanup_retries = 1;
            if unsafe { candidate.cleanup(allocator, &mut self.access) }.is_err() {
                fail("loader_mapping_cleanup_retained");
            }
        }
        if candidate.phase() != runtime_phase::released
            || candidate.mapping().retained_leases() != 0
            || allocator.accounting() != before
            || self.root(caller) != Ok(caller_root)
            || paging::entry_address(paging::read_cr3()) != current_root
            || table_fingerprint(&mut self.access, tables) != Some(before_tree)
            || self.suspended_caller_index(
                context.tasks,
                context.circles,
                context.scheduler,
                context.authorization,
            ) != Ok((caller, current_root))
            || scratch
                .0
                .iter()
                .chain(scratch.1.iter())
                .any(|byte| *byte != 0)
        {
            fail("loader_mapping_cancel_effect");
        }
        if prepared.is_err() {
            return Err(native_status::exhausted);
        }
        if !counts_match
            || population.source_bytes != request.populated_byte_length()
            || (population.source_bytes == reader.length()
                && population.source_fnv64 != context.binding.fnv64())
            || population.destination_fnv64 != population.source_fnv64
        {
            fail("loader_mapping_population_accounting");
        }
        Ok(loader_mapping_observation {
            population,
            virtual_address: request.base(),
            byte_length: request.byte_length(),
            virtual_pages: geometry.page_count(),
            table_fingerprint: before_tree,
            cleanup_retries,
        })
    }
}

/// Inert diagnostic fingerprint, not an authority token. Hash every entry bit:
/// the suspended caller's owned table arena is inactive on the sole BSP, and
/// shared alias descendants outside that arena are not included.
pub(super) fn table_fingerprint(
    access: &mut bootstrap_physical_access_group<3>,
    tables: physical_range,
) -> Option<u64> {
    if tables.length == 0 || !tables.length.is_multiple_of(page_size) {
        return None;
    }
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for page in 0..tables.length / page_size {
        for index in 0..512 {
            // SAFETY: The caller retains this complete table lease; the live
            // alias checks the current root and each bounded entry address.
            let value = unsafe { access.read_entry(tables.base + page * page_size, index) }?;
            for byte in value.to_le_bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    Some(hash)
}
