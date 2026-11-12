#![cfg_attr(not(test), allow(dead_code))]

//! Retained native source bytes, independent of executable formats and policy.
//! A caller identity or numeric grant is inert metadata. The embedding must
//! authenticate the current task and its copy before offering kernel bytes.
//! The opt-in provisioning origin instead requires an explicit privileged boot
//! controller; it cannot impersonate a task or use the task upload operations.

use crate::identity::task_identity;
use crate::memory::physical::{
    frame_allocator, frame_error, frame_lease, frame_purpose, frame_size, physical_range,
};

pub(super) const source_chunk_limit: usize = 4096;
const readback_bytes: usize = 64;

/// Privileged access which remains independently available through cleanup.
///
/// # Safety
/// Implementations access only the supplied extent through valid RAM mappings,
/// never retain a raw range or slice, and never mutate unrelated storage.
/// Successful clearing means complete zeroing and readback. Successful reads
/// and writes cover exactly the supplied slice and extent. Failed operations
/// may modify a prefix of their destination; they cannot report false success.
pub(super) unsafe trait source_memory {
    /// # Safety
    /// The caller exclusively owns this extent, or is inside the allocator's
    /// synchronous initialization callback. No consumer can still use it.
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool;

    /// # Safety
    /// The caller owns the complete writable extent and excludes consumers.
    /// The nonempty source has the same length and does not alias this extent.
    unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool;

    /// # Safety
    /// The caller retains readable ownership of the complete extent. The
    /// nonempty destination has the same length and does not alias the extent.
    unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct source_budget {
    pub max_bytes: u64,
    pub max_pages: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum source_error {
    invalid_binding,
    invalid_budget,
    invalid_state,
    invalid_extent,
    invalid_offset,
    chunk_size,
    incomplete,
    write_failed,
    read_failed,
    readback_mismatch,
    frame(frame_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum source_phase {
    receiving,
    sealed,
    releasing,
    released,
}

/// Provenance metadata, never a constructor for execution or access authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum source_origin {
    task(task_identity),
    #[cfg(any(
        test,
        feature = "native_program_test",
        feature = "native_manifest_test",
        feature = "linux_loader_entry_test"
    ))]
    provisioned(core::num::NonZeroU64),
}

struct retained_source {
    lease: frame_lease,
    bound: bool,
}

/// One contiguous, accounted snapshot with no automatic fallible Drop cleanup.
/// Retain the owner after every error until explicit cleanup succeeds. Grants
/// must be freshly assigned by the trusted embedding, not reused for another
/// upload from the same task incarnation. No user pointer is retained here.
#[must_use = "retain the source owner until verified cleanup succeeds"]
pub(super) struct owned_source {
    origin: source_origin,
    grant: u64,
    length: u64,
    pages: usize,
    received: u64,
    storage: Option<retained_source>,
    phase: source_phase,
}

/// A reader borrows the sealed owner, preventing mutation or cleanup for its
/// lifetime. Possessing copied identity/length fields cannot create a reader.
pub(super) struct sealed_source_reader<'a> {
    owner: &'a owned_source,
}

impl owned_source {
    pub(super) fn new(
        uploader: task_identity,
        grant: u64,
        length: u64,
        budget: source_budget,
    ) -> Result<Self, source_error> {
        Self::new_origin(source_origin::task(uploader), grant, length, budget)
    }

    /// Create an inert owner for bytes supplied by a privileged boot controller.
    /// Numeric grants or firmware contents never authorize this operation.
    ///
    /// # Safety
    /// The caller is the explicitly selected boot provisioning controller, has
    /// assigned a fresh nonzero grant and retains the owner through cleanup.
    /// It must not attribute these bytes to a task or permit task upload APIs.
    #[cfg(any(
        test,
        feature = "native_program_test",
        feature = "native_manifest_test",
        feature = "linux_loader_entry_test"
    ))]
    pub(super) unsafe fn new_provisioned(
        grant: u64,
        length: u64,
        budget: source_budget,
    ) -> Result<Self, source_error> {
        let nonzero = core::num::NonZeroU64::new(grant).ok_or(source_error::invalid_binding)?;
        Self::new_origin(source_origin::provisioned(nonzero), grant, length, budget)
    }

    fn new_origin(
        origin: source_origin,
        grant: u64,
        length: u64,
        budget: source_budget,
    ) -> Result<Self, source_error> {
        if grant == 0 {
            return Err(source_error::invalid_binding);
        }
        let pages = length
            .checked_add(frame_size - 1)
            .map(|rounded| rounded / frame_size)
            .and_then(|count| usize::try_from(count).ok())
            .filter(|count| *count != 0 && *count <= budget.max_pages)
            .ok_or(source_error::invalid_budget)?;
        if length > budget.max_bytes {
            return Err(source_error::invalid_budget);
        }
        Ok(Self {
            origin,
            grant,
            length,
            pages,
            received: 0,
            storage: None,
            phase: source_phase::receiving,
        })
    }

    /// Allocate once, storing every acquired lease before its fallible bind.
    /// An initialization failure creates no lease under the allocator contract;
    /// the inert owner can retry allocation or be cancelled by cleanup.
    ///
    /// # Safety
    /// Sole BSP, IF clear/NMI masked; the memory implementation can exclusively
    /// clear any extent selected by this allocator. Retain both owners on error.
    pub(super) unsafe fn allocate(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        if self.phase != source_phase::receiving || self.storage.is_some() || self.received != 0 {
            return Err(source_error::invalid_state);
        }
        // SAFETY: The embedding supplies exclusive allocator/backend access.
        let lease = unsafe {
            allocator.allocate_with(self.pages, 1, frame_purpose::data, |range| {
                memory.clear_and_verify(range)
            })
        }
        .map_err(source_error::frame)?;
        self.storage = Some(retained_source {
            lease,
            bound: false,
        });
        let retained = self.storage.as_mut().ok_or(source_error::invalid_state)?;
        allocator
            .bind(&retained.lease)
            .map_err(source_error::frame)?;
        retained.bound = true;
        Ok(())
    }

    /// All metadata rejection precedes writes. Storage failures may dirty the
    /// unaccepted suffix, but never advance `received`; retry the same offset.
    /// A complete successful write/readback is required before sealing.
    ///
    /// # Safety
    /// The embedding authenticated the exact live uploader/current root/stack,
    /// copied this chunk into nonaliasing kernel RAM, and retains exclusive
    /// source ownership and independent access with IF clear/NMI masked.
    pub(super) unsafe fn append(
        &mut self,
        caller: task_identity,
        grant: u64,
        offset: u64,
        bytes: &[u8],
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        self.validate_append(caller, grant, offset, bytes.len())?;
        // SAFETY: Task authentication and the caller's retained copy contract
        // precede the same owned write/readback path used by provisioning.
        unsafe { self.append_bytes(offset, bytes, memory) }
    }

    /// Append a sequential controller-owned chunk, with the same complete
    /// write/readback and retry semantics as a task-origin source.
    ///
    /// # Safety
    /// The selected privileged controller supplies nonaliasing kernel bytes;
    /// it retains the independent source lease and physical-access owner under
    /// sole-BSP IF-clear/NMI-masked exclusion. This is not a task upload gate.
    #[cfg(any(
        test,
        feature = "native_program_test",
        feature = "native_manifest_test",
        feature = "linux_loader_entry_test"
    ))]
    pub(super) unsafe fn append_provisioned(
        &mut self,
        grant: u64,
        offset: u64,
        bytes: &[u8],
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        self.validate_provisioned(grant)?;
        self.validate_chunk(offset, bytes.len())?;
        // SAFETY: Checked provisioned origin and caller's exclusive ownership.
        unsafe { self.append_bytes(offset, bytes, memory) }
    }

    unsafe fn append_bytes(
        &mut self,
        offset: u64,
        bytes: &[u8],
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        let range = self.chunk_range(offset, bytes.len())?;
        // SAFETY: Complete owned destination, checked chunk and nonaliasing
        // trusted slice under the embedding's exclusion interval.
        if !unsafe { memory.write(range, bytes) } {
            return Err(source_error::write_failed);
        }
        let mut observed = [0; readback_bytes];
        for (index, expected) in bytes.chunks(readback_bytes).enumerate() {
            let part = physical_range::new(
                range.base + (index * readback_bytes) as u64,
                expected.len() as u64,
            );
            // SAFETY: Every part lies within the checked complete chunk. The
            // local scratch is separate from owned source storage.
            if !unsafe { memory.read(part, &mut observed[..expected.len()]) } {
                return Err(source_error::read_failed);
            }
            if &observed[..expected.len()] != expected {
                return Err(source_error::readback_mismatch);
            }
        }
        // chunk_range proved this addition and the logical extent beforehand.
        self.received = offset + bytes.len() as u64;
        Ok(())
    }

    /// Inert request validation before the embedding reads any uploader bytes.
    /// It grants no user-copy or physical-access authority.
    pub(super) fn validate_append(
        &self,
        caller: task_identity,
        grant: u64,
        offset: u64,
        length: usize,
    ) -> Result<(), source_error> {
        self.validate_uploader(caller, grant)?;
        self.validate_chunk(offset, length)
    }

    fn validate_chunk(&self, offset: u64, length: usize) -> Result<(), source_error> {
        if self.phase != source_phase::receiving {
            return Err(source_error::invalid_state);
        }
        if offset != self.received {
            return Err(source_error::invalid_offset);
        }
        self.chunk_range(offset, length)?;
        Ok(())
    }

    pub(super) fn seal(&mut self, caller: task_identity, grant: u64) -> Result<(), source_error> {
        self.validate_uploader(caller, grant)?;
        self.seal_complete()
    }

    /// # Safety
    /// Only the selected privileged provisioning controller may seal this
    /// source after checking its complete external-input contract. IF is clear
    /// and no importer or consumer can concurrently access the retained owner.
    #[cfg(any(
        test,
        feature = "native_program_test",
        feature = "native_manifest_test",
        feature = "linux_loader_entry_test"
    ))]
    pub(super) unsafe fn seal_provisioned(&mut self, grant: u64) -> Result<(), source_error> {
        self.validate_provisioned(grant)?;
        self.seal_complete()
    }

    fn seal_complete(&mut self) -> Result<(), source_error> {
        if self.phase != source_phase::receiving {
            return Err(source_error::invalid_state);
        }
        self.owned_range()?;
        if self.received != self.length {
            return Err(source_error::incomplete);
        }
        self.phase = source_phase::sealed;
        Ok(())
    }

    pub(super) fn reader(&self) -> Result<sealed_source_reader<'_>, source_error> {
        if self.phase != source_phase::sealed || self.received != self.length {
            return Err(source_error::invalid_state);
        }
        self.owned_range()?;
        Ok(sealed_source_reader { owner: self })
    }

    /// Withdraw access before the first fallible clear. A failed release keeps
    /// its unbound lease and cleanup progress, while append/seal/read stay shut.
    /// Repeated successful cleanup is harmless; this owner cannot be reopened.
    ///
    /// # Safety
    /// No hardware or hidden consumer references the snapshot. Its independently
    /// live access remains valid through complete clear/readback and release.
    /// Sole BSP, IF clear/NMI masked; retain the owner on failure and retry.
    pub(super) unsafe fn cleanup(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        if self.phase == source_phase::released {
            return Ok(());
        }
        if let Some(retained) = self.storage.as_mut() {
            if allocator
                .is_bound(&retained.lease)
                .map_err(source_error::frame)?
                != retained.bound
            {
                return Err(source_error::invalid_state);
            }
            self.phase = source_phase::releasing;
            if retained.bound {
                allocator
                    .unbind(&retained.lease)
                    .map_err(source_error::frame)?;
                retained.bound = false;
            }
            // SAFETY: Uses were withdrawn and this exact lease stays retained
            // if clearing fails. No raw range escapes the synchronous callback.
            unsafe {
                allocator.release_with(&mut retained.lease, |range| memory.clear_and_verify(range))
            }
            .map_err(source_error::frame)?;
            self.storage = None;
        }
        self.received = 0;
        self.phase = source_phase::released;
        Ok(())
    }

    pub(super) const fn origin(&self) -> source_origin {
        self.origin
    }

    pub(super) const fn uploader(&self) -> Option<task_identity> {
        match self.origin {
            source_origin::task(identity) => Some(identity),
            #[cfg(any(
                test,
                feature = "native_program_test",
                feature = "native_manifest_test",
                feature = "linux_loader_entry_test"
            ))]
            source_origin::provisioned(_) => None,
        }
    }

    pub(super) const fn grant(&self) -> u64 {
        self.grant
    }

    pub(super) const fn length(&self) -> u64 {
        self.length
    }

    pub(super) const fn received(&self) -> u64 {
        self.received
    }

    pub(super) fn is_sealed(&self) -> bool {
        self.phase == source_phase::sealed
    }

    pub(super) fn is_released(&self) -> bool {
        self.phase == source_phase::released
    }

    /// Inspection geometry only; never a physical access or release grant.
    pub(super) fn retained_range(&self) -> Option<physical_range> {
        self.storage.as_ref()?.lease.range()
    }

    fn validate_uploader(&self, caller: task_identity, grant: u64) -> Result<(), source_error> {
        if self.origin != source_origin::task(caller) || grant == 0 || grant != self.grant {
            return Err(source_error::invalid_binding);
        }
        Ok(())
    }

    #[cfg(any(
        test,
        feature = "native_program_test",
        feature = "native_manifest_test",
        feature = "linux_loader_entry_test"
    ))]
    fn validate_provisioned(&self, grant: u64) -> Result<(), source_error> {
        match self.origin {
            source_origin::provisioned(expected)
                if expected.get() == grant && self.grant == grant =>
            {
                Ok(())
            }
            _ => Err(source_error::invalid_binding),
        }
    }

    fn owned_range(&self) -> Result<physical_range, source_error> {
        let retained = self.storage.as_ref().ok_or(source_error::invalid_state)?;
        let range = retained.lease.range().ok_or(source_error::invalid_state)?;
        if !retained.bound
            || retained.lease.purpose() != frame_purpose::data
            || retained.lease.frame_count() != self.pages
            || range.length < self.length
            || range.end().is_none()
        {
            return Err(source_error::invalid_state);
        }
        Ok(range)
    }

    fn chunk_range(&self, offset: u64, length: usize) -> Result<physical_range, source_error> {
        if length == 0 || length > source_chunk_limit {
            return Err(source_error::chunk_size);
        }
        let end = offset
            .checked_add(length as u64)
            .filter(|end| *end <= self.length)
            .ok_or(source_error::invalid_extent)?;
        let storage = self.owned_range()?;
        let start = storage
            .base
            .checked_add(offset)
            .ok_or(source_error::invalid_extent)?;
        storage
            .base
            .checked_add(end)
            .ok_or(source_error::invalid_extent)?;
        Ok(physical_range::new(start, length as u64))
    }
}

impl sealed_source_reader<'_> {
    pub(super) const fn length(&self) -> u64 {
        self.owner.length
    }

    pub(super) const fn origin(&self) -> source_origin {
        self.owner.origin
    }

    pub(super) const fn uploader(&self) -> Option<task_identity> {
        self.owner.uploader()
    }

    pub(super) const fn grant(&self) -> u64 {
        self.owner.grant
    }

    /// Metadata errors leave the output untouched. A failed backend read may
    /// modify its output prefix; callers must discard it on Err.
    ///
    /// # Safety
    /// Independently valid privileged access covers the retained snapshot.
    /// The output does not alias it, and IF-clear/NMI-masked ownership exclusion
    /// prevents concurrent mutation or alias withdrawal for this complete read.
    pub(super) unsafe fn read(
        &self,
        offset: u64,
        destination: &mut [u8],
        memory: &mut impl source_memory,
    ) -> Result<(), source_error> {
        let range = self.owner.chunk_range(offset, destination.len())?;
        // SAFETY: The shared owner borrow excludes cleanup and every returned
        // extent is bounded inside its complete sealed logical contents.
        if !unsafe { memory.read(range, destination) } {
            return Err(source_error::read_failed);
        }
        Ok(())
    }
}

/// Lend a genuine modeled snapshot only for the callback. The result cannot
/// borrow its local owner; cleanup and accounting checks run before returning.
#[cfg(test)]
pub(super) fn with_test_sealed_reader<R>(
    bytes: &[u8],
    callback: impl for<'a> FnOnce(sealed_source_reader<'a>) -> R,
) -> R {
    tests::with_sealed_reader(bytes, callback)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::boot_omen::memory_region;
    use crate::identity::{circle_id, task_id};
    use std::{vec, vec::Vec};

    const frames: usize = 16;
    const grant: u64 = 71;

    struct fake_memory {
        bytes: Vec<u8>,
        clears: usize,
        writes: usize,
        reads: usize,
        fail_clear: Option<usize>,
        fail_write: bool,
        fail_read: Option<usize>,
        corrupt_write: bool,
    }

    impl fake_memory {
        fn new() -> Self {
            Self {
                bytes: vec![0xa5; frames * frame_size as usize],
                clears: 0,
                writes: 0,
                reads: 0,
                fail_clear: None,
                fail_write: false,
                fail_read: None,
                corrupt_write: false,
            }
        }

        fn bounds(&self, range: physical_range) -> Option<core::ops::Range<usize>> {
            let start = usize::try_from(range.base).ok()?;
            let end = usize::try_from(range.end()?).ok()?;
            (start < end && end <= self.bytes.len()).then_some(start..end)
        }
    }

    // SAFETY: Isolated modeled RAM, complete range validation, exact copies and
    // verified clears. Failure injection changes only a destination prefix.
    unsafe impl source_memory for fake_memory {
        unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
            self.clears += 1;
            let Some(bounds) = self.bounds(range) else {
                return false;
            };
            if self.fail_clear == Some(self.clears) {
                let middle = bounds.start + bounds.len() / 2;
                self.bytes[bounds.start..middle].fill(0);
                return false;
            }
            self.bytes[bounds.clone()].fill(0);
            self.bytes[bounds].iter().all(|byte| *byte == 0)
        }

        unsafe fn write(&mut self, range: physical_range, source: &[u8]) -> bool {
            self.writes += 1;
            let Some(bounds) = self.bounds(range) else {
                return false;
            };
            if bounds.len() != source.len() {
                return false;
            }
            if self.fail_write {
                let count = bounds.len().div_ceil(2);
                self.bytes[bounds.start..bounds.start + count].copy_from_slice(&source[..count]);
                return false;
            }
            self.bytes[bounds.clone()].copy_from_slice(source);
            if self.corrupt_write {
                self.bytes[bounds.end - 1] ^= 1;
            }
            true
        }

        unsafe fn read(&mut self, range: physical_range, destination: &mut [u8]) -> bool {
            self.reads += 1;
            let Some(bounds) = self.bounds(range) else {
                return false;
            };
            if bounds.len() != destination.len() {
                return false;
            }
            if self.fail_read == Some(self.reads) {
                let count = bounds.len().div_ceil(2);
                destination[..count]
                    .copy_from_slice(&self.bytes[bounds.start..bounds.start + count]);
                return false;
            }
            destination.copy_from_slice(&self.bytes[bounds]);
            true
        }
    }

    fn uploader() -> task_identity {
        identity(1, 3, 2, 5)
    }

    fn identity(
        task: usize,
        task_generation: u64,
        circle: usize,
        circle_generation: u64,
    ) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(task, task_generation).unwrap(),
            circle_id::try_from_parts(circle, circle_generation).unwrap(),
        )
    }

    fn allocator(ledger: &mut [u8]) -> frame_allocator<'_> {
        frame_allocator::initialize(
            ledger,
            &[memory_region {
                base: 0,
                length: frames as u64 * frame_size,
                kind: 1,
                attributes: 1,
            }],
            &[physical_range::new(0, frame_size)],
        )
        .unwrap()
    }

    fn source(length: u64) -> owned_source {
        owned_source::new(
            uploader(),
            grant,
            length,
            source_budget {
                max_bytes: 3 * frame_size,
                max_pages: 3,
            },
        )
        .unwrap()
    }

    pub(super) fn with_sealed_reader<R>(
        bytes: &[u8],
        callback: impl for<'a> FnOnce(sealed_source_reader<'a>) -> R,
    ) -> R {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let before = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut owner = source(bytes.len() as u64);
        // SAFETY: Local isolated modeled RAM, exact sole allocator, no aliases.
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        let range = owner.retained_range().unwrap();
        let mut uploader_bytes = bytes.to_vec();
        for (index, chunk) in uploader_bytes.chunks(source_chunk_limit).enumerate() {
            // SAFETY: The local uploader copy does not alias owned modeled RAM.
            unsafe {
                owner.append(
                    uploader(),
                    grant,
                    (index * source_chunk_limit) as u64,
                    chunk,
                    &mut memory,
                )
            }
            .unwrap();
        }
        owner.seal(uploader(), grant).unwrap();
        uploader_bytes.fill(0xcc);
        let result = callback(owner.reader().unwrap());
        // A real reader still observes the original bytes after uploader mutation
        // and after the callback has moved it through the launch-plan wrapper.
        let mut observed = [0; readback_bytes];
        for (index, chunk) in bytes.chunks(readback_bytes).enumerate() {
            // SAFETY: Local sealed owner and independently live modeled RAM.
            unsafe {
                owner.reader().unwrap().read(
                    (index * readback_bytes) as u64,
                    &mut observed[..chunk.len()],
                    &mut memory,
                )
            }
            .unwrap();
            assert_eq!(&observed[..chunk.len()], chunk);
        }
        // SAFETY: The higher-ranked callback cannot return the local reader.
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert!(owner.is_released());
        assert_eq!(allocator.accounting(), before);
        assert!(
            memory.bytes[memory.bounds(range).unwrap()]
                .iter()
                .all(|byte| *byte == 0)
        );
        result
    }

    #[test]
    fn caller_budgets_nonzero_binding_and_rounding_reject_without_allocation() {
        assert!(core::mem::size_of::<owned_source>() <= 160);
        for (length, max_bytes, max_pages) in [
            (0, 4096, 1),
            (1, 0, 1),
            (1, 1, 0),
            (4097, 4097, 1),
            (4097, 4096, 2),
            (u64::MAX, u64::MAX, usize::MAX),
        ] {
            assert!(matches!(
                owned_source::new(
                    uploader(),
                    grant,
                    length,
                    source_budget {
                        max_bytes,
                        max_pages
                    }
                ),
                Err(source_error::invalid_budget)
            ));
        }
        assert!(matches!(
            owned_source::new(
                uploader(),
                0,
                1,
                source_budget {
                    max_bytes: 1,
                    max_pages: 1
                }
            ),
            Err(source_error::invalid_binding)
        ));
        let owner = source(4097);
        assert_eq!(owner.pages, 2);
        assert_eq!((owner.length(), owner.received()), (4097, 0));
        assert_eq!((owner.uploader(), owner.grant()), (Some(uploader()), grant));
        assert_eq!(owner.retained_range(), None);
    }

    #[test]
    fn contiguous_zeroed_storage_seals_exact_bytes_and_clears_page_slack_on_release() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let before = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut owner = source(4101);
        // SAFETY: Local modeled RAM and its sole allocator; no live consumers.
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        let range = owner.retained_range().unwrap();
        assert_eq!(range.length, 2 * frame_size);
        assert!(
            memory.bytes[memory.bounds(range).unwrap()]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            allocator.accounting().active_leases,
            before.active_leases + 1
        );
        assert_eq!(allocator.accounting().bound_leases, before.bound_leases + 1);
        assert_eq!(owner.reader().err(), Some(source_error::invalid_state));
        let mut first = vec![0x51; source_chunk_limit];
        unsafe { owner.append(uploader(), grant, 0, &first, &mut memory) }.unwrap();
        first.fill(0xcc);
        assert_eq!(owner.seal(uploader(), grant), Err(source_error::incomplete));
        unsafe { owner.append(uploader(), grant, 4096, b"later", &mut memory) }.unwrap();
        owner.seal(uploader(), grant).unwrap();
        let reader = owner.reader().unwrap();
        assert_eq!(
            (reader.length(), reader.uploader(), reader.grant()),
            (4101, Some(uploader()), grant)
        );
        let mut observed = [0; 8];
        unsafe { reader.read(4093, &mut observed, &mut memory) }.unwrap();
        assert_eq!(&observed, b"QQQlater");
        assert!(owner.is_sealed());
        assert_eq!(
            owner.seal(uploader(), grant),
            Err(source_error::invalid_state)
        );
        assert_eq!(
            unsafe { owner.append(uploader(), grant, 4101, b"x", &mut memory) },
            Err(source_error::invalid_state)
        );
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert!(owner.is_released());
        assert_eq!(owner.retained_range(), None);
        assert_eq!(allocator.accounting(), before);
        assert!(
            memory.bytes[memory.bounds(range).unwrap()]
                .iter()
                .all(|byte| *byte == 0)
        );
        let clears = memory.clears;
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(memory.clears, clears);
        assert_eq!(
            unsafe { owner.allocate(&mut allocator, &mut memory) },
            Err(source_error::invalid_state)
        );
    }

    #[test]
    fn foreign_stale_or_invalid_requests_never_touch_storage_or_advance() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = source(5);
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        for (caller, offered) in [
            (identity(0, 3, 2, 5), grant),
            (identity(1, 4, 2, 5), grant),
            (identity(1, 3, 3, 5), grant),
            (identity(1, 3, 2, 6), grant),
            (uploader(), 0),
            (uploader(), grant + 1),
        ] {
            assert_eq!(
                unsafe { owner.append(caller, offered, 0, b"a", &mut memory) },
                Err(source_error::invalid_binding)
            );
            assert_eq!(
                owner.seal(caller, offered),
                Err(source_error::invalid_binding)
            );
        }
        let oversized = vec![1; source_chunk_limit + 1];
        for (offset, bytes, expected) in [
            (1, &b"a"[..], source_error::invalid_offset),
            (u64::MAX, &b"a"[..], source_error::invalid_offset),
            (0, &b""[..], source_error::chunk_size),
            (0, oversized.as_slice(), source_error::chunk_size),
            (0, &b"abcdef"[..], source_error::invalid_extent),
        ] {
            assert_eq!(
                unsafe { owner.append(uploader(), grant, offset, bytes, &mut memory) },
                Err(expected)
            );
        }
        assert_eq!((memory.writes, memory.reads, owner.received()), (0, 0, 0));
        unsafe { owner.append(uploader(), grant, 0, b"ab", &mut memory) }.unwrap();
        let before = (memory.writes, memory.reads, memory.bytes.clone());
        assert_eq!(
            unsafe { owner.append(uploader(), grant, 0, b"ab", &mut memory) },
            Err(source_error::invalid_offset)
        );
        assert_eq!(
            (memory.writes, memory.reads, &memory.bytes),
            (before.0, before.1, &before.2)
        );
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    }

    #[test]
    fn partial_write_readback_failure_and_corruption_remain_unaccepted_until_retry() {
        for failure in 0..3 {
            let mut ledger = [0; frames / 4];
            let mut allocator = allocator(&mut ledger);
            let before = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut owner = source(132);
            unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
            unsafe { owner.append(uploader(), grant, 0, b"old", &mut memory) }.unwrap();
            memory.fail_write = failure == 0;
            memory.fail_read = (failure == 1).then_some(memory.reads + 2);
            memory.corrupt_write = failure == 2;
            let expected = [
                source_error::write_failed,
                source_error::read_failed,
                source_error::readback_mismatch,
            ][failure];
            let body = [0x69; 129];
            assert_eq!(
                unsafe { owner.append(uploader(), grant, 3, &body, &mut memory) },
                Err(expected)
            );
            assert_eq!(owner.received(), 3);
            let range = owner.retained_range().unwrap();
            assert_eq!(
                &memory.bytes[range.base as usize..range.base as usize + 3],
                b"old"
            );
            assert_eq!(owner.seal(uploader(), grant), Err(source_error::incomplete));
            assert_eq!(owner.reader().err(), Some(source_error::invalid_state));
            memory.fail_write = false;
            memory.fail_read = None;
            memory.corrupt_write = false;
            unsafe { owner.append(uploader(), grant, 3, &body, &mut memory) }.unwrap();
            owner.seal(uploader(), grant).unwrap();
            let mut observed = [0; 132];
            unsafe { owner.reader().unwrap().read(0, &mut observed, &mut memory) }.unwrap();
            assert_eq!(&observed[..3], b"old");
            assert_eq!(&observed[3..], &body);
            unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
            assert_eq!(allocator.accounting(), before);
        }
    }

    #[test]
    fn initialization_failure_and_oom_acquire_no_lease_and_can_be_retried_or_cancelled() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let before = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut owner = source(100);
        memory.fail_clear = Some(1);
        assert_eq!(
            unsafe { owner.allocate(&mut allocator, &mut memory) },
            Err(source_error::frame(frame_error::clearing_failed))
        );
        assert_eq!(allocator.accounting(), before);
        assert_eq!(owner.retained_range(), None);
        assert_eq!(
            unsafe { owner.append(uploader(), grant, 0, b"x", &mut memory) },
            Err(source_error::invalid_state)
        );
        memory.fail_clear = None;
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(
            unsafe { owner.allocate(&mut allocator, &mut memory) },
            Err(source_error::invalid_state)
        );
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        let mut huge = owned_source::new(
            uploader(),
            grant,
            frames as u64 * frame_size,
            source_budget {
                max_bytes: frames as u64 * frame_size,
                max_pages: frames,
            },
        )
        .unwrap();
        let clears = memory.clears;
        assert_eq!(
            unsafe { huge.allocate(&mut allocator, &mut memory) },
            Err(source_error::frame(frame_error::out_of_memory))
        );
        assert_eq!(memory.clears, clears);
        assert_eq!(allocator.accounting(), before);
        unsafe { huge.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert!(huge.is_released());
    }

    #[test]
    fn failed_clear_revokes_read_and_write_while_retaining_exact_lease_for_retry() {
        for sealed in [false, true] {
            let mut ledger = [0; frames / 4];
            let mut allocator = allocator(&mut ledger);
            let before = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut owner = source(4097);
            unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
            unsafe { owner.append(uploader(), grant, 0, &[0x33; 4096], &mut memory) }.unwrap();
            if sealed {
                unsafe { owner.append(uploader(), grant, 4096, b"z", &mut memory) }.unwrap();
                owner.seal(uploader(), grant).unwrap();
            }
            let range = owner.retained_range().unwrap();
            memory.fail_clear = Some(memory.clears + 1);
            assert_eq!(
                unsafe { owner.cleanup(&mut allocator, &mut memory) },
                Err(source_error::frame(frame_error::clearing_failed))
            );
            assert_eq!(owner.retained_range(), Some(range));
            assert!(!owner.is_released());
            assert!(!owner.is_sealed());
            assert_eq!(
                allocator.accounting().active_leases,
                before.active_leases + 1
            );
            assert_eq!(allocator.accounting().bound_leases, before.bound_leases);
            let io = (memory.writes, memory.reads);
            assert_eq!(owner.reader().err(), Some(source_error::invalid_state));
            assert_eq!(
                owner.seal(uploader(), grant),
                Err(source_error::invalid_state)
            );
            assert_eq!(
                unsafe { owner.append(uploader(), grant, owner.received(), b"x", &mut memory) },
                Err(source_error::invalid_state)
            );
            assert_eq!((memory.writes, memory.reads), io);
            memory.fail_clear = None;
            unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
            assert_eq!(allocator.accounting(), before);
            assert!(
                memory.bytes[memory.bounds(range).unwrap()]
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
    }

    #[test]
    fn sealed_read_bounds_reject_before_output_or_backend_access() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = source(3);
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        unsafe { owner.append(uploader(), grant, 0, b"abc", &mut memory) }.unwrap();
        owner.seal(uploader(), grant).unwrap();
        let reader = owner.reader().unwrap();
        for (offset, length, expected) in [
            (0, 0, source_error::chunk_size),
            (0, 4097, source_error::chunk_size),
            (3, 1, source_error::invalid_extent),
            (u64::MAX, 1, source_error::invalid_extent),
            (1, 3, source_error::invalid_extent),
        ] {
            let mut output = vec![0x77; length];
            let reads = memory.reads;
            assert_eq!(
                unsafe { reader.read(offset, &mut output, &mut memory) },
                Err(expected)
            );
            assert!(output.iter().all(|byte| *byte == 0x77));
            assert_eq!(memory.reads, reads);
        }
        memory.fail_read = Some(memory.reads + 1);
        assert_eq!(
            unsafe { reader.read(0, &mut [0; 3], &mut memory) },
            Err(source_error::read_failed)
        );
        memory.fail_read = None;
        let mut output = [0; 1];
        unsafe { reader.read(2, &mut output, &mut memory) }.unwrap();
        assert_eq!(output, [b'c']);
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
    }

    #[test]
    fn wrong_allocator_cannot_release_source_and_retired_grant_cannot_reopen_it() {
        let mut ledger = [0; frames / 4];
        let mut foreign_ledger = [0; frames / 4];
        let mut foreign = allocator(&mut foreign_ledger);
        let mut allocator = allocator(&mut ledger);
        let mut memory = fake_memory::new();
        let mut owner = source(1);
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        let range = owner.retained_range();
        let clears = memory.clears;
        assert_eq!(
            unsafe { owner.cleanup(&mut foreign, &mut memory) },
            Err(source_error::frame(frame_error::foreign_lease))
        );
        assert_eq!(owner.retained_range(), range);
        assert_eq!(memory.clears, clears);
        unsafe { owner.append(uploader(), grant, 0, b"x", &mut memory) }.unwrap();
        owner.seal(uploader(), grant).unwrap();
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(owner.reader().err(), Some(source_error::invalid_state));
        assert_eq!(
            owner.seal(uploader(), grant),
            Err(source_error::invalid_state)
        );
        assert_eq!(
            unsafe { owner.append(uploader(), grant, 0, b"x", &mut memory) },
            Err(source_error::invalid_state)
        );
    }

    fn provisioned(length: u64) -> owned_source {
        // SAFETY: This isolated modeled test is the explicit provisioning
        // controller. It does not fabricate a task or expose a runtime grant.
        unsafe {
            owned_source::new_provisioned(
                grant,
                length,
                source_budget {
                    max_bytes: 3 * frame_size,
                    max_pages: 3,
                },
            )
        }
        .unwrap()
    }

    #[test]
    fn provisioned_and_task_origins_cannot_exchange_append_or_seal_authority() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let before = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut task = source(3);
        let mut boot = provisioned(3);
        unsafe { task.allocate(&mut allocator, &mut memory) }.unwrap();
        unsafe { boot.allocate(&mut allocator, &mut memory) }.unwrap();
        let io = (memory.writes, memory.reads);
        assert_eq!(boot.uploader(), None);
        assert_eq!(task.origin(), source_origin::task(uploader()));
        assert_eq!(
            boot.origin(),
            source_origin::provisioned(core::num::NonZeroU64::new(grant).unwrap())
        );
        assert_eq!(
            boot.validate_append(uploader(), grant, 0, 3),
            Err(source_error::invalid_binding)
        );
        assert_eq!(
            unsafe { boot.append(uploader(), grant, 0, b"bad", &mut memory) },
            Err(source_error::invalid_binding)
        );
        assert_eq!(
            boot.seal(uploader(), grant),
            Err(source_error::invalid_binding)
        );
        assert_eq!(
            unsafe { task.append_provisioned(grant, 0, b"bad", &mut memory) },
            Err(source_error::invalid_binding)
        );
        assert_eq!(
            unsafe { task.seal_provisioned(grant) },
            Err(source_error::invalid_binding)
        );
        for wrong in [0, grant + 1] {
            assert_eq!(
                unsafe { boot.append_provisioned(wrong, 0, b"bad", &mut memory) },
                Err(source_error::invalid_binding)
            );
            assert_eq!(
                unsafe { boot.seal_provisioned(wrong) },
                Err(source_error::invalid_binding)
            );
        }
        assert_eq!((memory.writes, memory.reads), io);
        assert_eq!((task.received(), boot.received()), (0, 0));
        unsafe { boot.append_provisioned(grant, 0, b"elf", &mut memory) }.unwrap();
        unsafe { boot.seal_provisioned(grant) }.unwrap();
        let reader = boot.reader().unwrap();
        assert_eq!(reader.origin(), boot.origin());
        assert_eq!(reader.uploader(), None);
        let mut output = [0; 3];
        unsafe { reader.read(0, &mut output, &mut memory) }.unwrap();
        assert_eq!(&output, b"elf");
        unsafe { boot.cleanup(&mut allocator, &mut memory) }.unwrap();
        unsafe { task.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), before);
    }

    #[test]
    fn provisioned_partial_write_and_failed_clear_keep_lease_and_retry_progress() {
        let mut ledger = [0; frames / 4];
        let mut allocator = allocator(&mut ledger);
        let before = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut owner = provisioned(4101);
        unsafe { owner.allocate(&mut allocator, &mut memory) }.unwrap();
        let range = owner.retained_range();
        memory.fail_write = true;
        assert_eq!(
            unsafe { owner.append_provisioned(grant, 0, &[0x51; 4096], &mut memory) },
            Err(source_error::write_failed)
        );
        assert_eq!(owner.received(), 0);
        assert_eq!(
            unsafe { owner.seal_provisioned(grant) },
            Err(source_error::incomplete)
        );
        memory.fail_write = false;
        unsafe { owner.append_provisioned(grant, 0, &[0x52; 4096], &mut memory) }.unwrap();
        assert_eq!(
            unsafe { owner.append_provisioned(grant, 0, b"replay", &mut memory) },
            Err(source_error::invalid_offset)
        );
        unsafe { owner.append_provisioned(grant, 4096, b"later", &mut memory) }.unwrap();
        unsafe { owner.seal_provisioned(grant) }.unwrap();
        memory.fail_clear = Some(memory.clears + 1);
        assert_eq!(
            unsafe { owner.cleanup(&mut allocator, &mut memory) },
            Err(source_error::frame(frame_error::clearing_failed))
        );
        assert_eq!(owner.retained_range(), range);
        assert_eq!(owner.reader().err(), Some(source_error::invalid_state));
        assert_eq!(
            unsafe { owner.seal_provisioned(grant) },
            Err(source_error::invalid_state)
        );
        assert_eq!(
            allocator.accounting().active_leases,
            before.active_leases + 1
        );
        assert_eq!(allocator.accounting().bound_leases, before.bound_leases);
        memory.fail_clear = None;
        unsafe { owner.cleanup(&mut allocator, &mut memory) }.unwrap();
        assert_eq!(allocator.accounting(), before);
        assert!(
            memory.bytes[memory.bounds(range.unwrap()).unwrap()]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn provisioned_constructor_rejects_zero_grant_and_overflowing_or_exceeded_budgets() {
        let budget = source_budget {
            max_bytes: 4096,
            max_pages: 1,
        };
        assert!(matches!(
            unsafe { owned_source::new_provisioned(0, 1, budget) },
            Err(source_error::invalid_binding)
        ));
        for length in [0, 4097, u64::MAX] {
            assert!(matches!(
                unsafe { owned_source::new_provisioned(grant, length, budget) },
                Err(source_error::invalid_budget)
            ));
        }
    }
}
