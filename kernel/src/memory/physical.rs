use crate::boot_omen::memory_region;
use core::sync::atomic::{AtomicU64, Ordering};

pub(crate) const frame_size: u64 = 4096;
pub(crate) const frames_per_ledger_byte: usize = 4;
pub(crate) const lease_record_capacity: usize = 128;

const frame_state_mask: u8 = 0b11;
const enabled_attribute: u32 = 1 << 0;
const usable_memory_kind: u32 = 1;
static next_allocator_identity: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum frame_state {
    unavailable = 0,
    free = 1,
    data_owned = 2,
    table_owned = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum frame_purpose {
    data,
    page_table,
}

impl frame_purpose {
    const fn state(self) -> frame_state {
        match self {
            Self::data => frame_state::data_owned,
            Self::page_table => frame_state::table_owned,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct physical_range {
    pub base: u64,
    pub length: u64,
}

impl physical_range {
    pub const fn new(base: u64, length: u64) -> Self {
        Self { base, length }
    }

    pub const fn end(self) -> Option<u64> {
        self.base.checked_add(self.length)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct frame_accounting {
    pub unavailable: usize,
    pub free: usize,
    pub data_owned: usize,
    pub table_owned: usize,
    pub active_leases: usize,
    pub bound_leases: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum frame_error {
    empty_ledger,
    represented_extent_overflow,
    memory_region_overflow { index: usize },
    reservation_overflow { index: usize },
    zero_frame_count,
    invalid_alignment,
    out_of_memory,
    lease_capacity_exhausted,
    lease_generation_exhausted,
    allocator_identity_exhausted,
    lease_inactive,
    foreign_lease,
    stale_lease,
    lease_mismatch,
    lease_already_bound,
    lease_not_bound,
    lease_bound,
    clearing_failed,
}

/// An opaque, generation-checked claim over one contiguous physical extent.
///
/// This type deliberately implements neither `Copy` nor `Clone`. Its private
/// record identity prevents safe callers from fabricating or duplicating a
/// claim. The allocator remains the authority for whether the claim is active
/// and whether it is currently bound into another ownership graph.
#[derive(Debug)]
pub(crate) struct frame_lease {
    allocator_identity: u64,
    record_index: usize,
    generation: u64,
    base_frame: usize,
    frame_count: usize,
    purpose: frame_purpose,
    active: bool,
}

impl frame_lease {
    pub fn range(&self) -> Option<physical_range> {
        if !self.active {
            return None;
        }
        Some(physical_range {
            base: frame_address(self.base_frame),
            length: frame_address(self.frame_count),
        })
    }

    pub const fn frame_count(&self) -> usize {
        self.frame_count
    }

    pub const fn purpose(&self) -> frame_purpose {
        self.purpose
    }

    fn retire_metadata(&mut self) {
        debug_assert!(!self.active);
        // SAFETY: Every value written is valid for its field. Volatile writes
        // make retirement of address-bearing handle metadata observable even
        // when the caller immediately abandons the stack slot.
        unsafe {
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.record_index), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.generation), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.base_frame), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.frame_count), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!(self.purpose), frame_purpose::data);
        }
    }
}

#[derive(Clone, Copy)]
struct lease_record {
    generation: u64,
    base_frame: usize,
    frame_count: usize,
    purpose: frame_purpose,
    active: bool,
    bound: bool,
}

impl lease_record {
    const fn empty() -> Self {
        Self {
            generation: 0,
            base_frame: 0,
            frame_count: 0,
            purpose: frame_purpose::data,
            active: false,
            bound: false,
        }
    }
}

/// Allocation and ownership state for a caller-supplied two-bit frame ledger.
///
/// One ledger byte represents four 4 KiB frames. No memory outside that exact
/// physical extent can become available through this allocator.
pub(crate) struct frame_allocator<'ledger> {
    ledger: &'ledger mut [u8],
    allocator_identity: u64,
    frame_count: usize,
    maximum_physical_extent: u64,
    leases: [lease_record; lease_record_capacity],
}

/// Exclusively borrowed, fully validated authority to clear and retire a lease.
///
/// Construction performs every allocator-owned rejection: provenance,
/// generation, extent, frame state, purpose, activity, and binding. Holding the
/// token prevents either allocator metadata or the lease handle from changing.
/// Completion can therefore fail only when the caller's physical clear and
/// verification callback reports failure; that failure leaves the lease
/// quarantined and all authoritative metadata unchanged.
///
/// This deliberately does not call physical clearing “infallible.” Doing so
/// would either trust an unchecked success assertion or release frames after a
/// detected clear failure, weakening the allocator's reuse invariant.
#[must_use = "a prepared release must be completed or deliberately abandoned"]
pub(crate) struct prepared_frame_release<'owner, 'ledger> {
    allocator: &'owner mut frame_allocator<'ledger>,
    lease: &'owner mut frame_lease,
    record_index: usize,
    record: lease_record,
    range: physical_range,
}

impl<'ledger> frame_allocator<'ledger> {
    /// Normalize firmware regions and fixed reservations into a fresh ledger.
    ///
    /// Validation completes before the supplied ledger is modified. Enabled,
    /// volatile type-1 regions (`attributes == 1`) contribute free frames in
    /// the first pass. Every other enabled descriptor blocks frames in the
    /// second pass, so reserved and nonvolatile overlaps win independent of
    /// firmware ordering. Fixed reservations are applied last.
    pub fn initialize(
        ledger: &'ledger mut [u8],
        regions: &[memory_region],
        reservations: &[physical_range],
    ) -> Result<Self, frame_error> {
        let frame_count = represented_frame_count(ledger)?;
        let maximum_physical_extent = u64::try_from(frame_count)
            .ok()
            .and_then(|count| count.checked_mul(frame_size))
            .ok_or(frame_error::represented_extent_overflow)?;

        validate_inputs(regions, reservations)?;
        let allocator_identity = next_allocator_identity
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |identity| {
                identity.checked_add(1)
            })
            .map_err(|_| frame_error::allocator_identity_exhausted)?;
        ledger.fill(frame_state::unavailable as u8);

        let mut allocator = Self {
            ledger,
            allocator_identity,
            frame_count,
            maximum_physical_extent,
            leases: [lease_record::empty(); lease_record_capacity],
        };

        for region in regions {
            if region_is_volatile_usable(region) {
                allocator.apply_usable_region(region.base, region.length);
            }
        }
        for region in regions {
            if region_is_enabled_blocker(region) {
                allocator.apply_blocking_range(region.base, region.length);
            }
        }
        for reservation in reservations {
            allocator.apply_blocking_range(reservation.base, reservation.length);
        }

        Ok(allocator)
    }

    pub const fn maximum_physical_extent(&self) -> u64 {
        self.maximum_physical_extent
    }

    pub fn accounting(&self) -> frame_accounting {
        let mut result = frame_accounting {
            unavailable: 0,
            free: 0,
            data_owned: 0,
            table_owned: 0,
            active_leases: 0,
            bound_leases: 0,
        };
        for index in 0..self.frame_count {
            match self.state(index) {
                frame_state::unavailable => result.unavailable += 1,
                frame_state::free => result.free += 1,
                frame_state::data_owned => result.data_owned += 1,
                frame_state::table_owned => result.table_owned += 1,
            }
        }
        for record in &self.leases {
            if record.active {
                result.active_leases += 1;
                if record.bound {
                    result.bound_leases += 1;
                }
            }
        }
        result
    }

    /// Claim the lowest free contiguous extent satisfying `alignment_frames`.
    ///
    /// `clear_and_verify` must clear the entire supplied physical range and
    /// return true only after verifying the required initialization state. A
    /// callback failure leaves all allocator metadata unchanged.
    ///
    /// # Safety
    ///
    /// The callback must have exclusive writable access to every range supplied
    /// by this allocator, must clear the complete range, and may return `true`
    /// only after reads verify that initialization state. It must not retain the
    /// raw range or create another owner for it.
    pub unsafe fn allocate_with<F>(
        &mut self,
        frame_count: usize,
        alignment_frames: usize,
        purpose: frame_purpose,
        mut clear_and_verify: F,
    ) -> Result<frame_lease, frame_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        if frame_count == 0 {
            return Err(frame_error::zero_frame_count);
        }
        if alignment_frames == 0 || !alignment_frames.is_power_of_two() {
            return Err(frame_error::invalid_alignment);
        }
        if alignment_frames > self.frame_count
            || u64::try_from(alignment_frames)
                .ok()
                .and_then(|alignment| alignment.checked_mul(frame_size))
                .is_none()
        {
            return Err(frame_error::invalid_alignment);
        }

        let (record_index, generation) = self.available_lease_record()?;
        let base_frame = self
            .find_free_extent(frame_count, alignment_frames)
            .ok_or(frame_error::out_of_memory)?;
        let range = physical_range {
            base: frame_address(base_frame),
            length: frame_address(frame_count),
        };
        if !clear_and_verify(range) {
            return Err(frame_error::clearing_failed);
        }

        let owned_state = purpose.state();
        for index in base_frame..base_frame + frame_count {
            self.set_state(index, owned_state);
        }
        self.leases[record_index] = lease_record {
            generation,
            base_frame,
            frame_count,
            purpose,
            active: true,
            bound: false,
        };

        Ok(frame_lease {
            allocator_identity: self.allocator_identity,
            record_index,
            generation,
            base_frame,
            frame_count,
            purpose,
            active: true,
        })
    }

    /// Mark a lease as owned by a mapping or another resource graph.
    pub fn bind(&mut self, lease: &frame_lease) -> Result<(), frame_error> {
        let index = self.validate_lease(lease)?;
        if self.leases[index].bound {
            return Err(frame_error::lease_already_bound);
        }
        self.leases[index].bound = true;
        Ok(())
    }

    /// Remove the mapping/resource-graph claim from a lease.
    pub fn unbind(&mut self, lease: &frame_lease) -> Result<(), frame_error> {
        let index = self.validate_lease(lease)?;
        if !self.leases[index].bound {
            return Err(frame_error::lease_not_bound);
        }
        self.leases[index].bound = false;
        Ok(())
    }

    pub fn is_bound(&self, lease: &frame_lease) -> Result<bool, frame_error> {
        let index = self.validate_lease(lease)?;
        Ok(self.leases[index].bound)
    }

    /// Validate and exclusively reserve an unbound lease for later release.
    ///
    /// No physical bytes are read or changed. This is suitable for a
    /// pre-publication phase that must remove allocator-owned rejection paths
    /// from a later teardown boundary.
    pub fn prepare_release<'owner>(
        &'owner mut self,
        lease: &'owner mut frame_lease,
    ) -> Result<prepared_frame_release<'owner, 'ledger>, frame_error> {
        let record_index = self.validate_lease(lease)?;
        let record = self.leases[record_index];
        if record.bound {
            return Err(frame_error::lease_bound);
        }

        Ok(prepared_frame_release {
            allocator: self,
            lease,
            record_index,
            record,
            range: physical_range {
                base: frame_address(record.base_frame),
                length: frame_address(record.frame_count),
            },
        })
    }

    /// Clear, verify, and return an unbound lease to the free pool.
    ///
    /// Metadata remains owned if validation or clearing fails. On success the
    /// caller's handle is invalidated as well as the authoritative record.
    ///
    /// # Safety
    ///
    /// The callback has the same exclusive clear-and-verify contract as
    /// `allocate_with`. No hardware, mapping, or hidden caller may still use the
    /// lease's physical extent.
    pub unsafe fn release_with<F>(
        &mut self,
        lease: &mut frame_lease,
        clear_and_verify: F,
    ) -> Result<(), frame_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        let prepared = self.prepare_release(lease)?;
        // SAFETY: `release_with` has the same callback contract as the
        // prepared token's completion operation.
        unsafe { prepared.clear_and_release_with(clear_and_verify) }
    }

    fn apply_usable_region(&mut self, base: u64, length: u64) {
        let end = base + length;
        let clipped_start = base.min(self.maximum_physical_extent);
        let clipped_end = end.min(self.maximum_physical_extent);
        let start_frame = clipped_start.div_ceil(frame_size) as usize;
        let end_frame = (clipped_end / frame_size) as usize;
        for index in start_frame.min(self.frame_count)..end_frame.min(self.frame_count) {
            self.set_state(index, frame_state::free);
        }
    }

    fn apply_blocking_range(&mut self, base: u64, length: u64) {
        if length == 0 || base >= self.maximum_physical_extent {
            return;
        }
        let end = (base + length).min(self.maximum_physical_extent);
        let start_frame = (base / frame_size) as usize;
        let end_frame = end.div_ceil(frame_size) as usize;
        for index in start_frame.min(self.frame_count)..end_frame.min(self.frame_count) {
            self.set_state(index, frame_state::unavailable);
        }
    }

    fn available_lease_record(&self) -> Result<(usize, u64), frame_error> {
        let mut saw_inactive = false;
        for (index, record) in self.leases.iter().enumerate() {
            if record.active {
                continue;
            }
            saw_inactive = true;
            if let Some(generation) = record.generation.checked_add(1) {
                return Ok((index, generation));
            }
        }
        if saw_inactive {
            Err(frame_error::lease_generation_exhausted)
        } else {
            Err(frame_error::lease_capacity_exhausted)
        }
    }

    fn find_free_extent(&self, frame_count: usize, alignment_frames: usize) -> Option<usize> {
        if frame_count > self.frame_count {
            return None;
        }
        let mut candidate = 0usize;
        loop {
            candidate = align_up_frames(candidate, alignment_frames)?;
            let end = candidate.checked_add(frame_count)?;
            if end > self.frame_count {
                return None;
            }
            if let Some(blocked_offset) =
                (candidate..end).position(|index| self.state(index) != frame_state::free)
            {
                candidate = candidate.checked_add(blocked_offset + 1)?;
                continue;
            }
            return Some(candidate);
        }
    }

    fn validate_lease(&self, lease: &frame_lease) -> Result<usize, frame_error> {
        if lease.allocator_identity != self.allocator_identity {
            return Err(frame_error::foreign_lease);
        }
        if !lease.active || lease.record_index >= lease_record_capacity {
            return Err(frame_error::lease_inactive);
        }
        let record = self.leases[lease.record_index];
        if !record.active {
            return Err(frame_error::lease_inactive);
        }
        if record.generation != lease.generation {
            return Err(frame_error::stale_lease);
        }
        if record.base_frame != lease.base_frame
            || record.frame_count != lease.frame_count
            || record.purpose != lease.purpose
        {
            return Err(frame_error::lease_mismatch);
        }
        let Some(end) = record.base_frame.checked_add(record.frame_count) else {
            return Err(frame_error::lease_mismatch);
        };
        if end > self.frame_count
            || (record.base_frame..end).any(|index| self.state(index) != record.purpose.state())
        {
            return Err(frame_error::lease_mismatch);
        }
        Ok(lease.record_index)
    }

    fn state(&self, frame: usize) -> frame_state {
        debug_assert!(frame < self.frame_count);
        let byte = self.ledger[frame / frames_per_ledger_byte];
        let shift = (frame % frames_per_ledger_byte) * 2;
        match (byte >> shift) & frame_state_mask {
            0 => frame_state::unavailable,
            1 => frame_state::free,
            2 => frame_state::data_owned,
            3 => frame_state::table_owned,
            _ => unreachable!(),
        }
    }

    fn set_state(&mut self, frame: usize, state: frame_state) {
        debug_assert!(frame < self.frame_count);
        let byte = &mut self.ledger[frame / frames_per_ledger_byte];
        let shift = (frame % frames_per_ledger_byte) * 2;
        *byte = (*byte & !(frame_state_mask << shift)) | ((state as u8) << shift);
    }
}

impl prepared_frame_release<'_, '_> {
    /// Return the exact already-validated physical extent to clear.
    #[must_use]
    #[cfg(any(
        test,
        feature = "supervised_renderer_test",
        feature = "semantic_surface_transport_test"
    ))]
    pub const fn range(&self) -> physical_range {
        self.range
    }

    /// Clear, verify, and retire the exclusively prepared lease.
    ///
    /// All allocator metadata checks happened during `prepare_release`. A
    /// callback failure is the sole error path and leaves the lease active and
    /// owned. Once the callback reports verified clearing, metadata retirement
    /// contains no further fallible decision.
    ///
    /// # Safety
    ///
    /// The callback must have exclusive writable access to the supplied range,
    /// clear its complete extent, return `true` only after verifying the
    /// required initialization state, and retain no raw range or hidden owner.
    pub unsafe fn clear_and_release_with<F>(
        self,
        mut clear_and_verify: F,
    ) -> Result<(), frame_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        if !clear_and_verify(self.range) {
            return Err(frame_error::clearing_failed);
        }

        for index in self.record.base_frame..self.record.base_frame + self.record.frame_count {
            self.allocator.set_state(index, frame_state::free);
        }
        self.allocator.leases[self.record_index] = lease_record {
            generation: self.record.generation,
            ..lease_record::empty()
        };
        self.lease.active = false;
        self.lease.retire_metadata();
        Ok(())
    }
}

fn represented_frame_count(ledger: &[u8]) -> Result<usize, frame_error> {
    if ledger.is_empty() {
        return Err(frame_error::empty_ledger);
    }
    ledger
        .len()
        .checked_mul(frames_per_ledger_byte)
        .ok_or(frame_error::represented_extent_overflow)
}

fn validate_inputs(
    regions: &[memory_region],
    reservations: &[physical_range],
) -> Result<(), frame_error> {
    for (index, region) in regions.iter().enumerate() {
        if region.length != 0
            && region.attributes & enabled_attribute != 0
            && region.base.checked_add(region.length).is_none()
        {
            return Err(frame_error::memory_region_overflow { index });
        }
    }
    for (index, reservation) in reservations.iter().enumerate() {
        if reservation.end().is_none() {
            return Err(frame_error::reservation_overflow { index });
        }
    }
    Ok(())
}

const fn region_is_volatile_usable(region: &memory_region) -> bool {
    region.kind == usable_memory_kind
        && region.attributes == enabled_attribute
        && region.length != 0
}

const fn region_is_enabled_blocker(region: &memory_region) -> bool {
    region.attributes & enabled_attribute != 0
        && !region_is_volatile_usable(region)
        && region.length != 0
}

const fn frame_address(frame: usize) -> u64 {
    frame as u64 * frame_size
}

const fn align_up_frames(value: usize, alignment: usize) -> Option<usize> {
    let mask = alignment - 1;
    match value.checked_add(mask) {
        Some(adjusted) => Some(adjusted & !mask),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    const page: u64 = frame_size;

    fn region(base: u64, length: u64, kind: u32, attributes: u32) -> memory_region {
        memory_region {
            base,
            length,
            kind,
            attributes,
        }
    }

    fn all_usable(frame_count: usize) -> memory_region {
        region(0, frame_count as u64 * page, 1, 1)
    }

    fn duplicate_for_test(lease: &frame_lease) -> frame_lease {
        frame_lease {
            allocator_identity: lease.allocator_identity,
            record_index: lease.record_index,
            generation: lease.generation,
            base_frame: lease.base_frame,
            frame_count: lease.frame_count,
            purpose: lease.purpose,
            active: lease.active,
        }
    }

    fn zero_backing(backing: &mut [u8], range: physical_range) -> bool {
        let Ok(start) = usize::try_from(range.base) else {
            return false;
        };
        let Ok(length) = usize::try_from(range.length) else {
            return false;
        };
        let Some(end) = start.checked_add(length) else {
            return false;
        };
        let Some(bytes) = backing.get_mut(start..end) else {
            return false;
        };
        bytes.fill(0);
        bytes.iter().all(|byte| *byte == 0)
    }

    fn allocate_for_test<F>(
        allocator: &mut frame_allocator<'_>,
        frame_count: usize,
        alignment_frames: usize,
        purpose: frame_purpose,
        clear_and_verify: F,
    ) -> Result<frame_lease, frame_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        // SAFETY: These unit tests own isolated logical ledgers with no
        // hardware users. Their callbacks model (or explicitly exercise)
        // complete synchronous clearing of each allocator-supplied range.
        unsafe { allocator.allocate_with(frame_count, alignment_frames, purpose, clear_and_verify) }
    }

    fn release_for_test<F>(
        allocator: &mut frame_allocator<'_>,
        lease: &mut frame_lease,
        clear_and_verify: F,
    ) -> Result<(), frame_error>
    where
        F: FnMut(physical_range) -> bool,
    {
        // SAFETY: No external user can retain or access extents in these
        // isolated ledger models; callbacks synchronously model full clearing.
        unsafe { allocator.release_with(lease, clear_and_verify) }
    }

    #[test]
    fn reserved_overlap_wins_independent_of_firmware_order() {
        let usable = region(0, 16 * page, 1, 1);
        let blocker = region(4 * page + 1, 2 * page, 2, 1);
        let mut first_ledger = [0xa5; 4];
        let mut second_ledger = [0x5a; 4];

        {
            let first =
                frame_allocator::initialize(&mut first_ledger, &[usable, blocker], &[]).unwrap();
            assert_eq!(first.accounting().free, 13);
        }
        {
            let second =
                frame_allocator::initialize(&mut second_ledger, &[blocker, usable], &[]).unwrap();
            assert_eq!(second.accounting().free, 13);
        }

        assert_eq!(first_ledger, second_ledger);
    }

    #[test]
    fn usable_rounds_inward_and_blockers_round_outward() {
        let regions = [
            region(1, 5 * page - 2, 1, 1),
            region(2 * page + 1, page - 2, 2, 1),
        ];
        let mut ledger = [0xff; 2];
        let allocator = frame_allocator::initialize(&mut ledger, &regions, &[]).unwrap();
        let accounting = allocator.accounting();

        assert_eq!(accounting.free, 2);
        assert_eq!(accounting.unavailable, 6);
    }

    #[test]
    fn invalid_input_does_not_mutate_the_supplied_ledger() {
        let original = [0xa5; 2];
        let mut region_ledger = original;
        let error = match frame_allocator::initialize(
            &mut region_ledger,
            &[region(u64::MAX - 1, 4, 1, 1)],
            &[],
        ) {
            Ok(_) => panic!("overflowing region was accepted"),
            Err(error) => error,
        };
        assert_eq!(error, frame_error::memory_region_overflow { index: 0 });
        assert_eq!(region_ledger, original);

        let mut reservation_ledger = original;
        let error = match frame_allocator::initialize(
            &mut reservation_ledger,
            &[all_usable(8)],
            &[physical_range::new(u64::MAX, 1)],
        ) {
            Ok(_) => panic!("overflowing reservation was accepted"),
            Err(error) => error,
        };
        assert_eq!(error, frame_error::reservation_overflow { index: 0 });
        assert_eq!(reservation_ledger, original);
    }

    #[test]
    fn disabled_and_empty_wrapping_descriptors_grant_nothing_without_rejection() {
        let regions = [
            region(u64::MAX, 2, 1, 0),
            region(u64::MAX, 0, 1, 1),
            region(0, 4 * page, 1, 1),
        ];
        let mut ledger = [0xa5; 1];
        let allocator = frame_allocator::initialize(&mut ledger, &regions, &[]).unwrap();

        assert_eq!(allocator.accounting().free, 4);
    }

    #[test]
    fn disabled_and_nonvolatile_memory_never_become_free() {
        let regions = [
            all_usable(12),
            region(2 * page, 2 * page, 1, 0),
            region(4 * page, 3 * page, 1, 3),
            region(8 * page, page, 9, 1),
        ];
        let mut ledger = [0; 3];
        let allocator = frame_allocator::initialize(&mut ledger, &regions, &[]).unwrap();

        // Disabled entries do not override an enabled usable descriptor.
        // Enabled nonvolatile and unknown memory do.
        assert_eq!(allocator.accounting().free, 8);
        assert_eq!(allocator.accounting().unavailable, 4);
    }

    #[test]
    fn enabled_unknown_kind_blocks_overlapping_usable_memory() {
        let regions = [all_usable(8), region(2 * page, 3 * page, u32::MAX, 1)];
        let mut ledger = [0; 2];
        let allocator = frame_allocator::initialize(&mut ledger, &regions, &[]).unwrap();

        assert_eq!(allocator.accounting().free, 5);
        assert_eq!(allocator.accounting().unavailable, 3);
    }

    #[test]
    fn represented_extent_clips_regions_and_reservations() {
        let mut ledger = [0; 2];
        let reservations = [physical_range::new(6 * page + 1, 8 * page)];
        let allocator =
            frame_allocator::initialize(&mut ledger, &[region(0, 32 * page, 1, 1)], &reservations)
                .unwrap();

        assert_eq!(allocator.maximum_physical_extent(), 8 * page);
        assert_eq!(allocator.accounting().free, 6);
        assert_eq!(allocator.accounting().unavailable, 2);
    }

    #[test]
    fn duplicate_usable_coverage_is_counted_once() {
        let regions = [
            region(0, 8 * page, 1, 1),
            region(2 * page, 4 * page, 1, 1),
            region(0, 8 * page, 1, 1),
        ];
        let mut ledger = [0; 2];
        let allocator = frame_allocator::initialize(&mut ledger, &regions, &[]).unwrap();

        assert_eq!(allocator.accounting().free, 8);
        assert_eq!(allocator.accounting().unavailable, 0);
    }

    #[test]
    fn each_explicit_reservation_is_applied_outward() {
        let reservations = [
            physical_range::new(0, page),
            physical_range::new(3 * page + 1, page),
            physical_range::new(7 * page, 2 * page),
        ];
        let mut ledger = [0; 3];
        let allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(12)], &reservations).unwrap();

        // Reservations block frame 0, outward-rounded frames 3 and 4, and
        // exact frames 7 and 8.
        assert_eq!(allocator.accounting().unavailable, 5);
        assert_eq!(allocator.accounting().free, 7);
    }

    #[test]
    fn allocation_is_lowest_fit_contiguous_aligned_and_purpose_typed() {
        let mut ledger = [0; 4];
        let reservations = [physical_range::new(0, page)];
        let mut backing = vec![0xa5; 16 * page as usize];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(16)], &reservations).unwrap();

        let first = allocate_for_test(&mut allocator, 2, 4, frame_purpose::page_table, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();
        let second = allocate_for_test(&mut allocator, 3, 2, frame_purpose::data, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();

        assert_eq!(first.range(), Some(physical_range::new(4 * page, 2 * page)));
        assert_eq!(first.purpose(), frame_purpose::page_table);
        assert_eq!(
            second.range(),
            Some(physical_range::new(6 * page, 3 * page))
        );
        assert_eq!(second.frame_count(), 3);
        assert_eq!(allocator.accounting().table_owned, 2);
        assert_eq!(allocator.accounting().data_owned, 3);
    }

    #[test]
    fn failed_allocation_and_clear_leave_accounting_unchanged() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let baseline = allocator.accounting();

        assert_eq!(
            allocate_for_test(&mut allocator, 5, 1, frame_purpose::data, |_| true).unwrap_err(),
            frame_error::out_of_memory
        );
        assert_eq!(allocator.accounting(), baseline);
        assert_eq!(
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| false).unwrap_err(),
            frame_error::clearing_failed
        );
        assert_eq!(allocator.accounting(), baseline);
    }

    #[test]
    fn zero_non_power_of_two_and_unsupported_alignments_are_rejected() {
        let mut ledger = [0; 2];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(8)], &[]).unwrap();
        let baseline = allocator.accounting();

        assert_eq!(
            allocate_for_test(&mut allocator, 0, 1, frame_purpose::data, |_| true).unwrap_err(),
            frame_error::zero_frame_count
        );
        for alignment in [0, 3, 16] {
            assert_eq!(
                allocate_for_test(&mut allocator, 1, alignment, frame_purpose::data, |_| true,)
                    .unwrap_err(),
                frame_error::invalid_alignment
            );
        }
        assert_eq!(allocator.accounting(), baseline);
    }

    #[test]
    fn bound_lease_cannot_be_released() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut lease =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();
        allocator.bind(&lease).unwrap();

        assert!(allocator.is_bound(&lease).unwrap());
        assert_eq!(
            allocator.bind(&lease),
            Err(frame_error::lease_already_bound)
        );
        assert_eq!(
            release_for_test(&mut allocator, &mut lease, |_| true),
            Err(frame_error::lease_bound)
        );
        assert_eq!(allocator.accounting().data_owned, 1);
        allocator.unbind(&lease).unwrap();
        release_for_test(&mut allocator, &mut lease, |_| true).unwrap();
        assert_eq!(allocator.accounting().free, 4);
    }

    #[test]
    fn double_release_and_stale_generation_are_rejected() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut first =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();
        let stale = duplicate_for_test(&first);
        release_for_test(&mut allocator, &mut first, |_| true).unwrap();
        assert_eq!(
            release_for_test(&mut allocator, &mut first, |_| true),
            Err(frame_error::lease_inactive)
        );

        let replacement =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();
        assert_eq!(allocator.bind(&stale), Err(frame_error::stale_lease));
        assert_eq!(replacement.range(), Some(physical_range::new(0, page)));
    }

    #[test]
    fn leases_are_bound_to_the_issuing_allocator_instance() {
        let mut first_ledger = [0; 1];
        let mut second_ledger = [0; 1];
        let mut first =
            frame_allocator::initialize(&mut first_ledger, &[all_usable(4)], &[]).unwrap();
        let mut second =
            frame_allocator::initialize(&mut second_ledger, &[all_usable(4)], &[]).unwrap();
        let lease = allocate_for_test(&mut first, 1, 1, frame_purpose::data, |_| true).unwrap();
        let mut foreign = duplicate_for_test(&lease);
        let baseline = second.accounting();

        assert_eq!(second.bind(&foreign), Err(frame_error::foreign_lease));
        assert_eq!(
            release_for_test(&mut second, &mut foreign, |_| true),
            Err(frame_error::foreign_lease)
        );
        assert_eq!(second.accounting(), baseline);
    }

    #[test]
    fn reinitializing_the_same_ledger_invalidates_old_leases() {
        let mut ledger = [0; 1];
        let old_lease = {
            let mut first =
                frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
            allocate_for_test(&mut first, 1, 1, frame_purpose::data, |_| true).unwrap()
        };
        let mut replacement =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let current =
            allocate_for_test(&mut replacement, 1, 1, frame_purpose::data, |_| true).unwrap();

        assert_eq!(
            replacement.bind(&old_lease),
            Err(frame_error::foreign_lease)
        );
        assert_eq!(current.range(), old_lease.range());
    }

    #[test]
    fn altered_lease_extent_and_purpose_are_rejected() {
        let mut ledger = [0; 2];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(8)], &[]).unwrap();
        let lease = allocate_for_test(&mut allocator, 2, 1, frame_purpose::data, |_| true).unwrap();
        let baseline = allocator.accounting();

        let mut altered_extent = duplicate_for_test(&lease);
        altered_extent.frame_count += 1;
        assert_eq!(
            allocator.bind(&altered_extent),
            Err(frame_error::lease_mismatch)
        );

        let mut altered_purpose = duplicate_for_test(&lease);
        altered_purpose.purpose = frame_purpose::page_table;
        assert_eq!(
            allocator.bind(&altered_purpose),
            Err(frame_error::lease_mismatch)
        );
        assert_eq!(allocator.accounting(), baseline);
    }

    #[test]
    fn failed_release_clear_keeps_the_lease_quarantined() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut lease =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();

        assert_eq!(
            release_for_test(&mut allocator, &mut lease, |_| false),
            Err(frame_error::clearing_failed)
        );
        assert_eq!(lease.range(), Some(physical_range::new(0, page)));
        assert_eq!(allocator.accounting().data_owned, 1);
        assert_eq!(allocator.accounting().free, 3);
        release_for_test(&mut allocator, &mut lease, |_| true).unwrap();
        assert_eq!(allocator.accounting().free, 4);
    }

    #[test]
    fn prepared_release_is_exclusive_and_retires_only_after_verified_clear() {
        let mut ledger = [0; 1];
        let mut backing = vec![0xa5; 4 * page as usize];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut lease = allocate_for_test(&mut allocator, 2, 1, frame_purpose::data, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();
        let expected = physical_range::new(0, 2 * page);
        backing[..expected.length as usize].fill(0x7b);

        let prepared = allocator
            .prepare_release(&mut lease)
            .expect("valid unbound release");
        assert_eq!(prepared.range(), expected);
        // SAFETY: The backing vector models the token's exact exclusive
        // physical extent, and the callback clears and verifies every byte.
        unsafe { prepared.clear_and_release_with(|range| zero_backing(&mut backing, range)) }
            .expect("verified prepared release");

        assert!(
            backing[..expected.length as usize]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert!(lease.range().is_none());
        assert_eq!(allocator.accounting().free, 4);
        assert_eq!(allocator.accounting().active_leases, 0);
    }

    #[test]
    fn prepared_release_rejects_bound_and_quarantines_clear_failure() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut lease =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();
        allocator.bind(&lease).unwrap();

        assert!(matches!(
            allocator.prepare_release(&mut lease),
            Err(frame_error::lease_bound)
        ));
        assert_eq!(allocator.accounting().data_owned, 1);
        assert_eq!(allocator.accounting().bound_leases, 1);

        allocator.unbind(&lease).unwrap();
        let owned = allocator.accounting();
        let prepared = allocator
            .prepare_release(&mut lease)
            .expect("unbound release can be prepared");
        assert_eq!(prepared.range(), physical_range::new(0, page));
        // SAFETY: This deliberate false callback exercises failure handling;
        // it claims no clear and therefore authorizes no frame reuse.
        assert_eq!(
            unsafe { prepared.clear_and_release_with(|_| false) },
            Err(frame_error::clearing_failed)
        );

        assert_eq!(lease.range(), Some(physical_range::new(0, page)));
        assert_eq!(allocator.accounting(), owned);
        release_for_test(&mut allocator, &mut lease, |_| true).unwrap();
        assert_eq!(allocator.accounting().free, 4);
    }

    #[test]
    fn released_frames_are_cleared_before_deterministic_reuse() {
        let mut ledger = [0; 2];
        let mut backing = vec![0xa5; 8 * page as usize];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(8)], &[]).unwrap();
        let mut lease = allocate_for_test(&mut allocator, 2, 1, frame_purpose::data, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();
        let range = lease.range().unwrap();
        backing[range.base as usize..(range.base + range.length) as usize].fill(0x6b);

        release_for_test(&mut allocator, &mut lease, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();
        assert!(backing[0..2 * page as usize].iter().all(|byte| *byte == 0));

        let reused = allocate_for_test(&mut allocator, 2, 1, frame_purpose::page_table, |range| {
            zero_backing(&mut backing, range)
        })
        .unwrap();
        assert_eq!(reused.range(), Some(range));
        assert_eq!(allocator.accounting().table_owned, 2);
    }

    #[test]
    fn successful_release_retires_address_bearing_lease_metadata() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        let mut lease =
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap();
        let record_index = lease.record_index;
        let generation = lease.generation;

        release_for_test(&mut allocator, &mut lease, |_| true).unwrap();

        assert!(!lease.active);
        assert_eq!(lease.record_index, 0);
        assert_eq!(lease.generation, 0);
        assert_eq!(lease.base_frame, 0);
        assert_eq!(lease.frame_count, 0);
        assert_eq!(lease.purpose, frame_purpose::data);
        assert_eq!(allocator.leases[record_index].generation, generation);
        assert_eq!(allocator.leases[record_index].base_frame, 0);
        assert_eq!(allocator.leases[record_index].frame_count, 0);
        assert!(!allocator.leases[record_index].active);
        assert!(!allocator.leases[record_index].bound);
    }

    #[test]
    fn lease_record_exhaustion_has_no_partial_state_change() {
        let mut ledger = [0; lease_record_capacity / frames_per_ledger_byte + 1];
        let region = all_usable(ledger.len() * frames_per_ledger_byte);
        let mut allocator = frame_allocator::initialize(&mut ledger, &[region], &[]).unwrap();
        let mut leases: Vec<frame_lease> = Vec::new();
        for _ in 0..lease_record_capacity {
            leases.push(
                allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap(),
            );
        }
        let baseline = allocator.accounting();

        assert_eq!(
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap_err(),
            frame_error::lease_capacity_exhausted
        );
        assert_eq!(allocator.accounting(), baseline);
        assert_eq!(leases.len(), lease_record_capacity);
    }

    #[test]
    fn exhausted_generations_never_wrap_to_a_stale_value() {
        let mut ledger = [0; 1];
        let mut allocator =
            frame_allocator::initialize(&mut ledger, &[all_usable(4)], &[]).unwrap();
        for record in &mut allocator.leases {
            record.generation = u64::MAX;
        }
        let baseline = allocator.accounting();

        assert_eq!(
            allocate_for_test(&mut allocator, 1, 1, frame_purpose::data, |_| true).unwrap_err(),
            frame_error::lease_generation_exhausted
        );
        assert_eq!(allocator.accounting(), baseline);
    }
}
