#![cfg_attr(not(test), allow(dead_code))]

//! Detached runtime mapping ownership, without a live address-space adapter.
//!
//! Complete virtual ownership and initialized physical backing are different
//! extents. This owner retains both, but neither its geometry nor a retained
//! backing value authenticates an operation or authorizes physical access.
//! Publication and withdrawal require a separate, explicitly unsafe adapter.

use super::mapping_plan::{
    mapping_budget, mapping_plan, mapping_request, plan_error, reserved_request,
    table_frame_capacity, user_address_limit,
};
use super::owned_mapping::{mapping_error, mapping_memory, staged_mapping};
use super::paging::{page_permissions, page_size};
use crate::memory::physical::frame_allocator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct runtime_request {
    pub virtual_address: u64,
    /// Requested extent before page rounding.
    pub byte_length: u64,
    /// Initialized prefix within the complete rounded extent. This may exceed
    /// byte_length when the intended source covers the rest of its last page.
    pub populated_byte_length: u64,
    pub permissions: page_permissions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct runtime_budget {
    pub virtual_pages: u64,
    pub data_pages: u64,
    pub table_frames: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum runtime_plan_error {
    invalid_extent,
    invalid_prefix,
    invalid_budget,
    virtual_budget_exhausted,
    existing_mapping_collision,
    existing_reservation_collision,
    mapping(plan_error),
}

/// Immutable geometry only. These addresses are not ownership or access grants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct runtime_geometry {
    request: runtime_request,
    end: u64,
    populated_end: u64,
}

impl runtime_geometry {
    pub(super) const fn start(self) -> u64 {
        self.request.virtual_address
    }

    /// End of complete virtual ownership, rounded to a page boundary.
    pub(super) const fn end(self) -> u64 {
        self.end
    }

    /// End of physical backing, rounded to a page boundary.
    pub(super) const fn populated_end(self) -> u64 {
        self.populated_end
    }

    pub(super) const fn byte_length(self) -> u64 {
        self.request.byte_length
    }

    pub(super) const fn populated_byte_length(self) -> u64 {
        self.request.populated_byte_length
    }

    pub(super) const fn page_count(self) -> u64 {
        (self.end - self.start()) / page_size
    }

    pub(super) const fn populated_page_count(self) -> u64 {
        (self.populated_end - self.start()) / page_size
    }

    pub(super) const fn permissions(self) -> page_permissions {
        self.request.permissions
    }
}

/// Checked construction-time geometry, not a reservation in a live space.
/// The isolated plan contains only the new prefix and its own absent tail.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct runtime_plan {
    geometry: runtime_geometry,
    mapping: mapping_plan,
}

impl runtime_plan {
    /// Reject collisions over the complete rounded extent, including the
    /// unpopulated tail. Existing reservations are exclusions in their entirety.
    /// No existing owner, plan, allocator or address space is changed here.
    ///
    /// An empty populated prefix is currently unsupported by this owner; this
    /// restriction is not a general virtual-memory or file-format policy.
    pub(super) fn new(
        existing: &mapping_plan,
        request: runtime_request,
        budget: runtime_budget,
    ) -> Result<Self, runtime_plan_error> {
        if budget.virtual_pages == 0
            || budget.data_pages == 0
            || !(1..=table_frame_capacity).contains(&budget.table_frames)
        {
            return Err(runtime_plan_error::invalid_budget);
        }
        let start = request.virtual_address;
        if start == 0 || !start.is_multiple_of(page_size) || request.byte_length == 0 {
            return Err(runtime_plan_error::invalid_extent);
        }
        let rounded_length =
            rounded_bytes(request.byte_length).ok_or(runtime_plan_error::invalid_extent)?;
        let end = start
            .checked_add(rounded_length)
            .filter(|end| *end <= user_address_limit)
            .ok_or(runtime_plan_error::invalid_extent)?;
        if request.populated_byte_length == 0 || request.populated_byte_length > rounded_length {
            return Err(runtime_plan_error::invalid_prefix);
        }
        let rounded_prefix = rounded_bytes(request.populated_byte_length)
            .ok_or(runtime_plan_error::invalid_prefix)?;
        let populated_end = start
            .checked_add(rounded_prefix)
            .filter(|populated_end| *populated_end <= end)
            .ok_or(runtime_plan_error::invalid_prefix)?;
        let geometry = runtime_geometry {
            request,
            end,
            populated_end,
        };
        if geometry.page_count() > budget.virtual_pages {
            return Err(runtime_plan_error::virtual_budget_exhausted);
        }
        if existing
            .regions()
            .any(|region| start < region.extent().end() && region.extent().start() < end)
        {
            return Err(runtime_plan_error::existing_mapping_collision);
        }
        if existing
            .reservations()
            .any(|reservation| start < reservation.end() && reservation.start() < end)
        {
            return Err(runtime_plan_error::existing_reservation_collision);
        }
        let prefix = mapping_request {
            virtual_address: start,
            page_count: geometry.populated_page_count(),
            permissions: request.permissions,
        };
        // This reservation describes only absent pages. Full virtual ownership
        // remains in geometry and must be admitted by the publication adapter.
        let tail = [reserved_request {
            virtual_address: populated_end,
            page_count: (end - populated_end) / page_size,
        }];
        let reservations = if populated_end < end { &tail[..] } else { &[] };
        let mapping = mapping_plan::new(
            &[prefix],
            reservations,
            mapping_budget {
                data_pages: budget.data_pages,
                table_frames: budget.table_frames,
            },
        )
        .map_err(runtime_plan_error::mapping)?;
        Ok(Self { geometry, mapping })
    }

    pub(super) const fn geometry(&self) -> runtime_geometry {
        self.geometry
    }

    pub(super) fn mapping(&self) -> &mapping_plan {
        &self.mapping
    }
}

fn rounded_bytes(length: u64) -> Option<u64> {
    length
        .checked_add(page_size - 1)
        .map(|length| length / page_size * page_size)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum runtime_phase {
    empty,
    preparing,
    ready,
    uncertain,
    published,
    withdrawn,
    cleaning,
    released,
}

/// Outcomes are assertions made by the unsafe adapter, not physical evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum publication_result {
    rejected,
    published,
    unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum runtime_error {
    invalid_phase,
    mapping(mapping_error),
    population_failed,
    publication_rejected,
    publication_unknown,
}

/// Retains actual private table/data leases and a backing value by ownership.
///
/// An owned source can move with this owner without borrowing itself. Callbacks
/// borrow it only for their synchronous operation; they cannot retain a reader
/// into it. A reference is also a valid backing value and retains its ordinary
/// borrow lifetime. A generic backing may contain interior mutability or just
/// an identifier; ownership alone proves neither immutable bytes nor authority.
/// Every adapter must independently authenticate and retain the actual source.
///
/// There is no Drop cleanup. Retain this owner after every failure or unwind
/// until explicit cleanup succeeds. Published or uncertain owners also retain
/// every lease until complete external withdrawal is explicitly confirmed.
#[must_use = "retain the runtime mapping and backing until verified cleanup succeeds"]
pub(super) struct runtime_mapping<backing_type> {
    geometry: runtime_geometry,
    mapping: staged_mapping,
    backing: backing_type,
    phase: runtime_phase,
}

impl<backing_type> runtime_mapping<backing_type> {
    pub(super) fn new(plan: runtime_plan, backing: backing_type) -> Self {
        Self {
            geometry: plan.geometry,
            mapping: staged_mapping::new(plan.mapping),
            backing,
            phase: runtime_phase::empty,
        }
    }

    pub(super) const fn phase(&self) -> runtime_phase {
        self.phase
    }

    pub(super) const fn geometry(&self) -> runtime_geometry {
        self.geometry
    }

    pub(super) fn mapping(&self) -> &staged_mapping {
        &self.mapping
    }

    pub(super) fn backing(&self) -> &backing_type {
        &self.backing
    }

    /// Recover the backing only after verified mapping cleanup has completed.
    /// Refusal returns the entire unchanged owner, including every outstanding
    /// lease and publication obligation. The caller must retain that owner.
    /// This does not clean up the backing; its own explicit lifecycle remains
    /// the caller's responsibility after recovery.
    #[allow(clippy::result_large_err)] // A failed recovery must retain ownership without allocation.
    pub(super) fn into_backing(self) -> Result<backing_type, Self> {
        if self.phase != runtime_phase::released
            || self.mapping.is_ready()
            || self.mapping.retained_leases() != 0
        {
            return Err(self);
        }
        Ok(self.backing)
    }

    /// Build private tables and initialize/read back all intended backing bytes.
    /// Failed preparation is cleanup-only; it cannot be retried or published.
    ///
    /// # Safety
    /// The allocator describes actual exclusive RAM accessible through memory's
    /// independently live privileged mapping. No root, allocator, access owner
    /// or retained storage may be concurrently changed. The backing owner must
    /// retain the intended immutable source, independently authenticated by the
    /// caller. The synchronous callback must copy and independently read back
    /// every intended byte in populated_byte_length, then verify zeros through
    /// the final backed page. It may write only retained data leases, must not
    /// modify tables or unrelated storage, publish references, activate roots,
    /// or retain access pointers. True asserts this entire contract. The caller
    /// must retain this owner after failure or unwind for explicit cleanup.
    pub(super) unsafe fn prepare<memory_type: mapping_memory>(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut memory_type,
        populate: impl FnOnce(&staged_mapping, &backing_type, &mut memory_type) -> bool,
    ) -> Result<(), runtime_error> {
        if self.phase != runtime_phase::empty
            || self.mapping.is_ready()
            || self.mapping.retained_leases() != 0
        {
            return Err(runtime_error::invalid_phase);
        }
        self.phase = runtime_phase::preparing;
        // SAFETY: The caller supplies exclusive leases and independent access;
        // the owner stays retained before every fallible build operation.
        unsafe { self.mapping.build(allocator, memory) }.map_err(runtime_error::mapping)?;
        if !populate(&self.mapping, &self.backing, memory) {
            return Err(runtime_error::population_failed);
        }
        // SAFETY: The callback preserves inactive exclusive ownership. Verify
        // the exact tree again before exposing a publication-ready state.
        unsafe { self.mapping.validate(allocator, memory) }.map_err(runtime_error::mapping)?;
        self.phase = runtime_phase::ready;
        Ok(())
    }

    /// Ask a separate adapter to publish this prepared owner synchronously.
    /// A known rejection preserves readiness; uncertainty forbids replay.
    ///
    /// # Safety
    /// The adapter must authenticate the target and backing, retain exclusive
    /// ownership of the entire virtual extent, and revalidate unchanged live
    /// address-space/collision state before publication. The construction-time
    /// existing plan is only an inert witness and may otherwise have gone stale.
    /// It must independently validate residency, permissions, caller state,
    /// executable admission when applicable, and every table/reference it uses.
    /// It must not mutate the prepared private tree or payload while publishing.
    /// Published certifies the adapter's complete publication contract. Rejected
    /// certifies no external reference, consumer or cached translation remains
    /// and the exact prepared candidate is unchanged, so a retry is safe. Any
    /// ambiguous result must be unknown. The owner and actual immutable backing
    /// must outlive all external consumers; callback borrows and pointers may
    /// not outlive those retained owners. On unwind the caller must keep this
    /// owner and treat publication as uncertain until confirmed withdrawal.
    pub(super) unsafe fn publish(
        &mut self,
        publish: impl FnOnce(&staged_mapping, &backing_type, runtime_geometry) -> publication_result,
    ) -> Result<(), runtime_error> {
        if self.phase != runtime_phase::ready || !self.mapping.is_ready() {
            return Err(runtime_error::invalid_phase);
        }
        // Set this before invoking external code: unwinding cannot make an
        // escaped reference look like an unpublished, freely cleanable owner.
        self.phase = runtime_phase::uncertain;
        match publish(&self.mapping, &self.backing, self.geometry) {
            publication_result::rejected => {
                self.phase = runtime_phase::ready;
                Err(runtime_error::publication_rejected)
            }
            publication_result::published => {
                self.phase = runtime_phase::published;
                Ok(())
            }
            publication_result::unknown => Err(runtime_error::publication_unknown),
        }
    }

    /// Confirm complete external withdrawal; only cleanup is permitted next.
    ///
    /// # Safety
    /// All consumers, saved continuations and hardware walkers that could reach
    /// owned tables/data are quiescent. Every external parent entry and other
    /// reference has been removed, and all required translation invalidations
    /// have completed. No consumer can resume with stale access. This applies
    /// even if publication was uncertain. The isolated staging root being
    /// inactive does not establish withdrawal: another root may reference its
    /// children. The actual immutable backing must remain retained until every
    /// such consumer is withdrawn. No new reference may be published afterward.
    pub(super) unsafe fn confirm_withdrawal(&mut self) -> Result<(), runtime_error> {
        if !matches!(
            self.phase,
            runtime_phase::published | runtime_phase::uncertain
        ) {
            return Err(runtime_error::invalid_phase);
        }
        self.phase = runtime_phase::withdrawn;
        Ok(())
    }

    /// Explicit, retryable release of private ownership. No implicit rollback.
    ///
    /// # Safety
    /// The allocator and independently live privileged access still describe
    /// these exact retained leases. No external software/hardware consumer or
    /// translation may reach them, including through another root. Publication
    /// withdrawal, if needed, has met confirm_withdrawal's complete contract.
    /// Exclude all concurrent owner/allocator/access changes until completion;
    /// keep this owner after every failure or unwind and retry cleanup only.
    pub(super) unsafe fn cleanup(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl mapping_memory,
    ) -> Result<(), runtime_error> {
        match self.phase {
            runtime_phase::published | runtime_phase::uncertain => {
                return Err(runtime_error::invalid_phase);
            }
            runtime_phase::released => return Ok(()),
            runtime_phase::empty
            | runtime_phase::preparing
            | runtime_phase::ready
            | runtime_phase::withdrawn
            | runtime_phase::cleaning => {}
        }
        self.phase = runtime_phase::cleaning;
        // SAFETY: The phase fence and caller's external-withdrawal/access
        // contracts cover more than staged_mapping's own inactive-root check.
        unsafe { self.mapping.cleanup(allocator, memory) }.map_err(runtime_error::mapping)?;
        self.phase = runtime_phase::released;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
