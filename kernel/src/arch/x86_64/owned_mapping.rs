#![cfg_attr(not(test), allow(dead_code))]

//! Retained ownership for a staged page-table tree with optional residency.
//!
//! This module never activates its root. The default user-only root cannot run
//! the kernel; an explicit resident profile can add a bounded supervisor island
//! and one borrowed high alias. Readiness establishes the exact planned tree,
//! not the residency/lifetime/transition proof required by an activation owner.

use super::mapping_plan::{mapping_plan, region_capacity, table_level};
use super::paging::{
    self, entries_per_table, ownership_marker, page_address_mask, page_size, present,
    user_accessible, writable,
};
use super::resident_plan::{resident_error, resident_plan};
#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
use super::user_copy::{copy_direction, copy_plan, copy_span_capacity, user_copy_error};
use crate::memory::physical::{
    frame_allocator, frame_error, frame_lease, frame_purpose, physical_range,
};

#[cfg(any(test, feature = "linux_loader_entry_test"))]
pub(super) mod pdpt_graft;

/// Independently live privileged access for retained page-table owners.
/// Construction and mutation require an inactive root; bounded live-copy
/// preflight additionally permits read-only access to retained active tables.
///
/// # Safety
///
/// Implementations must report the actual current root, perform access through
/// a valid privileged mapping with suitable RAM permissions/cacheability, and
/// never switch roots or mutate unrelated storage. Successful clearing means
/// the entire extent was written and read back as zero. Failed writes/clears
/// may modify a prefix; failed reads cannot fabricate successful evidence.
/// Access backing this implementation must survive cleanup of the staged tree.
pub(super) unsafe trait mapping_memory {
    /// # Safety
    /// The caller exclusively owns the entire extent, or is executing the
    /// allocator's synchronous clearing callback. No live consumer may use it.
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool;

    /// # Safety
    /// The caller retains the table frame's live ownership; `index` is below
    /// 512, and the complete page is available through privileged access.
    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64>;

    /// # Safety
    /// The caller exclusively owns this inactive table frame and bounds
    /// `index` below 512. No hardware walker or other caller may use the tree.
    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool;

    fn active_root(&self) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum mapping_error {
    frame(frame_error),
    resident(resident_error),
    not_empty,
    not_ready,
    invalid_plan,
    invalid_lease,
    binding_mismatch,
    active_root,
    write_failed,
    read_failed,
    entry_mismatch,
    clear_failed,
    no_residency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum stage {
    empty,
    building,
    ready,
    withdrawing,
}

struct retained_lease {
    lease: frame_lease,
    bound: bool,
}

/// Every acquired lease remains here until verified release succeeds.
///
/// There is deliberately no automatic `Drop` cleanup: physical clearing can
/// fail and its ownership must remain inspectable and retryable. The caller
/// must retain this owner after errors until cleanup succeeds. Table storage
/// uses one exact-sized contiguous lease; fragmentation may reject a build.
#[must_use = "retain the mapping owner until verified cleanup succeeds"]
pub(super) struct staged_mapping {
    plan: mapping_plan,
    residency: Option<resident_plan>,
    tables: Option<retained_lease>,
    data: [Option<retained_lease>; region_capacity],
    stage: stage,
    tables_cleared: bool,
    hardware_used: bool,
}

/// Checked geometry borrowed from a live owner, still not a dereference grant.
/// The physical-access owner must independently admit every alias span before
/// committing bytes, and neither owner may be changed during that operation.
#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
pub(super) struct checked_copy_spans<'a> {
    ranges: [physical_range; copy_span_capacity],
    count: usize,
    _owner: &'a staged_mapping,
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    _additional_owner: Option<&'a staged_mapping>,
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    _graft: Option<&'a pdpt_graft::pdpt_graft>,
}

#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
impl checked_copy_spans<'_> {
    pub(super) fn ranges(&self) -> &[physical_range] {
        &self.ranges[..self.count]
    }
}

impl staged_mapping {
    pub(super) fn new(plan: mapping_plan) -> Self {
        Self {
            plan,
            residency: None,
            tables: None,
            data: [const { None }; region_capacity],
            stage: stage::empty,
            tables_cleared: false,
            hardware_used: false,
        }
    }

    pub(super) fn new_resident(
        plan: mapping_plan,
        residency: resident_plan,
    ) -> Result<Self, mapping_error> {
        residency
            .validate_user_plan(&plan)
            .map_err(mapping_error::resident)?;
        let mut owner = Self::new(plan);
        owner.residency = Some(residency);
        Ok(owner)
    }

    pub(super) fn residency(&self) -> Option<&resident_plan> {
        self.residency.as_ref()
    }

    pub(super) fn plan(&self) -> &mapping_plan {
        &self.plan
    }

    pub(super) fn table_range(&self) -> Option<physical_range> {
        self.tables.as_ref()?.lease.range()
    }

    /// The retained root's physical address, including incomplete builds.
    /// This is inspection metadata, never permission to select this root.
    pub(super) fn root_address(&self) -> Option<u64> {
        self.tables.as_ref()?.lease.range().map(|range| range.base)
    }

    /// Physical geometry of the sorted plan region, not an access grant.
    pub(super) fn data_range(&self, region_index: usize) -> Option<physical_range> {
        self.data.get(region_index)?.as_ref()?.lease.range()
    }

    /// Check every touched path without changing readiness or the active tree.
    /// Unlike inactive full-tree validation, this admits the retained current
    /// root and reads only owned expected table pages, never a PTE-provided
    /// address. Geometry/permission rejection cannot modify a payload prefix.
    ///
    /// # Safety
    /// The activation owner retains this root, all bound leases, and an
    /// independent physical alias. The sole BSP has IF clear/NMI masked; no
    /// allocator, table, task or alias mutation can occur until the copy ends.
    #[cfg(any(
        test,
        feature = "application_profile_test",
        feature = "owned_source_test"
    ))]
    pub(super) unsafe fn preflight_user_copy<'a>(
        &'a self,
        memory: &mut impl mapping_memory,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if self.root_address() != Some(memory.active_root()) {
            return Err(user_copy_error::owner_unavailable);
        }
        // SAFETY: The current-root check preserves this entry point's contract.
        unsafe { self.preflight_owned_user_copy(memory, direction, address, length) }
    }

    /// Check an inactive suspended caller's retained tree through the current
    /// veil's independent physical alias. This is not a cross-task grant.
    ///
    /// # Safety
    /// The activation owner has authenticated a sealed scheduler call authority,
    /// the exact blocked caller and its frame/root, and the running veil's root
    /// and privileged stack. `current_root` is that veil root. All owners remain
    /// live, IF is clear and NMI masked, until the entire copy finishes.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(super) unsafe fn preflight_inactive_user_copy<'a>(
        &'a self,
        memory: &mut impl mapping_memory,
        current_root: u64,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if current_root == 0
            || memory.active_root() != current_root
            || self.root_address() == Some(current_root)
        {
            return Err(user_copy_error::owner_unavailable);
        }
        // SAFETY: The caller retains both authenticated roots and excludes any
        // concurrent caller execution, tree change or alias withdrawal.
        unsafe { self.preflight_owned_user_copy(memory, direction, address, length) }
    }

    #[cfg(any(
        test,
        feature = "application_profile_test",
        feature = "owned_source_test"
    ))]
    unsafe fn preflight_owned_user_copy<'a>(
        &'a self,
        memory: &mut impl mapping_memory,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if !self.is_ready() || !self.hardware_used || self.tables_cleared {
            return Err(user_copy_error::owner_unavailable);
        }
        let plan = copy_plan::new(&self.plan, direction, address, length)?;
        let tables = self
            .tables
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        checked_range(&tables.lease, frame_purpose::page_table, self.table_count())
            .map_err(|_| user_copy_error::owner_unavailable)?;
        if !tables.bound {
            return Err(user_copy_error::owner_unavailable);
        }
        let mut result = checked_copy_spans {
            ranges: [physical_range::new(0, 0); copy_span_capacity],
            count: 0,
            _owner: self,
            #[cfg(any(test, feature = "linux_loader_entry_test"))]
            _additional_owner: None,
            #[cfg(any(test, feature = "linux_loader_entry_test"))]
            _graft: None,
        };
        for span in plan.spans() {
            let region = self
                .plan
                .regions()
                .nth(span.region)
                .ok_or(user_copy_error::owner_unavailable)?;
            let owner = self
                .data
                .get(span.region)
                .and_then(Option::as_ref)
                .ok_or(user_copy_error::owner_unavailable)?;
            let range = checked_range(
                &owner.lease,
                frame_purpose::data,
                region.extent().page_count() as usize,
            )
            .map_err(|_| user_copy_error::owner_unavailable)?;
            if !owner.bound
                || span
                    .region_offset
                    .checked_add(span.length as u64)
                    .is_none_or(|end| end > range.length)
            {
                return Err(user_copy_error::owner_unavailable);
            }
            let physical = range
                .base
                .checked_add(span.region_offset)
                .ok_or(user_copy_error::owner_unavailable)?;
            let indexes = paging::indexes(span.virtual_address);
            let mut path = [0; 4];
            for (depth, (level, prefix, entry_index)) in [
                (table_level::root, 0, indexes.pml4),
                (
                    table_level::directory_pointer,
                    span.virtual_address >> 39,
                    indexes.directory_pointer,
                ),
                (
                    table_level::directory,
                    span.virtual_address >> 30,
                    indexes.directory,
                ),
                (table_level::leaf, span.virtual_address >> 21, indexes.table),
            ]
            .into_iter()
            .enumerate()
            {
                let table_index = self
                    .plan
                    .tables()
                    .iter()
                    .position(|node| node.level() == level && node.prefix() == prefix)
                    .ok_or(user_copy_error::translation_mismatch)?;
                let table = self
                    .table_address(table_index)
                    .map_err(|_| user_copy_error::owner_unavailable)?;
                let expected = self
                    .expected_entry(table_index, entry_index)
                    .map_err(|_| user_copy_error::owner_unavailable)?;
                // SAFETY: This address is derived from our retained table
                // lease, not from the actual (possibly corrupt) parent entry.
                let actual = unsafe { memory.read_entry(table, entry_index) }
                    .ok_or(user_copy_error::access_unavailable)?;
                if expected & present == 0
                    || actual & !self.permitted_runtime_bits(table_index, expected) != expected
                {
                    return Err(user_copy_error::translation_mismatch);
                }
                path[depth] = actual;
            }
            let permissions = paging::effective_permissions_from_path(&path);
            if !permissions.present
                || !permissions.user
                || (direction == copy_direction::to_user && !permissions.writable)
                || paging::entry_address(path[3]) != physical & !(page_size - 1)
            {
                return Err(user_copy_error::translation_mismatch);
            }
            result.ranges[result.count] = physical_range::new(physical, span.length as u64);
            result.count += 1;
        }
        Ok(result)
    }

    pub(super) fn is_ready(&self) -> bool {
        self.stage == stage::ready
    }

    pub(super) fn retained_leases(&self) -> usize {
        usize::from(self.tables.is_some()) + self.data.iter().filter(|slot| slot.is_some()).count()
    }

    /// Allocate and bind all ownership, populate, then independently read back.
    /// Every rejection retains acquired leases and requires explicit cleanup.
    ///
    /// # Safety
    ///
    /// The allocator must describe actual exclusive RAM that `memory` can
    /// access under its contract. No caller may activate this root or mutate
    /// the allocator, tables, or access mapping during the operation. Geometry
    /// alone, including a physical-window plan, does not establish this contract.
    pub(super) unsafe fn build(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), mapping_error> {
        if self.stage != stage::empty || self.retained_leases() != 0 {
            return Err(mapping_error::not_empty);
        }
        self.stage = stage::building;
        let count = self.table_count();
        if count == 0
            || self
                .plan
                .tables()
                .first()
                .is_none_or(|node| node.level() != table_level::root)
        {
            return Err(mapping_error::invalid_plan);
        }
        // SAFETY: The caller provides the allocator and privileged-access
        // contracts; callbacks run synchronously before any lease publication.
        let lease = unsafe {
            allocator.allocate_with(count, 1, frame_purpose::page_table, |range| {
                memory.clear_and_verify(range)
            })
        }
        .map_err(mapping_error::frame)?;
        self.tables = Some(retained_lease {
            lease,
            bound: false,
        });
        self.require_inactive(memory)?;
        let tables = self.tables.as_mut().ok_or(mapping_error::invalid_lease)?;
        checked_range(&tables.lease, frame_purpose::page_table, count)?;
        allocator
            .bind(&tables.lease)
            .map_err(mapping_error::frame)?;
        tables.bound = true;

        for (index, region) in self.plan.regions().enumerate() {
            let count = usize::try_from(region.extent().page_count())
                .map_err(|_| mapping_error::invalid_plan)?;
            let slot = self
                .data
                .get_mut(index)
                .ok_or(mapping_error::invalid_plan)?;
            // SAFETY: The same exclusive allocation and access contract covers
            // each complete data extent. Retain it before subsequent checks.
            let lease = unsafe {
                allocator.allocate_with(count, 1, frame_purpose::data, |range| {
                    memory.clear_and_verify(range)
                })
            }
            .map_err(mapping_error::frame)?;
            *slot = Some(retained_lease {
                lease,
                bound: false,
            });
            let owner = slot.as_mut().ok_or(mapping_error::invalid_lease)?;
            checked_range(&owner.lease, frame_purpose::data, count)?;
            allocator.bind(&owner.lease).map_err(mapping_error::frame)?;
            owner.bound = true;
        }
        self.require_inactive(memory)?;
        // Children precede parents. No partially built tree is published, and
        // failed writes may have changed storage, so cleanup uses only leases.
        for table_index in (0..self.table_count()).rev() {
            let table = self.table_address(table_index)?;
            for entry_index in 0..entries_per_table {
                let expected = self.expected_entry(table_index, entry_index)?;
                if expected != 0 {
                    // SAFETY: This inactive frame is within the retained table
                    // lease, and the loop bounds every entry index.
                    if !unsafe { memory.write_entry(table, entry_index, expected) } {
                        return Err(mapping_error::write_failed);
                    }
                }
            }
        }
        // SAFETY: All ownership and the inactive access contract remain live.
        unsafe { self.verify(allocator, memory) }?;
        self.stage = stage::ready;
        Ok(())
    }

    /// Validate an inactive mixed tree before a separate owner selects it.
    /// This changes only validation policy: present entries may subsequently
    /// contain the precisely permitted hardware A/D additions. No root switches.
    ///
    /// # Safety
    /// The build/access contract still holds. The caller must independently
    /// validate complete kernel, stack, descriptor and exception residency,
    /// retain borrowed resident/alias storage, and prevent cleanup until the
    /// root has been detached with all hardware/software users withdrawn.
    pub(super) unsafe fn prepare_activation(
        &mut self,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, mapping_error> {
        if self.residency.is_none() {
            return Err(mapping_error::no_residency);
        }
        // SAFETY: The caller preserves the inactive ownership/access contract.
        unsafe { self.validate(allocator, memory) }?;
        let root = self.root_address().ok_or(mapping_error::invalid_lease)?;
        self.hardware_used = true;
        Ok(root)
    }

    /// Recheck every entry and ownership binding. Failure withdraws readiness.
    ///
    /// # Safety
    /// The live physical-access and exclusion contracts of `build` still hold.
    pub(super) unsafe fn validate(
        &mut self,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), mapping_error> {
        if !self.is_ready() {
            return Err(mapping_error::not_ready);
        }
        self.stage = stage::building;
        // SAFETY: The caller preserves the same ownership/access contract.
        unsafe { self.verify(allocator, memory) }?;
        self.stage = stage::ready;
        Ok(())
    }

    /// Withdraw and release retained ownership, with retryable partial cleanup.
    ///
    /// Tables are cleared and verified before any data binding is withdrawn.
    /// A slot is removed only after allocator release succeeds. An interrupted
    /// cleanup cannot build or validate until all retained leases are released.
    ///
    /// # Safety
    /// The root has never been active or has been fully detached with no stale
    /// hardware/software consumer. The independent privileged access mapping
    /// remains live while every owned extent is cleared and reclaimed.
    pub(super) unsafe fn cleanup(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), mapping_error> {
        self.require_inactive(memory)?;
        self.check_bindings(allocator)?;
        self.stage = stage::withdrawing;
        if !self.tables_cleared {
            if let Some(tables) = self.tables.as_ref() {
                let range = tables.lease.range().ok_or(mapping_error::invalid_lease)?;
                // SAFETY: The entire inactive private arena is still owned;
                // access is independent and no data has been unbound yet.
                if !unsafe { memory.clear_and_verify(range) } {
                    return Err(mapping_error::clear_failed);
                }
            }
            self.tables_cleared = true;
        }
        for slot in &mut self.data {
            // SAFETY: All private tables are verified zero and cannot name any
            // data. The caller excludes other consumers of retained storage.
            unsafe { release_slot(slot, allocator, memory) }?;
        }
        // SAFETY: The root is inactive, its arena is detached and verified
        // zero, and its privileged access mapping has independent ownership.
        unsafe { release_slot(&mut self.tables, allocator, memory) }?;
        self.tables_cleared = false;
        self.hardware_used = false;
        self.stage = stage::empty;
        Ok(())
    }

    fn require_inactive(&self, memory: &impl mapping_memory) -> Result<(), mapping_error> {
        if self
            .root_address()
            .is_some_and(|root| paging::entry_address(memory.active_root()) == root)
        {
            return Err(mapping_error::active_root);
        }
        Ok(())
    }

    fn check_bindings(&self, allocator: &frame_allocator<'_>) -> Result<(), mapping_error> {
        for owner in self.tables.iter().chain(self.data.iter().flatten()) {
            let bound = allocator
                .is_bound(&owner.lease)
                .map_err(mapping_error::frame)?;
            if bound != owner.bound {
                return Err(mapping_error::binding_mismatch);
            }
        }
        Ok(())
    }

    /// Read-only validation of every owned entry before an external candidate
    /// can be reclaimed. Failure never authorizes cleanup or caller resume.
    ///
    /// # Safety
    /// Actual allocator and independently live access remain retained. This
    /// original owner is inactive, all external grafts are detached, and no
    /// concurrent caller, hardware walker or owner mutation can intervene.
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    pub(super) unsafe fn verify_inactive(
        &self,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), mapping_error> {
        if !self.is_ready() {
            return Err(mapping_error::not_ready);
        }
        // SAFETY: Caller establishes the inactive ownership/access contract.
        unsafe { self.verify(allocator, memory) }
    }

    /// Verify the complete inactive tree and fingerprint its exact expected
    /// entries. Only independently permitted hardware A/D additions disappear
    /// from this diagnostic; validation, not hash equality, proves ownership.
    ///
    /// # Safety
    /// The verify_inactive contract holds, including complete external graft
    /// withdrawal and exclusion of every hardware/software consumer.
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    pub(super) unsafe fn verified_inactive_fingerprint(
        &self,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, mapping_error> {
        // SAFETY: The caller retains the same inactive ownership/access scope.
        unsafe { self.verify_inactive(allocator, memory) }?;
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for table_index in 0..self.table_count() {
            for entry_index in 0..entries_per_table {
                let expected = self.expected_entry(table_index, entry_index)?;
                for byte in expected.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
                }
            }
        }
        Ok(hash)
    }

    unsafe fn verify(
        &self,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), mapping_error> {
        self.require_inactive(memory)?;
        self.verify_ownership(allocator)?;
        for table_index in 0..self.table_count() {
            let table = self.table_address(table_index)?;
            for entry_index in 0..entries_per_table {
                let expected = self.expected_entry(table_index, entry_index)?;
                // SAFETY: Table ownership/extent/binding were checked above;
                // the loop bounds each entry, and the root is inactive.
                let actual = unsafe { memory.read_entry(table, entry_index) }
                    .ok_or(mapping_error::read_failed)?;
                let permitted = self.permitted_runtime_bits(table_index, expected);
                if actual & !permitted != expected {
                    return Err(mapping_error::entry_mismatch);
                }
            }
        }
        Ok(())
    }

    // Shared with the graft's complete composed-tree validator. Ownership is
    // established independently of observed entries or the selected root.
    fn verify_ownership(&self, allocator: &frame_allocator<'_>) -> Result<(), mapping_error> {
        self.check_bindings(allocator)?;
        self.verify_lease_geometry()
    }

    // An activation owner can repeat this without borrowing the allocator only
    // while its exclusion interval preserves previously verified bindings.
    fn verify_lease_geometry(&self) -> Result<(), mapping_error> {
        let tables = self.tables.as_ref().ok_or(mapping_error::invalid_lease)?;
        checked_range(&tables.lease, frame_purpose::page_table, self.table_count())?;
        if !tables.bound || self.tables_cleared {
            return Err(mapping_error::binding_mismatch);
        }
        for (index, region) in self.plan.regions().enumerate() {
            let owner = self
                .data
                .get(index)
                .and_then(Option::as_ref)
                .ok_or(mapping_error::invalid_lease)?;
            let count = usize::try_from(region.extent().page_count())
                .map_err(|_| mapping_error::invalid_plan)?;
            checked_range(&owner.lease, frame_purpose::data, count)?;
            if !owner.bound {
                return Err(mapping_error::binding_mismatch);
            }
        }
        if self.data.iter().flatten().count() != self.plan.regions().count() {
            return Err(mapping_error::invalid_lease);
        }
        Ok(())
    }

    fn table_address(&self, index: usize) -> Result<u64, mapping_error> {
        if index >= self.table_count() {
            return Err(mapping_error::invalid_plan);
        }
        let tables = self.tables.as_ref().ok_or(mapping_error::invalid_lease)?;
        let range = checked_range(&tables.lease, frame_purpose::page_table, self.table_count())?;
        range
            .base
            .checked_add(
                (index as u64)
                    .checked_mul(page_size)
                    .ok_or(mapping_error::invalid_plan)?,
            )
            .ok_or(mapping_error::invalid_lease)
    }

    fn expected_entry(&self, table_index: usize, entry_index: usize) -> Result<u64, mapping_error> {
        if entry_index >= entries_per_table {
            return Err(mapping_error::invalid_plan);
        }
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if self
            .residency
            .as_ref()
            .is_some_and(resident_plan::completes_ancestry)
        {
            return self.expected_entry_with_ancestry(table_index, entry_index);
        }
        if table_index == self.plan.tables().len() {
            let residency = self.residency.as_ref().ok_or(mapping_error::invalid_plan)?;
            let address = entry_index as u64 * page_size;
            for region in residency.regions() {
                if address >= region.start() && address < region.end() {
                    let flags = paging::leaf_flags(region.permissions())
                        .ok_or(mapping_error::invalid_plan)?;
                    return Ok(address | flags | ownership_marker);
                }
            }
            return Ok(0);
        }
        let requirement = self
            .plan
            .tables()
            .get(table_index)
            .ok_or(mapping_error::invalid_plan)?;
        if let Some(residency) = &self.residency {
            if requirement.level() == table_level::root && entry_index == residency.alias_slot() {
                return Ok(residency.alias_entry());
            }
            if requirement.level() == table_level::directory
                && requirement.prefix() == 0
                && entry_index == 0
            {
                return Ok(self.table_address(self.plan.tables().len())?
                    | present
                    | writable
                    | ownership_marker);
            }
        }
        let child_level = match requirement.level() {
            table_level::root => Some(table_level::directory_pointer),
            table_level::directory_pointer => Some(table_level::directory),
            table_level::directory => Some(table_level::leaf),
            table_level::leaf => None,
        };
        if let Some(level) = child_level {
            let prefix = requirement
                .prefix()
                .checked_mul(entries_per_table as u64)
                .and_then(|prefix| prefix.checked_add(entry_index as u64))
                .ok_or(mapping_error::invalid_plan)?;
            let Some(child) = self
                .plan
                .tables()
                .iter()
                .position(|node| node.level() == level && node.prefix() == prefix)
            else {
                return Ok(0);
            };
            return Ok(self.table_address(child)?
                | present
                | writable
                | user_accessible
                | ownership_marker);
        }
        let address = requirement
            .prefix()
            .checked_mul(entries_per_table as u64 * page_size)
            .and_then(|base| base.checked_add(entry_index as u64 * page_size))
            .ok_or(mapping_error::invalid_plan)?;
        for (index, region) in self.plan.regions().enumerate() {
            let extent = region.extent();
            if address >= extent.start() && address < extent.end() {
                let range = self.data_range(index).ok_or(mapping_error::invalid_lease)?;
                let physical = range
                    .base
                    .checked_add(address - extent.start())
                    .ok_or(mapping_error::invalid_lease)?;
                let flags =
                    paging::leaf_flags(region.permissions()).ok_or(mapping_error::invalid_plan)?;
                return Ok(physical | flags | ownership_marker);
            }
        }
        Ok(0)
    }

    #[cfg(any(test, feature = "owned_multi_region_test"))]
    fn complete_table_descriptor(&self, index: usize) -> Option<(table_level, u64)> {
        if let Some(node) = self.plan.tables().get(index) {
            Some((node.level(), node.prefix()))
        } else {
            self.residency
                .as_ref()?
                .additional_table(index.checked_sub(self.plan.tables().len())?)
        }
    }

    /// Build only from the retained union of user and supervisor descriptors.
    /// Shared ancestors retain U/S for their user descendants; supplemental
    /// edges clear U/S. NX and writable leaf rights remain independently exact.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    fn expected_entry_with_ancestry(
        &self,
        table_index: usize,
        entry_index: usize,
    ) -> Result<u64, mapping_error> {
        let residency = self.residency.as_ref().ok_or(mapping_error::invalid_plan)?;
        let (level, prefix) = self
            .complete_table_descriptor(table_index)
            .ok_or(mapping_error::invalid_plan)?;
        if level == table_level::root && entry_index == residency.alias_slot() {
            return Ok(residency.alias_entry());
        }
        let child_level = match level {
            table_level::root => Some(table_level::directory_pointer),
            table_level::directory_pointer => Some(table_level::directory),
            table_level::directory => Some(table_level::leaf),
            table_level::leaf => None,
        };
        if let Some(child_level) = child_level {
            let child_prefix = prefix
                .checked_mul(entries_per_table as u64)
                .and_then(|base| base.checked_add(entry_index as u64))
                .ok_or(mapping_error::invalid_plan)?;
            let Some(child) = (0..self.table_count()).find(|index| {
                self.complete_table_descriptor(*index) == Some((child_level, child_prefix))
            }) else {
                return Ok(0);
            };
            let user = if child < self.plan.tables().len() {
                user_accessible
            } else {
                0
            };
            return Ok(self.table_address(child)? | present | writable | user | ownership_marker);
        }
        let address = prefix
            .checked_mul(entries_per_table as u64 * page_size)
            .and_then(|base| base.checked_add(entry_index as u64 * page_size))
            .ok_or(mapping_error::invalid_plan)?;
        if table_index >= self.plan.tables().len() {
            for region in residency.regions() {
                if address >= region.start() && address < region.end() {
                    let flags = paging::leaf_flags(region.permissions())
                        .ok_or(mapping_error::invalid_plan)?;
                    return Ok(address | flags | ownership_marker);
                }
            }
            return Ok(0);
        }
        for (index, region) in self.plan.regions().enumerate() {
            let extent = region.extent();
            if address >= extent.start() && address < extent.end() {
                let range = self.data_range(index).ok_or(mapping_error::invalid_lease)?;
                let physical = range
                    .base
                    .checked_add(address - extent.start())
                    .ok_or(mapping_error::invalid_lease)?;
                let flags =
                    paging::leaf_flags(region.permissions()).ok_or(mapping_error::invalid_plan)?;
                return Ok(physical | flags | ownership_marker);
            }
        }
        Ok(0)
    }

    fn table_count(&self) -> usize {
        self.residency
            .as_ref()
            .map_or(self.plan.tables().len(), resident_plan::table_count)
    }

    fn permitted_runtime_bits(&self, table_index: usize, expected: u64) -> u64 {
        const accessed: u64 = 1 << 5;
        const dirty: u64 = 1 << 6;
        if !self.hardware_used || expected & present == 0 {
            return 0;
        }
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if self
            .residency
            .as_ref()
            .is_some_and(resident_plan::completes_ancestry)
        {
            let leaf = self
                .complete_table_descriptor(table_index)
                .is_some_and(|(level, _)| level == table_level::leaf);
            return accessed
                | if leaf && expected & writable != 0 {
                    dirty
                } else {
                    0
                };
        }
        let leaf = table_index == self.plan.tables().len()
            || self
                .plan
                .tables()
                .get(table_index)
                .is_some_and(|node| node.level() == table_level::leaf);
        accessed
            | if leaf && expected & writable != 0 {
                dirty
            } else {
                0
            }
    }
}

fn checked_range(
    lease: &frame_lease,
    purpose: frame_purpose,
    count: usize,
) -> Result<physical_range, mapping_error> {
    let range = lease.range().ok_or(mapping_error::invalid_lease)?;
    let length = (count as u64)
        .checked_mul(page_size)
        .ok_or(mapping_error::invalid_lease)?;
    let end = range.end().ok_or(mapping_error::invalid_lease)?;
    if count == 0
        || lease.purpose() != purpose
        || lease.frame_count() != count
        || range.length != length
        || !range.base.is_multiple_of(page_size)
        || range.base & !page_address_mask != 0
        || (end - 1) & !(page_address_mask | (page_size - 1)) != 0
    {
        return Err(mapping_error::invalid_lease);
    }
    Ok(range)
}

unsafe fn release_slot(
    slot: &mut Option<retained_lease>,
    allocator: &mut frame_allocator<'_>,
    memory: &mut impl mapping_memory,
) -> Result<(), mapping_error> {
    let Some(owner) = slot.as_mut() else {
        return Ok(());
    };
    if owner.bound {
        allocator
            .unbind(&owner.lease)
            .map_err(mapping_error::frame)?;
        owner.bound = false;
    }
    // SAFETY: The caller detached all uses, retains the complete live lease,
    // and provides the backend's exclusive clear/readback contract.
    unsafe { allocator.release_with(&mut owner.lease, |range| memory.clear_and_verify(range)) }
        .map_err(mapping_error::frame)?;
    *slot = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::super::mapping_plan::{
        mapping_budget, mapping_request, reserved_request, table_frame_capacity,
    };
    use super::super::paging::{effective_permissions, page_permissions};
    use super::super::resident_plan::resident_request;
    use super::*;
    use crate::boot_omen::memory_region;
    use std::vec;
    use std::vec::Vec;

    const frame_count: usize = 64;
    const read_execute: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: true,
    };
    const read_write: page_permissions = page_permissions {
        user: true,
        writable: true,
        executable: false,
    };
    const read_only: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: false,
    };
    const distant_stack: u64 = 0x0000_4000_0000_0000;

    struct fake_memory {
        words: Vec<u64>,
        active: u64,
        clears: usize,
        reads: usize,
        writes: usize,
        fail_clear: Option<usize>,
        fail_read: Option<usize>,
        fail_write: Option<usize>,
    }

    impl fake_memory {
        fn new() -> Self {
            Self {
                words: vec![0xa5a5_a5a5_a5a5_a5a5; frame_count * entries_per_table],
                active: 0,
                clears: 0,
                reads: 0,
                writes: 0,
                fail_clear: None,
                fail_read: None,
                fail_write: None,
            }
        }

        fn bounds(&self, range: physical_range) -> Option<core::ops::Range<usize>> {
            if !range.base.is_multiple_of(8) || !range.length.is_multiple_of(8) {
                return None;
            }
            let start = usize::try_from(range.base / 8).ok()?;
            let end = usize::try_from(range.end()? / 8).ok()?;
            (start < end && end <= self.words.len()).then_some(start..end)
        }

        fn word(&self, table: u64, index: usize) -> Option<usize> {
            if !table.is_multiple_of(page_size) || index >= entries_per_table {
                return None;
            }
            let offset = usize::try_from(table / 8).ok()?.checked_add(index)?;
            (offset < self.words.len()).then_some(offset)
        }

        fn path(&self, root: u64, address: u64) -> Option<[u64; 4]> {
            let indexes = paging::indexes(address);
            let mut table = root;
            let mut path = [0; 4];
            for (depth, index) in [
                indexes.pml4,
                indexes.directory_pointer,
                indexes.directory,
                indexes.table,
            ]
            .into_iter()
            .enumerate()
            {
                let entry = self.words[self.word(table, index)?];
                if entry & present == 0 {
                    return None;
                }
                path[depth] = entry;
                table = paging::entry_address(entry);
            }
            Some(path)
        }

        fn reset_faults(&mut self) {
            self.fail_clear = None;
            self.fail_read = None;
            self.fail_write = None;
        }
    }

    // SAFETY: The isolated vector models exactly the allocator's physical RAM.
    // No host physical address is dereferenced; every access bounds the vector,
    // successful clearing checks all words, and the active root never changes.
    unsafe impl mapping_memory for fake_memory {
        unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
            self.clears += 1;
            let Some(bounds) = self.bounds(range) else {
                return false;
            };
            if self.fail_clear == Some(self.clears) {
                let middle = bounds.start + (bounds.end - bounds.start) / 2;
                self.words[bounds.start..middle].fill(0);
                return false;
            }
            self.words[bounds.clone()].fill(0);
            self.words[bounds].iter().all(|word| *word == 0)
        }

        unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
            self.reads += 1;
            if self.fail_read == Some(self.reads) {
                return None;
            }
            Some(self.words[self.word(table, index)?])
        }

        unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
            self.writes += 1;
            let Some(offset) = self.word(table, index) else {
                return false;
            };
            self.words[offset] = value;
            self.fail_write != Some(self.writes)
        }

        fn active_root(&self) -> u64 {
            self.active
        }
    }

    fn allocator(ledger: &mut [u8]) -> frame_allocator<'_> {
        frame_allocator::initialize(
            ledger,
            &[memory_region {
                base: 0,
                length: frame_count as u64 * page_size,
                kind: 1,
                attributes: 1,
            }],
            &[physical_range::new(0, page_size)],
        )
        .unwrap()
    }

    fn plan() -> mapping_plan {
        mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: distant_stack,
                    page_count: 2,
                    permissions: read_write,
                },
                mapping_request {
                    virtual_address: 0x400000,
                    page_count: 1,
                    permissions: read_execute,
                },
                mapping_request {
                    virtual_address: 0x1ff000,
                    page_count: 2,
                    permissions: read_write,
                },
                mapping_request {
                    virtual_address: 0x40003000,
                    page_count: 1,
                    permissions: read_only,
                },
            ],
            &[reserved_request {
                virtual_address: 0x40002000,
                page_count: 1,
            }],
            mapping_budget {
                data_pages: 6,
                table_frames: table_frame_capacity,
            },
        )
        .unwrap()
    }

    fn small_plan() -> mapping_plan {
        mapping_plan::new(
            &[mapping_request {
                virtual_address: 0x400000,
                page_count: 1,
                permissions: read_execute,
            }],
            &[],
            mapping_budget {
                data_pages: 1,
                table_frames: 4,
            },
        )
        .unwrap()
    }

    fn mixed_owner() -> staged_mapping {
        let user = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: 0x400000,
                    page_count: 1,
                    permissions: read_execute,
                },
                mapping_request {
                    virtual_address: 0x5ff000,
                    page_count: 2,
                    permissions: read_only,
                },
                mapping_request {
                    virtual_address: 0x800000,
                    page_count: 2,
                    permissions: read_write,
                },
                mapping_request {
                    virtual_address: distant_stack,
                    page_count: 2,
                    permissions: read_write,
                },
            ],
            &[],
            mapping_budget {
                data_pages: 7,
                table_frames: 9,
            },
        )
        .unwrap();
        // The borrowed child's number is outside the allocator model. Its
        // contents are never read by this inactive construction test; proving
        // its live hierarchy remains the separate activation owner's contract.
        let alias = frame_count as u64 * page_size | present | writable | paging::no_execute;
        let resident = resident_plan::new(
            &user,
            &[
                resident_request {
                    virtual_address: 0x18000,
                    page_count: 16,
                    permissions: page_permissions {
                        user: false,
                        ..read_write
                    },
                },
                resident_request {
                    virtual_address: 0x100000,
                    page_count: 2,
                    permissions: page_permissions {
                        user: false,
                        ..read_execute
                    },
                },
                resident_request {
                    virtual_address: 0x102000,
                    page_count: 1,
                    permissions: page_permissions {
                        user: false,
                        ..read_only
                    },
                },
                resident_request {
                    virtual_address: 0x104000,
                    page_count: 2,
                    permissions: page_permissions {
                        user: false,
                        ..read_write
                    },
                },
            ],
            alias,
            10,
        )
        .unwrap();
        staged_mapping::new_resident(user, resident).unwrap()
    }

    fn assert_retained(owner: &staged_mapping, allocator: &frame_allocator<'_>) {
        assert_eq!(
            owner.retained_leases(),
            allocator.accounting().active_leases
        );
        for retained in owner.tables.iter().chain(owner.data.iter().flatten()) {
            assert!(retained.lease.range().is_some());
            assert_eq!(allocator.is_bound(&retained.lease), Ok(retained.bound));
        }
    }

    fn finish(
        owner: &mut staged_mapping,
        allocator: &mut frame_allocator<'_>,
        memory: &mut fake_memory,
    ) {
        memory.reset_faults();
        // SAFETY: The isolated backing vector implements the exact allocator
        // extents and no consumer has activated or retained this staged tree.
        unsafe { owner.cleanup(allocator, memory) }.unwrap();
        assert_eq!(owner.retained_leases(), 0);
        assert_eq!(owner.root_address(), None);
        assert!(!owner.is_ready());
        assert_eq!(allocator.accounting().active_leases, 0);
        assert_eq!(allocator.accounting().bound_leases, 0);
        assert_eq!(allocator.accounting().free, frame_count - 1);
        assert_eq!(memory.active, 0);
    }

    #[test]
    fn multiple_branches_preserve_exact_leaf_permissions_and_absent_gaps() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let baseline = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut owner = staged_mapping::new(plan());
        // SAFETY: Logical physical RAM is the isolated fake-memory vector.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        assert!(owner.is_ready());
        assert_eq!(owner.retained_leases(), 5);
        assert_retained(&owner, &allocator);
        let root = owner.root_address().unwrap();
        let table_range = owner.tables.as_ref().unwrap().lease.range().unwrap();
        let mut released_ranges = vec![table_range];
        for (region_index, region) in owner.plan.regions().enumerate() {
            let data = owner.data_range(region_index).unwrap();
            released_ranges.push(data);
            for page in 0..region.extent().page_count() {
                let address = region.extent().start() + page * page_size;
                let path = memory
                    .path(root, address)
                    .expect("independent complete walk");
                assert_eq!(paging::entry_address(path[3]), data.base + page * page_size);
                assert_eq!(
                    paging::effective_permissions_from_path(&path),
                    effective_permissions {
                        present: true,
                        user: true,
                        writable: region.permissions().writable,
                        executable: region.permissions().executable,
                    }
                );
            }
        }
        for gap in [
            0,
            0x1fe000,
            0x201000,
            0x401000,
            0x40002000,
            distant_stack - page_size,
            distant_stack + 2 * page_size,
            0xffff_9000_0000_0000,
        ] {
            assert!(memory.path(root, gap).is_none(), "gap {gap:x}");
        }
        // SAFETY: Ownership and the isolated inactive model remain unchanged.
        unsafe { owner.validate(&allocator, &mut memory) }.unwrap();
        finish(&mut owner, &mut allocator, &mut memory);
        assert_eq!(allocator.accounting(), baseline);
        for range in released_ranges {
            assert!(
                memory.words[memory.bounds(range).unwrap()]
                    .iter()
                    .all(|word| *word == 0)
            );
        }
        // Reusing the owner requires successful complete cleanup first.
        // SAFETY: All old resources were released; the same model remains live.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn each_allocation_clear_failure_retains_every_preceding_owner() {
        for failure in 1..=5 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            memory.fail_clear = Some(failure);
            let mut owner = staged_mapping::new(plan());
            // SAFETY: Injected partial clear never reports verified success.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::frame(frame_error::clearing_failed))
            );
            assert!(!owner.is_ready());
            assert_eq!(owner.retained_leases(), failure - 1);
            assert_retained(&owner, &allocator);
            let accounting = allocator.accounting();
            // SAFETY: This rejects before allocation; it cannot lose owners.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::not_empty)
            );
            assert_eq!(allocator.accounting(), accounting);
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn allocation_exhaustion_at_each_resource_boundary_retains_partial_build() {
        let geometry = plan();
        let mut preceding_frames = geometry.tables().len();
        let mut capacities = vec![preceding_frames - 1];
        for region in geometry.regions() {
            capacities.push(preceding_frames);
            preceding_frames += region.extent().page_count() as usize;
        }
        for free_frames in capacities {
            let mut ledger = [0; frame_count / 4];
            let end = (free_frames as u64 + 1) * page_size;
            let mut allocator = frame_allocator::initialize(
                &mut ledger,
                &[memory_region {
                    base: page_size,
                    length: free_frames as u64 * page_size,
                    kind: 1,
                    attributes: 1,
                }],
                &[],
            )
            .unwrap();
            let baseline = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut owner = staged_mapping::new(plan());
            // SAFETY: Only the represented modeled RAM is allocatable.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::frame(frame_error::out_of_memory))
            );
            assert!(!owner.is_ready());
            assert_retained(&owner, &allocator);
            for lease in owner.tables.iter().chain(owner.data.iter().flatten()) {
                assert!(lease.lease.range().unwrap().end().unwrap() <= end);
            }
            // SAFETY: The failed build was never active or published.
            unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
            assert_eq!(allocator.accounting(), baseline);
            assert_eq!(owner.retained_leases(), 0);
        }
    }

    #[test]
    fn every_population_write_failure_remains_owned_and_retryable() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = staged_mapping::new(plan());
        // SAFETY: The isolated vector models exclusive physical RAM.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let writes = memory.writes;
        finish(&mut owner, &mut allocator, &mut memory);
        for failure in 1..=writes {
            memory.writes = 0;
            memory.fail_write = Some(failure);
            // SAFETY: Injected failure writes the entry then reports failure,
            // testing that unknown partial population retains all ownership.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::write_failed)
            );
            assert!(!owner.is_ready());
            assert_eq!(owner.retained_leases(), 5);
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn every_readback_position_is_fallible_without_losing_ownership() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = staged_mapping::new(small_plan());
        for failure in 1..=4 * entries_per_table {
            memory.reads = 0;
            memory.fail_read = Some(failure);
            // SAFETY: This bounded model fails exactly one read of the four
            // private frames while retaining exclusive vector-backed storage.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::read_failed)
            );
            assert!(!owner.is_ready());
            assert_eq!(owner.retained_leases(), 2);
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn each_cleanup_clear_failure_keeps_remaining_leases_for_retry() {
        // One arena detachment, four data releases, and one table release.
        for failure in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = staged_mapping::new(plan());
            // SAFETY: Exclusive vector-backed logical RAM; root stays inactive.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            // SAFETY: The incomplete wipe reports failure and cannot free its
            // extent. Cleanup must preserve its handle even after unbinding.
            let error = unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap_err();
            assert_eq!(
                error,
                if failure == 1 {
                    mapping_error::clear_failed
                } else {
                    mapping_error::frame(frame_error::clearing_failed)
                }
            );
            assert!(!owner.is_ready());
            assert_retained(&owner, &allocator);
            assert_eq!(
                owner.retained_leases(),
                if failure <= 2 { 5 } else { 7 - failure }
            );
            // SAFETY: Both operations reject the incomplete cleanup phase.
            assert_eq!(
                unsafe { owner.build(&mut allocator, &mut memory) },
                Err(mapping_error::not_empty)
            );
            // SAFETY: Readiness was withdrawn before clearing began.
            assert_eq!(
                unsafe { owner.validate(&allocator, &mut memory) },
                Err(mapping_error::not_ready)
            );
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn foreign_allocator_and_active_root_preflight_do_not_mutate_storage() {
        let mut first_ledger = [0; frame_count / 4];
        let mut second_ledger = [0; frame_count / 4];
        let mut first = allocator(&mut first_ledger);
        let mut second = allocator(&mut second_ledger);
        let mut memory = fake_memory::new();
        let mut owner = staged_mapping::new(small_plan());
        // SAFETY: Only the first allocator's model is used to own this tree.
        unsafe { owner.build(&mut first, &mut memory) }.unwrap();
        let before = memory.words.clone();
        let accounting = first.accounting();
        // SAFETY: The foreign allocator must be rejected before physical access.
        assert_eq!(
            unsafe { owner.cleanup(&mut second, &mut memory) },
            Err(mapping_error::frame(frame_error::foreign_lease))
        );
        assert_eq!(memory.words, before);
        assert_eq!(first.accounting(), accounting);
        memory.active = owner.root_address().unwrap();
        // SAFETY: No real CPU selects this root; the model exercises rejection
        // before destructive access when the backend reports it as active.
        assert_eq!(
            unsafe { owner.cleanup(&mut first, &mut memory) },
            Err(mapping_error::active_root)
        );
        assert_eq!(memory.words, before);
        assert_eq!(first.accounting(), accounting);
        memory.active = 0;
        finish(&mut owner, &mut first, &mut memory);
    }

    #[test]
    fn unexpected_entries_wrong_targets_and_permission_changes_withdraw_readiness() {
        for probe in 0..5 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = staged_mapping::new(small_plan());
            // SAFETY: The fake memory models exclusive inactive owned RAM.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            let root = owner.root_address().unwrap();
            let leaf = owner.table_address(3).unwrap();
            let (table, index, mask) = match probe {
                0 => (root, entries_per_table - 1, present),
                1 => (leaf, 0, writable),
                2 => (leaf, 0, page_size),
                3 => (leaf, 0, 1 << 5), // A/D is unexpected in an inactive tree.
                _ => (leaf, 1, present),
            };
            let offset = memory.word(table, index).unwrap();
            memory.words[offset] ^= mask;
            // SAFETY: Corrupt bytes remain within owned vector-backed frames.
            assert_eq!(
                unsafe { owner.validate(&allocator, &mut memory) },
                Err(mapping_error::entry_mismatch)
            );
            assert!(!owner.is_ready());
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn mixed_tree_has_exact_supervisor_identity_island_and_user_ownership() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        // SAFETY: The fake vector owns all accessed allocator RAM. Resident
        // and borrowed alias entries are inert metadata; this root is unused.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(owner.table_range().unwrap().length, 10 * page_size);
        assert_eq!(owner.retained_leases(), 5);
        assert_eq!(allocator.accounting().table_owned, 10);
        assert_eq!(allocator.accounting().data_owned, 7);
        let root = owner.root_address().unwrap();
        let residency = owner.residency().unwrap();
        assert_eq!(
            memory.words[memory.word(root, residency.alias_slot()).unwrap()],
            residency.alias_entry()
        );
        for region in residency.regions() {
            let mut address = region.start();
            while address < region.end() {
                let path = memory.path(root, address).unwrap();
                assert_eq!(paging::entry_address(path[3]), address);
                assert_eq!(path[2] & user_accessible, 0);
                assert_eq!(
                    paging::effective_permissions_from_path(&path),
                    effective_permissions {
                        present: true,
                        user: false,
                        writable: region.permissions().writable,
                        executable: region.permissions().executable,
                    }
                );
                address += page_size;
            }
        }
        for (index, region) in owner.plan().regions().enumerate() {
            let path = memory.path(root, region.extent().start()).unwrap();
            assert_eq!(
                paging::entry_address(path[3]),
                owner.data_range(index).unwrap().base
            );
            assert_eq!(
                paging::effective_permissions_from_path(&path),
                effective_permissions {
                    present: true,
                    user: true,
                    writable: region.permissions().writable,
                    executable: region.permissions().executable,
                }
            );
        }
        for address in [0, 0x17000, 0x28000, 0x103000, 0x106000, 0x1ff000, 0x401000] {
            assert!(memory.path(root, address).is_none());
        }
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn activation_preparation_is_explicit_and_runtime_bits_are_narrow() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut user_only = staged_mapping::new(small_plan());
        // SAFETY: This is rejected without accessing physical memory.
        assert_eq!(
            unsafe { user_only.prepare_activation(&allocator, &mut memory) },
            Err(mapping_error::no_residency)
        );
        let mut owner = mixed_owner();
        // SAFETY: This is rejected because no live tree has been built.
        assert_eq!(
            unsafe { owner.prepare_activation(&allocator, &mut memory) },
            Err(mapping_error::not_ready)
        );
        // SAFETY: The fake vector models every physical access to this owner.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let root = owner.root_address().unwrap();
        let slot = memory.word(root, 0).unwrap();
        memory.words[slot] |= 1 << 5;
        // SAFETY: Inert table mutation exercises exact pre-activation evidence.
        assert_eq!(
            unsafe { owner.prepare_activation(&allocator, &mut memory) },
            Err(mapping_error::entry_mismatch)
        );
        assert!(!owner.is_ready());
        finish(&mut owner, &mut allocator, &mut memory);
        // SAFETY: Successful cleanup restored the empty owner and RAM model.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        // SAFETY: This models a separate activation owner's authorization to
        // use A/D-aware validation. No root or borrowed physical page is used.
        assert_eq!(
            unsafe { owner.prepare_activation(&allocator, &mut memory) },
            Ok(root)
        );
        let path = memory.path(root, 0x400000).unwrap();
        for (table, index) in [
            (root, 0),
            (paging::entry_address(path[0]), 0),
            (paging::entry_address(path[1]), 2),
            (root, 288),
        ] {
            let slot = memory.word(table, index).unwrap();
            memory.words[slot] |= 1 << 5;
        }
        for (address, bits) in [
            (0x400000, 1 << 5),
            (0x100000, 1 << 5),
            (0x102000, 1 << 5),
            (0x104000, (1 << 5) | (1 << 6)),
            (0x800000, (1 << 5) | (1 << 6)),
        ] {
            let path = memory.path(root, address).unwrap();
            let slot = memory
                .word(
                    paging::entry_address(path[2]),
                    paging::indexes(address).table,
                )
                .unwrap();
            memory.words[slot] |= bits;
        }
        // SAFETY: Modeled hardware additions are within owned inactive tables.
        unsafe { owner.validate(&allocator, &mut memory) }.unwrap();
        assert!(owner.is_ready());
        assert_eq!(memory.active, 0);
        finish(&mut owner, &mut allocator, &mut memory);
        assert!(!owner.hardware_used);
    }

    #[test]
    fn runtime_validation_rejects_dirty_parents_read_only_dirty_and_privilege_changes() {
        for probe in 0..7 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = mixed_owner();
            // SAFETY: All construction accesses use the isolated RAM vector.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            // SAFETY: This models preparation only, never CPU root selection.
            let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
            let resident = memory.path(root, 0x100000).unwrap();
            let user = memory.path(root, 0x400000).unwrap();
            let (table, index, mask) = match probe {
                0 => (root, 0, 1 << 6),
                1 => (
                    paging::entry_address(resident[2]),
                    0x100000 / page_size as usize,
                    1 << 6,
                ),
                2 => (paging::entry_address(user[2]), 0, 1 << 6),
                3 => (root, 1, 1 << 5),
                4 => (root, 288, user_accessible),
                5 => (paging::entry_address(resident[1]), 0, user_accessible),
                _ => (paging::entry_address(resident[2]), 0, 1 << 5),
            };
            let slot = memory.word(table, index).unwrap();
            memory.words[slot] ^= mask;
            // SAFETY: The corrupt entry is confined to owned inactive storage.
            assert_eq!(
                unsafe { owner.validate(&allocator, &mut memory) },
                Err(mapping_error::entry_mismatch)
            );
            assert!(!owner.is_ready());
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
        }
    }

    #[test]
    fn user_copy_preflight_accepts_active_cross_page_ranges_and_hardware_bits() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        // SAFETY: All physical accesses use the isolated allocator RAM model.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
        memory.active = root;
        for base in [0x800000, distant_stack] {
            for address in [base, base + page_size] {
                let path = memory.path(root, address).unwrap();
                let indexes = paging::indexes(address);
                for (table, index, bits) in [
                    (root, indexes.pml4, 1 << 5),
                    (
                        paging::entry_address(path[0]),
                        indexes.directory_pointer,
                        1 << 5,
                    ),
                    (paging::entry_address(path[1]), indexes.directory, 1 << 5),
                    (
                        paging::entry_address(path[2]),
                        indexes.table,
                        (1 << 5) | (1 << 6),
                    ),
                ] {
                    let slot = memory.word(table, index).unwrap();
                    memory.words[slot] |= bits;
                }
            }
        }
        let accounting = allocator.accounting();
        let words = memory.words.clone();
        let writes = memory.writes;
        let clears = memory.clears;
        for (region, base) in [(2, 0x800000), (3, distant_stack)] {
            for direction in [copy_direction::from_user, copy_direction::to_user] {
                let spans = unsafe {
                    owner.preflight_user_copy(&mut memory, direction, base + page_size - 7, 19)
                }
                .unwrap();
                let range = owner.data_range(region).unwrap();
                assert_eq!(
                    spans.ranges(),
                    &[
                        physical_range::new(range.base + page_size - 7, 7),
                        physical_range::new(range.base + page_size, 12),
                    ]
                );
            }
        }
        // User-readable RX and RO regions are accepted for reads only.
        for address in [0x400003, 0x5ffffd] {
            assert!(
                unsafe {
                    owner.preflight_user_copy(&mut memory, copy_direction::from_user, address, 11)
                }
                .is_ok()
            );
            assert_eq!(
                unsafe {
                    owner.preflight_user_copy(&mut memory, copy_direction::to_user, address, 11)
                }
                .err(),
                Some(user_copy_error::permission_denied)
            );
        }
        assert_eq!(memory.words, words);
        assert_eq!(memory.writes, writes);
        assert_eq!(memory.clears, clears);
        assert_eq!(allocator.accounting(), accounting);
        assert!(owner.is_ready());
        memory.active = 0;
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn user_copy_rejects_invalid_complete_ranges_before_reading_any_translation() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        memory.active = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
        let before = memory.words.clone();
        let reads = memory.reads;
        for (address, length, expected) in [
            (0x800000, 0, user_copy_error::empty),
            (0x800000, 4097, user_copy_error::too_large),
            (0x801fff, 2, user_copy_error::unmapped),
            (
                distant_stack + 2 * page_size - 1,
                2,
                user_copy_error::unmapped,
            ),
            (0x100000, 8, user_copy_error::unmapped),
            (0xffff_9000_0000_0000, 8, user_copy_error::non_user_address),
            (u64::MAX - 3, 8, user_copy_error::address_overflow),
        ] {
            assert_eq!(
                unsafe {
                    owner.preflight_user_copy(&mut memory, copy_direction::to_user, address, length)
                }
                .err(),
                Some(expected)
            );
        }
        assert_eq!(memory.reads, reads);
        assert_eq!(memory.words, before);
        assert!(owner.is_ready());
        memory.active = 0;
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn user_copy_rejects_mutated_paths_without_following_unowned_parent_addresses() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
        memory.active = root;
        let address = 0x800003;
        let path = memory.path(root, address).unwrap();
        let indexes = paging::indexes(address);
        let locations = [
            (root, indexes.pml4),
            (paging::entry_address(path[0]), indexes.directory_pointer),
            (paging::entry_address(path[1]), indexes.directory),
            (paging::entry_address(path[2]), indexes.table),
        ];
        for (depth, (table, index)) in locations.into_iter().enumerate() {
            let slot = memory.word(table, index).unwrap();
            let original = memory.words[slot];
            for mask in [
                present,
                user_accessible,
                writable,
                paging::huge_page,
                paging::no_execute,
                1 << 40,
            ] {
                memory.words[slot] = original ^ mask;
                let before = memory.words.clone();
                let reads = memory.reads;
                assert_eq!(
                    unsafe {
                        owner.preflight_user_copy(&mut memory, copy_direction::to_user, address, 17)
                    }
                    .err(),
                    Some(user_copy_error::translation_mismatch)
                );
                assert_eq!(memory.reads - reads, depth + 1);
                assert_eq!(memory.words, before);
                assert!(owner.is_ready());
                memory.words[slot] = original;
            }
        }
        // A valid first page cannot hide an invalid final page or permit a
        // prefix write: the whole request still produces no usable spans.
        let tail = memory
            .word(paging::entry_address(path[2]), indexes.table + 1)
            .unwrap();
        let original = memory.words[tail];
        memory.words[tail] &= !writable;
        let before = memory.words.clone();
        assert_eq!(
            unsafe {
                owner.preflight_user_copy(&mut memory, copy_direction::to_user, 0x800ff9, 19)
            }
            .err(),
            Some(user_copy_error::translation_mismatch)
        );
        assert_eq!(memory.words, before);
        memory.words[tail] = original;
        memory.active = 0;
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn user_copy_read_failures_and_owner_state_reject_without_withdrawing_readiness() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(
            unsafe {
                owner.preflight_user_copy(&mut memory, copy_direction::from_user, 0x800ff9, 19)
            }
            .err(),
            Some(user_copy_error::owner_unavailable)
        );
        let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
        memory.active = root;
        let before = memory.words.clone();
        for read in 1..=8 {
            memory.fail_read = Some(memory.reads + read);
            assert_eq!(
                unsafe {
                    owner.preflight_user_copy(&mut memory, copy_direction::from_user, 0x800ff9, 19)
                }
                .err(),
                Some(user_copy_error::access_unavailable)
            );
            assert_eq!(memory.words, before);
            assert!(owner.is_ready());
        }
        memory.reset_faults();
        for mutation in 0..5 {
            match mutation {
                0 => memory.active = 0,
                1 => owner.tables.as_mut().unwrap().bound = false,
                2 => owner.data[2].as_mut().unwrap().bound = false,
                3 => owner.tables_cleared = true,
                4 => owner.stage = stage::withdrawing,
                _ => unreachable!(),
            }
            assert_eq!(
                unsafe {
                    owner.preflight_user_copy(&mut memory, copy_direction::from_user, 0x800ff9, 19)
                }
                .err(),
                Some(user_copy_error::owner_unavailable)
            );
            memory.active = root;
            owner.tables.as_mut().unwrap().bound = true;
            owner.data[2].as_mut().unwrap().bound = true;
            owner.tables_cleared = false;
            owner.stage = stage::ready;
        }
        assert_eq!(memory.words, before);
        memory.active = 0;
        finish(&mut owner, &mut allocator, &mut memory);
        assert_eq!(
            unsafe {
                owner.preflight_user_copy(&mut memory, copy_direction::from_user, 0x800000, 1)
            }
            .err(),
            Some(user_copy_error::owner_unavailable)
        );
    }

    #[test]
    fn inactive_copy_requires_distinct_live_root_and_checks_every_touched_caller_page() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        // SAFETY: The isolated model owns all table/data storage. The unrelated
        // active root is inert test metadata; every read uses modeled RAM.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
        let current_root = 0x9000000;
        memory.active = current_root;
        let before = memory.words.clone();
        let accounting = allocator.accounting();
        for (region, base) in [(2, 0x800000), (3, distant_stack)] {
            for direction in [copy_direction::from_user, copy_direction::to_user] {
                let spans = unsafe {
                    owner.preflight_inactive_user_copy(
                        &mut memory,
                        current_root,
                        direction,
                        base + page_size - 7,
                        19,
                    )
                }
                .unwrap();
                let range = owner.data_range(region).unwrap();
                assert_eq!(
                    spans.ranges(),
                    &[
                        physical_range::new(range.base + page_size - 7, 7),
                        physical_range::new(range.base + page_size, 12),
                    ]
                );
            }
        }
        // The old entry point still refuses to copy a noncurrent root.
        assert_eq!(
            unsafe {
                owner.preflight_user_copy(&mut memory, copy_direction::from_user, 0x800000, 1)
            }
            .err(),
            Some(user_copy_error::owner_unavailable)
        );
        for (address, length, error) in [
            (0x400000, 8, user_copy_error::permission_denied),
            (0x801fff, 2, user_copy_error::unmapped),
            (
                distant_stack + 2 * page_size - 1,
                2,
                user_copy_error::unmapped,
            ),
            (0xffff_9000_0000_0000, 8, user_copy_error::non_user_address),
            (u64::MAX - 3, 8, user_copy_error::address_overflow),
            (0x800000, 0, user_copy_error::empty),
            (0x800000, 4097, user_copy_error::too_large),
        ] {
            let reads = memory.reads;
            assert_eq!(
                unsafe {
                    owner.preflight_inactive_user_copy(
                        &mut memory,
                        current_root,
                        copy_direction::to_user,
                        address,
                        length,
                    )
                }
                .err(),
                Some(error)
            );
            assert_eq!(memory.reads, reads);
        }
        for (active, expected) in [(0, 0), (current_root, root), (root, root)] {
            memory.active = active;
            let reads = memory.reads;
            assert_eq!(
                unsafe {
                    owner.preflight_inactive_user_copy(
                        &mut memory,
                        expected,
                        copy_direction::to_user,
                        0x800ff9,
                        19,
                    )
                }
                .err(),
                Some(user_copy_error::owner_unavailable)
            );
            assert_eq!(memory.reads, reads);
        }
        memory.active = current_root;
        for read in 1..=8 {
            memory.fail_read = Some(memory.reads + read);
            assert_eq!(
                unsafe {
                    owner.preflight_inactive_user_copy(
                        &mut memory,
                        current_root,
                        copy_direction::to_user,
                        0x800ff9,
                        19,
                    )
                }
                .err(),
                Some(user_copy_error::access_unavailable)
            );
        }
        memory.reset_faults();
        let path = memory.path(root, 0x801000).unwrap();
        let leaf = memory
            .word(
                paging::entry_address(path[2]),
                paging::indexes(0x801000).table,
            )
            .unwrap();
        memory.words[leaf] &= !writable;
        let corrupted = memory.words.clone();
        assert_eq!(
            unsafe {
                owner.preflight_inactive_user_copy(
                    &mut memory,
                    current_root,
                    copy_direction::to_user,
                    0x800ff9,
                    19,
                )
            }
            .err(),
            Some(user_copy_error::translation_mismatch)
        );
        assert_eq!(memory.words, corrupted);
        memory.words[leaf] = before[leaf];
        assert_eq!(memory.words, before);
        assert_eq!(allocator.accounting(), accounting);
        assert!(owner.is_ready());
        memory.active = 0;
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn mixed_arena_cleanup_failures_retain_extra_table_ownership() {
        for failure in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = mixed_owner();
            // SAFETY: The isolated model supplies complete owned physical RAM.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            let table_range = owner.table_range().unwrap();
            assert_eq!(table_range.length, 10 * page_size);
            memory.fail_clear = Some(memory.clears + failure);
            // SAFETY: Partial clear reports failure and cannot authorize reuse.
            assert!(unsafe { owner.cleanup(&mut allocator, &mut memory) }.is_err());
            assert!(!owner.is_ready());
            assert_eq!(owner.table_range(), Some(table_range));
            assert_eq!(allocator.accounting().table_owned, 10);
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
            assert!(
                memory.words[memory.bounds(table_range).unwrap()]
                    .iter()
                    .all(|word| *word == 0)
            );
        }
    }

    fn ancestry_owner(high_only: bool) -> staged_mapping {
        // Same nine-region branch structure as main/interpreter/stack, with
        // small extents so failure injection stays within the 64-frame model.
        let requests = [
            (0x40000000, 1, read_only),
            (0x40002000, 2, read_execute),
            (0x40006000, 1, read_only),
            (0x40007000, 2, read_write),
            (0x60000000, 1, read_only),
            (0x60001000, 2, read_execute),
            (0x60030000, 1, read_only),
            (0x6003b000, 2, read_write),
            (0x7fff00000000, 2, read_write),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let user = mapping_plan::new(
            if high_only { &requests[8..] } else { &requests },
            &[],
            mapping_budget {
                data_pages: 14,
                table_frames: 8,
            },
        )
        .unwrap();
        let resident = resident_plan::new_with_ancestry(
            &user,
            &[
                resident_request {
                    virtual_address: 0x100000,
                    page_count: 2,
                    permissions: page_permissions {
                        user: false,
                        ..read_execute
                    },
                },
                resident_request {
                    virtual_address: 0x102000,
                    page_count: 1,
                    permissions: page_permissions {
                        user: false,
                        ..read_only
                    },
                },
                resident_request {
                    virtual_address: 0x104000,
                    page_count: 2,
                    permissions: page_permissions {
                        user: false,
                        ..read_write
                    },
                },
            ],
            frame_count as u64 * page_size | present | writable | paging::no_execute,
            if high_only { 7 } else { 10 },
        )
        .unwrap();
        staged_mapping::new_resident(user, resident).unwrap()
    }

    #[test]
    fn complete_ancestry_keeps_existing_shared_branch_tables_byte_identical() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = mixed_owner();
        // SAFETY: Both profiles use the same isolated RAM and inactive tree.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        let original_range = owner.table_range().unwrap();
        let original_tables = memory.words[memory.bounds(original_range).unwrap()].to_vec();
        finish(&mut owner, &mut allocator, &mut memory);
        let resident = owner.residency().unwrap();
        let requests: Vec<_> = resident
            .regions()
            .map(|region| resident_request {
                virtual_address: region.start(),
                page_count: (region.end() - region.start()) / page_size,
                permissions: region.permissions(),
            })
            .collect();
        let complete = resident_plan::new_with_ancestry(
            owner.plan(),
            &requests,
            resident.alias_entry(),
            resident.table_count(),
        )
        .unwrap();
        assert_eq!(complete.additional_table(0), Some((table_level::leaf, 0)));
        assert_eq!(complete.additional_table(1), None);
        owner.residency = Some(complete);
        // SAFETY: Full prior cleanup returned every lease before replacement.
        unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(owner.table_range(), Some(original_range));
        assert_eq!(
            memory.words[memory.bounds(original_range).unwrap()],
            original_tables
        );
        finish(&mut owner, &mut allocator, &mut memory);
    }

    #[test]
    fn supplemental_supervisor_paths_preserve_permissions_gaps_and_hardware_bits() {
        for high_only in [false, true] {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let baseline = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut owner = ancestry_owner(high_only);
            // SAFETY: Only the isolated fake-memory vector is accessed.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            assert_eq!(owner.table_count(), if high_only { 7 } else { 10 });
            assert_eq!(owner.retained_leases(), if high_only { 2 } else { 10 });
            let root = owner.root_address().unwrap();
            let residency = owner.residency().unwrap();
            for region in residency.regions() {
                for address in (region.start()..region.end()).step_by(page_size as usize) {
                    let path = memory.path(root, address).unwrap();
                    assert_eq!(paging::entry_address(path[3]), address);
                    assert_eq!(path[1] & user_accessible, 0);
                    assert_eq!(path[2] & user_accessible, 0);
                    assert_eq!(
                        path[0] & user_accessible,
                        if high_only { 0 } else { user_accessible }
                    );
                    assert_eq!(
                        paging::effective_permissions_from_path(&path),
                        effective_permissions {
                            present: true,
                            user: false,
                            writable: region.permissions().writable,
                            executable: region.permissions().executable,
                        },
                    );
                }
            }
            for (index, region) in owner.plan().regions().enumerate() {
                let range = owner.data_range(index).unwrap();
                for address in
                    (region.extent().start()..region.extent().end()).step_by(page_size as usize)
                {
                    let path = memory.path(root, address).unwrap();
                    assert_eq!(
                        paging::entry_address(path[3]),
                        range.base + address - region.extent().start()
                    );
                    assert_eq!(
                        paging::effective_permissions_from_path(&path),
                        effective_permissions {
                            present: true,
                            user: true,
                            writable: region.permissions().writable,
                            executable: region.permissions().executable,
                        },
                    );
                }
            }
            for gap in [
                0,
                0xff000,
                0x103000,
                0x106000,
                0x200000,
                0x40001000,
                0x40004000,
                0x60004000,
                0x7ffefffff000,
                0x7fff00002000,
            ] {
                assert!(memory.path(root, gap).is_none(), "gap {gap:x}");
            }
            // SAFETY: This authorizes modeled A/D validation, not CPU entry.
            unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
            // Exercise every present entry, including every supplemental level.
            // D is hardware-permitted only on writable leaves, never a parent.
            for index in 0..owner.table_count() {
                let table = owner.table_address(index).unwrap();
                let leaf = owner.complete_table_descriptor(index).unwrap().0 == table_level::leaf;
                for entry in 0..entries_per_table {
                    let slot = memory.word(table, entry).unwrap();
                    if memory.words[slot] & present != 0 {
                        memory.words[slot] |= 1 << 5;
                        if leaf && memory.words[slot] & writable != 0 {
                            memory.words[slot] |= 1 << 6;
                        }
                    }
                }
            }
            // SAFETY: All modifications stay in modeled retained tables.
            unsafe { owner.validate(&allocator, &mut memory) }.unwrap();
            finish(&mut owner, &mut allocator, &mut memory);
            assert_eq!(allocator.accounting(), baseline);
        }
    }

    #[test]
    fn supplemental_paths_reject_dirty_parents_privilege_changes_and_forged_gaps() {
        for high_only in [false, true] {
            for probe in 0..9 {
                let mut ledger = [0; frame_count / 4];
                let mut allocator = allocator(&mut ledger);
                let mut memory = fake_memory::new();
                let mut owner = ancestry_owner(high_only);
                // SAFETY: The fake vector models exclusive retained RAM.
                unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
                // SAFETY: The root remains inactive; permit only hardware A/D.
                let root = unsafe { owner.prepare_activation(&allocator, &mut memory) }.unwrap();
                let path = memory.path(root, 0x100000).unwrap();
                let (table, index, mask) = match probe {
                    0 => (root, 0, 1 << 6),
                    1 => (paging::entry_address(path[0]), 0, 1 << 6),
                    2 => (paging::entry_address(path[1]), 0, 1 << 6),
                    3 => (paging::entry_address(path[0]), 0, user_accessible),
                    4 => (paging::entry_address(path[1]), 0, user_accessible),
                    5 => (paging::entry_address(path[2]), 0x100, user_accessible),
                    6 => (paging::entry_address(path[2]), 0x100, 1 << 6),
                    7 => (paging::entry_address(path[2]), 0x103, present | writable),
                    _ => (paging::entry_address(path[0]), 0, page_size),
                };
                let slot = memory.word(table, index).unwrap();
                memory.words[slot] ^= mask;
                // SAFETY: The mutation cannot escape the inactive RAM model.
                assert_eq!(
                    unsafe { owner.validate(&allocator, &mut memory) },
                    Err(mapping_error::entry_mismatch),
                );
                assert!(!owner.is_ready());
                assert_retained(&owner, &allocator);
                finish(&mut owner, &mut allocator, &mut memory);
            }
        }
    }

    #[test]
    fn supplemental_tree_failures_retain_all_ownership_until_cleanup_succeeds() {
        // Arena detachment, nine data clears and final table-lease clear.
        for failure in 1..=11 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = ancestry_owner(false);
            // SAFETY: The isolated model supplies complete owned physical RAM.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            let tables = owner.table_range().unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            // SAFETY: A failed partial wipe cannot authorize physical reuse.
            assert!(unsafe { owner.cleanup(&mut allocator, &mut memory) }.is_err());
            assert!(!owner.is_ready());
            assert_eq!(owner.table_range(), Some(tables));
            assert_eq!(allocator.accounting().table_owned, 10);
            assert_retained(&owner, &allocator);
            finish(&mut owner, &mut allocator, &mut memory);
            assert!(
                memory.words[memory.bounds(tables).unwrap()]
                    .iter()
                    .all(|word| *word == 0)
            );
        }
        // Fail after writing each synthesized path link. No hardware table path
        // is ever followed to discover cleanup ownership.
        for high_only in [false, true] {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let mut memory = fake_memory::new();
            let mut owner = ancestry_owner(high_only);
            // SAFETY: Establish deterministic population positions in owned RAM.
            unsafe { owner.build(&mut allocator, &mut memory) }.unwrap();
            let first_extra = owner.table_address(owner.plan.tables().len()).unwrap();
            let table_end = owner.table_range().unwrap().end().unwrap();
            let mut positions = Vec::new();
            let mut position = 0;
            for table in (0..owner.table_count()).rev() {
                for entry in 0..entries_per_table {
                    let expected = owner.expected_entry(table, entry).unwrap();
                    if expected == 0 {
                        continue;
                    }
                    position += 1;
                    let child = paging::entry_address(expected);
                    if owner.complete_table_descriptor(table).unwrap().0 != table_level::leaf
                        && child >= first_extra
                        && child < table_end
                    {
                        positions.push(position);
                    }
                }
            }
            assert_eq!(positions.len(), if high_only { 3 } else { 2 });
            finish(&mut owner, &mut allocator, &mut memory);
            for position in positions {
                memory.writes = 0;
                memory.fail_write = Some(position);
                // SAFETY: The failed write changes modeled storage, then reports it.
                assert_eq!(
                    unsafe { owner.build(&mut allocator, &mut memory) },
                    Err(mapping_error::write_failed),
                );
                assert!(!owner.is_ready());
                assert_retained(&owner, &allocator);
                finish(&mut owner, &mut allocator, &mut memory);
            }
        }
    }
}

// A host-only specification model; no runtime permission adapter is admitted.
#[cfg(all(test, feature = "linux_loader_entry_test"))]
mod permission_replacement;
