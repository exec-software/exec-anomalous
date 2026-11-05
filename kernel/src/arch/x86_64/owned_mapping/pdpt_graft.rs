//! One retained caller graft at the 4 GiB directory-pointer slot.
//!
//! Publication alone cannot authorize caller activation. A separate complete
//! composed-tree admission permits the retained subtree's hardware A bits,
//! while the activation owner still proves residency and caller authority.
//! Publication uncertainty and translation invalidation remain distinct gates.

use super::*;
use crate::arch::x86_64::user_copy::{copy_byte_limit, copy_direction, user_copy_error};

const window_base: u64 = 1 << 32;
const window_end: u64 = window_base + (1 << 30);
const parent_slot: usize = 4;
const accessed: u64 = 1 << 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) enum graft_phase {
    prepared,
    uncertain,
    published,
    detached,
    withdrawn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::arch::x86_64) enum graft_error {
    invalid_state,
    invalid_geometry,
    owner_mismatch,
    active_root,
    mapping(mapping_error),
    publication_unknown,
    withdrawal_unknown,
}

/// An exact external-reference receipt, never a substitute for either owner.
/// No Drop removes references or frees storage. The runtime owner must fence
/// cleanup before invoking publish and retain that fence through invalidation.
#[must_use = "retain the graft and both owners until withdrawal and invalidation succeed"]
pub(in crate::arch::x86_64) struct pdpt_graft {
    caller_tables: physical_range,
    candidate_tables: physical_range,
    candidate_data: physical_range,
    parent_table: u64,
    directory_entry: u64,
    virtual_end: u64,
    populated_end: u64,
    phase: graft_phase,
    hardware_admitted: bool,
}

impl pdpt_graft {
    /// Inspect complete retained trees and derive the sole permitted edge.
    /// Full virtual ownership includes the candidate's exact absent tail.
    ///
    /// # Safety
    /// The activation owner authenticates the blocked caller and immutable
    /// source, retains both mappings and their independent physical alias, and
    /// excludes interrupts, NMI, other CPUs and owner mutation. Neither root is
    /// current. No pointer or receipt permits dropping or cleaning either owner.
    pub(in crate::arch::x86_64) unsafe fn prepare(
        caller: &staged_mapping,
        candidate: &staged_mapping,
        virtual_end: u64,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<Self, graft_error> {
        let populated_end = validate_geometry(caller, candidate, virtual_end)?;
        require_inactive(caller, candidate, memory)?;
        // SAFETY: Both complete inactive owners and the allocator remain live.
        unsafe { caller.verify(allocator, memory) }.map_err(graft_error::mapping)?;
        // SAFETY: The same exclusion and retained access cover this private tree.
        unsafe { candidate.verify(allocator, memory) }.map_err(graft_error::mapping)?;
        let parent_index = table_index(caller, table_level::directory_pointer, 0)?;
        let candidate_parent = table_index(candidate, table_level::directory_pointer, 0)?;
        let directory = table_index(candidate, table_level::directory, 4)?;
        if caller
            .expected_entry(parent_index, parent_slot)
            .map_err(graft_error::mapping)?
            != 0
        {
            return Err(graft_error::invalid_geometry);
        }
        let directory_entry = candidate
            .expected_entry(candidate_parent, parent_slot)
            .map_err(graft_error::mapping)?;
        let expected = candidate
            .table_address(directory)
            .map_err(graft_error::mapping)?
            | present
            | writable
            | user_accessible
            | ownership_marker;
        if directory_entry != expected {
            return Err(graft_error::invalid_geometry);
        }
        Ok(Self {
            caller_tables: caller.table_range().ok_or(graft_error::owner_mismatch)?,
            candidate_tables: candidate.table_range().ok_or(graft_error::owner_mismatch)?,
            candidate_data: candidate.data_range(0).ok_or(graft_error::owner_mismatch)?,
            parent_table: caller
                .table_address(parent_index)
                .map_err(graft_error::mapping)?,
            directory_entry,
            virtual_end,
            populated_end,
            phase: graft_phase::prepared,
            hardware_admitted: false,
        })
    }

    pub(in crate::arch::x86_64) const fn phase(&self) -> graft_phase {
        self.phase
    }

    pub(in crate::arch::x86_64) const fn parent_table_address(&self) -> u64 {
        self.parent_table
    }

    pub(in crate::arch::x86_64) const fn installed_entry(&self) -> u64 {
        self.directory_entry
    }

    pub(in crate::arch::x86_64) const fn hardware_admitted(&self) -> bool {
        self.hardware_admitted
    }

    /// Publish exactly one edge after repeating the complete preflight.
    /// Any failed store/readback is uncertain, including false-after-write.
    ///
    /// # Safety
    /// The prepare contract still holds, and all fallible logical ownership and
    /// circle admission has completed. A surrounding runtime owner is already
    /// uncertain before entry to this method. The caller cannot resume, and the
    /// candidate cannot be cleaned, after any attempted external table store.
    pub(in crate::arch::x86_64) unsafe fn publish(
        &mut self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), graft_error> {
        if self.phase != graft_phase::prepared {
            return Err(graft_error::invalid_state);
        }
        self.validate_identity(caller, candidate)?;
        require_inactive(caller, candidate, memory)?;
        // SAFETY: Recheck both unchanged trees immediately before the store.
        unsafe { caller.verify(allocator, memory) }.map_err(graft_error::mapping)?;
        // SAFETY: Candidate ownership and source initialization remain retained.
        unsafe { candidate.verify(allocator, memory) }.map_err(graft_error::mapping)?;
        self.phase = graft_phase::uncertain;
        // SAFETY: The exact caller-owned parent frame is inactive, exclusively
        // retained and independently verified zero at the bounded slot.
        if !unsafe { memory.write_entry(self.parent_table, parent_slot, self.directory_entry) } {
            return Err(graft_error::publication_unknown);
        }
        // SAFETY: Read back through the same retained independent physical alias.
        if unsafe { memory.read_entry(self.parent_table, parent_slot) }
            != Some(self.directory_entry)
        {
            return Err(graft_error::publication_unknown);
        }
        self.phase = graft_phase::published;
        Ok(())
    }

    /// Admit hardware use of the exact published subtree, without selecting a
    /// root. The private candidate PML4/PDPT are never selected and remain exact.
    /// An admission is conservative: A bits are allowed after this succeeds even
    /// if the activation owner subsequently cancels before running the caller.
    ///
    /// # Safety
    /// Both complete owners, source, alias and circle attachment remain live.
    /// The authenticated caller is blocked and both roots are inactive under
    /// sole-BSP, IF-clear/NMI-masked exclusion. The activation owner separately
    /// proves the caller's supervisor residency, CPU state and saved frame. It
    /// prevents all cleanup until that caller and all saved continuations are
    /// quiescent, the exact graft is detached and translations are invalidated.
    pub(in crate::arch::x86_64) unsafe fn admit_hardware_use(
        &mut self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, graft_error> {
        if self.phase != graft_phase::published || self.hardware_admitted {
            return Err(graft_error::invalid_state);
        }
        caller
            .verify_ownership(allocator)
            .map_err(graft_error::mapping)?;
        candidate
            .verify_ownership(allocator)
            .map_err(graft_error::mapping)?;
        // SAFETY: The caller retains both complete inactive trees and source.
        let root = unsafe { self.verify_composed(caller, candidate, memory) }?;
        self.hardware_admitted = true;
        Ok(root)
    }

    /// Revalidate the complete admitted composition before another selection.
    /// This preserves the same ownership and narrow hardware-bit policy.
    ///
    /// # Safety
    /// The complete admission contract still holds. The caller and candidate
    /// roots are inactive, and neither may be selected until this check succeeds.
    pub(in crate::arch::x86_64) unsafe fn verify_hardware_use(
        &self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        allocator: &frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, graft_error> {
        if self.phase != graft_phase::published || !self.hardware_admitted {
            return Err(graft_error::invalid_state);
        }
        caller
            .verify_ownership(allocator)
            .map_err(graft_error::mapping)?;
        candidate
            .verify_ownership(allocator)
            .map_err(graft_error::mapping)?;
        // SAFETY: The activation owner retains the same inactive composition.
        unsafe { self.verify_composed(caller, candidate, memory) }
    }

    /// Repeat complete tree and retained-lease checks without an allocator borrow.
    ///
    /// # Safety
    /// The activation owner preserves every allocator binding from successful
    /// admission through this sole-BSP exclusion interval. Both roots remain
    /// inactive, with live source, aliases, owner identities and circle admission.
    /// No allocator or owner mutation may intervene before root selection.
    pub(in crate::arch::x86_64) unsafe fn verify_selection(
        &self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, graft_error> {
        if self.phase != graft_phase::published || !self.hardware_admitted {
            return Err(graft_error::invalid_state);
        }
        // SAFETY: The activation owner preserves the admitted allocator bindings.
        unsafe { self.verify_composed(caller, candidate, memory) }
    }

    /// Remove the exact external edge, with retry after uncertain progress.
    /// This alone does not permit cleanup; invalidation remains a separate gate.
    ///
    /// # Safety
    /// Both owners and their alias remain live, the caller and every possible
    /// continuation are barred from execution and both roots are inactive.
    /// Admitted hardware use may have added A to this exact edge. No owner,
    /// table or logical attachment can change before invalidation completes.
    pub(in crate::arch::x86_64) unsafe fn detach(
        &mut self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
    ) -> Result<(), graft_error> {
        if !matches!(
            self.phase,
            graft_phase::published | graft_phase::uncertain | graft_phase::detached
        ) {
            return Err(graft_error::invalid_state);
        }
        self.phase = graft_phase::uncertain;
        self.validate_identity(caller, candidate)?;
        require_inactive(caller, candidate, memory)?;
        // SAFETY: Only the recorded caller-owned parent is inspected; no actual
        // PTE value can select a different address for subsequent access.
        match unsafe { memory.read_entry(self.parent_table, parent_slot) } {
            Some(0) => {
                // SAFETY: An earlier uncertain attempt may have removed the
                // edge, but retained candidate entries still require validation.
                unsafe { self.verify_candidate(candidate, memory) }?;
            }
            Some(entry) if self.matches_external_entry(entry) => {
                // SAFETY: Every candidate table remains owned and inactive;
                // only the admitted subtree may contain hardware A additions.
                unsafe { self.verify_candidate(candidate, memory) }?;
                // SAFETY: All consumers are blocked and the entry is our exact
                // retained reference. False may still have stored zero.
                if !unsafe { memory.write_entry(self.parent_table, parent_slot, 0) } {
                    return Err(graft_error::withdrawal_unknown);
                }
                // SAFETY: The complete parent remains live until confirmation.
                if unsafe { memory.read_entry(self.parent_table, parent_slot) } != Some(0) {
                    return Err(graft_error::withdrawal_unknown);
                }
            }
            _ => return Err(graft_error::withdrawal_unknown),
        }
        self.phase = graft_phase::detached;
        Ok(())
    }

    /// Record the activation owner's completed translation/consumer withdrawal.
    /// A successful return is the prerequisite for runtime confirm_withdrawal.
    ///
    /// # Safety
    /// No external consumer or saved continuation may reach this candidate.
    /// The exact edge is detached, and required TLB invalidation has completed.
    /// In the present sole-BSP proof, no PCID or global translations exist and
    /// caller withdrawal has selected an independent root, then invalidated all
    /// prior non-global translations after detach. The caller and every saved
    /// continuation remain barred until cleanup ends. Reloading only a different
    /// PCID would not suffice; this contract does not admit SMP shootdowns.
    pub(in crate::arch::x86_64) unsafe fn confirm_invalidation(
        &mut self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
    ) -> Result<(), graft_error> {
        if self.phase != graft_phase::detached {
            return Err(graft_error::invalid_state);
        }
        self.phase = graft_phase::uncertain;
        self.validate_identity(caller, candidate)?;
        require_inactive(caller, candidate, memory)?;
        // SAFETY: No consumer can run between the final check and confirmation.
        if unsafe { memory.read_entry(self.parent_table, parent_slot) } != Some(0) {
            return Err(graft_error::withdrawal_unknown);
        }
        self.phase = graft_phase::withdrawn;
        Ok(())
    }

    /// Compose ordinary caller regions and the owned runtime prefix. Each page
    /// is checked before any copy; crossing into an absent tail never exposes a
    /// successful prefix. Both owners and this receipt remain borrowed.
    ///
    /// # Safety
    /// The activation owner authenticates both retained tasks, saved caller
    /// frame and current veil root/stack under one sole-BSP exclusion interval.
    /// The caller remains inactive; no owner, table, alias or attachment changes
    /// until the returned physical spans have been consumed.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::arch::x86_64) unsafe fn preflight_inactive_user_copy<'a>(
        &'a self,
        caller: &'a staged_mapping,
        candidate: &'a staged_mapping,
        memory: &mut impl mapping_memory,
        current_root: u64,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if current_root == 0
            || memory.active_root() != current_root
            || require_inactive(caller, candidate, memory).is_err()
        {
            return Err(user_copy_error::owner_unavailable);
        }
        // SAFETY: The authenticated veil owns the current root and excludes
        // caller execution until all returned physical spans are consumed.
        unsafe {
            self.preflight_composed_user_copy(caller, candidate, memory, direction, address, length)
        }
    }

    /// Compose both retained owners while the exact admitted caller is current.
    /// Every touched translation is checked before any byte can be exposed.
    ///
    /// # Safety
    /// The activation owner authenticates the exact current caller task, frame,
    /// circle and privileged stack. Both owners, source and physical alias are
    /// retained under sole-BSP IF-clear/NMI-masked exclusion until the spans are
    /// consumed. No allocator, alias, table or attachment mutation may intervene.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::arch::x86_64) unsafe fn preflight_user_copy<'a>(
        &'a self,
        caller: &'a staged_mapping,
        candidate: &'a staged_mapping,
        memory: &mut impl mapping_memory,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if !self.hardware_admitted
            || caller.root_address() != Some(memory.active_root())
            || candidate.root_address() == Some(memory.active_root())
        {
            return Err(user_copy_error::owner_unavailable);
        }
        // SAFETY: The current root is the authenticated retained caller, and
        // the same excluded lifetime protects both owners and the graft.
        unsafe {
            self.preflight_composed_user_copy(caller, candidate, memory, direction, address, length)
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn preflight_composed_user_copy<'a>(
        &'a self,
        caller: &'a staged_mapping,
        candidate: &'a staged_mapping,
        memory: &mut impl mapping_memory,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<checked_copy_spans<'a>, user_copy_error> {
        if self.phase != graft_phase::published
            || self.validate_identity(caller, candidate).is_err()
        {
            return Err(user_copy_error::owner_unavailable);
        }
        if length == 0 {
            return Err(user_copy_error::empty);
        }
        if length > copy_byte_limit {
            return Err(user_copy_error::too_large);
        }
        let end = address
            .checked_add(length as u64)
            .ok_or(user_copy_error::address_overflow)?;
        if address >= super::super::mapping_plan::user_address_limit
            || end > super::super::mapping_plan::user_address_limit
        {
            return Err(user_copy_error::non_user_address);
        }
        // SAFETY: The edge is in the retained caller table, not an actual-entry
        // supplied address. Check it even for copies of original caller pages.
        if !unsafe { memory.read_entry(self.parent_table, parent_slot) }
            .is_some_and(|entry| self.matches_external_entry(entry))
        {
            return Err(user_copy_error::translation_mismatch);
        }
        let mut result = checked_copy_spans {
            ranges: [physical_range::new(0, 0); copy_span_capacity],
            count: 0,
            _owner: caller,
            _additional_owner: Some(candidate),
            _graft: Some(self),
        };
        let mut cursor = address;
        while cursor < end {
            let next = end.min((cursor & !(page_size - 1)) + page_size);
            let count = (next - cursor) as usize;
            let range = if cursor >= window_base && cursor < self.virtual_end {
                // SAFETY: The same caller/veil exclusion covers both owners.
                unsafe {
                    self.preflight_candidate_page(
                        caller, candidate, memory, direction, cursor, count,
                    )
                }?
            } else {
                // SAFETY: The public entry point authenticated either the exact
                // current caller or its suspended copy authority. The original
                // owner validates its paths; none of them use the graft edge.
                let spans =
                    unsafe { caller.preflight_owned_user_copy(memory, direction, cursor, count) }?;
                if spans.ranges().len() != 1 {
                    return Err(user_copy_error::translation_mismatch);
                }
                spans.ranges()[0]
            };
            let slot = result
                .ranges
                .get_mut(result.count)
                .ok_or(user_copy_error::too_large)?;
            *slot = range;
            result.count += 1;
            cursor = next;
        }
        Ok(result)
    }

    fn validate_identity(
        &self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
    ) -> Result<(), graft_error> {
        if validate_geometry(caller, candidate, self.virtual_end)? != self.populated_end
            || caller.table_range() != Some(self.caller_tables)
            || candidate.table_range() != Some(self.candidate_tables)
            || candidate.data_range(0) != Some(self.candidate_data)
        {
            return Err(graft_error::owner_mismatch);
        }
        let parent = table_index(caller, table_level::directory_pointer, 0)?;
        let directory = table_index(candidate, table_level::directory, 4)?;
        if caller.table_address(parent).map_err(graft_error::mapping)? != self.parent_table
            || caller
                .expected_entry(parent, parent_slot)
                .map_err(graft_error::mapping)?
                != 0
            || candidate
                .table_address(directory)
                .map_err(graft_error::mapping)?
                | present
                | writable
                | user_accessible
                | ownership_marker
                != self.directory_entry
        {
            return Err(graft_error::owner_mismatch);
        }
        Ok(())
    }

    fn external_runtime_bits(&self) -> u64 {
        if self.hardware_admitted { accessed } else { 0 }
    }

    fn matches_external_entry(&self, actual: u64) -> bool {
        actual & !self.external_runtime_bits() == self.directory_entry
    }

    fn candidate_runtime_bits(
        &self,
        candidate: &staged_mapping,
        table_index: usize,
        expected: u64,
    ) -> u64 {
        if self.hardware_admitted
            && expected & present != 0
            && candidate
                .plan
                .tables()
                .get(table_index)
                .is_some_and(|node| {
                    matches!(node.level(), table_level::directory | table_level::leaf)
                })
        {
            accessed
        } else {
            0
        }
    }

    unsafe fn verify_candidate(
        &self,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
    ) -> Result<(), graft_error> {
        candidate
            .verify_lease_geometry()
            .map_err(graft_error::mapping)?;
        for index in 0..candidate.table_count() {
            let table = candidate
                .table_address(index)
                .map_err(graft_error::mapping)?;
            for slot in 0..entries_per_table {
                let expected = candidate
                    .expected_entry(index, slot)
                    .map_err(graft_error::mapping)?;
                let permitted = self.candidate_runtime_bits(candidate, index, expected);
                // SAFETY: The address comes from the retained candidate lease;
                // no observed parent value selects a subsequent table address.
                let actual = unsafe { memory.read_entry(table, slot) }
                    .ok_or(graft_error::mapping(mapping_error::read_failed))?;
                if actual & !permitted != expected {
                    return Err(graft_error::mapping(mapping_error::entry_mismatch));
                }
            }
        }
        Ok(())
    }

    unsafe fn verify_composed(
        &self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
    ) -> Result<u64, graft_error> {
        self.validate_identity(caller, candidate)?;
        require_inactive(caller, candidate, memory)?;
        caller
            .verify_lease_geometry()
            .map_err(graft_error::mapping)?;
        // SAFETY: Both complete owners are inactive and their retained geometry
        // is checked separately from any actual table contents.
        unsafe { self.verify_candidate(candidate, memory) }?;
        for index in 0..caller.table_count() {
            let table = caller.table_address(index).map_err(graft_error::mapping)?;
            for slot in 0..entries_per_table {
                let external = table == self.parent_table && slot == parent_slot;
                let expected = if external {
                    self.directory_entry
                } else {
                    caller
                        .expected_entry(index, slot)
                        .map_err(graft_error::mapping)?
                };
                let permitted = if external {
                    self.external_runtime_bits()
                } else {
                    caller.permitted_runtime_bits(index, expected)
                };
                // SAFETY: Only retained caller-owned table pages are read; the
                // one external edge is fixed by the authenticated graft receipt.
                let actual = unsafe { memory.read_entry(table, slot) }
                    .ok_or(graft_error::mapping(mapping_error::read_failed))?;
                if actual & !permitted != expected {
                    return Err(graft_error::mapping(mapping_error::entry_mismatch));
                }
            }
        }
        caller.root_address().ok_or(graft_error::owner_mismatch)
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn preflight_candidate_page(
        &self,
        caller: &staged_mapping,
        candidate: &staged_mapping,
        memory: &mut impl mapping_memory,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<physical_range, user_copy_error> {
        let plan = copy_plan::new(candidate.plan(), direction, address, length)?;
        let span = plan.spans().first().ok_or(user_copy_error::unmapped)?;
        if plan.spans().len() != 1 || span.region != 0 {
            return Err(user_copy_error::translation_mismatch);
        }
        let data = candidate.data[0]
            .as_ref()
            .ok_or(user_copy_error::owner_unavailable)?;
        let pages = (self.populated_end - window_base) / page_size;
        let range = checked_range(&data.lease, frame_purpose::data, pages as usize)
            .map_err(|_| user_copy_error::owner_unavailable)?;
        if !data.bound || range != self.candidate_data {
            return Err(user_copy_error::owner_unavailable);
        }
        let physical = range
            .base
            .checked_add(span.region_offset)
            .ok_or(user_copy_error::owner_unavailable)?;
        if span
            .region_offset
            .checked_add(length as u64)
            .is_none_or(|end| end > range.length)
        {
            return Err(user_copy_error::owner_unavailable);
        }
        let indexes = paging::indexes(address);
        let mut path = [0; 4];
        for (depth, (owner, level, prefix, slot)) in [
            (caller, table_level::root, 0, indexes.pml4),
            (caller, table_level::directory_pointer, 0, parent_slot),
            (candidate, table_level::directory, 4, indexes.directory),
            (candidate, table_level::leaf, address >> 21, indexes.table),
        ]
        .into_iter()
        .enumerate()
        {
            let index = table_index(owner, level, prefix)
                .map_err(|_| user_copy_error::translation_mismatch)?;
            let table = owner
                .table_address(index)
                .map_err(|_| user_copy_error::owner_unavailable)?;
            let expected = if depth == 1 {
                self.directory_entry
            } else {
                owner
                    .expected_entry(index, slot)
                    .map_err(|_| user_copy_error::owner_unavailable)?
            };
            let permitted = if depth == 1 {
                self.external_runtime_bits()
            } else if depth >= 2 {
                self.candidate_runtime_bits(candidate, index, expected)
            } else {
                owner.permitted_runtime_bits(index, expected)
            };
            // SAFETY: Every table address derives from one retained lease and
            // an immutable plan descriptor, never the preceding actual entry.
            let actual = unsafe { memory.read_entry(table, slot) }
                .ok_or(user_copy_error::access_unavailable)?;
            if expected & present == 0 || actual & !permitted != expected {
                return Err(user_copy_error::translation_mismatch);
            }
            path[depth] = actual;
        }
        let permissions = paging::effective_permissions_from_path(&path);
        if !permissions.present
            || !permissions.user
            || permissions.writable
            || permissions.executable
            || paging::entry_address(path[3]) != physical & !(page_size - 1)
        {
            return Err(user_copy_error::translation_mismatch);
        }
        Ok(physical_range::new(physical, length as u64))
    }
}

fn table_index(
    owner: &staged_mapping,
    level: table_level,
    prefix: u64,
) -> Result<usize, graft_error> {
    owner
        .plan
        .tables()
        .iter()
        .position(|node| node.level() == level && node.prefix() == prefix)
        .ok_or(graft_error::invalid_geometry)
}

fn require_inactive(
    caller: &staged_mapping,
    candidate: &staged_mapping,
    memory: &impl mapping_memory,
) -> Result<(), graft_error> {
    let current = paging::entry_address(memory.active_root());
    if current == 0
        || caller.root_address() == Some(current)
        || candidate.root_address() == Some(current)
    {
        return Err(graft_error::active_root);
    }
    Ok(())
}

fn validate_geometry(
    caller: &staged_mapping,
    candidate: &staged_mapping,
    virtual_end: u64,
) -> Result<u64, graft_error> {
    if !caller.is_ready()
        || !caller.hardware_used
        || caller.tables_cleared
        || !candidate.is_ready()
        || candidate.hardware_used
        || candidate.tables_cleared
        || candidate.residency.is_some()
        || virtual_end <= window_base
        || virtual_end > window_end
        || !virtual_end.is_multiple_of(page_size)
    {
        return Err(graft_error::invalid_geometry);
    }
    for owner in [caller, candidate] {
        let tables = owner.tables.as_ref().ok_or(graft_error::owner_mismatch)?;
        checked_range(
            &tables.lease,
            frame_purpose::page_table,
            owner.table_count(),
        )
        .map_err(graft_error::mapping)?;
        if !tables.bound || owner.data.iter().flatten().any(|data| !data.bound) {
            return Err(graft_error::owner_mismatch);
        }
    }
    let region = candidate
        .plan
        .regions()
        .next()
        .ok_or(graft_error::invalid_geometry)?;
    let populated_end = region.extent().end();
    if candidate.plan.regions().count() != 1
        || candidate.data.iter().flatten().count() != 1
        || region.extent().start() != window_base
        || populated_end > virtual_end
        || !region.permissions().user
        || region.permissions().writable
        || region.permissions().executable
        || caller.plan.regions().any(|region| {
            region.extent().start() < virtual_end && window_base < region.extent().end()
        })
        || caller
            .plan
            .reservations()
            .any(|region| region.start() < virtual_end && window_base < region.end())
    {
        return Err(graft_error::invalid_geometry);
    }
    let tail = candidate.plan.reservations().next();
    if candidate.plan.reservations().count() != usize::from(populated_end < virtual_end)
        || tail.is_some_and(|tail| tail.start() != populated_end || tail.end() != virtual_end)
    {
        return Err(graft_error::invalid_geometry);
    }
    Ok(populated_end)
}

#[cfg(test)]
mod tests;
