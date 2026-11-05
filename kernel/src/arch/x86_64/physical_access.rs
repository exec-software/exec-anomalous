#![cfg_attr(not(test), allow(dead_code))]

//! Temporary, feature-proof access to allocator-owned RAM through a high alias.
//! The aperture also spans holes and devices; its geometry grants no RAM access.
//! Threshold's borrowed tables remain reserved. Explicitly registered resident
//! roots may share the aperture while a separate session retains their owners.

use core::arch::asm;
use core::ptr;

use super::owned_mapping::mapping_memory;
use super::paging::{self, entries_per_table, huge_page, no_execute, page_size, present, writable};
use super::physical_window::planned_physical_window;
#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
use super::user_copy::{copy_byte_limit, copy_direction, copy_span_capacity};
use crate::memory::physical::physical_range;

pub(super) const proof_window_base: u64 = 0xffff_9000_0000_0000;
const window_slot: usize = 288;
const accessed: u64 = 1 << 5;
const dirty: u64 = 1 << 6;
const bootstrap_table_count: u64 = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum access_error {
    invalid_state,
    unsupported_cpu_state,
    invalid_bootstrap_tree,
    changed_root,
}

pub(super) type bootstrap_physical_access = bootstrap_physical_access_group<2>;

/// Non-copyable installation owner for two or three dependent roots. There is
/// deliberately no fallible Drop; the legacy alias retains its two-root layout.
#[must_use = "retain this owner until every dependent stage is cleaned and the alias is removed"]
pub(super) struct bootstrap_physical_access_group<const task_count: usize> {
    root: u64,
    entry: u64,
    window: Option<planned_physical_window>,
    borrowers: [u64; task_count],
}

#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
#[derive(Clone, Copy)]
struct byte_span {
    address: *mut u8,
    length: usize,
}

/// A single-use, bounded byte-copy admission borrowing the live alias owner.
/// Construction is unsafe because alias geometry alone grants no RAM access.
/// The copying methods perform no fallible checks after their first byte.
#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
pub(super) struct physical_byte_spans<'a, const task_count: usize> {
    spans: [byte_span; copy_span_capacity],
    count: usize,
    length: usize,
    direction: copy_direction,
    _access: &'a mut bootstrap_physical_access_group<task_count>,
}

#[cfg(any(
    test,
    feature = "application_profile_test",
    feature = "owned_source_test"
))]
impl<const task_count: usize> physical_byte_spans<'_, task_count> {
    pub(super) fn read_into(self, destination: &mut [u8]) {
        assert_eq!(self.direction, copy_direction::from_user);
        assert_eq!(destination.len(), self.length);
        let mut cursor = 0;
        for span in &self.spans[..self.count] {
            for (offset, target) in destination[cursor..cursor + span.length]
                .iter_mut()
                .enumerate()
            {
                // SAFETY: Construction admitted every byte in live owned RAM;
                // the exclusive alias borrow excludes root/alias withdrawal.
                *target = unsafe { ptr::read_volatile(span.address.add(offset)) };
            }
            cursor += span.length;
        }
    }

    pub(super) fn write_from(self, source: &[u8]) {
        assert_eq!(self.direction, copy_direction::to_user);
        assert_eq!(source.len(), self.length);
        let mut cursor = 0;
        for span in &self.spans[..self.count] {
            for (offset, value) in source[cursor..cursor + span.length].iter().enumerate() {
                // SAFETY: Construction admitted exclusive writable RAM for
                // all spans before any mutation. No fallible backend is used.
                unsafe { ptr::write_volatile(span.address.add(offset), *value) };
            }
            cursor += span.length;
        }
    }

    /// Observe the actual destination through the same retained admission.
    /// All shape checks precede mutation; the readback cannot reacquire or
    /// change authority, select another root, or introduce a recoverable error.
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    pub(super) fn write_from_observed(self, source: &[u8], observed: &mut [u8]) {
        assert_eq!(self.direction, copy_direction::to_user);
        assert_eq!(source.len(), self.length);
        assert_eq!(observed.len(), self.length);
        let mut cursor = 0;
        for span in &self.spans[..self.count] {
            for (offset, value) in source[cursor..cursor + span.length].iter().enumerate() {
                // SAFETY: Complete exclusive writable owned-RAM admission.
                unsafe { ptr::write_volatile(span.address.add(offset), *value) };
            }
            cursor += span.length;
        }
        cursor = 0;
        for span in &self.spans[..self.count] {
            for (offset, value) in observed[cursor..cursor + span.length]
                .iter_mut()
                .enumerate()
            {
                // SAFETY: The same readable owned RAM and alias remain held.
                *value = unsafe { ptr::read_volatile(span.address.add(offset)) };
            }
            cursor += span.length;
        }
    }
}

impl<const task_count: usize> bootstrap_physical_access_group<task_count> {
    pub(super) const fn new() -> Self {
        Self {
            root: 0,
            entry: 0,
            window: None,
            borrowers: [0; task_count],
        }
    }

    /// Install only into the unused proof slot of the exact Threshold hierarchy.
    ///
    /// # Safety
    /// The BSP exclusively owns bootstrap paging with interrupts disabled. The
    /// six reserved Threshold tables are identity-accessible RAM and remain live
    /// and unmodified throughout this owner's lifetime. No other CPU runs or can
    /// retain translations. The caller must retain this owner on every error.
    pub(super) unsafe fn install(&mut self) -> Result<(), access_error> {
        if !(matches!(task_count, 2 | 3)
            || (cfg!(feature = "shared_veil_test") && task_count == 5)
            || (cfg!(feature = "owned_workspace_test") && task_count == 1)
            || (cfg!(feature = "native_program_test") && task_count == 1))
            || self.window.is_some()
        {
            return Err(access_error::invalid_state);
        }
        if !supported_cpu_state() || !paging::protections_are_enabled() {
            return Err(access_error::unsupported_cpu_state);
        }
        let root = paging::read_cr3();
        // SAFETY: The caller supplies the exclusive, identity-accessible
        // Threshold hierarchy; the validator checks addresses before each read.
        if !validate_bootstrap_tree(root, |table, index| unsafe {
            Some(ptr::read_volatile((table as *const u64).add(index)))
        }) {
            return Err(access_error::invalid_bootstrap_tree);
        }
        let window = planned_physical_window::new(
            physical_range::new(0, paging::identity_map_limit),
            proof_window_base,
        )
        .map_err(|_| access_error::invalid_state)?;
        let entry = (root + page_size) | present | writable | no_execute;
        // Retain installation metadata before the first mutation. Failure below
        // cannot discard the only owner of an installed alias.
        self.root = root;
        self.entry = entry;
        self.window = Some(window);
        // SAFETY: The validated root has an empty proof slot. Its borrowed
        // children are unchanged, and only this new parent receives NX.
        unsafe {
            ptr::write_volatile((root as *mut u64).add(window_slot), entry);
            paging::write_cr3(root);
        }
        if !self.is_current() {
            return Err(access_error::changed_root);
        }
        Ok(())
    }

    /// # Safety
    /// All dependent staging owners must be fully cleaned; no raw pointer or
    /// callback may outlive removal. The install exclusivity contract still holds.
    pub(super) unsafe fn remove(&mut self) -> Result<(), access_error> {
        if !self.is_current()
            || paging::read_cr3() != self.root
            || self.borrowers != [0; task_count]
        {
            return Err(access_error::changed_root);
        }
        // SAFETY: This owner is the sole writer of its exact installed slot.
        // PCID/global pages and concurrent processors are excluded.
        unsafe {
            ptr::write_volatile((self.root as *mut u64).add(window_slot), 0);
            paging::write_cr3(self.root);
        }
        // SAFETY: The original reserved root remains live after alias removal.
        if unsafe { ptr::read_volatile((self.root as *const u64).add(window_slot)) } != 0 {
            return Err(access_error::invalid_state);
        }
        self.window = None;
        Ok(())
    }

    pub(super) fn is_current(&self) -> bool {
        let current = paging::read_cr3();
        if self.window.is_none()
            || !self.accepts_root(current)
            || !supported_cpu_state()
            || !paging::protections_are_enabled()
        {
            return false;
        }
        // SAFETY: Installation and registration retain the identical alias in
        // every admitted root. Their unsafe contracts prohibit its mutation.
        // Physical table addresses need not have any low identity mapping.
        let actual = unsafe {
            ptr::read_volatile(((proof_window_base + current) as *const u64).add(window_slot))
        };
        actual & !accessed == self.entry
    }

    pub(super) const fn bootstrap_root(&self) -> u64 {
        self.root
    }

    pub(super) fn alias_entry(&self) -> Option<u64> {
        self.window.map(|_| self.entry)
    }

    pub(super) fn bootstrap_stack(&self) -> Option<physical_range> {
        self.window?;
        let base = self.root.checked_add(bootstrap_table_count * page_size)?;
        let end = base.checked_add(64 * 1024)?;
        (end <= 0x30000).then_some(physical_range::new(base, end - base))
    }

    pub(super) fn accepts_root(&self, root: u64) -> bool {
        root != 0 && (root == self.root || self.borrowers.contains(&root))
    }

    /// # Safety
    /// The caller retains exclusive ownership of a completely validated root
    /// and its resident mappings until unregistration. Its alias entry and
    /// borrowed subtree cannot change; only the caller may select this root.
    pub(super) unsafe fn register_root(&mut self, root: u64) -> Result<(), access_error> {
        if paging::read_cr3() != self.root || !self.is_current() {
            return Err(access_error::invalid_state);
        }
        let slot = self.registration_slot(root)?;
        // SAFETY: Registration's caller owns the root page, and the bootstrap
        // aperture is retained independently of the root under examination.
        if unsafe { self.read_word(root + window_slot as u64 * 8) }
            .is_none_or(|entry| entry & !accessed != self.entry)
        {
            return Err(access_error::invalid_state);
        }
        self.borrowers[slot] = root;
        Ok(())
    }

    /// Inert capacity/identity preflight. The hardware/alias admission remains
    /// in register_root and no borrower slot changes before its final readback.
    fn registration_slot(&self, root: u64) -> Result<usize, access_error> {
        if !(matches!(task_count, 2 | 3)
            || (cfg!(feature = "shared_veil_test") && task_count == 5)
            || (cfg!(feature = "owned_workspace_test") && task_count == 1)
            || (cfg!(feature = "native_program_test") && task_count == 1))
            || root == 0
            || !root.is_multiple_of(page_size)
            || root >= paging::identity_map_limit
            || self.accepts_root(root)
        {
            return Err(access_error::invalid_state);
        }
        self.borrowers
            .iter()
            .position(|value| *value == 0)
            .ok_or(access_error::invalid_state)
    }

    /// # Safety
    /// Bootstrap CR3 has been restored and the caller prevents all subsequent
    /// selection or access through this root. No other CPU retains translations.
    pub(super) unsafe fn unregister_root(&mut self, root: u64) -> Result<(), access_error> {
        if paging::read_cr3() != self.root || !self.is_current() {
            return Err(access_error::changed_root);
        }
        let slot = self
            .borrowers
            .iter()
            .position(|value| *value == root && root != 0)
            .ok_or(access_error::invalid_state)?;
        self.borrowers[slot] = 0;
        Ok(())
    }

    fn address(&self, range: physical_range) -> Option<u64> {
        if !self.is_current() {
            return None;
        }
        self.window?
            .plan_virtual_extent(range)
            .ok()
            .map(|extent| extent.base())
    }

    /// Preflight all aliases before exposing the single-use copy operation.
    ///
    /// # Safety
    /// The caller retains live owned RAM for every complete range, with read
    /// authority for from_user and exclusive write authority for to_user. The
    /// kernel slice passed to the returned operation must not alias this RAM.
    /// Sole BSP, IF clear/NMI masked; owners, bindings and roots cannot change
    /// before the operation is consumed. A privileged hardware fault is fatal,
    /// not a recoverable copy error or a promise of rollback after mutation.
    #[cfg(any(
        test,
        feature = "application_profile_test",
        feature = "owned_source_test"
    ))]
    pub(super) unsafe fn prepare_byte_spans(
        &mut self,
        ranges: &[physical_range],
        direction: copy_direction,
    ) -> Result<physical_byte_spans<'_, task_count>, access_error> {
        if ranges.is_empty() || ranges.len() > copy_span_capacity {
            return Err(access_error::invalid_state);
        }
        let mut spans = [byte_span {
            address: ptr::null_mut(),
            length: 0,
        }; copy_span_capacity];
        let mut length = 0;
        for (index, range) in ranges.iter().copied().enumerate() {
            let count = usize::try_from(range.length).map_err(|_| access_error::invalid_state)?;
            length = usize::checked_add(length, count)
                .filter(|total| count > 0 && *total <= copy_byte_limit)
                .ok_or(access_error::invalid_state)?;
            let address = self.address(range).ok_or(access_error::changed_root)?;
            spans[index] = byte_span {
                address: address as *mut u8,
                length: count,
            };
        }
        Ok(physical_byte_spans {
            spans,
            count: ranges.len(),
            length,
            direction,
            _access: self,
        })
    }

    /// # Safety
    /// The caller owns readable RAM covering this aligned word for the operation.
    pub(super) unsafe fn read_word(&self, physical: u64) -> Option<u64> {
        if !physical.is_multiple_of(8) {
            return None;
        }
        let address = self.address(physical_range::new(physical, 8))?;
        // SAFETY: The caller supplies read authority; the complete aligned word
        // is inside this still-installed checked alias.
        Some(unsafe { ptr::read_volatile(address as *const u64) })
    }

    /// Observe one complete private proof region without repeating privileged
    /// control checks for every word. This exposes only a hash, never an alias.
    ///
    /// # Safety
    /// The caller retains read authority over the complete bounded owned range.
    /// Sole BSP, IF clear and NMI masked throughout; roots, mapping leases and
    /// alias tables remain unchanged until return. No callback or pointer escapes.
    #[cfg(any(feature = "shared_veil_test", feature = "native_relaunch_test"))]
    pub(super) unsafe fn proof_hash_range(&self, range: physical_range) -> Option<u64> {
        if !(range.length == 8192
            || (cfg!(feature = "native_relaunch_test") && matches!(range.length, 16384 | 32768)))
            || !range.base.is_multiple_of(8)
        {
            return None;
        }
        // address validates the complete range and retained current root/alias,
        // including the IF-clear, NX and write-protection control requirements.
        let address = self.address(range)?;
        // SAFETY: Full readable aligned range is checked above and retained by
        // the caller's exclusion contract through this nonescaping observation.
        let words = unsafe {
            core::slice::from_raw_parts(address as *const u64, range.length as usize / 8)
        };
        let hash = proof_hash_words(words);
        self.is_current().then_some(hash)
    }

    /// # Safety
    /// The caller exclusively owns writable RAM covering this aligned word.
    pub(super) unsafe fn write_word(&self, physical: u64, value: u64) -> bool {
        if !physical.is_multiple_of(8) {
            return false;
        }
        let Some(address) = self.address(physical_range::new(physical, 8)) else {
            return false;
        };
        // SAFETY: The caller supplies exclusive authority for this checked word.
        unsafe { ptr::write_volatile(address as *mut u64, value) };
        true
    }
}

// Volatile word reads retain full observation coverage even when other kernel
// code owns the writable mapping. Little-endian byte order matches the existing
// per-word proof and packet FNV hashes; this function has no privileged accesses.
#[cfg(any(test, feature = "shared_veil_test", feature = "native_relaunch_test"))]
fn proof_hash_words(words: &[u64]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for word in words {
        // SAFETY: The immutable slice covers this complete aligned word for the
        // entire read; volatile enforces observation rather than cached reuse.
        let value = unsafe { ptr::read_volatile(word) };
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    hash
}

// SAFETY: Every physical operation checks the whole range and current retained
// alias. Its unsafe methods additionally require allocator/lease authority for
// RAM; no references to the whole aperture or unowned physical data are created.
unsafe impl<const task_count: usize> mapping_memory
    for bootstrap_physical_access_group<task_count>
{
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        if range.length == 0
            || !range.base.is_multiple_of(page_size)
            || !range.length.is_multiple_of(page_size)
        {
            return false;
        }
        let Some(address) = self.address(range) else {
            return false;
        };
        let count = range.length / 8;
        let words = address as *mut u64;
        // SAFETY: The caller grants the whole aligned range exclusively. The
        // loop covers every byte, then verifies it through the same live alias.
        unsafe {
            for i in 0..count {
                ptr::write_volatile(words.add(i as usize), 0);
            }
            for i in 0..count {
                if ptr::read_volatile(words.add(i as usize)) != 0 {
                    return false;
                }
            }
        }
        true
    }

    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        let physical = entry_offset(table, index)?;
        // SAFETY: The trait caller owns this table and checked entry.
        unsafe { self.read_word(physical) }
    }

    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
        let Some(physical) = entry_offset(table, index) else {
            return false;
        };
        // SAFETY: The trait caller exclusively owns this table and checked entry.
        unsafe { self.write_word(physical, value) }
    }

    fn active_root(&self) -> u64 {
        paging::entry_address(paging::read_cr3())
    }
}

fn entry_offset(table: u64, index: usize) -> Option<u64> {
    if !table.is_multiple_of(page_size) || index >= entries_per_table {
        return None;
    }
    table.checked_add(index as u64 * 8)
}

fn supported_cpu_state() -> bool {
    let cr4: u64;
    let flags: u64;
    // SAFETY: This feature is a CPL0 BSP proof; flags are restored unchanged.
    unsafe {
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
    }
    // Four-level paging, no global translations/PCID, SMEP/SMAP, protection
    // keys or CET, and no interrupt-driven mutation during this CPL0 proof.
    cr4 & ((1 << 7)
        | (1 << 12)
        | (1 << 17)
        | (1 << 20)
        | (1 << 21)
        | (1 << 22)
        | (1 << 23)
        | (1 << 24))
        == 0
        && flags & (1 << 9) == 0
}

/// Pure validation of the exact six-page Threshold layout, before any mutation.
fn validate_bootstrap_tree<F>(root: u64, mut read: F) -> bool
where
    F: FnMut(u64, usize) -> Option<u64>,
{
    if root == 0
        || !root.is_multiple_of(page_size)
        || root
            .checked_add(bootstrap_table_count * page_size)
            .is_none_or(|end| end > 0x100000)
    {
        return false;
    }
    for i in 0..entries_per_table {
        let expected = if i == 0 {
            (root + page_size) | present | writable
        } else {
            0
        };
        let Some(actual) = read(root, i) else {
            return false;
        };
        if actual & !(if i == 0 { accessed } else { 0 }) != expected {
            return false;
        }
    }
    for i in 0..entries_per_table {
        let expected = if i < 4 {
            (root + (2 + i as u64) * page_size) | present | writable
        } else {
            0
        };
        let Some(actual) = read(root + page_size, i) else {
            return false;
        };
        if actual & !(if i < 4 { accessed } else { 0 }) != expected {
            return false;
        }
    }
    for branch in 0..4 {
        for i in 0..entries_per_table {
            let expected =
                (((branch * entries_per_table + i) as u64) << 21) | present | writable | huge_page;
            let Some(actual) = read(root + (2 + branch as u64) * page_size, i) else {
                return false;
            };
            if actual & !(accessed | dirty) != expected {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[cfg(feature = "shared_veil_test")]
    #[test]
    fn five_roots_reject_duplicates_and_exhaustion_then_reuse_only_withdrawn_slot() {
        let mut group = bootstrap_physical_access_group::<5>::new();
        group.root = root;
        for index in 0..5 {
            let address = 0x100000 + index as u64 * page_size;
            assert_eq!(group.registration_slot(address), Ok(index));
            group.borrowers[index] = address;
            assert!(group.accepts_root(address));
            assert_eq!(
                group.registration_slot(address),
                Err(access_error::invalid_state)
            );
        }
        let before = group.borrowers;
        assert_eq!(
            group.registration_slot(0x110000),
            Err(access_error::invalid_state)
        );
        assert_eq!(group.borrowers, before);
        group.borrowers[3] = 0;
        assert!(!group.accepts_root(0x103000));
        assert_eq!(group.registration_slot(0x110000), Ok(3));
        for index in [0, 1, 2, 4] {
            assert!(group.accepts_root(0x100000 + index * page_size));
        }
    }

    #[test]
    fn workspace_single_root_requires_explicit_feature_and_has_no_spare_borrower() {
        let mut single = bootstrap_physical_access_group::<1>::new();
        single.root = root;
        let first = single.registration_slot(0x100000);
        if cfg!(feature = "owned_workspace_test") {
            assert_eq!(first, Ok(0));
            single.borrowers[0] = 0x100000;
            for candidate in [root, 0x100000, 0x101000] {
                assert_eq!(
                    single.registration_slot(candidate),
                    Err(access_error::invalid_state)
                );
                assert_eq!(single.borrowers, [0x100000]);
            }
            single.borrowers[0] = 0;
            assert_eq!(single.registration_slot(0x101000), Ok(0));
        } else {
            assert_eq!(first, Err(access_error::invalid_state));
            assert_eq!(single.borrowers, [0]);
        }
    }

    #[test]
    fn third_root_capacity_is_explicit_and_rejected_registration_is_inert() {
        let mut pair = bootstrap_physical_access::new();
        let mut triple = bootstrap_physical_access_group::<3>::new();
        pair.root = root;
        triple.root = root;
        for (index, address) in [0x100000, 0x101000, 0x102000].into_iter().enumerate() {
            assert_eq!(triple.registration_slot(address), Ok(index));
            triple.borrowers[index] = address;
            if index < 2 {
                assert_eq!(pair.registration_slot(address), Ok(index));
                pair.borrowers[index] = address;
            } else {
                assert_eq!(
                    pair.registration_slot(address),
                    Err(access_error::invalid_state)
                );
            }
        }
        let before = triple.borrowers;
        for invalid in [
            0,
            1,
            root,
            0x100000,
            0x103000,
            paging::identity_map_limit,
            u64::MAX,
        ] {
            assert_eq!(
                triple.registration_slot(invalid),
                Err(access_error::invalid_state)
            );
            assert_eq!(triple.borrowers, before);
        }
        triple.borrowers[1] = 0;
        assert!(!triple.accepts_root(0x101000));
        assert!(triple.accepts_root(0x100000));
        assert!(triple.accepts_root(0x102000));
        assert_eq!(triple.registration_slot(0x103000), Ok(1));
        assert_eq!(
            core::mem::size_of::<bootstrap_physical_access_group<3>>(),
            core::mem::size_of::<bootstrap_physical_access>() + 8
        );
        // SAFETY: Unsupported capacity returns before any privileged access.
        assert_eq!(
            unsafe { bootstrap_physical_access_group::<4>::new().install() },
            Err(access_error::invalid_state)
        );
    }
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn private_region_hash_covers_endpoints_and_interior_without_neighbor_reads() {
        let mut storage = [0u64; 1026];
        for (index, word) in storage.iter_mut().enumerate() {
            *word = (index as u64).wrapping_mul(0x1020_3040_5060_7081);
        }
        let original = storage;
        let hash = proof_hash_words(&storage[1..1025]);
        assert_eq!(storage, original);
        storage[0] ^= u64::MAX;
        storage[1025] ^= u64::MAX;
        assert_eq!(proof_hash_words(&storage[1..1025]), hash);
        for index in [1, 2, 511, 512, 513, 1023, 1024] {
            storage[index] ^= 0x0101_0101_0101_0101;
            assert_ne!(proof_hash_words(&storage[1..1025]), hash);
            storage[index] ^= 0x0101_0101_0101_0101;
        }
        assert_eq!(proof_hash_words(&storage[1..1025]), hash);
        // Explicit byte reference proves the word-based reader preserves the
        // existing little-endian packet hash rather than host word arithmetic.
        let mut expected = 0xcbf2_9ce4_8422_2325u64;
        for word in &original[1..1025] {
            for byte in word.to_le_bytes() {
                expected ^= u64::from(byte);
                expected = expected.wrapping_mul(0x100_0000_01b3);
            }
        }
        assert_eq!(hash, expected);
    }

    const root: u64 = 0x20000;
    fn fixture() -> Vec<[u64; entries_per_table]> {
        let mut tables = vec![[0; entries_per_table]; 6];
        tables[0][0] = (root + page_size) | present | writable | accessed;
        for branch in 0..4 {
            tables[1][branch] = (root + (2 + branch as u64) * page_size) | present | writable;
            for i in 0..entries_per_table {
                tables[2 + branch][i] = (((branch * entries_per_table + i) as u64) << 21)
                    | present
                    | writable
                    | huge_page
                    | accessed
                    | dirty;
            }
        }
        tables
    }
    fn valid(tables: &[[u64; entries_per_table]]) -> bool {
        validate_bootstrap_tree(root, |table, i| {
            tables
                .get(((table - root) / page_size) as usize)
                .map(|t| t[i])
        })
    }
    #[test]
    fn exact_bootstrap_tree_accepts_only_architectural_access_bits() {
        assert!(valid(&fixture()));
        for (table, index, bit) in [
            (0, 0, dirty),
            (1, 0, huge_page),
            (2, 0, 1 << 8),
            (5, 511, no_execute),
            (4, 33, 1 << 2),
            (3, 4, 1 << 3),
        ] {
            let mut tables = fixture();
            tables[table][index] |= bit;
            assert!(!valid(&tables));
        }
    }
    #[test]
    fn occupied_window_extra_branches_wrong_targets_and_holes_are_rejected() {
        for (table, index) in [(0, window_slot), (0, 1), (1, 4), (1, 511)] {
            let mut tables = fixture();
            tables[table][index] = present;
            assert!(!valid(&tables));
        }
        for (table, index) in [(0, 0), (1, 3), (2, 0), (5, 511)] {
            let mut tables = fixture();
            tables[table][index] ^= page_size;
            assert!(!valid(&tables));
            tables[table][index] = 0;
            assert!(!valid(&tables));
        }
    }
    #[test]
    fn every_read_failure_and_bad_root_reject_without_accessing_outside_tables() {
        let tables = fixture();
        for stop in [0, 511, 512, 1023, 1024, 3071] {
            let mut count = 0;
            assert!(!validate_bootstrap_tree(root, |table, i| {
                count += 1;
                if count == stop + 1 {
                    return None;
                }
                Some(tables[((table - root) / page_size) as usize][i])
            }));
        }
        for bad in [0, 1, 0xfb000, u64::MAX & !(page_size - 1)] {
            assert!(!validate_bootstrap_tree(bad, |_, _| panic!(
                "invalid root read"
            )));
        }
        assert_eq!(entry_offset(root, 511), Some(root + 4088));
        assert_eq!(entry_offset(root, 512), None);
        assert_eq!(entry_offset(root + 1, 0), None);
    }

    #[test]
    fn admitted_byte_copy_handles_unaligned_disjoint_spans_without_touching_neighbors() {
        let mut backing = [0xa5_u8; 64];
        let mut access = bootstrap_physical_access::new();
        // This commit-only model uses ordinary live RAM. Production admission
        // constructs these pointers only after checking retained physical spans.
        let spans = [
            byte_span {
                address: backing.as_mut_ptr().wrapping_add(3),
                length: 7,
            },
            byte_span {
                address: backing.as_mut_ptr().wrapping_add(35),
                length: 12,
            },
        ];
        let source = core::array::from_fn::<_, 19, _>(|index| index as u8 + 1);
        physical_byte_spans {
            spans,
            count: 2,
            length: 19,
            direction: copy_direction::to_user,
            _access: &mut access,
        }
        .write_from(&source);
        assert_eq!(&backing[3..10], &source[..7]);
        assert_eq!(&backing[35..47], &source[7..]);
        for untouched in [&backing[..3], &backing[10..35], &backing[47..]] {
            assert!(untouched.iter().all(|byte| *byte == 0xa5));
        }
        let mut destination = [0x5a; 21];
        physical_byte_spans {
            spans,
            count: 2,
            length: 19,
            direction: copy_direction::from_user,
            _access: &mut access,
        }
        .read_into(&mut destination[1..20]);
        assert_eq!(&destination[1..20], &source);
        assert_eq!(destination[0], 0x5a);
        assert_eq!(destination[20], 0x5a);
    }

    #[test]
    fn invalid_byte_span_shapes_reject_before_any_privileged_alias_access() {
        let mut access = bootstrap_physical_access::new();
        for ranges in [
            &[][..],
            &[physical_range::new(0, 0)][..],
            &[physical_range::new(0, copy_byte_limit as u64 + 1)][..],
            &[physical_range::new(0, 1); copy_span_capacity + 1][..],
        ] {
            // SAFETY: Each shape is rejected before any address inspection or
            // dereference; no hardware register instruction can be reached.
            assert_eq!(
                unsafe { access.prepare_byte_spans(ranges, copy_direction::to_user) }.err(),
                Some(access_error::invalid_state)
            );
        }
    }

    #[test]
    fn observed_copy_reads_actual_scattered_destinations_and_checks_lengths_before_writes() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let mut access = bootstrap_physical_access::new();
        let mut backing = [0xa5; 48];
        let mut spans = [byte_span {
            address: core::ptr::null_mut(),
            length: 0,
        }; copy_span_capacity];
        spans[0] = byte_span {
            address: backing.as_mut_ptr().wrapping_add(3),
            length: 3,
        };
        spans[1] = byte_span {
            address: backing.as_mut_ptr().wrapping_add(35),
            length: 4,
        };
        let source = [1, 2, 3, 4, 5, 6, 7];
        let mut observed = [0x5a; 9];
        for (direction, source_length, observed_length) in [
            (copy_direction::from_user, 7, 7),
            (copy_direction::to_user, 6, 7),
            (copy_direction::to_user, 7, 6),
        ] {
            let result = catch_unwind(AssertUnwindSafe(|| {
                physical_byte_spans {
                    spans,
                    count: 2,
                    length: 7,
                    direction,
                    _access: &mut access,
                }
                .write_from_observed(
                    &source[..source_length],
                    &mut observed[1..1 + observed_length],
                );
            }));
            assert!(result.is_err());
            assert_eq!(backing, [0xa5; 48]);
            assert_eq!(observed, [0x5a; 9]);
        }
        physical_byte_spans {
            spans,
            count: 2,
            length: 7,
            direction: copy_direction::to_user,
            _access: &mut access,
        }
        .write_from_observed(&source, &mut observed[1..8]);
        assert_eq!(&observed[1..8], &source);
        assert_eq!(&backing[3..6], &source[..3]);
        assert_eq!(&backing[35..39], &source[3..]);
        assert_eq!((observed[0], observed[8]), (0x5a, 0x5a));
        for untouched in [&backing[..3], &backing[6..35], &backing[39..]] {
            assert!(untouched.iter().all(|byte| *byte == 0xa5));
        }
    }
}

// SAFETY: The alias owner preflights the complete bounded RAM extent before
// reading/writing. Source owners independently retain its physical lease. No
// alias pointer survives the synchronous operation or root/owner withdrawal.
#[cfg(any(test, feature = "owned_source_test"))]
unsafe impl<const task_count: usize> super::owned_source::source_memory
    for bootstrap_physical_access_group<task_count>
{
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        // SAFETY: Forward the caller's exclusive initialization/cleanup grant.
        unsafe { mapping_memory::clear_and_verify(self, range) }
    }

    unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool {
        if range.length != source.len() as u64 {
            return false;
        }
        // SAFETY: Caller owns this complete writable extent and source is
        // nonaliasing trusted kernel memory under sole-BSP exclusion.
        let Ok(access) = (unsafe { self.prepare_byte_spans(&[range], copy_direction::to_user) })
        else {
            return false;
        };
        access.write_from(source);
        true
    }

    unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool {
        if range.length != destination.len() as u64 {
            return false;
        }
        // SAFETY: Caller retains the complete readable lease and nonaliasing
        // destination, with alias/root changes excluded until return.
        let Ok(access) = (unsafe { self.prepare_byte_spans(&[range], copy_direction::from_user) })
        else {
            return false;
        };
        access.read_into(destination);
        true
    }
}
