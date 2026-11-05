#![cfg_attr(not(test), allow(dead_code))]

//! Fixed-capacity, architecture-neutral task-local authority bindings.
//!
//! A user-visible [`sigil_reference`] names one slot in the current task's
//! binding namespace. It is not a global object identity, pointer, bearer
//! secret, or authenticated principal. Every authority operation re-reads the
//! kernel task table and resolves the local reference through a kernel-only
//! record and lineage.

use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use crate::circle::{circle_child_kind, circle_error, circle_table};
use crate::identity::{
    endpoint_id, fixture_principal_id, lineage_id, next_generation, sigil_id, task_identity,
};
use crate::task::{task_capacity, task_error, task_snapshot, task_state, task_table};

pub(crate) const sigil_reference_tag: u32 = u32::from_le_bytes(*b"SIG1");
pub(crate) const sigil_binding_capacity: usize = 4;
pub(crate) const sigil_record_capacity: usize = 16;
pub(crate) const sigil_lineage_capacity: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct sigil_rights(u16);

impl sigil_rights {
    pub(crate) const none: Self = Self(0);
    pub(crate) const send: Self = Self(1 << 0);
    pub(crate) const receive: Self = Self(1 << 1);
    pub(crate) const delegate: Self = Self(1 << 2);
    pub(crate) const revoke: Self = Self(1 << 3);
    const known_mask: u16 = Self::send.0 | Self::receive.0 | Self::delegate.0 | Self::revoke.0;

    pub(crate) const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    pub(crate) const fn bits(self) -> u16 {
        self.0
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }

    pub(crate) const fn is_known(self) -> bool {
        self.0 & !Self::known_mask == 0
    }

    pub(crate) const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Hostile/user-shaped reference bits. Resolution is always task-local.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(crate) struct sigil_reference {
    tag: u32,
    local_slot: u32,
    binding_generation: u64,
}

impl sigil_reference {
    pub(crate) const fn from_raw(tag: u32, local_slot: u32, binding_generation: u64) -> Self {
        Self {
            tag,
            local_slot,
            binding_generation,
        }
    }

    const fn issued(local_slot: usize, binding_generation: u64) -> Self {
        Self::from_raw(sigil_reference_tag, local_slot as u32, binding_generation)
    }

    pub(crate) const fn tag(self) -> u32 {
        self.tag
    }

    pub(crate) const fn local_slot(self) -> u32 {
        self.local_slot
    }

    pub(crate) const fn binding_generation(self) -> u64 {
        self.binding_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum sigil_error {
    invalid_reference,
    stale_reference,
    revoked,
    endpoint_mismatch,
    insufficient_rights,
    invalid_rights,
    task_not_running,
    target_not_live,
    binding_capacity_exhausted,
    binding_generation_exhausted,
    sigil_capacity_exhausted,
    sigil_generation_exhausted,
    lineage_capacity_exhausted,
    lineage_generation_exhausted,
    circle_accounting_mismatch,
    task(task_error),
    circle(circle_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum sigil_revocation {
    revoked { bindings: usize },
    already_revoked { bindings: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct sigil_withdrawal {
    bindings: usize,
    revoked_lineages: usize,
}

impl sigil_withdrawal {
    pub(crate) const fn bindings(self) -> usize {
        self.bindings
    }

    pub(crate) const fn revoked_lineages(self) -> usize {
        self.revoked_lineages
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct sigil_accounting {
    pub bindings: usize,
    pub records: usize,
    pub lineages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct sigil_holder {
    identity: task_identity,
    principal: fixture_principal_id,
}

impl sigil_holder {
    fn from_snapshot(snapshot: task_snapshot) -> Self {
        Self {
            identity: snapshot.identity(),
            principal: snapshot.principal(),
        }
    }
}

// Storage records use only integer fields. Consequently an all-zero byte
// pattern is valid while volatile privacy clearing is in progress.
#[derive(Clone, Copy)]
#[repr(C)]
struct binding_slot {
    generation: u64,
    active: u64,
    task_slot: u64,
    task_generation: u64,
    circle_slot: u64,
    circle_generation: u64,
    principal: u64,
    sigil_slot: u64,
    sigil_generation: u64,
}

impl binding_slot {
    const fn empty() -> Self {
        Self {
            generation: 0,
            active: 0,
            task_slot: 0,
            task_generation: 0,
            circle_slot: 0,
            circle_generation: 0,
            principal: 0,
            sigil_slot: 0,
            sigil_generation: 0,
        }
    }

    fn install(&mut self, generation: u64, holder: sigil_holder, sigil: sigil_id) {
        *self = Self {
            generation,
            active: 1,
            task_slot: holder.identity.task().slot() as u64,
            task_generation: holder.identity.task().generation_value(),
            circle_slot: holder.identity.circle().slot() as u64,
            circle_generation: holder.identity.circle().generation_value(),
            principal: holder.principal.value(),
            sigil_slot: sigil.slot() as u64,
            sigil_generation: sigil.generation_value(),
        };
    }

    fn holder(&self) -> Option<sigil_holder> {
        if self.active != 1 {
            return None;
        }
        let task = crate::identity::task_id::try_from_parts(
            usize::try_from(self.task_slot).ok()?,
            self.task_generation,
        )?;
        let circle = crate::identity::circle_id::try_from_parts(
            usize::try_from(self.circle_slot).ok()?,
            self.circle_generation,
        )?;
        Some(sigil_holder {
            identity: task_identity::new(task, circle),
            principal: fixture_principal_id::new(self.principal)?,
        })
    }

    fn sigil(&self) -> Option<sigil_id> {
        if self.active != 1 {
            return None;
        }
        sigil_id::try_from_parts(
            usize::try_from(self.sigil_slot).ok()?,
            self.sigil_generation,
        )
    }

    fn clear_preserving_generation(&mut self) -> bool {
        let generation = self.generation;
        volatile_tombstone_and_verify(self, generation)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct sigil_slot {
    generation: u64,
    active: u64,
    task_slot: u64,
    task_generation: u64,
    circle_slot: u64,
    circle_generation: u64,
    principal: u64,
    endpoint_slot: u64,
    endpoint_generation: u64,
    rights: u64,
    lineage_slot: u64,
    lineage_generation: u64,
    lineage_root: u64,
}

impl sigil_slot {
    const fn empty() -> Self {
        Self {
            generation: 0,
            active: 0,
            task_slot: 0,
            task_generation: 0,
            circle_slot: 0,
            circle_generation: 0,
            principal: 0,
            endpoint_slot: 0,
            endpoint_generation: 0,
            rights: 0,
            lineage_slot: 0,
            lineage_generation: 0,
            lineage_root: 0,
        }
    }

    fn install(
        &mut self,
        generation: u64,
        holder: sigil_holder,
        endpoint: endpoint_id,
        rights: sigil_rights,
        lineage: lineage_id,
        lineage_root: bool,
    ) {
        *self = Self {
            generation,
            active: 1,
            task_slot: holder.identity.task().slot() as u64,
            task_generation: holder.identity.task().generation_value(),
            circle_slot: holder.identity.circle().slot() as u64,
            circle_generation: holder.identity.circle().generation_value(),
            principal: holder.principal.value(),
            endpoint_slot: endpoint.slot() as u64,
            endpoint_generation: endpoint.generation_value(),
            rights: u64::from(rights.bits()),
            lineage_slot: lineage.slot() as u64,
            lineage_generation: lineage.generation_value(),
            lineage_root: u64::from(lineage_root),
        };
    }

    fn holder(&self) -> Option<sigil_holder> {
        if self.active != 1 {
            return None;
        }
        let task = crate::identity::task_id::try_from_parts(
            usize::try_from(self.task_slot).ok()?,
            self.task_generation,
        )?;
        let circle = crate::identity::circle_id::try_from_parts(
            usize::try_from(self.circle_slot).ok()?,
            self.circle_generation,
        )?;
        Some(sigil_holder {
            identity: task_identity::new(task, circle),
            principal: fixture_principal_id::new(self.principal)?,
        })
    }

    fn set_holder(&mut self, holder: sigil_holder) {
        self.task_slot = holder.identity.task().slot() as u64;
        self.task_generation = holder.identity.task().generation_value();
        self.circle_slot = holder.identity.circle().slot() as u64;
        self.circle_generation = holder.identity.circle().generation_value();
        self.principal = holder.principal.value();
    }

    fn endpoint(&self) -> Option<endpoint_id> {
        endpoint_id::try_from_parts(
            usize::try_from(self.endpoint_slot).ok()?,
            self.endpoint_generation,
        )
    }

    fn rights(&self) -> Option<sigil_rights> {
        let bits = u16::try_from(self.rights).ok()?;
        Some(sigil_rights::from_bits(bits))
    }

    fn lineage(&self) -> Option<lineage_id> {
        lineage_id::try_from_parts(
            usize::try_from(self.lineage_slot).ok()?,
            self.lineage_generation,
        )
    }

    fn is_root(&self) -> bool {
        self.lineage_root == 1
    }

    fn clear_preserving_generation(&mut self) -> bool {
        let generation = self.generation;
        volatile_tombstone_and_verify(self, generation)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct lineage_slot {
    generation: u64,
    occupied: u64,
    active: u64,
}

impl lineage_slot {
    const fn empty() -> Self {
        Self {
            generation: 0,
            occupied: 0,
            active: 0,
        }
    }

    fn clear_preserving_generation(&mut self) -> bool {
        let generation = self.generation;
        volatile_tombstone_and_verify(self, generation)
    }
}

struct resolved_sigil {
    binding_slot: usize,
    sigil_slot: usize,
    lineage_slot: usize,
    rights: sigil_rights,
}

/// Fixed table used by the control proof.
pub(crate) struct sigil_table<
    const binding_capacity: usize,
    const record_capacity: usize,
    const lineage_capacity: usize,
> {
    bindings: [[binding_slot; binding_capacity]; task_capacity],
    records: [sigil_slot; record_capacity],
    lineages: [lineage_slot; lineage_capacity],
}

pub(crate) type control_sigil_table =
    sigil_table<sigil_binding_capacity, sigil_record_capacity, sigil_lineage_capacity>;

impl<const binding_capacity: usize, const record_capacity: usize, const lineage_capacity: usize>
    sigil_table<binding_capacity, record_capacity, lineage_capacity>
{
    pub(crate) const fn new() -> Self {
        Self {
            bindings: [[binding_slot::empty(); binding_capacity]; task_capacity],
            records: [sigil_slot::empty(); record_capacity],
            lineages: [lineage_slot::empty(); lineage_capacity],
        }
    }

    /// Kernel-bootstrap-only root issuance. The crate-private type is not a
    /// user-callable authority surface.
    pub(crate) fn issue_root(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        owner: task_identity,
        endpoint: endpoint_id,
        rights: sigil_rights,
    ) -> Result<sigil_reference, sigil_error> {
        validate_rights(rights)?;
        let holder = self.live_holder(tasks, owner, false)?;
        let (binding_slot, binding_generation) = self.next_binding(owner)?;
        let (record_slot, record_generation) = self.next_record()?;
        let (lineage_slot, lineage_generation) = self.next_lineage()?;

        circles
            .attach_child(owner.circle(), circle_child_kind::sigil)
            .map_err(sigil_error::circle)?;

        let sigil = sigil_id::from_parts(record_slot, record_generation);
        let lineage = lineage_id::from_parts(lineage_slot, lineage_generation);
        self.lineages[lineage_slot] = lineage_slot_record(lineage_generation.get());
        self.records[record_slot].install(
            record_generation.get(),
            holder,
            endpoint,
            rights,
            lineage,
            true,
        );
        self.bindings[owner.task().slot()][binding_slot].install(
            binding_generation.get(),
            holder,
            sigil,
        );
        Ok(sigil_reference::issued(
            binding_slot,
            binding_generation.get(),
        ))
    }

    pub(crate) fn authorize_endpoint(
        &self,
        tasks: &task_table,
        caller: task_identity,
        reference: sigil_reference,
        endpoint: endpoint_id,
        required: sigil_rights,
    ) -> Result<(), sigil_error> {
        validate_rights(required)?;
        let holder = self.live_holder(tasks, caller, true)?;
        let resolved = self.resolve(holder, reference, endpoint, false)?;
        if !resolved.rights.contains(required) {
            return Err(sigil_error::insufficient_rights);
        }
        Ok(())
    }

    /// Move one binding without creating a second authority record.
    pub(crate) fn transfer(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        sender: task_identity,
        source: sigil_reference,
        target: task_identity,
        endpoint: endpoint_id,
    ) -> Result<sigil_reference, sigil_error> {
        let sender_holder = self.live_holder(tasks, sender, true)?;
        let target_holder = self.live_holder(tasks, target, false)?;
        let resolved = self.resolve(sender_holder, source, endpoint, false)?;
        if !resolved.rights.contains(sigil_rights::delegate) {
            return Err(sigil_error::insufficient_rights);
        }
        let (target_slot, target_generation) = self.next_binding(target)?;
        let sigil = sigil_id::try_from_parts(
            resolved.sigil_slot,
            self.records[resolved.sigil_slot].generation,
        )
        .ok_or(sigil_error::stale_reference)?;

        self.move_circle_binding(circles, sender.circle(), target.circle())?;

        self.bindings[target.task().slot()][target_slot].install(
            target_generation.get(),
            target_holder,
            sigil,
        );
        self.records[resolved.sigil_slot].set_holder(target_holder);
        assert!(
            self.bindings[sender.task().slot()][resolved.binding_slot]
                .clear_preserving_generation(),
            "fatal: transferred sigil source did not clear"
        );
        Ok(sigil_reference::issued(
            target_slot,
            target_generation.get(),
        ))
    }

    /// Derive a new binding in the same lineage with no rights broadening.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attenuate(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        sender: task_identity,
        source: sigil_reference,
        target: task_identity,
        endpoint: endpoint_id,
        requested: sigil_rights,
    ) -> Result<sigil_reference, sigil_error> {
        validate_rights(requested)?;
        let sender_holder = self.live_holder(tasks, sender, true)?;
        let target_holder = self.live_holder(tasks, target, false)?;
        let resolved = self.resolve(sender_holder, source, endpoint, false)?;
        if !resolved.rights.contains(sigil_rights::delegate) || !resolved.rights.contains(requested)
        {
            return Err(sigil_error::insufficient_rights);
        }
        let (target_slot, target_generation) = self.next_binding(target)?;
        let (record_slot, record_generation) = self.next_record()?;

        let lineage_generation = self.lineages[resolved.lineage_slot].generation;
        let lineage = lineage_id::try_from_parts(resolved.lineage_slot, lineage_generation)
            .ok_or(sigil_error::stale_reference)?;

        circles
            .attach_child(target.circle(), circle_child_kind::sigil)
            .map_err(sigil_error::circle)?;

        let sigil = sigil_id::from_parts(record_slot, record_generation);
        self.records[record_slot].install(
            record_generation.get(),
            target_holder,
            endpoint,
            requested,
            lineage,
            false,
        );
        self.bindings[target.task().slot()][target_slot].install(
            target_generation.get(),
            target_holder,
            sigil,
        );
        Ok(sigil_reference::issued(
            target_slot,
            target_generation.get(),
        ))
    }

    pub(crate) fn revoke_lineage(
        &mut self,
        tasks: &task_table,
        caller: task_identity,
        reference: sigil_reference,
        endpoint: endpoint_id,
    ) -> Result<sigil_revocation, sigil_error> {
        let holder = self.live_holder(tasks, caller, true)?;
        let resolved = self.resolve(holder, reference, endpoint, true)?;
        if !resolved.rights.contains(sigil_rights::revoke) {
            return Err(sigil_error::insufficient_rights);
        }
        let bindings = self.bindings_in_lineage(resolved.lineage_slot);
        if self.lineages[resolved.lineage_slot].active == 0 {
            return Ok(sigil_revocation::already_revoked { bindings });
        }
        self.lineages[resolved.lineage_slot].active = 0;
        Ok(sigil_revocation::revoked { bindings })
    }

    /// Withdraw every local binding held by a live task. In the serialized
    /// proof, circle accounting is preflighted before the batch detach, after
    /// which record clearing is infallible except for verified byte erasure.
    pub(crate) fn withdraw_task(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        owner: task_identity,
    ) -> Result<sigil_withdrawal, sigil_error> {
        let holder = sigil_holder::from_snapshot(tasks.snapshot(owner).map_err(sigil_error::task)?);
        let row = owner.task().slot();
        let mut count = 0usize;
        let mut record_slots = [usize::MAX; binding_capacity];
        let mut rooted = [false; lineage_capacity];
        let mut index = 0usize;
        while index < binding_capacity {
            let binding = &self.bindings[row][index];
            if binding.active == 1 {
                if binding.holder() != Some(holder) {
                    return Err(sigil_error::stale_reference);
                }
                let sigil = binding.sigil().ok_or(sigil_error::stale_reference)?;
                let record = self
                    .records
                    .get(sigil.slot())
                    .ok_or(sigil_error::stale_reference)?;
                if record.active != 1
                    || record.generation != sigil.generation_value()
                    || record.holder() != Some(holder)
                    || record.endpoint().is_none()
                    || record
                        .rights()
                        .is_none_or(|rights| !rights.is_known() || rights.is_empty())
                    || record.lineage_root > 1
                    || record_slots[..index].contains(&sigil.slot())
                {
                    return Err(sigil_error::stale_reference);
                }
                let lineage = record.lineage().ok_or(sigil_error::stale_reference)?;
                let lineage_record = self
                    .lineages
                    .get(lineage.slot())
                    .ok_or(sigil_error::stale_reference)?;
                if lineage_record.occupied != 1
                    || lineage_record.active > 1
                    || lineage_record.generation != lineage.generation_value()
                {
                    return Err(sigil_error::stale_reference);
                }
                record_slots[index] = sigil.slot();
                if record.is_root() {
                    rooted[lineage.slot()] = true;
                }
                count += 1;
            } else if binding.active != 0 {
                return Err(sigil_error::stale_reference);
            }
            index += 1;
        }
        let count_u16 =
            u16::try_from(count).map_err(|_| sigil_error::circle_accounting_mismatch)?;
        let circle_count = circles
            .snapshot(owner.circle())
            .map_err(sigil_error::circle)?
            .children()
            .sigils();
        if circle_count != count_u16 {
            return Err(sigil_error::circle_accounting_mismatch);
        }
        circles
            .detach_children(owner.circle(), circle_child_kind::sigil, count_u16)
            .map_err(sigil_error::circle)?;

        let mut revoked_lineages = 0usize;
        for (lineage_slot, rooted_here) in rooted.iter().copied().enumerate() {
            if rooted_here && self.lineages[lineage_slot].active == 1 {
                self.lineages[lineage_slot].active = 0;
                revoked_lineages += 1;
            }
        }

        for (binding_slot, record_slot) in record_slots.iter().copied().enumerate() {
            if record_slot == usize::MAX {
                continue;
            }
            assert!(
                self.records[record_slot].clear_preserving_generation(),
                "fatal: withdrawn sigil record did not clear"
            );
            assert!(
                self.bindings[row][binding_slot].clear_preserving_generation(),
                "fatal: withdrawn sigil binding did not clear"
            );
        }
        self.reclaim_empty_lineages()?;
        Ok(sigil_withdrawal {
            bindings: count,
            revoked_lineages,
        })
    }

    pub(crate) fn accounting(&self) -> sigil_accounting {
        let bindings = self
            .bindings
            .iter()
            .flat_map(|row| row.iter())
            .filter(|slot| slot.active == 1)
            .count();
        let records = self.records.iter().filter(|slot| slot.active == 1).count();
        let lineages = self
            .lineages
            .iter()
            .filter(|slot| slot.occupied == 1)
            .count();
        sigil_accounting {
            bindings,
            records,
            lineages,
        }
    }

    fn live_holder(
        &self,
        tasks: &task_table,
        identity: task_identity,
        require_running: bool,
    ) -> Result<sigil_holder, sigil_error> {
        let snapshot = tasks.snapshot(identity).map_err(sigil_error::task)?;
        if require_running {
            if snapshot.state() != task_state::running {
                return Err(sigil_error::task_not_running);
            }
        } else if !matches!(snapshot.state(), task_state::ready | task_state::running) {
            return Err(sigil_error::target_not_live);
        }
        Ok(sigil_holder::from_snapshot(snapshot))
    }

    fn resolve(
        &self,
        holder: sigil_holder,
        reference: sigil_reference,
        endpoint: endpoint_id,
        allow_revoked: bool,
    ) -> Result<resolved_sigil, sigil_error> {
        if reference.tag != sigil_reference_tag
            || reference.binding_generation == 0
            || usize::try_from(reference.local_slot)
                .ok()
                .filter(|slot| *slot < binding_capacity)
                .is_none()
        {
            return Err(sigil_error::invalid_reference);
        }
        let local_slot = reference.local_slot as usize;
        let binding = &self.bindings[holder.identity.task().slot()][local_slot];
        if binding.active != 1
            || binding.generation != reference.binding_generation
            || binding.holder() != Some(holder)
        {
            return Err(sigil_error::stale_reference);
        }
        let sigil = binding.sigil().ok_or(sigil_error::stale_reference)?;
        let record = self
            .records
            .get(sigil.slot())
            .ok_or(sigil_error::stale_reference)?;
        if record.active != 1
            || record.generation != sigil.generation_value()
            || record.holder() != Some(holder)
        {
            return Err(sigil_error::stale_reference);
        }
        if record.endpoint() != Some(endpoint) {
            return Err(sigil_error::endpoint_mismatch);
        }
        let rights = record.rights().ok_or(sigil_error::stale_reference)?;
        if !rights.is_known() || rights.is_empty() {
            return Err(sigil_error::stale_reference);
        }
        let lineage = record.lineage().ok_or(sigil_error::stale_reference)?;
        let lineage_record = self
            .lineages
            .get(lineage.slot())
            .ok_or(sigil_error::stale_reference)?;
        if lineage_record.occupied != 1
            || lineage_record.active > 1
            || lineage_record.generation != lineage.generation_value()
        {
            return Err(sigil_error::stale_reference);
        }
        if !allow_revoked && lineage_record.active != 1 {
            return Err(sigil_error::revoked);
        }
        Ok(resolved_sigil {
            binding_slot: local_slot,
            sigil_slot: sigil.slot(),
            lineage_slot: lineage.slot(),
            rights,
        })
    }

    fn next_binding(
        &self,
        owner: task_identity,
    ) -> Result<(usize, core::num::NonZeroU64), sigil_error> {
        let row = self
            .bindings
            .get(owner.task().slot())
            .ok_or(sigil_error::stale_reference)?;
        let mut saw_free = false;
        for (slot, binding) in row.iter().enumerate() {
            if binding.active == 1 {
                continue;
            }
            if binding.active != 0 {
                return Err(sigil_error::stale_reference);
            }
            saw_free = true;
            if let Some(generation) = next_generation(binding.generation) {
                return Ok((slot, generation));
            }
        }
        Err(if saw_free {
            sigil_error::binding_generation_exhausted
        } else {
            sigil_error::binding_capacity_exhausted
        })
    }

    fn next_record(&self) -> Result<(usize, core::num::NonZeroU64), sigil_error> {
        let mut saw_free = false;
        for (slot, record) in self.records.iter().enumerate() {
            if record.active == 1 {
                continue;
            }
            if record.active != 0 {
                return Err(sigil_error::stale_reference);
            }
            saw_free = true;
            if let Some(generation) = next_generation(record.generation) {
                return Ok((slot, generation));
            }
        }
        Err(if saw_free {
            sigil_error::sigil_generation_exhausted
        } else {
            sigil_error::sigil_capacity_exhausted
        })
    }

    fn next_lineage(&self) -> Result<(usize, core::num::NonZeroU64), sigil_error> {
        let mut saw_free = false;
        for (slot, lineage) in self.lineages.iter().enumerate() {
            if lineage.occupied == 1 {
                if lineage.active > 1 {
                    return Err(sigil_error::stale_reference);
                }
                continue;
            }
            if lineage.occupied != 0 || lineage.active != 0 {
                return Err(sigil_error::stale_reference);
            }
            saw_free = true;
            if let Some(generation) = next_generation(lineage.generation) {
                return Ok((slot, generation));
            }
        }
        Err(if saw_free {
            sigil_error::lineage_generation_exhausted
        } else {
            sigil_error::lineage_capacity_exhausted
        })
    }

    fn move_circle_binding(
        &mut self,
        circles: &mut circle_table,
        source: crate::identity::circle_id,
        target: crate::identity::circle_id,
    ) -> Result<(), sigil_error> {
        let source_snapshot = circles.snapshot(source).map_err(sigil_error::circle)?;
        if source_snapshot.children().sigils() == 0 {
            return Err(sigil_error::circle_accounting_mismatch);
        }
        if source == target {
            return Ok(());
        }
        let target_snapshot = circles.snapshot(target).map_err(sigil_error::circle)?;
        if target_snapshot.children().sigils() == u16::MAX {
            return Err(sigil_error::circle(circle_error::child_count_exhausted));
        }
        circles
            .attach_child(target, circle_child_kind::sigil)
            .map_err(sigil_error::circle)?;
        if circles
            .detach_child(source, circle_child_kind::sigil)
            .is_err()
        {
            let _ = circles.detach_child(target, circle_child_kind::sigil);
            return Err(sigil_error::circle_accounting_mismatch);
        }
        Ok(())
    }

    fn bindings_in_lineage(&self, lineage_slot: usize) -> usize {
        self.records
            .iter()
            .filter(|record| {
                record.active == 1
                    && record
                        .lineage()
                        .is_some_and(|lineage| lineage.slot() == lineage_slot)
            })
            .count()
    }

    fn reclaim_empty_lineages(&mut self) -> Result<(), sigil_error> {
        for lineage_slot in 0..lineage_capacity {
            if self.lineages[lineage_slot].occupied == 1
                && self.bindings_in_lineage(lineage_slot) == 0
                && !self.lineages[lineage_slot].clear_preserving_generation()
            {
                panic!("fatal: reclaimed sigil lineage did not clear");
            }
        }
        Ok(())
    }
}

fn validate_rights(rights: sigil_rights) -> Result<(), sigil_error> {
    if !rights.is_known() || rights.is_empty() {
        return Err(sigil_error::invalid_rights);
    }
    Ok(())
}

fn lineage_slot_record(generation: u64) -> lineage_slot {
    lineage_slot {
        generation,
        occupied: 1,
        active: 1,
    }
}

fn volatile_zero_and_verify<T>(value: &mut T) -> bool {
    let bytes = (value as *mut T).cast::<u8>();
    for offset in 0..size_of::<T>() {
        // SAFETY: `bytes` covers the complete uniquely borrowed object. All
        // retained table-record types have a valid all-zero representation.
        unsafe {
            ptr::write_volatile(bytes.add(offset), 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
    (0..size_of::<T>()).all(|offset| {
        // SAFETY: the complete object representation was initialized to zero
        // immediately above and remains uniquely borrowed.
        unsafe { ptr::read_volatile(bytes.add(offset)) == 0 }
    })
}

fn volatile_tombstone_and_verify<T>(value: &mut T, generation: u64) -> bool {
    if !volatile_zero_and_verify(value) || size_of::<T>() < size_of::<u64>() {
        return false;
    }
    let bytes = (value as *mut T).cast::<u8>();
    // Every retained record is repr(C) with generation as its first u64.
    unsafe {
        ptr::write_volatile(bytes.cast::<u64>(), generation);
    }
    compiler_fence(Ordering::SeqCst);
    let expected_generation = generation.to_ne_bytes();
    (0..size_of::<T>()).all(|offset| {
        let expected = expected_generation.get(offset).copied().unwrap_or(0);
        // SAFETY: `bytes` spans the live, uniquely borrowed record.
        unsafe { ptr::read_volatile(bytes.add(offset)) == expected }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::circle_table;
    use crate::scheduler::scheduler_table;
    use crate::task::{synthetic_origin_metadata, task_binding, task_bounds};

    fn endpoint(slot: usize, generation: u64) -> endpoint_id {
        endpoint_id::try_from_parts(slot, generation).expect("endpoint fixture")
    }

    fn bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("valid task bounds")
    }

    fn add_task(
        circles: &mut circle_table,
        tasks: &mut task_table,
        principal: u64,
        origin: synthetic_origin_metadata,
    ) -> task_identity {
        let circle = circles.create().expect("circle");
        tasks
            .create(
                circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(principal).expect("principal"),
                    origin,
                    bounds(),
                ),
            )
            .expect("task")
    }

    fn all_rights() -> sigil_rights {
        sigil_rights::send
            .union(sigil_rights::receive)
            .union(sigil_rights::delegate)
            .union(sigil_rights::revoke)
    }

    #[test]
    fn root_reference_is_local_tagged_and_exactly_scoped() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let expected_endpoint = endpoint(0, 1);
        let reference = sigils
            .issue_root(&mut circles, &tasks, owner, expected_endpoint, all_rights())
            .expect("root sigil");

        assert_eq!(reference.tag(), sigil_reference_tag);
        assert_eq!(reference.local_slot(), 0);
        assert_eq!(reference.binding_generation(), 1);
        assert_eq!(
            circles
                .snapshot(owner.circle())
                .expect("circle")
                .children()
                .sigils(),
            1
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                owner,
                reference,
                expected_endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::task_not_running)
        );

        tasks.start(owner).expect("start owner");
        assert!(
            sigils
                .authorize_endpoint(
                    &tasks,
                    owner,
                    reference,
                    expected_endpoint,
                    sigil_rights::send
                )
                .is_ok()
        );
        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, reference, endpoint(1, 1), sigil_rights::send),
            Err(sigil_error::endpoint_mismatch)
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                owner,
                sigil_reference::from_raw(0, 0, 1),
                expected_endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::invalid_reference)
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                owner,
                sigil_reference::from_raw(sigil_reference_tag, 0, 0),
                expected_endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::invalid_reference)
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                owner,
                sigil_reference::from_raw(sigil_reference_tag, sigil_binding_capacity as u32, 1),
                expected_endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::invalid_reference)
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                owner,
                reference,
                expected_endpoint,
                sigil_rights::from_bits(1 << 15)
            ),
            Err(sigil_error::invalid_rights)
        );
    }

    #[test]
    fn copied_reference_and_cached_running_state_confer_no_authority() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let other = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let reference = sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("root");
        tasks.start(owner).expect("start owner");
        tasks.exit(owner).expect("exit owner");

        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, reference, endpoint, sigil_rights::send),
            Err(sigil_error::task_not_running)
        );
        tasks.start(other).expect("start other");
        assert_eq!(
            sigils.authorize_endpoint(&tasks, other, reference, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );
    }

    #[test]
    fn scheduler_states_gate_sigil_targets_and_active_authority() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let target = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let sender = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let exited_target = add_task(
            &mut circles,
            &mut tasks,
            3,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let scoped_endpoint = endpoint(0, 1);
        let mut sigils = control_sigil_table::new();

        let target_reference = sigils
            .issue_root(&mut circles, &tasks, target, scoped_endpoint, all_rights())
            .expect("ready target accepts root authority");
        let source = sigils
            .issue_root(&mut circles, &tasks, sender, scoped_endpoint, all_rights())
            .expect("sender root");
        tasks.start(exited_target).expect("start exiting target");
        tasks.exit(exited_target).expect("exit target");
        let before_exited = (
            sigils.accounting(),
            circles.snapshot(exited_target.circle()).expect("circle"),
        );
        assert_eq!(
            sigils.issue_root(
                &mut circles,
                &tasks,
                exited_target,
                endpoint(1, 1),
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            (
                sigils.accounting(),
                circles.snapshot(exited_target.circle()).expect("circle"),
            ),
            before_exited
        );

        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, target)
            .expect("admit target");
        scheduler
            .admit(&mut circles, &mut tasks, sender)
            .expect("admit sender");
        assert_eq!(
            circles
                .snapshot(target.circle())
                .expect("target circle")
                .children()
                .task_contexts(),
            1
        );
        assert_eq!(
            circles
                .snapshot(sender.circle())
                .expect("sender circle")
                .children()
                .task_contexts(),
            1
        );

        assert_eq!(
            scheduler
                .start_next(&mut tasks)
                .expect("start target")
                .task(),
            target
        );
        sigils
            .issue_root(
                &mut circles,
                &tasks,
                target,
                endpoint(1, 1),
                sigil_rights::send,
            )
            .expect("running target accepts root authority");
        assert!(
            sigils
                .authorize_endpoint(
                    &tasks,
                    target,
                    target_reference,
                    scoped_endpoint,
                    sigil_rights::send,
                )
                .is_ok()
        );

        scheduler
            .preempt_current(&mut tasks, target)
            .expect("suspend target");
        assert_eq!(
            tasks.snapshot(target).expect("target").state(),
            task_state::suspended
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                target,
                target_reference,
                scoped_endpoint,
                sigil_rights::send,
            ),
            Err(sigil_error::task_not_running)
        );
        let before_suspended = (
            sigils.accounting(),
            circles.snapshot(target.circle()).expect("target circle"),
            circles.snapshot(sender.circle()).expect("sender circle"),
        );
        assert_eq!(
            sigils.issue_root(
                &mut circles,
                &tasks,
                target,
                endpoint(2, 1),
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            sigils.transfer(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                scoped_endpoint,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                scoped_endpoint,
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            (
                sigils.accounting(),
                circles.snapshot(target.circle()).expect("target circle"),
                circles.snapshot(sender.circle()).expect("sender circle"),
            ),
            before_suspended
        );
        assert_eq!(
            sigils.transfer(
                &mut circles,
                &tasks,
                sender,
                source,
                exited_target,
                scoped_endpoint,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                exited_target,
                scoped_endpoint,
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );

        scheduler
            .preempt_current(&mut tasks, sender)
            .expect("resume target");
        scheduler
            .fault_current(&mut tasks, target)
            .expect("fault target and continue sender");
        assert_eq!(
            tasks.snapshot(target).expect("target").state(),
            task_state::faulted
        );
        assert_eq!(
            sigils.authorize_endpoint(
                &tasks,
                target,
                target_reference,
                scoped_endpoint,
                sigil_rights::send,
            ),
            Err(sigil_error::task_not_running)
        );
        let before_faulted = (
            sigils.accounting(),
            circles.snapshot(target.circle()).expect("target circle"),
            circles.snapshot(sender.circle()).expect("sender circle"),
        );
        assert_eq!(
            sigils.issue_root(
                &mut circles,
                &tasks,
                target,
                endpoint(2, 1),
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            sigils.transfer(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                scoped_endpoint,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                scoped_endpoint,
                sigil_rights::send,
            ),
            Err(sigil_error::target_not_live)
        );
        assert_eq!(
            (
                sigils.accounting(),
                circles.snapshot(target.circle()).expect("target circle"),
                circles.snapshot(sender.circle()).expect("sender circle"),
            ),
            before_faulted
        );
        assert!(
            sigils
                .authorize_endpoint(&tasks, sender, source, scoped_endpoint, sigil_rights::send,)
                .is_ok()
        );
    }

    #[test]
    fn transfer_is_an_equivalent_move_and_moves_circle_accounting() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let sender = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let target = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::receiver_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let source = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, all_rights())
            .expect("root");
        tasks.start(sender).expect("start sender");
        let before = sigils.accounting();
        let target_reference = sigils
            .transfer(&mut circles, &tasks, sender, source, target, endpoint)
            .expect("move");

        assert_eq!(sigils.accounting(), before);
        assert_eq!(
            circles
                .snapshot(sender.circle())
                .expect("source circle")
                .children()
                .sigils(),
            0
        );
        assert_eq!(
            circles
                .snapshot(target.circle())
                .expect("target circle")
                .children()
                .sigils(),
            1
        );
        assert_eq!(
            sigils.authorize_endpoint(&tasks, sender, source, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );
        tasks.exit(sender).expect("exit sender");
        tasks.start(target).expect("start target");
        assert!(
            sigils
                .authorize_endpoint(
                    &tasks,
                    target,
                    target_reference,
                    endpoint,
                    sigil_rights::receive
                )
                .is_ok()
        );
    }

    #[test]
    fn failed_transfer_to_full_namespace_keeps_source_and_counts() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let sender = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let target = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let source = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, all_rights())
            .expect("root");
        tasks.start(sender).expect("start sender");
        for _ in 0..sigil_binding_capacity {
            sigils
                .attenuate(
                    &mut circles,
                    &tasks,
                    sender,
                    source,
                    target,
                    endpoint,
                    sigil_rights::send,
                )
                .expect("fill target binding");
        }
        let accounting = sigils.accounting();
        let source_count = circles
            .snapshot(sender.circle())
            .expect("source circle")
            .children()
            .sigils();
        let target_count = circles
            .snapshot(target.circle())
            .expect("target circle")
            .children()
            .sigils();

        assert_eq!(
            sigils.transfer(&mut circles, &tasks, sender, source, target, endpoint),
            Err(sigil_error::binding_capacity_exhausted)
        );
        assert_eq!(sigils.accounting(), accounting);
        assert_eq!(
            circles
                .snapshot(sender.circle())
                .expect("source circle")
                .children()
                .sigils(),
            source_count
        );
        assert_eq!(
            circles
                .snapshot(target.circle())
                .expect("target circle")
                .children()
                .sigils(),
            target_count
        );
        assert!(
            sigils
                .authorize_endpoint(&tasks, sender, source, endpoint, sigil_rights::send)
                .is_ok()
        );
    }

    #[test]
    fn attenuation_requires_delegate_known_nonempty_subset_and_is_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let sender = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let target = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let source = sigils
            .issue_root(
                &mut circles,
                &tasks,
                sender,
                endpoint,
                sigil_rights::send.union(sigil_rights::delegate),
            )
            .expect("delegating root");
        let no_delegate = sigils
            .issue_root(&mut circles, &tasks, sender, endpoint, sigil_rights::send)
            .expect("nondelegating root");
        tasks.start(sender).expect("start sender");
        let before = sigils.accounting();

        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                endpoint,
                sigil_rights::none
            ),
            Err(sigil_error::invalid_rights)
        );
        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                endpoint,
                sigil_rights::receive
            ),
            Err(sigil_error::insufficient_rights)
        );
        assert_eq!(
            sigils.attenuate(
                &mut circles,
                &tasks,
                sender,
                no_delegate,
                target,
                endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::insufficient_rights)
        );
        assert_eq!(sigils.accounting(), before);

        let child = sigils
            .attenuate(
                &mut circles,
                &tasks,
                sender,
                source,
                target,
                endpoint,
                sigil_rights::send,
            )
            .expect("attenuated child");
        assert!(
            sigils
                .authorize_endpoint(&tasks, sender, source, endpoint, sigil_rights::send)
                .is_ok()
        );
        tasks.exit(sender).expect("exit sender");
        tasks.start(target).expect("start target");
        assert!(
            sigils
                .authorize_endpoint(&tasks, target, child, endpoint, sigil_rights::send)
                .is_ok()
        );
    }

    #[test]
    fn revocation_is_lineage_wide_and_repeat_is_typed() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let root_task = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let child_task = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let root = sigils
            .issue_root(&mut circles, &tasks, root_task, endpoint, all_rights())
            .expect("root");
        tasks.start(root_task).expect("start root task");
        let child = sigils
            .attenuate(
                &mut circles,
                &tasks,
                root_task,
                root,
                child_task,
                endpoint,
                sigil_rights::send,
            )
            .expect("child");

        assert_eq!(
            sigils.revoke_lineage(&tasks, root_task, root, endpoint),
            Ok(sigil_revocation::revoked { bindings: 2 })
        );
        assert_eq!(
            sigils.revoke_lineage(&tasks, root_task, root, endpoint),
            Ok(sigil_revocation::already_revoked { bindings: 2 })
        );
        assert_eq!(
            sigils.authorize_endpoint(&tasks, root_task, root, endpoint, sigil_rights::send),
            Err(sigil_error::revoked)
        );
        tasks.exit(root_task).expect("exit root task");
        tasks.start(child_task).expect("start child task");
        assert_eq!(
            sigils.authorize_endpoint(&tasks, child_task, child, endpoint, sigil_rights::send),
            Err(sigil_error::revoked)
        );
    }

    #[test]
    fn exited_root_withdrawal_revokes_descendants_and_clears_storage() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let root_task = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let child_task = add_task(
            &mut circles,
            &mut tasks,
            2,
            synthetic_origin_metadata::machine_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let root = sigils
            .issue_root(&mut circles, &tasks, root_task, endpoint, all_rights())
            .expect("root");
        tasks.start(root_task).expect("start root");
        let child = sigils
            .attenuate(
                &mut circles,
                &tasks,
                root_task,
                root,
                child_task,
                endpoint,
                sigil_rights::send,
            )
            .expect("child");
        tasks.exit(root_task).expect("exit root");

        let withdrawal = sigils
            .withdraw_task(&mut circles, &tasks, root_task)
            .expect("withdraw exited root");
        assert_eq!(withdrawal.bindings(), 1);
        assert_eq!(withdrawal.revoked_lineages(), 1);
        assert_eq!(sigils.accounting().bindings, 1);
        assert_eq!(sigils.bindings[root_task.task().slot()][0].active, 0);
        assert_eq!(
            sigils.bindings[root_task.task().slot()][0].task_generation,
            0
        );
        assert_eq!(sigils.records[0].active, 0);
        assert_eq!(sigils.records[0].principal, 0);

        tasks.start(child_task).expect("start child");
        assert_eq!(
            sigils.authorize_endpoint(&tasks, child_task, child, endpoint, sigil_rights::send),
            Err(sigil_error::revoked)
        );
        tasks.exit(child_task).expect("exit child");
        sigils
            .withdraw_task(&mut circles, &tasks, child_task)
            .expect("withdraw child");
        assert_eq!(
            sigils.accounting(),
            sigil_accounting {
                bindings: 0,
                records: 0,
                lineages: 0
            }
        );
        assert_eq!(sigils.lineages[0].occupied, 0);
        assert_eq!(sigils.lineages[0].active, 0);
    }

    #[test]
    fn reuse_advances_binding_record_and_lineage_generations() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut sigils = control_sigil_table::new();
        let endpoint = endpoint(0, 1);
        let old = sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("first root");
        sigils
            .withdraw_task(&mut circles, &tasks, owner)
            .expect("withdraw first");
        let replacement = sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("replacement root");

        assert_eq!(replacement.local_slot(), old.local_slot());
        assert_eq!(replacement.binding_generation(), 2);
        assert_eq!(sigils.records[0].generation, 2);
        assert_eq!(sigils.lineages[0].generation, 2);
        tasks.start(owner).expect("start owner");
        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, old, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );
        assert!(
            sigils
                .authorize_endpoint(&tasks, owner, replacement, endpoint, sigil_rights::send)
                .is_ok()
        );
    }

    #[test]
    fn binding_capacity_and_generation_exhaustion_are_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);
        let mut sigils = control_sigil_table::new();
        for _ in 0..sigil_binding_capacity {
            sigils
                .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
                .expect("within binding capacity");
        }
        let before = sigils.accounting();
        let children = circles
            .snapshot(owner.circle())
            .expect("circle")
            .children()
            .sigils();
        assert_eq!(
            sigils.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::binding_capacity_exhausted)
        );
        assert_eq!(sigils.accounting(), before);
        assert_eq!(
            circles
                .snapshot(owner.circle())
                .expect("circle")
                .children()
                .sigils(),
            children
        );

        let mut exhausted = control_sigil_table::new();
        for binding in &mut exhausted.bindings[owner.task().slot()] {
            binding.generation = u64::MAX;
        }
        assert_eq!(
            exhausted.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::binding_generation_exhausted)
        );
        assert_eq!(exhausted.accounting().bindings, 0);
    }

    #[test]
    fn corrupt_high_right_bits_and_circle_mismatch_fail_before_mutation() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);
        let mut sigils = control_sigil_table::new();
        let reference = sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("root");
        tasks.start(owner).expect("start owner");
        sigils.records[0].rights = 1 << 20;
        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, reference, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );

        sigils.records[0].rights = u64::from(sigil_rights::send.bits());
        circles
            .detach_child(owner.circle(), circle_child_kind::sigil)
            .expect("induce accounting mismatch");
        let before = sigils.accounting();
        assert_eq!(
            sigils.withdraw_task(&mut circles, &tasks, owner),
            Err(sigil_error::circle_accounting_mismatch)
        );
        assert_eq!(sigils.accounting(), before);
    }

    #[test]
    fn every_nonempty_known_rights_subset_attenuates_including_equal_set() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let root_task = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let mut targets = [None; 15];
        for (index, target) in targets.iter_mut().enumerate() {
            *target = Some(add_task(
                &mut circles,
                &mut tasks,
                (index + 2) as u64,
                synthetic_origin_metadata::machine_role_fixture,
            ));
        }
        let endpoint = endpoint(0, 1);
        let mut sigils = control_sigil_table::new();
        let root = sigils
            .issue_root(&mut circles, &tasks, root_task, endpoint, all_rights())
            .expect("all-rights root");
        tasks.start(root_task).expect("start root");
        let mut references = [None; 15];
        for bits in 1u16..=15 {
            let target = targets[usize::from(bits - 1)].expect("target");
            references[usize::from(bits - 1)] = Some(
                sigils
                    .attenuate(
                        &mut circles,
                        &tasks,
                        root_task,
                        root,
                        target,
                        endpoint,
                        sigil_rights::from_bits(bits),
                    )
                    .expect("known subset, including equal set"),
            );
        }
        assert_eq!(sigils.accounting().records, sigil_record_capacity);
        tasks.exit(root_task).expect("exit root");

        for bits in 1u16..=15 {
            let target = targets[usize::from(bits - 1)].expect("target");
            let reference = references[usize::from(bits - 1)].expect("reference");
            tasks.start(target).expect("start target");
            assert!(
                sigils
                    .authorize_endpoint(
                        &tasks,
                        target,
                        reference,
                        endpoint,
                        sigil_rights::from_bits(bits)
                    )
                    .is_ok()
            );
            tasks.exit(target).expect("exit target");
        }
    }

    #[test]
    fn record_capacity_and_generation_exhaustion_are_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);
        let mut sigils: sigil_table<4, 1, 4> = sigil_table::new();
        sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("first record");
        let accounting = sigils.accounting();
        let children = circles
            .snapshot(owner.circle())
            .expect("circle")
            .children()
            .sigils();
        assert_eq!(
            sigils.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::sigil_capacity_exhausted)
        );
        assert_eq!(sigils.accounting(), accounting);
        assert_eq!(
            circles
                .snapshot(owner.circle())
                .expect("circle")
                .children()
                .sigils(),
            children
        );

        let mut exhausted: sigil_table<4, 1, 4> = sigil_table::new();
        exhausted.records[0].generation = u64::MAX;
        assert_eq!(
            exhausted.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::sigil_generation_exhausted)
        );
        assert_eq!(exhausted.accounting().records, 0);
    }

    #[test]
    fn lineage_capacity_and_generation_exhaustion_are_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);
        let mut sigils: sigil_table<4, 4, 1> = sigil_table::new();
        sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("first lineage");
        let accounting = sigils.accounting();
        let children = circles
            .snapshot(owner.circle())
            .expect("circle")
            .children()
            .sigils();
        assert_eq!(
            sigils.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::lineage_capacity_exhausted)
        );
        assert_eq!(sigils.accounting(), accounting);
        assert_eq!(
            circles
                .snapshot(owner.circle())
                .expect("circle")
                .children()
                .sigils(),
            children
        );

        let mut exhausted: sigil_table<4, 4, 1> = sigil_table::new();
        exhausted.lineages[0].generation = u64::MAX;
        assert_eq!(
            exhausted.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::lineage_generation_exhausted)
        );
        assert_eq!(exhausted.accounting().lineages, 0);
    }

    #[test]
    fn stale_kernel_record_and_lineage_generations_deny_lookup() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);
        let mut sigils = control_sigil_table::new();
        let reference = sigils
            .issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send)
            .expect("root");
        tasks.start(owner).expect("start owner");

        sigils.records[0].generation = 2;
        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, reference, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );
        sigils.records[0].generation = 1;
        sigils.lineages[0].generation = 2;
        assert_eq!(
            sigils.authorize_endpoint(&tasks, owner, reference, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );
    }

    #[test]
    fn allocation_fails_closed_on_nonbinary_table_state() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let owner = add_task(
            &mut circles,
            &mut tasks,
            1,
            synthetic_origin_metadata::human_role_fixture,
        );
        let endpoint = endpoint(0, 1);

        let mut corrupt_binding = control_sigil_table::new();
        corrupt_binding.bindings[owner.task().slot()][0].active = 2;
        assert_eq!(
            corrupt_binding.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );

        let mut corrupt_record = control_sigil_table::new();
        corrupt_record.records[0].active = 2;
        assert_eq!(
            corrupt_record.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );

        let mut corrupt_lineage = control_sigil_table::new();
        corrupt_lineage.lineages[0].occupied = 2;
        assert_eq!(
            corrupt_lineage.issue_root(&mut circles, &tasks, owner, endpoint, sigil_rights::send),
            Err(sigil_error::stale_reference)
        );

        let mut corrupt_revocation_state = control_sigil_table::new();
        corrupt_revocation_state.lineages[0].occupied = 1;
        corrupt_revocation_state.lineages[0].active = 2;
        assert_eq!(
            corrupt_revocation_state.issue_root(
                &mut circles,
                &tasks,
                owner,
                endpoint,
                sigil_rights::send
            ),
            Err(sigil_error::stale_reference)
        );

        assert_eq!(
            circles
                .snapshot(owner.circle())
                .expect("circle")
                .children()
                .sigils(),
            0
        );
    }
}
