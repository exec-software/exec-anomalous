//! One retained source owner and one caller subtree, admitted for hardware only
//! by an exact creation-bound reply transaction.

use super::*;
use crate::arch::x86_64::dispatcher_runtime::fail;
use crate::arch::x86_64::interrupts;
use crate::arch::x86_64::linux_loader_source::source_controller;
use crate::arch::x86_64::owned_mapping::pdpt_graft::pdpt_graft;
use crate::arch::x86_64::owned_source::source_origin;
use crate::arch::x86_64::runtime_mapping::{
    publication_result, runtime_budget, runtime_mapping, runtime_phase, runtime_plan,
    runtime_request,
};
use crate::arch::x86_64::runtime_source_mapping::populate_and_verify;
use crate::memory::physical::frame_accounting;
use anomalous_veil_protocol::native_status;
use anomalous_veil_protocol::source_mapping::{page_limit, source_mapping_request};

pub(in crate::arch::x86_64) struct lifetime_context<'a> {
    pub tasks: &'a task_table,
    pub scheduler: &'a scheduler_table,
    pub authorization: &'a scheduler_call_authorization,
    pub binding: anomalous_veil_protocol::source_read::source_binding,
    pub sequence: u64,
    pub incarnation: u64,
}

/// A single-use kernel ticket minted while the creation call is still sealed.
/// It carries no userspace authority and cannot be constructed outside this module.
#[must_use = "commit only after the exact prepared reply and frame are committed"]
pub(in crate::arch::x86_64) struct loader_mapping_resume {
    caller: task_identity,
    root: u64,
    sequence: u64,
    incarnation: u64,
    correlation: [u64; 4],
    result: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum resume_phase {
    blocked,
    admitted,
    committed,
    revoked,
}

pub(super) struct retained_mapping {
    owner: runtime_mapping<source_controller>,
    graft: Option<pdpt_graft>,
    caller: task_identity,
    veil: task_identity,
    sequence: u64,
    incarnation: u64,
    correlation: [u64; 4],
    binding: anomalous_veil_protocol::source_read::source_binding,
    caller_index: usize,
    caller_root: u64,
    before: frame_accounting,
    live: frame_accounting,
    observation: loader_mapping_observation,
    original_address: u64,
    original_bytes: [u8; 16],
    pub(super) circle_attached: bool,
    inspected: bool,
    resume: resume_phase,
    verified_fingerprint: u64,
}

impl retained_mapping {
    pub(super) fn belongs_to(&self, caller: task_identity) -> bool {
        self.caller == caller
    }

    pub(super) fn attached_to(&self, caller: task_identity, index: usize, root: u64) -> bool {
        self.circle_attached
            && self.caller == caller
            && self.caller_index == index
            && self.caller_root == root
            && self.owner.phase() == runtime_phase::published
            && self.graft.is_some()
    }

    pub(super) fn controls_selection(
        &self,
        index: usize,
        task: Option<task_identity>,
        root: u64,
    ) -> bool {
        self.caller_index == index || task == Some(self.caller) || self.caller_root == root
    }

    pub(super) fn resume_committed(&self) -> bool {
        self.resume == resume_phase::committed
            && self
                .graft
                .as_ref()
                .is_some_and(pdpt_graft::hardware_admitted)
    }

    pub(super) fn mapping(&self) -> &super::super::owned_mapping::staged_mapping {
        self.owner.mapping()
    }

    pub(super) fn graft(&self) -> Option<&pdpt_graft> {
        self.graft.as_ref().filter(|_| self.circle_attached)
    }
}

impl owned_execution_group<3> {
    pub(in crate::arch::x86_64) fn has_loader_mapping(&self) -> bool {
        self.loader_mapping.is_some()
    }

    pub(in crate::arch::x86_64) fn loader_mapping_resumed(&self) -> bool {
        self.loader_mapping
            .as_ref()
            .is_some_and(retained_mapping::resume_committed)
    }

    fn lifetime_index(
        &self,
        context: &lifetime_context<'_>,
        circles: &circle_table,
    ) -> Result<(usize, u64), native_status> {
        let result = self
            .suspended_caller_index(
                context.tasks,
                circles,
                context.scheduler,
                context.authorization,
            )
            .map_err(|_| native_status::denied)?;
        let slot = self.loader_mapping.as_ref().ok_or(native_status::invalid)?;
        if slot.caller != context.authorization.caller()
            || slot.veil != context.authorization.veil()
            || slot.correlation != context.authorization.correlation_words()
            || slot.sequence != context.sequence
            || slot.incarnation != context.incarnation
            || slot.binding != context.binding
            || slot.caller_index != result.0
            || self.root(result.0) != Ok(slot.caller_root)
            || !slot.owner.backing().is_ready()
        {
            return Err(native_status::denied);
        }
        Ok(result)
    }

    /// # Safety
    /// Sole BSP, IF clear/NMI masked; actual source and execution owners, live
    /// allocator and disjoint BSS scratch remain retained. No dispatch occurs
    /// here. Any partial owner is stored before physical mutation and retained
    /// through cleanup failure; the caller cannot select Linux while it exists.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn retain_loader_mapping(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        context: lifetime_context<'_>,
        circles: &mut circle_table,
        request: source_mapping_request,
        source: &mut Option<source_controller>,
        scratch: (&mut [u8; 4096], &mut [u8; 4096]),
    ) -> Result<loader_mapping_observation, native_status> {
        if self.loader_mapping.is_some()
            || !self.access.is_current()
            || !self.current_stack_is_owned()
            || context.sequence == 0
            || context.incarnation == 0
            || request.sequence() != context.sequence
            || request.grant() != context.binding.grant()
            || scratch.0.iter().chain(scratch.1.iter()).any(|b| *b != 0)
        {
            return Err(native_status::invalid);
        }
        let (caller, _) = self
            .suspended_caller_index(
                context.tasks,
                circles,
                context.scheduler,
                context.authorization,
            )
            .map_err(|_| native_status::denied)?;
        let backing = source.as_ref().ok_or(native_status::invalid)?;
        let reader = backing.reader().map_err(|_| native_status::denied)?;
        if reader.grant() != context.binding.grant()
            || reader.length() != context.binding.length()
            || !matches!(reader.origin(), source_origin::provisioned(grant)
                if grant.get() == context.binding.grant())
        {
            return Err(native_status::denied);
        }
        let mapping = self.launches[caller].as_ref().unwrap().mapping();
        let original_address = mapping.plan().regions().next().unwrap().extent().start();
        let plan = runtime_plan::new(
            mapping.plan(),
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
        // This adapter reserves one otherwise empty 1-GiB PDPT window only.
        if plan.geometry().start() != 0x1_0000_0000 || plan.geometry().end() > 0x1_4000_0000 {
            return Err(native_status::invalid);
        }
        let tables = mapping.table_range().ok_or(native_status::denied)?;
        let before_tree = super::loader_mapping_probe::table_fingerprint(&mut self.access, tables)
            .ok_or(native_status::bad_copy)?;
        // SAFETY: The complete inactive caller and allocator ownership remain
        // live. Keep an A/D-normalized diagnostic in addition to the old raw
        // fingerprint; exact tree verification always remains authoritative.
        let verified_fingerprint =
            unsafe { mapping.verified_inactive_fingerprint(allocator, &mut self.access) }
                .map_err(|_| native_status::bad_copy)?;
        let mut original_bytes = [0; 16];
        self.copy_from_suspended_caller(
            context.tasks,
            circles,
            context.scheduler,
            context.authorization,
            original_address,
            &mut original_bytes,
        )
        .map_err(|_| native_status::bad_copy)?;
        let before = allocator.accounting();
        let geometry = plan.geometry();
        let caller_root = self.root(caller).map_err(|_| native_status::denied)?;
        self.loader_mapping = Some(retained_mapping {
            owner: runtime_mapping::new(plan, source.take().unwrap()),
            graft: None,
            caller: context.authorization.caller(),
            veil: context.authorization.veil(),
            sequence: context.sequence,
            incarnation: context.incarnation,
            correlation: context.authorization.correlation_words(),
            binding: context.binding,
            caller_index: caller,
            caller_root,
            before,
            live: before,
            observation: loader_mapping_observation::default(),
            original_address,
            original_bytes,
            circle_attached: false,
            inspected: false,
            resume: resume_phase::blocked,
            verified_fingerprint,
        });
        let slot = self.loader_mapping.as_mut().unwrap();
        let population = &mut slot.observation.population;
        // SAFETY: The actual source is now owned by the BSS transaction. Its
        // short reader and allocator/access borrows all end before native return.
        let prepared = unsafe {
            slot.owner
                .prepare(allocator, &mut self.access, |mapping, source, memory| {
                    let Ok(reader) = source.reader() else {
                        return false;
                    };
                    populate_and_verify(
                        mapping, &reader, geometry, memory, scratch.0, scratch.1, population,
                    )
                })
        };
        slot.live = allocator.accounting();
        if prepared.is_err() {
            // SAFETY: No reference escaped; cleanup uses this same BSS owner.
            unsafe { self.release_loader_mapping_storage(allocator, circles, source) };
            return Err(native_status::exhausted);
        }
        if slot.live.unavailable != before.unavailable
            || slot.live.data_owned != before.data_owned + population.populated_pages as usize
            || slot.live.table_owned != before.table_owned + population.table_frames as usize
            || slot
                .live
                .free
                .checked_add((population.populated_pages + population.table_frames) as usize)
                != Some(before.free)
            || slot.live.active_leases != before.active_leases + 2
            || slot.live.bound_leases != before.bound_leases + 2
            || (geometry.populated_byte_length() == context.binding.length()
                && (population.source_fnv64 != context.binding.fnv64()
                    || population.destination_fnv64 != context.binding.fnv64()))
        {
            fail("loader_retained_mapping_accounting");
        }
        slot.observation.virtual_address = geometry.start();
        slot.observation.byte_length = geometry.byte_length();
        slot.observation.virtual_pages = geometry.page_count();
        slot.observation.table_fingerprint = before_tree;
        Ok(slot.observation)
    }

    /// # Safety
    /// Same retained sole-BSP interval as preparation. Caller remains blocked;
    /// no root selection, reply or other consumer may escape publication.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn publish_loader_mapping(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        context: lifetime_context<'_>,
        circles: &mut circle_table,
    ) -> Result<loader_mapping_observation, native_status> {
        let (caller, _) = self.lifetime_index(&context, circles)?;
        let slot = self.loader_mapping.as_mut().unwrap();
        if slot.owner.phase() != runtime_phase::ready
            || slot.graft.is_some()
            || slot.circle_attached
        {
            return Err(native_status::invalid);
        }
        if allocator.accounting() != slot.live {
            fail("loader_publish_accounting");
        }
        let base = self.launches[caller].as_ref().unwrap().mapping();
        // SAFETY: Both inactive owners and the authentic allocator are retained.
        let graft = unsafe {
            pdpt_graft::prepare(
                base,
                slot.owner.mapping(),
                slot.owner.geometry().end(),
                allocator,
                &mut self.access,
            )
        }
        .map_err(|_| native_status::invalid)?;
        circles
            .attach_child(
                slot.caller.circle(),
                crate::circle::circle_child_kind::mapping,
            )
            .map_err(|_| native_status::exhausted)?;
        slot.circle_attached = true;
        slot.graft = Some(graft);
        // SAFETY: All fallible ownership preflight preceded the external store.
        // An ambiguous adapter result fail-stops with both owners retained;
        // the runtime owner's uncertain phase is set before this callback.
        unsafe {
            slot.owner.publish(|candidate, _, _| {
                if slot
                    .graft
                    .as_mut()
                    .unwrap()
                    .publish(base, candidate, allocator, &mut self.access)
                    .is_err()
                {
                    fail("loader_publication_retained");
                }
                publication_result::published
            })
        }
        .unwrap_or_else(|_| fail("loader_publication_phase"));
        if allocator.accounting() != slot.live {
            fail("loader_publish_effect");
        }
        Ok(slot.observation)
    }

    /// Authenticate and admit the one mapping result while the creation
    /// request is still sealed. This never selects the caller or candidate root.
    ///
    /// # Safety
    /// Retain the exact pending scheduler call, saved frame, both mapping
    /// owners, source and allocator bindings without dispatch or mutation until
    /// the prepared reply and returned ticket commit or the caller is contained.
    /// Sole BSP, IF clear/NMI masked; caller and candidate roots are inactive.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn prepare_loader_mapping_resume(
        &mut self,
        allocator: &frame_allocator<'_>,
        context: lifetime_context<'_>,
        circles: &circle_table,
        result: u64,
    ) -> Result<loader_mapping_resume, native_status> {
        let (caller, _) = self.lifetime_index(&context, circles)?;
        let slot = self.loader_mapping.as_mut().unwrap();
        if slot.resume != resume_phase::blocked
            || !slot.inspected
            || !slot.circle_attached
            || slot.owner.phase() != runtime_phase::published
            || result != slot.owner.geometry().start()
            || allocator.accounting() != slot.live
        {
            return Err(native_status::denied);
        }
        let base = self.launches[caller].as_ref().unwrap().mapping();
        let graft = slot.graft.as_mut().ok_or(native_status::denied)?;
        // SAFETY: The sealed creation call proves the caller is blocked; its
        // exact owners, source, frame and independent current veil are retained.
        let root = unsafe {
            graft.admit_hardware_use(base, slot.owner.mapping(), allocator, &mut self.access)
        }
        .map_err(|_| native_status::denied)?;
        if root != slot.caller_root {
            fail("loader_resume_admitted_root");
        }
        slot.resume = resume_phase::admitted;
        Ok(loader_mapping_resume {
            caller: slot.caller,
            root,
            sequence: slot.sequence,
            incarnation: slot.incarnation,
            correlation: slot.correlation,
            result,
        })
    }

    /// Consume the ticket only after the prepared scheduler reply and exact
    /// caller result frame have committed. A failure is terminal: the caller
    /// must never be selected and all owners remain retained for containment.
    pub(in crate::arch::x86_64) fn commit_loader_mapping_resume(
        &mut self,
        ticket: loader_mapping_resume,
    ) -> Result<(), native_status> {
        let slot = self.loader_mapping.as_mut().ok_or(native_status::denied)?;
        if slot.resume != resume_phase::admitted
            || !slot.circle_attached
            || slot.owner.phase() != runtime_phase::published
            || slot.caller != ticket.caller
            || slot.caller_root != ticket.root
            || slot.sequence != ticket.sequence
            || slot.incarnation != ticket.incarnation
            || slot.correlation != ticket.correlation
            || slot.owner.geometry().start() != ticket.result
            || !slot
                .graft
                .as_ref()
                .is_some_and(pdpt_graft::hardware_admitted)
        {
            return Err(native_status::denied);
        }
        slot.resume = resume_phase::committed;
        Ok(())
    }

    /// # Safety
    /// Every caller consumer is blocked or terminal, current root is independent,
    /// no PCID/global translations/SMP, IF clear/NMI masked. On any ambiguous
    /// withdrawal or repeated cleanup error retain the entire BSS owner forever.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn release_loader_mapping_storage(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        circles: &mut circle_table,
        source: &mut Option<source_controller>,
    ) -> loader_mapping_observation {
        let slot = self
            .loader_mapping
            .as_mut()
            .unwrap_or_else(|| fail("loader_release_missing"));
        if source.is_some()
            || !self.access.is_current()
            || interrupts::interrupts_enabled()
            || paging::entry_address(paging::read_cr3()) == slot.caller_root
        {
            fail("loader_release_authority");
        }
        // Revocation precedes the first detach. Neither a stale ticket nor a
        // saved continuation may re-select this caller while release proceeds.
        slot.resume = resume_phase::revoked;
        let base = self.launches[slot.caller_index].as_ref().unwrap().mapping();
        let hardware_admitted = slot
            .graft
            .as_ref()
            .is_some_and(pdpt_graft::hardware_admitted);
        if let Some(graft) = slot.graft.as_mut() {
            // SAFETY: Caller cannot execute; exact external parent is removed
            // before the current-root reload flushes all non-global translations.
            unsafe { graft.detach(base, slot.owner.mapping(), &mut self.access) }
                .unwrap_or_else(|_| fail("loader_withdraw_retained"));
            // SAFETY: The complete original tree must be restored before any
            // candidate lease is freed. This exact owner/entry verification is
            // authoritative; the retained fingerprint is diagnostic only.
            if unsafe { base.verified_inactive_fingerprint(allocator, &mut self.access) }
                .unwrap_or_else(|_| fail("loader_withdraw_full_tree"))
                != slot.verified_fingerprint
            {
                fail("loader_withdraw_normalized_tree");
            }
            let tables = base
                .table_range()
                .unwrap_or_else(|| fail("loader_base_tables"));
            if !hardware_admitted
                && super::loader_mapping_probe::table_fingerprint(&mut self.access, tables)
                    != Some(slot.observation.table_fingerprint)
            {
                fail("loader_withdraw_exact_tree");
            }
            unsafe { paging::write_cr3(paging::read_cr3()) };
            if !self.access.is_current() {
                fail("loader_withdraw_flush");
            }
            unsafe { graft.confirm_invalidation(base, slot.owner.mapping(), &mut self.access) }
                .unwrap_or_else(|_| fail("loader_invalidation_retained"));
            unsafe { slot.owner.confirm_withdrawal() }
                .unwrap_or_else(|_| fail("loader_withdraw_phase"));
        }
        // SAFETY: No external reference or cached consumer remains. The same
        // owner retains partial cleanup progress for one bounded explicit retry.
        if unsafe { slot.owner.cleanup(allocator, &mut self.access) }.is_err() {
            slot.observation.cleanup_retries += 1;
            if unsafe { slot.owner.cleanup(allocator, &mut self.access) }.is_err() {
                fail("loader_retained_cleanup");
            }
        }
        if slot.circle_attached {
            circles
                .detach_child(
                    slot.caller.circle(),
                    crate::circle::circle_child_kind::mapping,
                )
                .unwrap_or_else(|_| fail("loader_mapping_detach_circle"));
            slot.circle_attached = false;
        }
        if allocator.accounting() != slot.before || slot.owner.mapping().retained_leases() != 0 {
            fail("loader_released_accounting");
        }
        let tables = base
            .table_range()
            .unwrap_or_else(|| fail("loader_base_tables"));
        if unsafe { base.verified_inactive_fingerprint(allocator, &mut self.access) }
            .unwrap_or_else(|_| fail("loader_released_full_tree"))
            != slot.verified_fingerprint
        {
            fail("loader_released_normalized_tree");
        }
        if !hardware_admitted
            && slot.observation.table_fingerprint != 0
            && super::loader_mapping_probe::table_fingerprint(&mut self.access, tables)
                != Some(slot.observation.table_fingerprint)
        {
            fail("loader_withdraw_base_tree");
        }
        let slot = self.loader_mapping.take().unwrap();
        let observation = slot.observation;
        *source = Some(
            slot.owner
                .into_backing()
                .unwrap_or_else(|_| fail("loader_source_recovery")),
        );
        observation
    }

    pub(in crate::arch::x86_64) fn loader_mapping_inspected(&self) -> bool {
        self.loader_mapping
            .as_ref()
            .is_some_and(|slot| slot.inspected)
    }

    /// Read the published prefix through the ordinary authenticated user-copy
    /// entry point, compare every byte against a fresh reader of the owned
    /// source, and exercise both original mappings and denied boundary copies.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked; caller remains suspended. Both scratch
    /// buffers are disjoint retained kernel storage, never user-accessible.
    #[inline(never)]
    pub(in crate::arch::x86_64) unsafe fn inspect_loader_mapping(
        &mut self,
        context: lifetime_context<'_>,
        circles: &circle_table,
        scratch: (&mut [u8; 4096], &mut [u8; 4096]),
    ) -> Result<loader_mapping_observation, native_status> {
        self.lifetime_index(&context, circles)?;
        let slot = self.loader_mapping.as_ref().unwrap();
        if slot.owner.phase() != runtime_phase::published || slot.inspected {
            return Err(native_status::invalid);
        }
        let geometry = slot.owner.geometry();
        let original_address = slot.original_address;
        let original_bytes = slot.original_bytes;
        let mut observed = [0; 16];
        let result = (|| {
            self.copy_from_suspended_caller(
                context.tasks,
                circles,
                context.scheduler,
                context.authorization,
                original_address,
                &mut observed,
            )
            .ok()?;
            if observed != original_bytes {
                return None;
            }
            let mut offset = 0;
            while offset < geometry.populated_byte_length() {
                let count = (geometry.populated_byte_length() - offset).min(4096) as usize;
                let source = self
                    .loader_mapping
                    .as_ref()?
                    .owner
                    .backing()
                    .reader()
                    .ok()?;
                // SAFETY: Source ownership, access and disjoint scratch survive
                // this lexical read. No reader survives the following copy.
                unsafe { source.read(offset, &mut scratch.0[..count], &mut self.access) }.ok()?;
                self.copy_from_suspended_caller(
                    context.tasks,
                    circles,
                    context.scheduler,
                    context.authorization,
                    geometry.start() + offset,
                    &mut scratch.1[..count],
                )
                .ok()?;
                if scratch.0[..count] != scratch.1[..count] {
                    return None;
                }
                offset += count as u64;
            }
            let zero = geometry.populated_end() - geometry.start() - offset;
            if zero != 0 {
                self.copy_from_suspended_caller(
                    context.tasks,
                    circles,
                    context.scheduler,
                    context.authorization,
                    geometry.start() + offset,
                    &mut scratch.1[..zero as usize],
                )
                .ok()?;
                if scratch.1[..zero as usize].iter().any(|b| *b != 0) {
                    return None;
                }
            }
            if self.preflight_to_suspended_caller(
                context.tasks,
                circles,
                context.scheduler,
                context.authorization,
                geometry.start(),
                1,
            ) != Err(user_copy_error::permission_denied)
            {
                return None;
            }
            let mut address = geometry.populated_end();
            while address < geometry.end() {
                if self.copy_from_suspended_caller(
                    context.tasks,
                    circles,
                    context.scheduler,
                    context.authorization,
                    address,
                    &mut observed[..1],
                ) != Err(user_copy_error::unmapped)
                {
                    return None;
                }
                address += page_size;
            }
            for address in [geometry.start() - 1, geometry.end()] {
                if self.copy_from_suspended_caller(
                    context.tasks,
                    circles,
                    context.scheduler,
                    context.authorization,
                    address,
                    &mut observed[..1],
                ) != Err(user_copy_error::unmapped)
                {
                    return None;
                }
            }
            Some(())
        })();
        for byte in scratch.0.iter_mut().chain(scratch.1.iter_mut()) {
            // SAFETY: Both initialized BSS buffers remain exclusively borrowed.
            unsafe { core::ptr::write_volatile(byte, 0) };
        }
        if result.is_none() {
            fail("loader_composed_copy_inspection");
        }
        self.lifetime_index(&context, circles)?;
        let slot = self.loader_mapping.as_mut().unwrap();
        slot.inspected = true;
        Ok(slot.observation)
    }

    pub(in crate::arch::x86_64) fn authenticate_loader_mapping(
        &self,
        context: lifetime_context<'_>,
        circles: &circle_table,
    ) -> Result<(), native_status> {
        self.lifetime_index(&context, circles).map(|_| ())
    }
}
