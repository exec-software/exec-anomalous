#![cfg_attr(not(test), allow(dead_code))]

use core::ptr;

use super::paging::{
    self, effective_permissions, entries_per_table, huge_page, no_execute, ownership_marker,
    page_address_mask, page_permissions, page_size, present, user_accessible, writable,
};
use crate::memory::physical::{
    frame_allocator, frame_error, frame_lease, frame_purpose, physical_range,
};

const private_table_frame_count: usize = 4;
pub(super) const mapping_capacity: usize = 3;
const accessed: u64 = 1 << 5;
const dirty: u64 = 1 << 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum address_space_error {
    frame(frame_error),
    invalid_kernel_root,
    unsafe_kernel_branch,
    invalid_table_extent,
    invalid_virtual_address,
    kernel_branch_reserved,
    invalid_data_lease,
    write_execute,
    mapping_exists,
    mapping_branch_mismatch,
    mapping_capacity_exhausted,
    mapping_missing,
    mapping_mismatch,
    corrupt_tables,
    active_root,
    activation_failed,
    rollback_failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct mapping_evidence {
    pub virtual_address: u64,
    pub physical_address: u64,
    pub permissions: effective_permissions,
}

struct mapped_extent {
    virtual_address: u64,
    data_lease: frame_lease,
    permissions: page_permissions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct virtual_extent {
    base: u64,
    length: u64,
}

impl virtual_extent {
    fn end(self) -> Option<u64> {
        self.base.checked_add(self.length)
    }

    fn page_count(self) -> Option<usize> {
        if self.length == 0 || !self.length.is_multiple_of(page_size) {
            return None;
        }
        usize::try_from(self.length / page_size).ok()
    }

    fn page_address(self, page: usize) -> Option<u64> {
        if page >= self.page_count()? {
            return None;
        }
        let offset = u64::try_from(page).ok()?.checked_mul(page_size)?;
        self.base.checked_add(offset)
    }

    fn contains_page(self, virtual_address: u64) -> bool {
        virtual_address & (page_size - 1) == 0
            && virtual_address >= self.base
            && self.end().is_some_and(|end| virtual_address < end)
    }

    fn overlaps(self, other: Self) -> bool {
        match (self.end(), other.end()) {
            (Some(end), Some(other_end)) => self.base < other_end && other.base < end,
            _ => false,
        }
    }

    fn contains_leaf_index(self, index: usize) -> bool {
        let start = paging::indexes(self.base).table;
        self.page_count()
            .and_then(|count| start.checked_add(count))
            .is_some_and(|end| (start..end).contains(&index))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct mapping_branch {
    pml4: usize,
    directory_pointer: usize,
    directory: usize,
}

impl mapping_branch {
    const fn from_indexes(indexes: paging::page_indexes) -> Self {
        Self {
            pml4: indexes.pml4,
            directory_pointer: indexes.directory_pointer,
            directory: indexes.directory,
        }
    }

    const fn contains(self, indexes: paging::page_indexes) -> bool {
        self.pml4 == indexes.pml4
            && self.directory_pointer == indexes.directory_pointer
            && self.directory == indexes.directory
    }
}

/// One allocator-backed x86-64 root with three deliberately bounded extents.
///
/// The root borrows only the active kernel's PML4 entry zero. Its four-frame
/// table lease owns the root and one PML4/PDPT/PD branch ending in one leaf
/// table. Up to three logical mappings may each occupy one or more consecutive
/// leaves in that shared two-MiB branch. A successful mapping transfers its
/// non-copyable contiguous data lease into this owner. The lease returns to the
/// caller only after `unmap_owned` clears every owned leaf and withdraws the
/// allocator binding.
pub(super) struct address_space {
    table_lease: frame_lease,
    borrowed_kernel_entry: u64,
    mappings: [Option<mapped_extent>; mapping_capacity],
    mapping_count: usize,
}

impl address_space {
    /// Create one root from allocator-authorized, cleared table frames.
    ///
    /// # Safety
    ///
    /// `clear_and_verify` must satisfy `frame_allocator::allocate_with`'s
    /// exclusive whole-range clearing contract for every supplied extent.
    pub(super) unsafe fn create<F>(
        allocator: &mut frame_allocator<'_>,
        kernel_cr3: u64,
        mut clear_and_verify: F,
    ) -> Result<Self, address_space_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        let kernel_root = paging::entry_address(kernel_cr3);
        if kernel_root == 0
            || kernel_root & (page_size - 1) != 0
            || kernel_root >= paging::identity_map_limit
            || paging::entry_address(paging::read_cr3()) != kernel_root
        {
            return Err(address_space_error::invalid_kernel_root);
        }
        // SAFETY: The active root is page aligned, below the identity limit,
        // and remains live while the new root borrows its first entry.
        let kernel_entry = unsafe { read_table_entry(kernel_root, 0) };
        let kernel_branch = paging::entry_address(kernel_entry);
        if kernel_entry & present == 0
            || kernel_entry & user_accessible != 0
            || kernel_entry & huge_page != 0
            || kernel_entry & no_execute != 0
            || kernel_entry & !(page_address_mask | 0x0fff) != 0
            || kernel_branch == 0
            || kernel_branch >= paging::identity_map_limit
        {
            return Err(address_space_error::unsafe_kernel_branch);
        }
        let borrowed_kernel_entry = kernel_entry & !ownership_marker;

        // SAFETY: The caller accepted the allocator callback contract above.
        let mut table_lease = unsafe {
            allocator.allocate_with(
                private_table_frame_count,
                1,
                frame_purpose::page_table,
                &mut clear_and_verify,
            )
        }
        .map_err(address_space_error::frame)?;
        let Some(table_range) = table_lease.range() else {
            // SAFETY: The new lease was never published or bound.
            if unsafe { allocator.release_with(&mut table_lease, &mut clear_and_verify) }.is_err() {
                return Err(address_space_error::rollback_failed);
            }
            return Err(address_space_error::invalid_table_extent);
        };
        if table_range.length != private_table_frame_count as u64 * page_size
            || table_range.base & (page_size - 1) != 0
            || table_range
                .end()
                .is_none_or(|end| end > paging::identity_map_limit)
        {
            // SAFETY: The invalid lease extent was never published or bound.
            if unsafe { allocator.release_with(&mut table_lease, &mut clear_and_verify) }.is_err() {
                return Err(address_space_error::rollback_failed);
            }
            return Err(address_space_error::invalid_table_extent);
        }
        if let Err(error) = allocator.bind(&table_lease) {
            // SAFETY: Binding failed, so no ownership graph can use the lease.
            if unsafe { allocator.release_with(&mut table_lease, &mut clear_and_verify) }.is_err() {
                return Err(address_space_error::rollback_failed);
            }
            return Err(address_space_error::frame(error));
        }

        // SAFETY: The allocator returned four exclusive, cleared table frames.
        unsafe { write_table_entry(table_range.base, 0, borrowed_kernel_entry) };
        Ok(Self {
            table_lease,
            borrowed_kernel_entry,
            mappings: [const { None }; mapping_capacity],
            mapping_count: 0,
        })
    }

    pub(super) fn root_address(&self) -> Result<u64, address_space_error> {
        self.table_address(0)
    }

    /// Bind one owned frame at a vacant page in the shared managed branch.
    ///
    /// Every failure returns the still-owned input lease. All fallible table,
    /// shape, capacity, and allocator checks complete before any page-table
    /// entry changes, so a rejected map leaves both ownership graphs intact.
    pub(super) fn map_owned(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        virtual_address: u64,
        data_lease: frame_lease,
        permissions: page_permissions,
    ) -> Result<(), (address_space_error, frame_lease)> {
        if let Err(error) = validate_virtual_mapping(virtual_address, &data_lease, permissions) {
            return Err((error, data_lease));
        }
        self.map_owned_extent(allocator, virtual_address, data_lease, permissions)
    }

    /// Bind one contiguous owned data extent to consecutive managed leaves.
    ///
    /// The lease remains caller-owned on every rejection. Shape, ownership,
    /// capacity, overlap, branch, table integrity, and every target leaf are
    /// validated before the allocator binding or any page-table entry changes.
    pub(super) fn map_owned_extent(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        virtual_address: u64,
        data_lease: frame_lease,
        permissions: page_permissions,
    ) -> Result<(), (address_space_error, frame_lease)> {
        let virtual_extent =
            match validate_virtual_extent(virtual_address, &data_lease, permissions) {
                Ok(extent) => extent,
                Err(error) => return Err((error, data_lease)),
            };
        match self.is_active() {
            Ok(true) => return Err((address_space_error::active_root, data_lease)),
            Ok(false) => {}
            Err(error) => return Err((error, data_lease)),
        }
        let Some(data_range) = data_lease.range() else {
            return Err((address_space_error::invalid_data_lease, data_lease));
        };
        let tables_bound = match allocator.is_bound(&self.table_lease) {
            Ok(bound) => bound,
            Err(error) => return Err((address_space_error::frame(error), data_lease)),
        };
        if !tables_bound {
            return Err((address_space_error::corrupt_tables, data_lease));
        }
        if let Err(error) = self.validate_structure() {
            return Err((error, data_lease));
        }
        let slot = match select_mapping_slot(&self.mapping_extents(), virtual_extent) {
            Ok(slot) => slot,
            Err(error) => return Err((error, data_lease)),
        };

        let indexes = paging::indexes(virtual_address);
        let root = match self.table_address(0) {
            Ok(address) => address,
            Err(error) => return Err((error, data_lease)),
        };
        let directory_pointer = match self.table_address(1) {
            Ok(address) => address,
            Err(error) => return Err((error, data_lease)),
        };
        let directory = match self.table_address(2) {
            Ok(address) => address,
            Err(error) => return Err((error, data_lease)),
        };
        let table = match self.table_address(3) {
            Ok(address) => address,
            Err(error) => return Err((error, data_lease)),
        };
        let parent_flags = branch_flags(
            permissions.user
                || self
                    .mappings
                    .iter()
                    .flatten()
                    .any(|mapping| mapping.permissions.user),
        );
        let Some(leaf_flags) = paging::leaf_flags(permissions) else {
            return Err((address_space_error::write_execute, data_lease));
        };
        let leaf_flags = leaf_flags | ownership_marker;
        let Some(page_count) = virtual_extent.page_count() else {
            return Err((address_space_error::invalid_virtual_address, data_lease));
        };
        // SAFETY: Shape validation proved every page lies in the one owned
        // leaf table, whose live table lease was validated above.
        if !unsafe { extent_leaves_are_absent(table, virtual_extent, page_count) } {
            return Err((address_space_error::mapping_mismatch, data_lease));
        }
        if let Err(error) = validate_unbound_lease(allocator, &data_lease) {
            return Err((error, data_lease));
        }
        if let Err(error) = allocator.bind(&data_lease) {
            return Err((address_space_error::frame(error), data_lease));
        }

        // SAFETY: Every fallible precondition was checked before binding. All
        // four frames are exclusively table-owned and identity mapped, and the
        // fixed mapping set reserves every target leaf in the shared branch.
        unsafe {
            write_table_entry(root, indexes.pml4, directory_pointer | parent_flags);
            write_table_entry(
                directory_pointer,
                indexes.directory_pointer,
                directory | parent_flags,
            );
            write_table_entry(directory, indexes.directory, table | parent_flags);
            install_extent_leaves(table, virtual_extent, data_range, leaf_flags, page_count);
        }
        self.mappings[slot] = Some(mapped_extent {
            virtual_address,
            data_lease,
            permissions,
        });
        self.mapping_count += 1;
        Ok(())
    }

    pub(super) fn validate_mapping(
        &self,
        virtual_address: u64,
    ) -> Result<mapping_evidence, address_space_error> {
        if !paging::is_canonical(virtual_address) || virtual_address & (page_size - 1) != 0 {
            return Err(address_space_error::invalid_virtual_address);
        }
        self.validate_all_mappings()?;
        let mapping = self
            .mappings
            .iter()
            .flatten()
            .find(|mapping| {
                mapping_virtual_extent(mapping)
                    .is_ok_and(|extent| extent.contains_page(virtual_address))
            })
            .ok_or(address_space_error::mapping_missing)?;
        self.mapping_evidence(mapping, virtual_address)
    }

    /// Prove that an identity-mapped low-memory range cannot be reached from
    /// CPL3 through this root's borrowed kernel branch.
    ///
    /// The borrowed PML4 entry is deliberately supervisor-only. Because x86
    /// user access requires the U/S bit at every translation level, checking
    /// that live upper entry proves every descendant translation in the range
    /// is supervisor-only without exposing device addresses to the task.
    pub(super) fn validate_borrowed_supervisor_range(
        &self,
        virtual_address: u64,
        length: u64,
    ) -> Result<(), address_space_error> {
        let Some(end) = virtual_address.checked_add(length) else {
            return Err(address_space_error::invalid_virtual_address);
        };
        if length == 0
            || virtual_address >= paging::identity_map_limit
            || end > paging::identity_map_limit
        {
            return Err(address_space_error::invalid_virtual_address);
        }
        self.validate_all_mappings()?;
        let root = self.table_address(0)?;
        // SAFETY: The live table lease owns the identity-mapped root frame.
        let entry = unsafe { read_table_entry(root, 0) };
        if !entry_matches(entry, self.borrowed_kernel_entry, accessed)
            || entry & present == 0
            || entry & user_accessible != 0
        {
            return Err(address_space_error::corrupt_tables);
        }
        Ok(())
    }

    /// Prove that one page in the managed branch has no owner or leaf entry.
    pub(super) fn validate_absent(&self, virtual_address: u64) -> Result<(), address_space_error> {
        if !paging::is_canonical(virtual_address) || virtual_address & (page_size - 1) != 0 {
            return Err(address_space_error::invalid_virtual_address);
        }
        self.validate_all_mappings()?;
        if self.mappings.iter().flatten().any(|mapping| {
            mapping_virtual_extent(mapping)
                .is_ok_and(|extent| extent.contains_page(virtual_address))
        }) {
            return Err(address_space_error::mapping_exists);
        }
        let indexes = paging::indexes(virtual_address);
        if !self.common_branch()?.contains(indexes) {
            return Err(address_space_error::mapping_branch_mismatch);
        }
        let table = self.table_address(3)?;
        // SAFETY: Complete structure validation proved this address uses the
        // one exclusively owned leaf table, and the index is architecturally
        // bounded to that table.
        if unsafe { read_table_entry(table, indexes.table) } != 0 {
            return Err(address_space_error::mapping_mismatch);
        }
        Ok(())
    }

    /// Validate every owned leaf, the one shared branch, and the absence of
    /// every mapping not represented by this owner.
    pub(super) fn validate_all_mappings(&self) -> Result<(), address_space_error> {
        if self.mapping_count == 0 {
            return Err(address_space_error::mapping_missing);
        }
        self.validate_structure()
    }

    fn mapping_evidence(
        &self,
        mapping: &mapped_extent,
        virtual_address: u64,
    ) -> Result<mapping_evidence, address_space_error> {
        let data_range = mapping
            .data_lease
            .range()
            .ok_or(address_space_error::invalid_data_lease)?;
        let virtual_extent = mapping_virtual_extent(mapping)?;
        if !virtual_extent.contains_page(virtual_address) {
            return Err(address_space_error::mapping_missing);
        }
        let physical_address = data_range
            .base
            .checked_add(virtual_address - mapping.virtual_address)
            .ok_or(address_space_error::invalid_data_lease)?;
        let indexes = paging::indexes(virtual_address);
        let root = self.table_address(0)?;
        let directory_pointer = self.table_address(1)?;
        let directory = self.table_address(2)?;
        let table = self.table_address(3)?;
        let parent_flags = branch_flags(
            self.mappings
                .iter()
                .flatten()
                .any(|mapping| mapping.permissions.user),
        );
        let leaf_flags = paging::leaf_flags(mapping.permissions)
            .ok_or(address_space_error::write_execute)?
            | ownership_marker;

        // SAFETY: The live table lease owns four identity-mapped frames.
        let path = unsafe {
            [
                read_table_entry(root, indexes.pml4),
                read_table_entry(directory_pointer, indexes.directory_pointer),
                read_table_entry(directory, indexes.directory),
                read_table_entry(table, indexes.table),
            ]
        };
        if !entry_matches(path[0], directory_pointer | parent_flags, accessed)
            || !entry_matches(path[1], directory | parent_flags, accessed)
            || !entry_matches(path[2], table | parent_flags, accessed)
            || !entry_matches(path[3], physical_address | leaf_flags, accessed | dirty)
        {
            return Err(address_space_error::corrupt_tables);
        }
        let permissions = paging::effective_permissions_from_path(&path);
        let expected_permissions = effective_permissions {
            present: true,
            user: mapping.permissions.user,
            writable: mapping.permissions.writable,
            executable: mapping.permissions.executable,
        };
        if permissions != expected_permissions {
            return Err(address_space_error::corrupt_tables);
        }
        Ok(mapping_evidence {
            virtual_address,
            physical_address,
            permissions,
        })
    }

    /// Activate this validated root on the bootstrap processor.
    ///
    /// # Safety
    ///
    /// The borrowed kernel branch must remain live, the caller must prevent
    /// concurrent access to CR3 and the address space, and teardown must select
    /// another root before unmapping or destroying this one.
    pub(super) unsafe fn activate(&self) -> Result<(), address_space_error> {
        if !paging::protections_are_enabled() {
            return Err(address_space_error::activation_failed);
        }
        self.validate_all_mappings()?;
        let root = self.root_address()?;
        // SAFETY: Validation proved the low executing branch and every owned
        // mapping in the sole managed high branch.
        unsafe { paging::write_cr3(root) };
        if paging::entry_address(paging::read_cr3()) != root {
            return Err(address_space_error::activation_failed);
        }
        Ok(())
    }

    /// Withdraw one exact virtual mapping from an inactive root.
    ///
    /// Other leaves retain the shared branch. If allocator unbinding fails,
    /// the previous branch and leaf entries are restored exactly and this
    /// address space keeps ownership of the lease.
    pub(super) fn unmap_owned(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        virtual_address: u64,
    ) -> Result<frame_lease, address_space_error> {
        if self.is_active()? {
            return Err(address_space_error::active_root);
        }
        self.validate_all_mappings()?;
        if !allocator
            .is_bound(&self.table_lease)
            .map_err(address_space_error::frame)?
        {
            return Err(address_space_error::corrupt_tables);
        }
        let mapping_index = self
            .mappings
            .iter()
            .position(|mapping| {
                mapping
                    .as_ref()
                    .is_some_and(|mapping| mapping.virtual_address == virtual_address)
            })
            .ok_or(address_space_error::mapping_missing)?;
        let mapping = self.mappings[mapping_index]
            .as_ref()
            .ok_or(address_space_error::mapping_missing)?;
        let virtual_extent = mapping_virtual_extent(mapping)?;
        let page_count = virtual_extent
            .page_count()
            .ok_or(address_space_error::invalid_virtual_address)?;
        let data_range = mapping
            .data_lease
            .range()
            .ok_or(address_space_error::invalid_data_lease)?;
        if !allocator
            .is_bound(&mapping.data_lease)
            .map_err(address_space_error::frame)?
        {
            return Err(address_space_error::mapping_mismatch);
        }

        let indexes = paging::indexes(virtual_address);
        let root = self.table_address(0)?;
        let directory_pointer = self.table_address(1)?;
        let directory = self.table_address(2)?;
        let table = self.table_address(3)?;
        // SAFETY: Exact validation above proved these parent entries belong to
        // this inactive address space. Preserve their hardware-set bits so a
        // failed allocator withdrawal can restore the byte-for-byte path.
        let old_parent_path = unsafe {
            [
                read_table_entry(root, indexes.pml4),
                read_table_entry(directory_pointer, indexes.directory_pointer),
                read_table_entry(directory, indexes.directory),
            ]
        };
        let mut leaf_runtime_bits = [0u8; entries_per_table];
        for (page, runtime_bits) in leaf_runtime_bits.iter_mut().enumerate().take(page_count) {
            let virtual_page = virtual_extent
                .page_address(page)
                .ok_or(address_space_error::invalid_virtual_address)?;
            // SAFETY: Full mapping validation proved each target leaf belongs
            // to this extent and only A/D may differ from its owned value.
            let entry = unsafe { read_table_entry(table, paging::indexes(virtual_page).table) };
            *runtime_bits = (entry & (accessed | dirty)) as u8;
        }
        let last_mapping = self.mapping_count == 1;
        let remaining_user_mapping = self.mappings.iter().enumerate().any(|(index, mapping)| {
            index != mapping_index
                && mapping
                    .as_ref()
                    .is_some_and(|mapping| mapping.permissions.user)
        });
        let leaf_flags = paging::leaf_flags(mapping.permissions)
            .ok_or(address_space_error::write_execute)?
            | ownership_marker;

        // SAFETY: The root is inactive and all four table frames are
        // exclusively owned. Remove every leaf first. Parent entries remain
        // present while another extent uses the branch, with their user bit
        // narrowed when the final user mapping leaves.
        unsafe {
            clear_extent_leaves(table, virtual_extent, page_count);
            if last_mapping {
                write_table_entry(directory, indexes.directory, 0);
                write_table_entry(directory_pointer, indexes.directory_pointer, 0);
                write_table_entry(root, indexes.pml4, 0);
            } else {
                let parent_flags = branch_flags(remaining_user_mapping);
                write_table_entry(directory, indexes.directory, table | parent_flags);
                write_table_entry(
                    directory_pointer,
                    indexes.directory_pointer,
                    directory | parent_flags,
                );
                write_table_entry(root, indexes.pml4, directory_pointer | parent_flags);
            }
        }
        if let Err(error) = allocator.unbind(&mapping.data_lease) {
            // SAFETY: Failed unbinding changed no ownership. The inactive
            // tables can therefore restore the exact saved parents and each
            // leaf, including hardware A/D bits, before the still-bound extent
            // remains observable through this owner.
            unsafe {
                write_table_entry(root, indexes.pml4, old_parent_path[0]);
                write_table_entry(
                    directory_pointer,
                    indexes.directory_pointer,
                    old_parent_path[1],
                );
                write_table_entry(directory, indexes.directory, old_parent_path[2]);
                restore_extent_leaves(
                    table,
                    virtual_extent,
                    data_range,
                    leaf_flags,
                    &leaf_runtime_bits,
                    page_count,
                );
            }
            return Err(address_space_error::frame(error));
        }
        let mapping = self.mappings[mapping_index]
            .take()
            .ok_or(address_space_error::mapping_missing)?;
        self.mapping_count -= 1;
        Ok(mapping.data_lease)
    }

    /// Destroy an inactive, empty root and clear its private table frames.
    ///
    /// # Safety
    ///
    /// `clear_and_verify` must satisfy `frame_allocator::release_with`'s
    /// exclusive whole-range clearing contract for this address space's lease.
    pub(super) unsafe fn destroy<F>(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        mut clear_and_verify: F,
    ) -> Result<(), address_space_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        if self.is_active()? {
            return Err(address_space_error::active_root);
        }
        if self.mapping_count != 0 || self.mappings.iter().any(Option::is_some) {
            return Err(address_space_error::mapping_exists);
        }
        if !allocator
            .is_bound(&self.table_lease)
            .map_err(address_space_error::frame)?
            || !self.empty_tables_are_valid()?
        {
            return Err(address_space_error::corrupt_tables);
        }
        let tables = [
            self.table_address(0)?,
            self.table_address(1)?,
            self.table_address(2)?,
            self.table_address(3)?,
        ];
        // SAFETY: The root is inactive and its table lease is still live.
        unsafe { write_table_entry(tables[0], 0, 0) };
        if let Err(error) = allocator.unbind(&self.table_lease) {
            // SAFETY: Failed unbinding preserved the bound table lease.
            unsafe { write_table_entry(tables[0], 0, self.borrowed_kernel_entry) };
            return Err(address_space_error::frame(error));
        }
        // SAFETY: Validation proved this inactive root has no mappings; the
        // caller accepted the exclusive release callback contract above.
        if let Err(error) =
            unsafe { allocator.release_with(&mut self.table_lease, &mut clear_and_verify) }
        {
            if allocator.bind(&self.table_lease).is_err() {
                return Err(address_space_error::rollback_failed);
            }
            // SAFETY: Failed release preserved the exclusive live lease. Its
            // callback may have cleared any prefix, so reconstruct all frames.
            unsafe { initialize_empty_tables(tables, self.borrowed_kernel_entry) };
            if !self.empty_tables_are_valid()? {
                return Err(address_space_error::rollback_failed);
            }
            return Err(address_space_error::frame(error));
        }
        // SAFETY: The root is now inactive, empty, and has no live table
        // lease. Clear its last address-bearing borrowed-branch metadata.
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.borrowed_kernel_entry), 0)
        };
        Ok(())
    }

    fn table_address(&self, offset: usize) -> Result<u64, address_space_error> {
        if offset >= private_table_frame_count
            || self.table_lease.purpose() != frame_purpose::page_table
        {
            return Err(address_space_error::invalid_table_extent);
        }
        let range = self
            .table_lease
            .range()
            .ok_or(address_space_error::invalid_table_extent)?;
        range
            .base
            .checked_add(offset as u64 * page_size)
            .filter(|address| *address < paging::identity_map_limit)
            .ok_or(address_space_error::invalid_table_extent)
    }

    fn is_active(&self) -> Result<bool, address_space_error> {
        Ok(paging::entry_address(paging::read_cr3()) == self.root_address()?)
    }

    fn validate_structure(&self) -> Result<(), address_space_error> {
        let actual_count = self.mappings.iter().flatten().count();
        if actual_count != self.mapping_count || actual_count > mapping_capacity {
            return Err(address_space_error::corrupt_tables);
        }
        if actual_count == 0 {
            return match self.empty_tables_are_valid()? {
                true => Ok(()),
                false => Err(address_space_error::corrupt_tables),
            };
        }

        for mapping in self.mappings.iter().flatten() {
            validate_virtual_extent(
                mapping.virtual_address,
                &mapping.data_lease,
                mapping.permissions,
            )
            .map_err(|_| address_space_error::corrupt_tables)?;
        }
        let branch = self.common_branch()?;
        let root = self.table_address(0)?;
        let directory_pointer = self.table_address(1)?;
        let directory = self.table_address(2)?;
        let table = self.table_address(3)?;
        let parent_flags = branch_flags(
            self.mappings
                .iter()
                .flatten()
                .any(|mapping| mapping.permissions.user),
        );

        // SAFETY: The live table lease owns all four identity-mapped frames.
        if !entry_matches(
            unsafe { read_table_entry(root, 0) },
            self.borrowed_kernel_entry,
            accessed,
        ) || !entry_matches(
            unsafe { read_table_entry(root, branch.pml4) },
            directory_pointer | parent_flags,
            accessed,
        ) || !entry_matches(
            unsafe { read_table_entry(directory_pointer, branch.directory_pointer) },
            directory | parent_flags,
            accessed,
        ) || !entry_matches(
            unsafe { read_table_entry(directory, branch.directory) },
            table | parent_flags,
            accessed,
        ) || !self.only_expected_entries_are_present(branch)?
        {
            return Err(address_space_error::corrupt_tables);
        }

        for mapping in self.mappings.iter().flatten() {
            let extent = mapping_virtual_extent(mapping)?;
            let page_count = extent
                .page_count()
                .ok_or(address_space_error::corrupt_tables)?;
            for page in 0..page_count {
                let virtual_address = extent
                    .page_address(page)
                    .ok_or(address_space_error::corrupt_tables)?;
                self.mapping_evidence(mapping, virtual_address)?;
            }
        }
        Ok(())
    }

    fn common_branch(&self) -> Result<mapping_branch, address_space_error> {
        let mut branch = None;
        for (index, mapping) in self.mappings.iter().enumerate() {
            let Some(mapping) = mapping else {
                continue;
            };
            let extent = mapping_virtual_extent(mapping)?;
            let indexes = paging::indexes(extent.base);
            let candidate = mapping_branch::from_indexes(indexes);
            let last_page = extent
                .page_address(
                    extent
                        .page_count()
                        .and_then(|count| count.checked_sub(1))
                        .ok_or(address_space_error::corrupt_tables)?,
                )
                .ok_or(address_space_error::corrupt_tables)?;
            if !candidate.contains(paging::indexes(last_page))
                || branch.is_some_and(|branch: mapping_branch| branch != candidate)
            {
                return Err(address_space_error::corrupt_tables);
            }
            if self.mappings[..index].iter().flatten().any(|existing| {
                mapping_virtual_extent(existing).is_ok_and(|existing| existing.overlaps(extent))
            }) {
                return Err(address_space_error::corrupt_tables);
            }
            branch = Some(candidate);
        }
        branch.ok_or(address_space_error::mapping_missing)
    }

    fn mapping_extents(&self) -> [Option<virtual_extent>; mapping_capacity] {
        let mut extents = [None; mapping_capacity];
        for (index, mapping) in self.mappings.iter().enumerate() {
            extents[index] = mapping
                .as_ref()
                .and_then(|mapping| mapping_virtual_extent(mapping).ok());
        }
        extents
    }

    fn empty_tables_are_valid(&self) -> Result<bool, address_space_error> {
        for offset in 0..private_table_frame_count {
            let table = self.table_address(offset)?;
            for index in 0..entries_per_table {
                let expected = if offset == 0 && index == 0 {
                    self.borrowed_kernel_entry
                } else {
                    0
                };
                // SAFETY: Every table is covered by the live table lease.
                let permitted_runtime_bits = if offset == 0 && index == 0 {
                    accessed
                } else {
                    0
                };
                if !entry_matches(
                    unsafe { read_table_entry(table, index) },
                    expected,
                    permitted_runtime_bits,
                ) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn only_expected_entries_are_present(
        &self,
        branch: mapping_branch,
    ) -> Result<bool, address_space_error> {
        for offset in 0..private_table_frame_count {
            let table = self.table_address(offset)?;
            for index in 0..entries_per_table {
                let expected = match offset {
                    0 => index == 0 || index == branch.pml4,
                    1 => index == branch.directory_pointer,
                    2 => index == branch.directory,
                    3 => self.mappings.iter().flatten().any(|mapping| {
                        mapping_virtual_extent(mapping)
                            .is_ok_and(|extent| extent.contains_leaf_index(index))
                    }),
                    _ => false,
                };
                if expected {
                    continue;
                }
                // SAFETY: Every table is covered by the live table lease.
                if unsafe { read_table_entry(table, index) } != 0 {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

fn select_mapping_slot<const capacity: usize>(
    mappings: &[Option<virtual_extent>; capacity],
    candidate: virtual_extent,
) -> Result<usize, address_space_error> {
    let candidate_branch = mapping_branch::from_indexes(paging::indexes(candidate.base));
    let mut vacant = None;
    let mut existing_branch = None;
    for (index, mapping) in mappings.iter().enumerate() {
        let Some(mapping) = mapping else {
            vacant.get_or_insert(index);
            continue;
        };
        if mapping.overlaps(candidate) {
            return Err(address_space_error::mapping_exists);
        }
        existing_branch
            .get_or_insert_with(|| mapping_branch::from_indexes(paging::indexes(mapping.base)));
    }
    if existing_branch.is_some_and(|branch| branch != candidate_branch) {
        return Err(address_space_error::mapping_branch_mismatch);
    }
    vacant.ok_or(address_space_error::mapping_capacity_exhausted)
}

const fn branch_flags(user: bool) -> u64 {
    let mut flags = present | writable | ownership_marker;
    if user {
        flags |= user_accessible;
    }
    flags
}

const fn entry_matches(actual: u64, expected: u64, permitted_runtime_bits: u64) -> bool {
    (actual ^ expected) & !permitted_runtime_bits == 0
}

fn validate_virtual_mapping(
    virtual_address: u64,
    lease: &frame_lease,
    permissions: page_permissions,
) -> Result<(), address_space_error> {
    if lease.frame_count() != 1 {
        return Err(address_space_error::invalid_data_lease);
    }
    validate_virtual_extent(virtual_address, lease, permissions).map(|_| ())
}

fn validate_virtual_extent(
    virtual_address: u64,
    lease: &frame_lease,
    permissions: page_permissions,
) -> Result<virtual_extent, address_space_error> {
    if !paging::is_canonical(virtual_address) || virtual_address & (page_size - 1) != 0 {
        return Err(address_space_error::invalid_virtual_address);
    }
    if paging::indexes(virtual_address).pml4 == 0 {
        return Err(address_space_error::kernel_branch_reserved);
    }
    if permissions.writable && permissions.executable {
        return Err(address_space_error::write_execute);
    }
    let range = lease
        .range()
        .ok_or(address_space_error::invalid_data_lease)?;
    let expected_length = u64::try_from(lease.frame_count())
        .ok()
        .and_then(|count| count.checked_mul(page_size))
        .ok_or(address_space_error::invalid_data_lease)?;
    if lease.purpose() != frame_purpose::data
        || lease.frame_count() == 0
        || range.base & (page_size - 1) != 0
        || range.length == 0
        || range.length & (page_size - 1) != 0
        || range.length != expected_length
        || range
            .end()
            .is_none_or(|end| end > paging::identity_map_limit)
    {
        return Err(address_space_error::invalid_data_lease);
    }
    let extent = virtual_extent {
        base: virtual_address,
        length: range.length,
    };
    let end = extent
        .end()
        .ok_or(address_space_error::invalid_virtual_address)?;
    let last_page = end
        .checked_sub(page_size)
        .ok_or(address_space_error::invalid_virtual_address)?;
    if !paging::is_canonical(end - 1) {
        return Err(address_space_error::invalid_virtual_address);
    }
    if !mapping_branch::from_indexes(paging::indexes(virtual_address))
        .contains(paging::indexes(last_page))
    {
        return Err(address_space_error::mapping_branch_mismatch);
    }
    Ok(extent)
}

fn mapping_virtual_extent(mapping: &mapped_extent) -> Result<virtual_extent, address_space_error> {
    let range = mapping
        .data_lease
        .range()
        .ok_or(address_space_error::invalid_data_lease)?;
    Ok(virtual_extent {
        base: mapping.virtual_address,
        length: range.length,
    })
}

fn validate_unbound_lease(
    allocator: &frame_allocator<'_>,
    lease: &frame_lease,
) -> Result<(), address_space_error> {
    match allocator.is_bound(lease) {
        Ok(false) => Ok(()),
        Ok(true) => Err(address_space_error::frame(frame_error::lease_already_bound)),
        Err(error) => Err(address_space_error::frame(error)),
    }
}

fn extent_page_pair(
    extent: virtual_extent,
    physical: physical_range,
    page: usize,
) -> Option<(u64, u64)> {
    let virtual_address = extent.page_address(page)?;
    if physical.length != extent.length {
        return None;
    }
    let offset = u64::try_from(page).ok()?.checked_mul(page_size)?;
    let physical_address = physical.base.checked_add(offset)?;
    if physical_address >= physical.end()? {
        return None;
    }
    Some((virtual_address, physical_address))
}

unsafe fn extent_leaves_are_absent(table: u64, extent: virtual_extent, page_count: usize) -> bool {
    for page in 0..page_count {
        let virtual_page = extent.base + page as u64 * page_size;
        // SAFETY: The caller guarantees that `table` names the exclusively
        // owned leaf table containing every page in the validated extent.
        if unsafe { read_table_entry(table, paging::indexes(virtual_page).table) } != 0 {
            return false;
        }
    }
    true
}

unsafe fn install_extent_leaves(
    table: u64,
    extent: virtual_extent,
    physical: physical_range,
    leaf_flags: u64,
    page_count: usize,
) {
    for page in 0..page_count {
        let offset = page as u64 * page_size;
        let virtual_page = extent.base + offset;
        let physical_page = physical.base + offset;
        // SAFETY: The caller guarantees that the target leaves are vacant and
        // that `table` exclusively owns the validated managed branch.
        unsafe {
            write_table_entry(
                table,
                paging::indexes(virtual_page).table,
                physical_page | leaf_flags,
            )
        };
    }
}

unsafe fn clear_extent_leaves(table: u64, extent: virtual_extent, page_count: usize) {
    for page in 0..page_count {
        let virtual_page = extent.base + page as u64 * page_size;
        // SAFETY: The caller guarantees that every target leaf belongs to this
        // extent in the exclusively owned inactive table.
        unsafe { write_table_entry(table, paging::indexes(virtual_page).table, 0) };
    }
}

unsafe fn restore_extent_leaves(
    table: u64,
    extent: virtual_extent,
    physical: physical_range,
    leaf_flags: u64,
    runtime_bits: &[u8; entries_per_table],
    page_count: usize,
) {
    for (page, runtime_bits_for_page) in runtime_bits.iter().enumerate().take(page_count) {
        let offset = page as u64 * page_size;
        let virtual_page = extent.base + offset;
        let physical_page = physical.base + offset;
        // SAFETY: The caller guarantees that failed allocator withdrawal left
        // ownership intact and that every target leaf is exclusively writable.
        unsafe {
            write_table_entry(
                table,
                paging::indexes(virtual_page).table,
                physical_page | leaf_flags | u64::from(*runtime_bits_for_page),
            )
        };
    }
}

unsafe fn initialize_empty_tables(
    table_addresses: [u64; private_table_frame_count],
    borrowed_kernel_entry: u64,
) {
    for table in table_addresses {
        for index in 0..entries_per_table {
            // SAFETY: The caller exclusively owns all supplied table frames.
            unsafe { write_table_entry(table, index, 0) };
        }
    }
    // SAFETY: The first supplied frame is the exclusively owned root.
    unsafe { write_table_entry(table_addresses[0], 0, borrowed_kernel_entry) };
}

unsafe fn read_table_entry(table_address: u64, index: usize) -> u64 {
    debug_assert!(table_address & (page_size - 1) == 0);
    debug_assert!(index < entries_per_table);
    // SAFETY: The caller owns or borrows an identity-mapped page-table frame
    // and bounds the index to its 512 u64 entries.
    unsafe { ptr::read_volatile((table_address as *const u64).add(index)) }
}

unsafe fn write_table_entry(table_address: u64, index: usize, value: u64) {
    debug_assert!(table_address & (page_size - 1) == 0);
    debug_assert!(index < entries_per_table);
    debug_assert!(value & !(page_address_mask | no_execute | 0x0fff) == 0);
    // SAFETY: The caller exclusively owns an identity-mapped page-table frame
    // and bounds the index to its 512 u64 entries.
    unsafe { ptr::write_volatile((table_address as *mut u64).add(index), value) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot_omen::memory_region;

    const test_virtual_base: u64 = 0x0000_5000_0000_0000;

    const fn test_extent(base: u64, pages: u64) -> virtual_extent {
        virtual_extent {
            base,
            length: pages * page_size,
        }
    }

    #[repr(align(4096))]
    struct aligned_test_table([u64; entries_per_table]);

    fn test_allocator<'ledger>(ledger: &'ledger mut [u8]) -> frame_allocator<'ledger> {
        let regions = [memory_region {
            base: 0,
            length: ledger.len() as u64 * 4 * page_size,
            kind: 1,
            attributes: 1,
        }];
        frame_allocator::initialize(ledger, &regions, &[]).expect("test allocator")
    }

    #[test]
    fn managed_mapping_validation_rejects_unsafe_shapes() {
        let mut ledger = [0; 4];
        let mut allocator = test_allocator(&mut ledger);
        // SAFETY: This logical unit test has no physical users; successful
        // callbacks model complete clearing of its ledger-only frame extents.
        let data = unsafe { allocator.allocate_with(1, 1, frame_purpose::data, |_| true) }.unwrap();
        // SAFETY: The same isolated ledger model applies to the table extent.
        let table =
            unsafe { allocator.allocate_with(1, 1, frame_purpose::page_table, |_| true) }.unwrap();
        // SAFETY: The same isolated ledger model applies to both data frames.
        let two_frames =
            unsafe { allocator.allocate_with(2, 1, frame_purpose::data, |_| true) }.unwrap();
        assert_eq!(
            validate_virtual_mapping(
                test_virtual_base,
                &data,
                page_permissions::supervisor_read_write,
            ),
            Ok(())
        );
        assert_eq!(
            validate_virtual_mapping(
                test_virtual_base + 1,
                &data,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::invalid_virtual_address)
        );
        assert_eq!(
            validate_virtual_mapping(0x1000, &data, page_permissions::supervisor_read_write),
            Err(address_space_error::kernel_branch_reserved)
        );
        assert_eq!(
            validate_virtual_mapping(
                test_virtual_base,
                &data,
                page_permissions {
                    user: false,
                    writable: true,
                    executable: true,
                },
            ),
            Err(address_space_error::write_execute)
        );
        assert_eq!(
            validate_virtual_mapping(
                test_virtual_base,
                &table,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::invalid_data_lease)
        );
        assert_eq!(
            validate_virtual_mapping(
                test_virtual_base,
                &two_frames,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::invalid_data_lease)
        );
        assert_eq!(
            validate_virtual_extent(
                test_virtual_base + 0x4000,
                &two_frames,
                page_permissions::supervisor_read_write,
            ),
            Ok(test_extent(test_virtual_base + 0x4000, 2))
        );
        assert_eq!(
            validate_virtual_extent(
                test_virtual_base + 0x1f_f000,
                &two_frames,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::mapping_branch_mismatch)
        );
        assert_eq!(
            validate_virtual_extent(
                0x0000_7fff_ffff_f000,
                &two_frames,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::invalid_virtual_address)
        );
    }

    #[test]
    fn extent_validation_rejects_inactive_foreign_bound_and_unsafe_leases() {
        let mut first_ledger = [0; 8];
        let mut second_ledger = [0; 8];
        let mut first = test_allocator(&mut first_ledger);
        let second = test_allocator(&mut second_ledger);
        // SAFETY: These logical tests have no physical consumers; the callback
        // models complete synchronous clearing of each ledger-only extent.
        let mut inactive = unsafe {
            first
                .allocate_with(2, 1, frame_purpose::data, |_| true)
                .expect("inactive probe")
        };
        // SAFETY: The unbound logical lease has no physical consumer.
        unsafe {
            first
                .release_with(&mut inactive, |_| true)
                .expect("retire inactive probe")
        };
        assert_eq!(
            validate_virtual_extent(
                test_virtual_base,
                &inactive,
                page_permissions::supervisor_read_write,
            ),
            Err(address_space_error::invalid_data_lease)
        );

        // SAFETY: The remaining logical lease is isolated from physical users.
        let bound = unsafe {
            first
                .allocate_with(3, 1, frame_purpose::data, |_| true)
                .expect("bound probe")
        };
        assert_eq!(validate_unbound_lease(&first, &bound), Ok(()));
        first.bind(&bound).expect("bind probe");
        assert_eq!(
            validate_unbound_lease(&first, &bound),
            Err(address_space_error::frame(frame_error::lease_already_bound))
        );
        assert_eq!(
            validate_unbound_lease(&second, &bound),
            Err(address_space_error::frame(frame_error::foreign_lease))
        );
        assert!(first.is_bound(&bound).expect("bound state preserved"));

        assert_eq!(
            validate_virtual_extent(
                test_virtual_base,
                &bound,
                page_permissions {
                    user: true,
                    writable: true,
                    executable: true,
                },
            ),
            Err(address_space_error::write_execute)
        );
    }

    #[test]
    fn branch_flags_preserve_requested_user_restriction() {
        let supervisor = page_permissions::supervisor_read_write;
        assert_eq!(branch_flags(false) & user_accessible, 0);

        let user = page_permissions {
            user: true,
            writable: false,
            executable: true,
        };
        let path = [
            branch_flags(true),
            branch_flags(true),
            branch_flags(true),
            paging::leaf_flags(user).unwrap() | ownership_marker,
        ];
        assert_eq!(
            paging::effective_permissions_from_path(&path),
            effective_permissions {
                present: true,
                user: true,
                writable: false,
                executable: true,
            }
        );

        let expected = 0x2000 | present | writable | ownership_marker;
        assert!(entry_matches(expected | accessed, expected, accessed));
        assert!(entry_matches(
            expected | accessed | dirty,
            expected,
            accessed | dirty
        ));
        assert!(!entry_matches(
            expected | user_accessible,
            expected,
            accessed
        ));
        assert!(!entry_matches(0x3000 | present, expected, accessed | dirty));

        let supervisor_path = [
            branch_flags(true),
            branch_flags(true),
            branch_flags(true),
            paging::leaf_flags(supervisor).unwrap() | ownership_marker,
        ];
        assert_eq!(
            paging::effective_permissions_from_path(&supervisor_path),
            effective_permissions {
                present: true,
                user: false,
                writable: true,
                executable: false,
            }
        );
    }

    #[test]
    fn mapping_slot_selection_enforces_one_branch_and_three_logical_extents() {
        const base: u64 = test_virtual_base;
        let empty = [None; mapping_capacity];
        assert_eq!(
            select_mapping_slot(&empty, test_extent(base + 0x1000, 2)),
            Ok(0)
        );

        let one = [Some(test_extent(base + 0x4000, 2)), None, None];
        assert_eq!(
            select_mapping_slot(&one, test_extent(base + 0x4000, 2)),
            Err(address_space_error::mapping_exists)
        );
        assert_eq!(
            select_mapping_slot(&one, test_extent(base + 0x3000, 1)),
            Ok(1)
        );
        assert_eq!(
            select_mapping_slot(&one, test_extent(base + 0x6000, 3)),
            Ok(1)
        );
        assert_eq!(
            select_mapping_slot(&one, test_extent(base + 0x20_0000, 1)),
            Err(address_space_error::mapping_branch_mismatch)
        );

        let with_hole = [
            Some(test_extent(base + 0x1000, 2)),
            None,
            Some(test_extent(base + 0x7000, 2)),
        ];
        assert_eq!(
            select_mapping_slot(&with_hole, test_extent(base + 0x4000, 2)),
            Ok(1)
        );

        let full = [
            Some(test_extent(base + 0x1000, 1)),
            Some(test_extent(base + 0x3000, 2)),
            Some(test_extent(base + 0x7000, 3)),
        ];
        assert_eq!(
            select_mapping_slot(&full, test_extent(base + 0xb000, 2)),
            Err(address_space_error::mapping_capacity_exhausted)
        );
        assert_eq!(
            select_mapping_slot(&full, test_extent(base + 0x4000, 1)),
            Err(address_space_error::mapping_exists)
        );
    }

    #[test]
    fn transactional_overlap_preflight_rejects_every_shape_but_accepts_adjacency() {
        let existing = test_extent(test_virtual_base + 0x8000, 4);
        let mappings = [Some(existing), None, None];
        let original = mappings;
        for candidate in [
            test_extent(test_virtual_base + 0x7000, 2),
            test_extent(test_virtual_base + 0x9000, 1),
            test_extent(test_virtual_base + 0xa000, 4),
            test_extent(test_virtual_base + 0x7000, 6),
        ] {
            assert_eq!(
                select_mapping_slot(&mappings, candidate),
                Err(address_space_error::mapping_exists)
            );
            assert_eq!(mappings, original);
        }
        assert_eq!(
            select_mapping_slot(&mappings, test_extent(test_virtual_base + 0x6000, 2)),
            Ok(1)
        );
        assert_eq!(
            select_mapping_slot(&mappings, test_extent(test_virtual_base + 0xc000, 2)),
            Ok(1)
        );
    }

    #[test]
    fn extent_page_plan_progresses_virtual_and_physical_pages_exactly() {
        let extent = test_extent(test_virtual_base + 0x11_000, 4);
        let physical = physical_range {
            base: 0x40_000,
            length: 4 * page_size,
        };
        for page in 0..4 {
            assert_eq!(
                extent_page_pair(extent, physical, page),
                Some((
                    extent.base + page as u64 * page_size,
                    physical.base + page as u64 * page_size,
                ))
            );
        }
        assert_eq!(extent_page_pair(extent, physical, 4), None);
        assert_eq!(
            extent_page_pair(
                extent,
                physical_range {
                    length: 3 * page_size,
                    ..physical
                },
                0,
            ),
            None
        );
        assert!(extent.contains_leaf_index(paging::indexes(extent.base).table));
        assert!(extent.contains_leaf_index(paging::indexes(extent.base).table + 3));
        assert!(!extent.contains_leaf_index(paging::indexes(extent.base).table + 4));
    }

    #[test]
    fn multi_page_leaf_transaction_installs_clears_and_restores_every_page() {
        let extent = test_extent(test_virtual_base + 0x20_000, 5);
        let page_count = extent.page_count().unwrap();
        let physical = physical_range {
            base: 0x80_000,
            length: extent.length,
        };
        let permissions = page_permissions {
            user: true,
            writable: true,
            executable: false,
        };
        let leaf_flags = paging::leaf_flags(permissions).unwrap() | ownership_marker;
        let mut table = aligned_test_table([0; entries_per_table]);
        let table_address = table.0.as_mut_ptr() as u64;
        let mut runtime_bits = [0u8; entries_per_table];

        // SAFETY: The aligned local array exclusively backs the complete test
        // table, and the validated test extent stays inside its leaf indexes.
        assert!(unsafe { extent_leaves_are_absent(table_address, extent, page_count) });
        // SAFETY: The same exclusive aligned table and valid extent apply.
        unsafe { install_extent_leaves(table_address, extent, physical, leaf_flags, page_count) };
        for page in 0..page_count {
            let (virtual_page, physical_page) = extent_page_pair(extent, physical, page).unwrap();
            assert_eq!(
                table.0[paging::indexes(virtual_page).table],
                physical_page | leaf_flags
            );
            let bits = if page % 2 == 0 { accessed } else { dirty };
            let index = paging::indexes(virtual_page).table;
            table.0[index] |= bits;
            runtime_bits[page] = bits as u8;
        }
        let installed = table.0;
        // SAFETY: Every target leaf was installed above and the table remains
        // exclusively borrowed by this test.
        unsafe { clear_extent_leaves(table_address, extent, page_count) };
        assert!((0..page_count).all(|page| {
            table.0[paging::indexes(extent.page_address(page).unwrap()).table] == 0
        }));
        assert_eq!(
            table.0[paging::indexes(extent.base).table - 1],
            installed[paging::indexes(extent.base).table - 1]
        );
        assert_eq!(
            table.0[paging::indexes(extent.base).table + page_count],
            installed[paging::indexes(extent.base).table + page_count]
        );

        // SAFETY: A modeled unbind failure leaves the extent owned and permits
        // exact restoration into the still-exclusive table.
        unsafe {
            restore_extent_leaves(
                table_address,
                extent,
                physical,
                leaf_flags,
                &runtime_bits,
                page_count,
            )
        };
        assert_eq!(table.0, installed);
    }

    #[test]
    fn multi_page_owned_lease_unbind_and_release_cover_the_complete_extent() {
        let mut ledger = [0; 8];
        let mut allocator = test_allocator(&mut ledger);
        let initial = allocator.accounting();
        // SAFETY: This logical test has no physical users; the callback models
        // complete initialization of the ledger-only extent.
        let mut lease = unsafe {
            allocator
                .allocate_with(5, 1, frame_purpose::data, |_| true)
                .expect("multi-page lease")
        };
        let complete_range = lease.range().expect("live complete range");
        assert_eq!(complete_range.length, 5 * page_size);
        allocator.bind(&lease).expect("bind complete extent");
        allocator.unbind(&lease).expect("unbind complete extent");
        let before_failed_release = allocator.accounting();
        let mut rejected_range = None;
        // SAFETY: The logical lease is unbound and has no physical consumer;
        // returning false deliberately models an incomplete wipe.
        let error = unsafe {
            allocator.release_with(&mut lease, |range| {
                rejected_range = Some(range);
                false
            })
        }
        .unwrap_err();
        assert_eq!(error, frame_error::clearing_failed);
        assert_eq!(rejected_range, Some(complete_range));
        assert_eq!(lease.range(), Some(complete_range));
        assert_eq!(allocator.accounting(), before_failed_release);

        let mut released_range = None;
        // SAFETY: The logical lease is unbound and has no physical consumer.
        unsafe {
            allocator
                .release_with(&mut lease, |range| {
                    released_range = Some(range);
                    range == complete_range
                })
                .expect("release complete extent")
        };
        assert_eq!(released_range, Some(complete_range));
        assert_eq!(allocator.accounting(), initial);
        assert_eq!(lease.range(), None);
    }

    #[test]
    fn mapping_branch_ignores_only_the_leaf_index() {
        const base: u64 = 0x0000_5000_0000_0000;
        let branch = mapping_branch::from_indexes(paging::indexes(base + 0x1000));
        assert!(branch.contains(paging::indexes(base + 0x1f_f000)));
        assert!(!branch.contains(paging::indexes(base + 0x20_0000)));
    }
}
