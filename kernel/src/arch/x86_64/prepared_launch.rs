#![cfg_attr(not(test), allow(dead_code))]

//! Retained preparation for explicitly selected bounded user profiles.
//!
//! Preparation owns initialized mappings and circle attachments but publishes
//! no task. Explicit commit creates the ready task as its final operation.
//! Neither operation activates a root, attaches a scheduler context, or supplies
//! execution authority from artifact metadata. This is not a general loader.

use super::owned_mapping::{mapping_error, mapping_memory, staged_mapping};
use super::paging::page_size;
use super::user_entry_plan::{user_entry_error, user_entry_plan};
use crate::circle::{circle_child_kind, circle_error, circle_table};
use crate::identity::{circle_id, fixture_principal_id, task_identity};
use crate::memory::physical::frame_allocator;
use crate::task::{synthetic_origin_metadata, task_binding, task_error, task_state, task_table};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum launch_error {
    invalid_state,
    invalid_profile,
    entry(user_entry_error),
    mapping(mapping_error),
    population_failed,
    circle(circle_error),
    task(task_error),
    attachment_mismatch,
    poisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum launch_phase {
    empty,
    preparing,
    prepared,
    committed,
    cancelling,
    released,
    poisoned,
}

#[cfg(any(test, feature = "owned_multi_region_test"))]
struct multi_region_profile {
    mapping_count: u16,
    stack_region: usize,
}

/// No fallible Drop: retain this value and its allocator/access/lifecycle tables
/// after any failure until explicit cancellation succeeds or the proof halts.
#[must_use = "retain launch ownership until cancellation or terminal cleanup succeeds"]
pub(super) struct prepared_launch {
    mapping: staged_mapping,
    admission: user_entry_plan,
    expected_code_pages: u64,
    #[cfg(any(test, feature = "mixed_workspace_test"))]
    mixed_workspace_profile: bool,
    #[cfg(any(test, feature = "owned_workspace_test"))]
    workspace_profile: bool,
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test"
    ))]
    arena_base: Option<u64>,
    /// The geometry this launch was admitted under, retained so the copy,
    /// cancel and cleanup paths revalidate the same profile the controller
    /// chose rather than a size someone passed in (ADR 0094).
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test"
    ))]
    arena_kind: Option<super::arena_profile::arena_profile>,
    circle: Option<circle_id>,
    task: Option<task_identity>,
    address_space_attached: bool,
    mappings_attached: bool,
    phase: launch_phase,
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    multi_region: Option<multi_region_profile>,
}

impl prepared_launch {
    /// Take ownership without granting readiness or discarding anything on an
    /// error. `prepare` rechecks the supplied admission against this exact plan.
    pub(super) fn new(mapping: staged_mapping, admission: user_entry_plan) -> Self {
        Self::new_with_code_pages(mapping, admission, 1)
    }

    /// Explicit trusted selection of the one-page proof or eight-page native
    /// application profile. Artifact metadata cannot select this policy;
    /// `prepare` rejects unsupported counts before acquiring any resources.
    pub(super) fn new_with_code_pages(
        mapping: staged_mapping,
        admission: user_entry_plan,
        code_pages: u64,
    ) -> Self {
        Self {
            mapping,
            admission,
            expected_code_pages: code_pages,
            #[cfg(any(test, feature = "mixed_workspace_test"))]
            mixed_workspace_profile: false,
            #[cfg(any(test, feature = "owned_workspace_test"))]
            workspace_profile: false,
            #[cfg(any(
                test,
                feature = "native_runtime_test",
                feature = "mixed_workspace_test"
            ))]
            arena_base: None,
            #[cfg(any(
                test,
                feature = "native_runtime_test",
                feature = "mixed_workspace_test"
            ))]
            arena_kind: None,
            circle: None,
            task: None,
            address_space_attached: false,
            mappings_attached: false,
            phase: launch_phase::empty,
            #[cfg(any(test, feature = "owned_multi_region_test"))]
            multi_region: None,
        }
    }

    /// Trusted opt-in selection, independent of every artifact field. The
    /// exact arena geometry and guards are rechecked before allocation.
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test"
    ))]
    pub(super) fn new_with_native_arena(
        mapping: staged_mapping,
        admission: user_entry_plan,
        arena_base: u64,
        profile: super::arena_profile::arena_profile,
    ) -> Self {
        let mut launch = Self::new_with_code_pages(mapping, admission, profile.code_pages());
        launch.arena_base = Some(arena_base);
        launch.arena_kind = Some(profile);
        launch
    }

    /// Explicit controller selection of the low-linked workspace role. This
    /// cannot be selected by executable metadata or the general page-count API.
    #[cfg(any(test, feature = "owned_workspace_test"))]
    pub(super) fn new_workspace(mapping: staged_mapping, admission: user_entry_plan) -> Self {
        let mut launch = Self::new_with_code_pages(mapping, admission, 32);
        launch.workspace_profile = true;
        launch
    }

    #[cfg(any(test, feature = "owned_workspace_test"))]
    pub(super) const fn is_workspace_profile(&self) -> bool {
        self.workspace_profile
    }

    /// Fixed workspace role for the three-owner admission proof. Artifact bytes
    /// cannot select this policy or the private frame arena.
    #[cfg(any(test, feature = "mixed_workspace_test"))]
    pub(super) fn new_mixed_workspace(mapping: staged_mapping, admission: user_entry_plan) -> Self {
        let mut launch = Self::new_with_code_pages(mapping, admission, 32);
        launch.mixed_workspace_profile = true;
        launch
    }

    /// Controller-selected nine-region ownership, independent of executable
    /// formats. The supplied admission and exact stack index are revalidated
    /// against this retained mapping before any physical allocation.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(super) fn new_multi_region(
        mapping: staged_mapping,
        admission: user_entry_plan,
        stack_region: usize,
    ) -> Self {
        let mapping_count = mapping.plan().regions().count() as u16;
        let mut launch = Self::new_with_code_pages(mapping, admission, 1);
        launch.multi_region = Some(multi_region_profile {
            mapping_count,
            stack_region,
        });
        launch
    }

    #[cfg(feature = "owned_source_test")]
    pub(super) fn has_multi_region_profile(&self) -> bool {
        self.multi_region.is_some()
    }

    pub(super) fn stack_region(&self) -> usize {
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if let Some(profile) = &self.multi_region {
            return profile.stack_region;
        }
        usize::from(self.mapping_count()) - 1
    }

    fn mapping_count(&self) -> u16 {
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if let Some(profile) = &self.multi_region {
            return profile.mapping_count;
        }
        #[cfg(any(test, feature = "mixed_workspace_test"))]
        if self.mixed_workspace_profile {
            return 4;
        }
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        if self.arena_base.is_some() {
            return 4;
        }
        3
    }

    /// Arena pages this launch was admitted for.
    ///
    /// Eight where no arena profile is compiled in, which is the native
    /// application's count and the only one such a build can produce.
    fn arena_pages(&self) -> u64 {
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        {
            self.arena_kind.map_or(8, |profile| profile.arena_pages())
        }
        #[cfg(not(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        )))]
        {
            8
        }
    }

    fn stack_pages(&self) -> u64 {
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        if let Some(profile) = self.arena_kind {
            return profile.stack_pages();
        }
        2
    }

    /// Geometry is informative, not a transferable access grant. Only a fully
    /// prepared or committed owner can report an admitted native arena.
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test"
    ))]
    pub(super) fn arena_extent(&self) -> Option<(u64, u64)> {
        if !(self.is_prepared() || self.is_committed()) || !self.mapping.is_ready() {
            return None;
        }
        #[cfg(any(test, feature = "mixed_workspace_test"))]
        if self.mixed_workspace_profile {
            return Some((0x810000, 9 * page_size));
        }
        // The length comes from the retained profile rather than a literal,
        // so a 64 KiB program arena is not reported as a 32 KiB one.
        match (self.arena_base, self.arena_kind) {
            (Some(base), Some(profile)) => Some((base, profile.arena_bytes())),
            _ => None,
        }
    }

    pub(super) fn mapping(&self) -> &staged_mapping {
        &self.mapping
    }

    pub(super) const fn admission(&self) -> user_entry_plan {
        self.admission
    }

    /// Inspection only; the circle number is not a grant to mutate its children.
    pub(super) const fn circle(&self) -> Option<circle_id> {
        self.circle
    }

    pub(super) const fn task(&self) -> Option<task_identity> {
        self.task
    }

    pub(super) fn is_prepared(&self) -> bool {
        self.phase == launch_phase::prepared
    }

    pub(super) fn is_committed(&self) -> bool {
        self.phase == launch_phase::committed
    }

    pub(super) fn is_released(&self) -> bool {
        self.phase == launch_phase::released
    }

    /// Borrow the mapping for the separate retained activation session.
    ///
    /// # Safety
    /// The caller must retain complete privileged residency and access, borrow
    /// this mapping through every active root/continuation, and restore and
    /// withdraw them before ending that borrow. It may validate or activate the
    /// mapping but must not replace, rebuild, clean, or change its payload.
    pub(super) unsafe fn committed_mapping_mut(
        &mut self,
    ) -> Result<&mut staged_mapping, launch_error> {
        if self.phase != launch_phase::committed {
            return Err(launch_error::invalid_state);
        }
        Ok(&mut self.mapping)
    }

    /// Build, populate/read back, and attach the retained circle resources.
    /// Errors retain every acquired resource. A failed preparation must be
    /// cancelled; it cannot be resumed with a different population callback.
    ///
    /// # Safety
    /// The staged-mapping build contract holds, with exclusive allocator,
    /// lifecycle tables, and independently live privileged access. `populate`
    /// must copy and independently verify every declared initialized code byte
    /// and every intended data byte, writing only retained data leases. It must
    /// not modify tables, activate roots, or retain pointers. Returning true
    /// asserts that complete contract, not merely that the callback ran.
    pub(super) unsafe fn prepare<M: mapping_memory>(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut M,
        circles: &mut circle_table,
        populate: impl FnOnce(&staged_mapping, &mut M) -> bool,
    ) -> Result<(), launch_error> {
        if self.phase != launch_phase::empty
            || self.mapping.is_ready()
            || self.mapping.retained_leases() != 0
        {
            return Err(launch_error::invalid_state);
        }
        self.phase = launch_phase::preparing;
        self.validate_profile()?;
        // SAFETY: The caller supplies exclusive RAM, inactive-root ownership,
        // and a privileged access mapping that survives all retained leases.
        unsafe { self.mapping.build(allocator, memory) }.map_err(launch_error::mapping)?;
        if !populate(&self.mapping, memory) {
            return Err(launch_error::population_failed);
        }
        // A callback must not mutate tables; revalidation also catches an
        // accidental violation before circle attachment or task publication.
        // SAFETY: The same inactive ownership/access contract remains in force.
        unsafe { self.mapping.validate(allocator, memory) }.map_err(launch_error::mapping)?;
        let circle = circles.create().map_err(launch_error::circle)?;
        self.circle = Some(circle);
        circles
            .attach_child(circle, circle_child_kind::address_space)
            .map_err(launch_error::circle)?;
        self.address_space_attached = true;
        circles
            .attach_children(circle, circle_child_kind::mapping, self.mapping_count())
            .map_err(launch_error::circle)?;
        self.mappings_attached = true;
        self.phase = launch_phase::prepared;
        Ok(())
    }

    /// Publish the first task only after complete preparation. Principal/origin
    /// are trusted fixture metadata supplied separately from the inert plan;
    /// they do not authenticate anyone or constitute a general authority API.
    /// A create failure leaves preparation intact for a retry or cancellation.
    pub(super) fn commit(
        &mut self,
        tasks: &mut task_table,
        circles: &mut circle_table,
        principal: fixture_principal_id,
        origin: synthetic_origin_metadata,
    ) -> Result<task_identity, launch_error> {
        if self.phase != launch_phase::prepared
            || self.task.is_some()
            || !self.mapping.is_ready()
            || !self.address_space_attached
            || !self.mappings_attached
        {
            return Err(launch_error::invalid_state);
        }
        let circle = self.circle.ok_or(launch_error::invalid_state)?;
        self.validate_attachments(circles)?;
        let task = tasks
            .create(
                circles,
                task_binding::new(circle, principal, origin, self.admission.bounds()),
            )
            .map_err(launch_error::task)?;
        // Nothing fallible may follow publication before retaining its identity.
        self.task = Some(task);
        self.phase = launch_phase::committed;
        Ok(task)
    }

    /// Cancel partial preparation, an unstarted ready task, or a terminal task.
    /// A running/scheduled task is rejected before ownership changes. Each
    /// successfully withdrawn attachment is recorded before the next fallible
    /// operation; mapping clear/release failures remain retryable.
    ///
    /// # Safety
    /// All roots, registrations, saved contexts and continuations using this
    /// mapping have been withdrawn and flushed. The privileged access and
    /// exclusive lifecycle-table contracts from preparation remain valid.
    pub(super) unsafe fn cancel(
        &mut self,
        allocator: &mut frame_allocator<'_>,
        memory: &mut impl mapping_memory,
        tasks: &mut task_table,
        circles: &mut circle_table,
    ) -> Result<(), launch_error> {
        if self.phase == launch_phase::poisoned {
            return Err(launch_error::poisoned);
        }
        if self.phase == launch_phase::released {
            return Ok(());
        }
        self.validate_attachments(circles)?;
        if let Some(identity) = self.task {
            let task = tasks.snapshot(identity).map_err(launch_error::task)?;
            if task.context_attached() {
                return Err(launch_error::task(task_error::context_still_attached));
            }
            if task.state() == task_state::ready {
                tasks
                    .cancel_unstarted(identity)
                    .map_err(launch_error::task)?;
            } else if !task.state().is_terminal() {
                return Err(launch_error::task(task_error::invalid_state));
            }
        }
        self.phase = launch_phase::cancelling;
        // SAFETY: The caller withdrew every active/software consumer, and the
        // mapping retains all leases even if a preceding cleanup released only
        // a prefix. Never require ready-state validation during cancellation.
        unsafe { self.mapping.cleanup(allocator, memory) }.map_err(launch_error::mapping)?;
        if let Some(circle) = self.circle {
            if self.mappings_attached {
                circles
                    .detach_children(circle, circle_child_kind::mapping, self.mapping_count())
                    .map_err(launch_error::circle)?;
                self.mappings_attached = false;
            }
            if self.address_space_attached {
                circles
                    .detach_child(circle, circle_child_kind::address_space)
                    .map_err(launch_error::circle)?;
                self.address_space_attached = false;
            }
            if let Some(task) = self.task {
                if let Err(error) = tasks.destroy(circles, task) {
                    if error == task_error::record_clearing_failed {
                        // The table deliberately tombstoned the record; retain
                        // provenance and stop instead of retrying a stale ID.
                        self.phase = launch_phase::poisoned;
                    }
                    return Err(launch_error::task(error));
                }
                self.task = None;
            }
            if let Err(error) = circles.destroy(circle) {
                if error == circle_error::record_clearing_failed {
                    self.phase = launch_phase::poisoned;
                }
                return Err(launch_error::circle(error));
            }
            self.circle = None;
        }
        self.phase = launch_phase::released;
        Ok(())
    }

    fn validate_attachments(&self, circles: &circle_table) -> Result<(), launch_error> {
        if let Some(circle) = self.circle {
            let children = circles
                .snapshot(circle)
                .map_err(launch_error::circle)?
                .children();
            let address_spaces = u16::from(self.address_space_attached);
            let mappings = if self.mappings_attached {
                self.mapping_count()
            } else {
                0
            };
            let tasks = u16::from(self.task.is_some());
            if children.address_spaces() != address_spaces
                || children.mappings() != mappings
                || children.tasks() != tasks
                || children.total() != u32::from(address_spaces + mappings + tasks)
            {
                return Err(launch_error::attachment_mismatch);
            }
        } else if self.task.is_some() || self.address_space_attached || self.mappings_attached {
            return Err(launch_error::attachment_mismatch);
        }
        Ok(())
    }

    fn validate_profile(&self) -> Result<(), launch_error> {
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if let Some(profile) = &self.multi_region {
            return self.validate_multi_region(profile);
        }
        let plan = self.mapping.plan();
        #[cfg(not(any(test, feature = "owned_workspace_test")))]
        let workspace = false;
        #[cfg(any(test, feature = "owned_workspace_test"))]
        let workspace = self.workspace_profile;
        #[cfg(not(any(test, feature = "mixed_workspace_test")))]
        let mixed_workspace = false;
        #[cfg(any(test, feature = "mixed_workspace_test"))]
        let mixed_workspace = self.mixed_workspace_profile;
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        if self.arena_base.is_some() != self.arena_kind.is_some() {
            return Err(launch_error::invalid_profile);
        }
        // Two code pages belong only to the controller-selected program
        // arena. Enabling the feature cannot widen the generic page-count API.
        #[cfg(any(test, feature = "native_program_test"))]
        let program = self.arena_base.is_some()
            && self.arena_kind == Some(super::arena_profile::arena_profile::program)
            && !workspace
            && !mixed_workspace;
        #[cfg(not(any(test, feature = "native_program_test")))]
        let program = false;
        #[cfg(any(test, feature = "native_manifest_test"))]
        let manifest_loader = self.arena_base.is_some()
            && self.arena_kind == Some(super::arena_profile::arena_profile::manifest_loader)
            && !workspace
            && !mixed_workspace;
        #[cfg(not(any(test, feature = "native_manifest_test")))]
        let manifest_loader = false;
        if !(matches!(self.expected_code_pages, 1 | 8)
            || (program && self.expected_code_pages == 2)
            || (manifest_loader && self.expected_code_pages == 5)
            || ((workspace || mixed_workspace) && self.expected_code_pages == 32))
            || plan.regions().count() != usize::from(self.mapping_count())
            || self.mapping.residency().is_none()
        {
            return Err(launch_error::invalid_profile);
        }
        if manifest_loader
            && self.admission.bounds().initial_stack_pointer()
                != super::mapping_plan::user_address_limit - 2 * page_size
        {
            return Err(launch_error::invalid_profile);
        }
        if mixed_workspace {
            let stack_top = super::mapping_plan::user_address_limit - 2 * page_size;
            let stack_base = stack_top - 16 * page_size;
            if workspace
                || self.expected_code_pages != 32
                || self.mapping_count() != 4
                || plan
                    .regions()
                    .map(|region| region.extent().start())
                    .ne([0x400000, 0x800000, 0x810000, stack_base])
                || plan.reservations().count() != 8
                || ![
                    (0, 512),
                    (0x3ff000, 1),
                    (0x420000, 1),
                    (0x7ff000, 1),
                    (0x808000, 8),
                    (0x819000, 1),
                    (stack_base - page_size, 1),
                    (stack_top, 1),
                ]
                .into_iter()
                .all(|(start, pages)| {
                    plan.reservations().any(|reservation| {
                        reservation.start() == start && reservation.page_count() == pages
                    })
                })
                || self.admission.bounds().entry() != 0x400000
                || self.admission.bounds().initial_stack_pointer() != stack_top
            {
                return Err(launch_error::invalid_profile);
            }
        }
        if workspace {
            let stack_top = super::mapping_plan::user_address_limit - 2 * page_size;
            let stack_base = stack_top - 16 * page_size;
            if self.expected_code_pages != 32
                || self.mapping_count() != 3
                || plan
                    .regions()
                    .map(|region| region.extent().start())
                    .ne([0x400000, 0x800000, stack_base])
                || plan.reservations().count() != 7
                || ![
                    (0, 512),
                    (0x3ff000, 1),
                    (0x420000, 1),
                    (0x7ff000, 1),
                    (0x808000, 1),
                    (stack_base - page_size, 1),
                    (stack_top, 1),
                ]
                .into_iter()
                .all(|(start, pages)| {
                    plan.reservations().any(|reservation| {
                        reservation.start() == start && reservation.page_count() == pages
                    })
                })
                || self.admission.bounds().entry() != 0x400000
                || self.admission.bounds().initial_stack_pointer() != stack_top
            {
                return Err(launch_error::invalid_profile);
            }
        }
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        if let Some(base) = self.arena_base {
            let arena = plan.regions().nth(2).ok_or(launch_error::invalid_profile)?;
            // Revalidated against the profile the controller selected, never
            // against anything the task could have written.
            let profile = self.arena_kind.ok_or(launch_error::invalid_profile)?;
            let arena_pages = profile.arena_pages();
            let stack_base = profile.stack_base();
            if mixed_workspace
                || !profile.admits_base(base)
                || !profile.is_coherent(base)
                || self.expected_code_pages != profile.code_pages()
                || arena.extent().start() != base
                || plan
                    .regions()
                    .map(|region| region.extent().start())
                    .ne([0x400000, 0x800000, base, stack_base])
                || plan.reservations().count() != 5
                || !plan
                    .reservations()
                    .any(|reservation| reservation.start() == 0 && reservation.page_count() == 512)
                || ![
                    base - page_size,
                    base + arena_pages * page_size,
                    stack_base - page_size,
                    stack_base + profile.stack_pages() * page_size,
                ]
                .into_iter()
                .all(|guard| {
                    plan.reservations().any(|reservation| {
                        reservation.start() == guard && reservation.page_count() == 1
                    })
                })
            {
                return Err(launch_error::invalid_profile);
            }
        }
        for (index, region) in plan.regions().enumerate() {
            let permissions = region.permissions();
            let pages = if mixed_workspace {
                [32, 8, 9, 16][index]
            } else if workspace {
                [32, 8, 16][index]
            } else if index == 0 {
                self.expected_code_pages
            } else if self.mapping_count() == 4 && index == 2 {
                self.arena_pages()
            } else if index == self.stack_region() {
                self.stack_pages()
            } else {
                2
            };
            if region.extent().page_count() != pages
                || !permissions.user
                || permissions.writable != (index != 0)
                || permissions.executable != (index == 0)
            {
                return Err(launch_error::invalid_profile);
            }
        }
        let bounds = self.admission.bounds();
        let length = bounds
            .code_end()
            .checked_sub(bounds.code_start())
            .filter(|length| *length > 0 && *length <= self.expected_code_pages * page_size)
            .ok_or(launch_error::invalid_profile)?;
        let admission = user_entry_plan::new(
            plan,
            0,
            length,
            bounds.entry(),
            self.stack_region(),
            bounds.initial_stack_pointer(),
        )
        .map_err(launch_error::entry)?;
        if admission != self.admission {
            return Err(launch_error::invalid_profile);
        }
        Ok(())
    }

    #[cfg(any(test, feature = "owned_multi_region_test"))]
    fn validate_multi_region(&self, profile: &multi_region_profile) -> Result<(), launch_error> {
        let plan = self.mapping.plan();
        let residency = self
            .mapping
            .residency()
            .ok_or(launch_error::invalid_profile)?;
        if profile.mapping_count != 9
            || plan.regions().count() != usize::from(profile.mapping_count)
            || plan.data_pages() > 128
            || residency.table_count() > 32
            || plan.regions().any(|region| {
                let permissions = region.permissions();
                !permissions.user || (permissions.writable && permissions.executable)
            })
        {
            return Err(launch_error::invalid_profile);
        }
        #[cfg(any(test, feature = "owned_workspace_test"))]
        if self.workspace_profile {
            return Err(launch_error::invalid_profile);
        }
        #[cfg(any(test, feature = "mixed_workspace_test"))]
        if self.mixed_workspace_profile {
            return Err(launch_error::invalid_profile);
        }
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test"
        ))]
        if self.arena_base.is_some() {
            return Err(launch_error::invalid_profile);
        }
        let bounds = self.admission.bounds();
        let intervals = bounds
            .executable_intervals()
            .ok_or(launch_error::invalid_profile)?;
        let initialized_regions = plan
            .regions()
            .filter(|region| {
                region.permissions().executable
                    && intervals.intervals().iter().any(|interval| {
                        interval.start >= region.extent().start()
                            && interval.end <= region.extent().end()
                    })
            })
            .count();
        if intervals.intervals().len() < 2 || initialized_regions < 2 {
            return Err(launch_error::invalid_profile);
        }
        let admission = user_entry_plan::with_executable_intervals(
            plan,
            intervals,
            bounds.entry(),
            profile.stack_region,
            bounds.initial_stack_pointer(),
        )
        .map_err(launch_error::entry)?;
        if admission != self.admission {
            return Err(launch_error::invalid_profile);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::arch::x86_64::mapping_plan::{
        mapping_budget, mapping_plan, mapping_request, reserved_request, user_address_limit,
    };
    use crate::arch::x86_64::paging::{self, entries_per_table, page_permissions};
    use crate::arch::x86_64::resident_plan::{resident_error, resident_plan, resident_request};
    use crate::boot_omen::memory_region;
    use crate::circle::circle_capacity;
    use crate::memory::physical::{frame_accounting, frame_purpose, physical_range};
    use std::vec;
    use std::vec::Vec;

    const frame_count: usize = 128;
    const code_base: u64 = 0x400000;
    const stack_base: u64 = user_address_limit - 4 * page_size;
    const stack_top: u64 = user_address_limit - 2 * page_size;
    const rx: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: true,
    };
    const rw: page_permissions = page_permissions {
        user: true,
        writable: true,
        executable: false,
    };

    struct fake_memory {
        words: Vec<u64>,
        active: u64,
        clears: usize,
        writes: usize,
        reads: usize,
        fail_clear: Option<usize>,
        fail_write: Option<usize>,
        fail_read: Option<usize>,
    }

    impl fake_memory {
        fn new() -> Self {
            Self {
                words: vec![u64::MAX; frame_count * entries_per_table],
                active: 0,
                clears: 0,
                writes: 0,
                reads: 0,
                fail_clear: None,
                fail_write: None,
                fail_read: None,
            }
        }

        fn range(&self, range: physical_range) -> Option<core::ops::Range<usize>> {
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

        fn reset_faults(&mut self) {
            self.fail_clear = None;
            self.fail_write = None;
            self.fail_read = None;
        }
    }

    // SAFETY: Every modeled address is bounded into an isolated vector, never
    // dereferenced as a host physical pointer. Partial failed clears/writes are
    // permitted; successful clear reads back the entire range. Roots do not
    // switch during any callback and no unrelated storage is mutated.
    unsafe impl mapping_memory for fake_memory {
        unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
            self.clears += 1;
            let Some(bounds) = self.range(range) else {
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

    fn mapping(code: u64) -> staged_mapping {
        mapping_with_code_pages(code, 1)
    }

    fn mapping_with_code_pages(code: u64, code_pages: u64) -> staged_mapping {
        let plan = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: code,
                    page_count: code_pages,
                    permissions: rx,
                },
                mapping_request {
                    virtual_address: 0x800000,
                    page_count: 2,
                    permissions: rw,
                },
                mapping_request {
                    virtual_address: stack_base,
                    page_count: 2,
                    permissions: rw,
                },
            ],
            &[
                reserved_request {
                    virtual_address: 0,
                    page_count: 512,
                },
                reserved_request {
                    virtual_address: stack_base - page_size,
                    page_count: 1,
                },
                reserved_request {
                    virtual_address: stack_top,
                    page_count: 1,
                },
            ],
            mapping_budget {
                data_pages: code_pages + 4,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = resident_plan::new(
            &plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            // The borrowed alias lives outside modeled RAM and is never
            // followed by these inactive host tests.
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            9,
        )
        .unwrap();
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    fn owner() -> prepared_launch {
        let mapping = mapping(code_base);
        let admission =
            user_entry_plan::new(mapping.plan(), 0, 4, code_base, 2, stack_top).unwrap();
        prepared_launch::new(mapping, admission)
    }

    fn multi_region_mapping(variant: usize) -> staged_mapping {
        let ro = page_permissions {
            writable: false,
            ..rw
        };
        // The stack deliberately sorts between the two groups of image pages.
        let mut requests: Vec<_> = [
            (0x400000, 1, ro),
            (0x402000, 1, rx),
            (0x404000, 1, ro),
            (0x406000, 1, rw),
            (0x500000, 2, rw),
            (0x600000, 1, ro),
            (0x602000, 1, if variant == 8 { ro } else { rx }),
            (0x604000, 1, ro),
            (0x606000, if variant == 3 { 121 } else { 1 }, rw),
        ]
        .into_iter()
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        )
        .collect();
        if variant == 1 {
            requests.pop();
        } else if variant == 2 {
            requests.push(mapping_request {
                virtual_address: 0x608000,
                page_count: 1,
                permissions: ro,
            });
        }
        let guards =
            [(0, 512), (0x4ff000, 1), (0x502000, 1)].map(|(virtual_address, page_count)| {
                reserved_request {
                    virtual_address,
                    page_count,
                }
            });
        let plan = mapping_plan::new(
            &requests,
            &guards,
            mapping_budget {
                data_pages: 256,
                table_frames: 32,
            },
        )
        .unwrap();
        if variant == 4 {
            return staged_mapping::new(plan);
        }
        let residency = resident_plan::new(
            &plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            32,
        )
        .unwrap();
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    fn multi_region_owner(variant: usize) -> prepared_launch {
        use crate::task::{executable_interval, executable_interval_set};
        let mapping = multi_region_mapping(variant);
        let original = multi_region_mapping(0);
        let intervals = executable_interval_set::new(&[
            executable_interval {
                start: 0x402000,
                end: 0x402008,
            },
            if variant == 6 {
                executable_interval {
                    start: 0x402010,
                    end: 0x402018,
                }
            } else {
                executable_interval {
                    start: 0x602000,
                    end: 0x602008,
                }
            },
        ])
        .unwrap();
        let admission = if variant == 5 {
            user_entry_plan::new(original.plan(), 1, 8, 0x402000, 4, 0x502000).unwrap()
        } else {
            user_entry_plan::with_executable_intervals(
                original.plan(),
                intervals,
                0x402000,
                4,
                0x502000,
            )
            .unwrap()
        };
        let mut owner =
            prepared_launch::new_multi_region(mapping, admission, if variant == 7 { 8 } else { 4 });
        match variant {
            9 => owner.workspace_profile = true,
            10 => owner.mixed_workspace_profile = true,
            11 => owner.arena_base = Some(0x804000),
            _ => {}
        }
        owner
    }

    fn populate_multi_region(mapping: &staged_mapping, memory: &mut fake_memory) -> bool {
        for index in 0..mapping.plan().regions().count() {
            let range = memory.range(mapping.data_range(index).unwrap()).unwrap();
            let marker = 0xa110_0000_u64 + index as u64;
            memory.words[range.start] = marker;
            if memory.words[range.start] != marker
                || memory.words[range.start + 1..range.end]
                    .iter()
                    .any(|word| *word != 0)
            {
                return false;
            }
        }
        true
    }

    #[test]
    fn multi_region_revalidates_profile_before_allocation() {
        for variant in 1..=11 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_multi_region();
            let mut tasks = task_table::new();
            let mut owner = multi_region_owner(variant);
            // SAFETY: Invalid inert profiles must reject before physical access.
            assert!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid multi-region population")
                    })
                }
                .is_err(),
                "variant {variant}"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(tasks.active_count(), 0);
            assert_eq!(circles.accounting(), circle_table::new().accounting());
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn multi_region_lifecycle_owns_nine_children_and_explicit_nonlast_stack() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new_multi_region();
        let mut tasks = task_table::new();
        let mut owner = multi_region_owner(0);
        assert_eq!(owner.stack_region(), 4);
        let tables = owner.mapping().residency().unwrap().table_count();
        // SAFETY: Local modeled RAM, every declared initialized interval copied/read back.
        unsafe {
            owner.prepare(
                &mut allocator,
                &mut memory,
                &mut circles,
                populate_multi_region,
            )
        }
        .unwrap();
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(owner.mapping().retained_leases(), 10);
        assert_eq!(circles.accounting().mappings, 9);
        assert_eq!(
            (
                allocator.accounting().data_owned,
                allocator.accounting().table_owned
            ),
            (10, tables)
        );
        let ranges: Vec<_> = (0..9)
            .map(|index| owner.mapping().data_range(index).unwrap())
            .collect();
        let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
        assert_eq!(
            tasks.snapshot(task).unwrap().bounds(),
            owner.admission().bounds()
        );
        assert_eq!(
            circles
                .snapshot(task.circle())
                .unwrap()
                .children()
                .mappings(),
            9
        );
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
        for range in ranges {
            assert!(
                memory.words[memory.range(range).unwrap()]
                    .iter()
                    .all(|word| *word == 0)
            );
        }
    }

    #[test]
    fn multi_region_old_circle_rejects_without_losing_allocated_ownership() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = multi_region_owner(0);
        assert_eq!(
            unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    populate_multi_region,
                )
            },
            Err(launch_error::circle(circle_error::child_limit_exceeded))
        );
        assert_eq!(owner.mapping().retained_leases(), 10);
        assert_eq!(circles.accounting().address_spaces, 1);
        assert_eq!(circles.accounting().mappings, 0);
        assert_eq!(tasks.active_count(), 0);
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn multi_region_late_population_and_each_cleanup_clear_keep_retryable_ownership() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new_multi_region();
        let mut tasks = task_table::new();
        let mut owner = multi_region_owner(0);
        assert_eq!(
            unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    |mapping, memory| {
                        assert!(populate_multi_region(mapping, memory));
                        let last = memory.range(mapping.data_range(8).unwrap()).unwrap();
                        memory.words[last].fill(u64::MAX);
                        false
                    },
                )
            },
            Err(launch_error::population_failed)
        );
        assert_eq!(owner.mapping().retained_leases(), 10);
        assert_eq!(tasks.active_count(), 0);
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
        // Inactive cleanup clears tables, nine data leases, then the table lease.
        for failure in 1..=11 {
            let mut owner = multi_region_owner(0);
            unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    populate_multi_region,
                )
            }
            .unwrap();
            let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            assert!(
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .is_err()
            );
            assert!(owner.mapping().retained_leases() > 0);
            assert_eq!(owner.task(), Some(task));
            assert_eq!(
                circles
                    .snapshot(task.circle())
                    .unwrap()
                    .children()
                    .mappings(),
                9
            );
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    fn workspace_mapping(variant: usize) -> staged_mapping {
        let workspace_stack = stack_top - 16 * page_size;
        let code = if variant == 4 { 0x600000 } else { code_base };
        let data = if variant == 5 { 0xa00000 } else { 0x800000 };
        let requests = [
            (code, if variant == 1 { 31 } else { 32 }, rx),
            (
                data,
                if variant == 2 { 7 } else { 8 },
                match variant {
                    6 => page_permissions {
                        writable: false,
                        ..rw
                    },
                    7 => rx,
                    _ => rw,
                },
            ),
            (workspace_stack, if variant == 3 { 15 } else { 16 }, rw),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let mut guards = vec![
            (0, 512),
            (code - page_size, 1),
            (code + 32 * page_size, 1),
            (data - page_size, 1),
            (data + 8 * page_size, 1),
            (workspace_stack - page_size, 1),
            (stack_top, 1),
        ];
        if variant == 3 {
            // Still a valid entry plan: the explicit workspace role rejects
            // this smaller stack and its correspondingly displaced upper guard.
            guards[6].0 -= page_size;
        }
        match variant {
            8 => {
                guards.remove(1);
            }
            9 => {
                guards.remove(3);
            }
            10 => {
                guards.remove(0);
            }
            11 => {
                guards[1] = guards[2];
            }
            _ => {}
        }
        let guards: Vec<_> = guards
            .into_iter()
            .map(|(virtual_address, page_count)| reserved_request {
                virtual_address,
                page_count,
            })
            .collect();
        let plan = mapping_plan::new(
            &requests,
            &guards,
            mapping_budget {
                data_pages: 56,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = resident_plan::new(
            &plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            9,
        )
        .unwrap();
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    fn workspace_owner(variant: usize) -> prepared_launch {
        let mapping = workspace_mapping(variant);
        let code = mapping.plan().regions().next().unwrap().extent().start();
        let stack_end = mapping.plan().regions().nth(2).unwrap().extent().end();
        let admission = user_entry_plan::new(
            mapping.plan(),
            0,
            4,
            code + u64::from(variant == 12),
            2,
            stack_end,
        )
        .unwrap();
        match variant {
            13 => prepared_launch::new_with_code_pages(mapping, admission, 32),
            14 => prepared_launch::new_with_code_pages(mapping, admission, 8),
            _ => prepared_launch::new_workspace(mapping, admission),
        }
    }

    #[test]
    fn workspace_role_rejects_shape_permissions_guards_and_legacy_selection_before_allocation() {
        for variant in 1..=14 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut owner = workspace_owner(variant);
            // SAFETY: The inert invalid role must reject before memory access.
            assert_eq!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid workspace populated")
                    })
                },
                Err(launch_error::invalid_profile),
                "variant {variant}"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(circles.accounting(), circle_table::new().accounting());
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn workspace_role_owns_exact_regions_and_sixty_nine_frames_with_transition_stack() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = workspace_owner(0);
        // SAFETY: Local modeled RAM, exact four-byte initialized RX prefix.
        unsafe {
            owner.prepare(
                &mut allocator,
                &mut memory,
                &mut circles,
                |mapping, memory| {
                    let code = memory.range(mapping.data_range(0).unwrap()).unwrap().start;
                    memory.words[code] = 0x90909090;
                    let data = memory.range(mapping.data_range(1).unwrap()).unwrap().start;
                    memory.words[data + page_size as usize / 8] = 0x517a;
                    true
                },
            )
        }
        .unwrap();
        assert!(owner.is_workspace_profile());
        assert_eq!(owner.stack_region(), 2);
        assert_eq!(owner.mapping().retained_leases(), 4);
        assert_eq!(
            owner
                .mapping()
                .plan()
                .regions()
                .map(|r| r.extent().page_count())
                .collect::<Vec<_>>(),
            vec![32, 8, 16]
        );
        let data = memory
            .range(owner.mapping().data_range(1).unwrap())
            .unwrap();
        assert!(
            memory.words[data.start..data.start + page_size as usize / 8]
                .iter()
                .all(|w| *w == 0)
        );
        assert_eq!(memory.words[data.start + page_size as usize / 8], 0x517a);
        assert!(
            memory.words[data.start + page_size as usize / 8 + 1..data.end]
                .iter()
                .all(|w| *w == 0)
        );
        let stack = memory
            .range(owner.mapping().data_range(2).unwrap())
            .unwrap();
        assert!(memory.words[stack.clone()].iter().all(|w| *w == 0));
        let mut transition = unsafe {
            allocator.allocate_with(4, 1, frame_purpose::data, |range| {
                memory.clear_and_verify(range)
            })
        }
        .unwrap();
        allocator.bind(&transition).unwrap();
        let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
        let held = allocator.accounting();
        assert_eq!(
            (
                held.data_owned,
                held.table_owned,
                held.active_leases,
                held.bound_leases
            ),
            (60, 9, 5, 5)
        );
        assert_eq!(initial.free - held.free, 69);
        assert_eq!(circles.accounting().mappings, 3);
        memory.words[stack].fill(u64::MAX);
        // Wrong attachment accounting must not begin release or erase bytes.
        circles
            .detach_child(task.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) },
            Err(launch_error::attachment_mismatch)
        );
        assert_eq!(allocator.accounting(), held);
        circles
            .attach_child(task.circle(), circle_child_kind::mapping)
            .unwrap();
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        allocator.unbind(&transition).unwrap();
        unsafe { allocator.release_with(&mut transition, |range| memory.clear_and_verify(range)) }
            .unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
        assert!(tasks.validate(task).is_err());
    }

    #[test]
    fn workspace_prepare_and_clear_failures_preserve_retryable_ownership() {
        // Four build clears (tables plus three regions), then partial payload.
        for failure in 1..=5 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut owner = workspace_owner(0);
            if failure <= 4 {
                memory.fail_clear = Some(failure);
            }
            assert!(
                unsafe {
                    owner.prepare(
                        &mut allocator,
                        &mut memory,
                        &mut circles,
                        |mapping, memory| {
                            let range = memory.range(mapping.data_range(1).unwrap()).unwrap();
                            memory.words[range].fill(u64::MAX);
                            false
                        },
                    )
                }
                .is_err()
            );
            assert_eq!(tasks.active_count(), 0);
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
        // Initial table clear, three region releases, final table release.
        for failure in 1..=5 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut owner = workspace_owner(0);
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
            let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            assert!(
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .is_err()
            );
            assert!(owner.mapping().retained_leases() > 0);
            assert_eq!(owner.task(), Some(task));
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    fn mixed_workspace_owner(variant: usize) -> prepared_launch {
        let stack = stack_top - 16 * page_size;
        let arena = if variant == 6 { 0x811000 } else { 0x810000 };
        let requests = [
            (code_base, if variant == 1 { 31 } else { 32 }, rx),
            (0x800000, if variant == 2 { 7 } else { 8 }, rw),
            (
                arena,
                if variant == 3 { 8 } else { 9 },
                if variant == 5 { rx } else { rw },
            ),
            (stack, if variant == 4 { 15 } else { 16 }, rw),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let mut guards = vec![
            (0, 512),
            (0x3ff000, 1),
            (0x420000, 1),
            (0x7ff000, 1),
            (
                0x808000,
                if variant == 8 {
                    7
                } else {
                    (arena - 0x808000) / page_size
                },
            ),
            (arena + 9 * page_size, 1),
            (stack - page_size, 1),
            (
                if variant == 4 {
                    stack_top - page_size
                } else {
                    stack_top
                },
                1,
            ),
        ];
        if variant == 7 {
            guards.remove(1);
        }
        if variant == 9 {
            guards.remove(5);
        }
        let guards: Vec<_> = guards
            .into_iter()
            .map(|(virtual_address, page_count)| reserved_request {
                virtual_address,
                page_count,
            })
            .collect();
        let plan = mapping_plan::new(
            &requests,
            &guards,
            mapping_budget {
                data_pages: 65,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = resident_plan::new(
            &plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            9,
        )
        .unwrap();
        let mapping = staged_mapping::new_resident(plan, residency).unwrap();
        let initial_stack = mapping.plan().regions().nth(3).unwrap().extent().end()
            - if variant == 12 { 16 } else { 0 };
        let admission =
            user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, initial_stack).unwrap();
        match variant {
            10 => prepared_launch::new_with_code_pages(mapping, admission, 32),
            11 => prepared_launch::new_with_native_arena(
                mapping,
                admission,
                0x804000,
                super::super::arena_profile::arena_profile::native_application,
            ),
            13 => {
                let mut owner = prepared_launch::new_mixed_workspace(mapping, admission);
                owner.workspace_profile = true;
                owner
            }
            _ => prepared_launch::new_mixed_workspace(mapping, admission),
        }
    }

    #[test]
    fn mixed_workspace_rejects_geometry_permissions_guards_and_other_profile_selection() {
        for variant in 1..=13 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = mixed_workspace_owner(variant);
            // SAFETY: Invalid inert geometry must reject without physical access.
            assert_eq!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid mixed workspace populated")
                    })
                },
                Err(launch_error::invalid_profile),
                "variant {variant}"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(tasks.active_count(), 0);
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn mixed_workspace_has_exact_nine_page_frame_arena_and_retains_dirty_ownership_on_mismatch() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new_native_arenas();
        let mut tasks = task_table::new();
        let mut owner = mixed_workspace_owner(0);
        assert_eq!(owner.arena_extent(), None);
        // SAFETY: Modeled inactive RAM, initialized four-byte code prefix.
        unsafe {
            owner.prepare(
                &mut allocator,
                &mut memory,
                &mut circles,
                |mapping, memory| {
                    let code = memory.range(mapping.data_range(0).unwrap()).unwrap().start;
                    memory.words[code] = 0x9090_9090;
                    memory.words[code] == 0x9090_9090
                },
            )
        }
        .unwrap();
        assert_eq!(owner.arena_extent(), Some((0x810000, 9 * page_size)));
        assert_eq!(owner.stack_region(), 3);
        assert_eq!(
            owner
                .mapping()
                .plan()
                .regions()
                .map(|r| r.extent().page_count())
                .collect::<Vec<_>>(),
            vec![32, 8, 9, 16]
        );
        assert_eq!(owner.mapping().retained_leases(), 5);
        assert_eq!(circles.accounting().mappings, 4);
        for region in 1..4 {
            let range = memory
                .range(owner.mapping().data_range(region).unwrap())
                .unwrap();
            assert!(memory.words[range].iter().all(|word| *word == 0));
        }
        let arena = memory
            .range(owner.mapping().data_range(2).unwrap())
            .unwrap();
        assert_eq!(arena.len() * 8, 2 * 128 * 48 * 3);
        memory.words[arena.clone()].fill(0x517a_a11c_ffff_ffff);
        let mut transition = unsafe {
            allocator.allocate_with(4, 1, frame_purpose::data, |range| {
                memory.clear_and_verify(range)
            })
        }
        .unwrap();
        allocator.bind(&transition).unwrap();
        let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
        let held = allocator.accounting();
        assert_eq!(
            (
                held.data_owned,
                held.table_owned,
                held.active_leases,
                held.bound_leases
            ),
            (69, 9, 6, 6)
        );
        assert_eq!(initial.free - held.free, 78);
        circles
            .detach_child(task.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) },
            Err(launch_error::attachment_mismatch)
        );
        assert_eq!(allocator.accounting(), held);
        assert!(
            memory.words[arena.clone()]
                .iter()
                .all(|word| *word == 0x517a_a11c_ffff_ffff)
        );
        circles
            .attach_child(task.circle(), circle_child_kind::mapping)
            .unwrap();
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        allocator.unbind(&transition).unwrap();
        unsafe { allocator.release_with(&mut transition, |range| memory.clear_and_verify(range)) }
            .unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
        assert_eq!(owner.arena_extent(), None);
        assert!(memory.words[arena].iter().all(|word| *word == 0));
    }

    #[test]
    fn mixed_workspace_partial_build_population_and_every_cleanup_clear_are_retryable() {
        // Five build clears: table lease, code, data, frame arena, user stack.
        for failure in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = mixed_workspace_owner(0);
            memory.fail_clear = (failure <= 5).then_some(failure);
            assert!(
                unsafe {
                    owner.prepare(
                        &mut allocator,
                        &mut memory,
                        &mut circles,
                        |mapping, memory| {
                            let arena = memory.range(mapping.data_range(2).unwrap()).unwrap();
                            memory.words[arena].fill(u64::MAX);
                            false
                        },
                    )
                }
                .is_err()
            );
            assert_eq!(tasks.active_count(), 0);
            assert_eq!(owner.arena_extent(), None);
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
        // Cleanup: initial table clear, four data releases, final table release.
        for failure in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = mixed_workspace_owner(0);
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
            let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            assert!(
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .is_err()
            );
            assert!(owner.mapping().retained_leases() > 0);
            assert_eq!(owner.task(), Some(task));
            assert_eq!(owner.arena_extent(), None);
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn mixed_workspace_legacy_circle_policy_retains_all_resources_for_explicit_cancel() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = mixed_workspace_owner(0);
        assert_eq!(
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) },
            Err(launch_error::circle(circle_error::child_limit_exceeded))
        );
        assert_eq!(owner.mapping().retained_leases(), 5);
        assert_eq!(circles.accounting().address_spaces, 1);
        assert_eq!(circles.accounting().mappings, 0);
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(owner.arena_extent(), None);
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    fn arena_mapping(
        base: u64,
        arena_permissions: page_permissions,
        guard: bool,
    ) -> staged_mapping {
        let requests = [
            (code_base, 8, rx),
            (0x800000, 2, rw),
            (base, 8, arena_permissions),
            (stack_base, 2, rw),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let guards = [
            (0, 512),
            (stack_base - page_size, 1),
            (stack_top, 1),
            (base - page_size, 1),
            (base + 8 * page_size, 1),
        ]
        .map(|(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        });
        let plan = mapping_plan::new(
            &requests,
            &guards[..if guard { 5 } else { 3 }],
            mapping_budget {
                data_pages: 20,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = resident_plan::new(
            &plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            9,
        )
        .unwrap();
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    /// ADR 0101's program geometry: two RX code pages and a 64 KiB arena.
    ///
    /// Written out rather than derived from `arena_profile`, for the same
    /// reason the golden plan tests are: a fixture that asks the profile what
    /// to expect cannot notice the profile changing.
    fn program_arena_mapping(base: u64, arena_permissions: page_permissions) -> staged_mapping {
        program_arena_mapping_with_code_pages(base, arena_permissions, 2)
    }

    fn program_arena_mapping_with_code_pages(
        base: u64,
        arena_permissions: page_permissions,
        code_pages: u64,
    ) -> staged_mapping {
        let requests = [
            (code_base, code_pages, rx),
            (0x800000, 2, rw),
            (base, 16, arena_permissions),
            (stack_base, 2, rw),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let guards = [
            (0, 512),
            (stack_base - page_size, 1),
            (stack_top, 1),
            (base - page_size, 1),
            (base + 16 * page_size, 1),
        ]
        .map(|(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        });
        let plan = mapping_plan::new(
            &requests,
            &guards,
            mapping_budget {
                data_pages: code_pages + 20,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = program_residency(&plan, 9).expect("nine resident tables suffice");
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    /// The resident profile for a program launch, at a stated table budget.
    ///
    /// Separated so a test can bracket the budget: `resident_plan` sets
    /// `table_count` to the user plan's table count plus one, so the exact
    /// requirement is pinned by showing which budgets admit and which refuse,
    /// without needing an accessor for a private field.
    fn program_residency(
        plan: &mapping_plan,
        table_budget: usize,
    ) -> Result<resident_plan, resident_error> {
        resident_plan::new(
            plan,
            &[resident_request {
                virtual_address: 0x100000,
                page_count: 1,
                permissions: page_permissions { user: false, ..rx },
            }],
            frame_count as u64 * page_size
                | paging::present
                | paging::writable
                | paging::no_execute,
            table_budget,
        )
    }

    fn program_owner(base: u64) -> prepared_launch {
        let mapping = program_arena_mapping(base, rw);
        let admission =
            user_entry_plan::new(mapping.plan(), 0, 2 * page_size, code_base, 3, stack_top)
                .unwrap();
        prepared_launch::new_with_native_arena(
            mapping,
            admission,
            base,
            super::super::arena_profile::arena_profile::program,
        )
    }

    /// Independent manifest-loader geometry; never ask the production profile
    /// for the expected dimensions used to test that same profile.
    fn manifest_loader_mapping(
        base: u64,
        widths: [u64; 3],
        arena_permissions: page_permissions,
        guarded: bool,
    ) -> staged_mapping {
        let [code_pages, arena_pages, stack_pages] = widths;
        let loader_stack = stack_top - stack_pages * page_size;
        let requests = [
            (code_base, code_pages, rx),
            (0x800000, 2, rw),
            (base, arena_pages, arena_permissions),
            (loader_stack, stack_pages, rw),
        ]
        .map(
            |(virtual_address, page_count, permissions)| mapping_request {
                virtual_address,
                page_count,
                permissions,
            },
        );
        let guards = [
            (0, 512),
            (loader_stack - page_size, 1),
            (stack_top, 1),
            (base - page_size, 1),
            (base + arena_pages * page_size, 1),
        ]
        .map(|(virtual_address, page_count)| reserved_request {
            virtual_address,
            page_count,
        });
        let plan = mapping_plan::new(
            &requests,
            &guards[..if guarded { 5 } else { 4 }],
            mapping_budget {
                data_pages: code_pages + 2 + arena_pages + stack_pages,
                table_frames: 8,
            },
        )
        .unwrap();
        let residency = program_residency(&plan, 9).unwrap();
        staged_mapping::new_resident(plan, residency).unwrap()
    }

    fn manifest_loader_owner(base: u64) -> prepared_launch {
        let mapping = manifest_loader_mapping(base, [5, 32, 8], rw, true);
        let admission =
            user_entry_plan::new(mapping.plan(), 0, 5 * page_size, code_base, 3, stack_top)
                .unwrap();
        prepared_launch::new_with_native_arena(
            mapping,
            admission,
            base,
            super::super::arena_profile::arena_profile::manifest_loader,
        )
    }

    fn arena_owner(base: u64) -> prepared_launch {
        let mapping = arena_mapping(base, rw, true);
        let admission =
            user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, stack_top).unwrap();
        prepared_launch::new_with_native_arena(
            mapping,
            admission,
            base,
            super::super::arena_profile::arena_profile::native_application,
        )
    }

    #[test]
    fn arena_launch_on_legacy_circle_policy_retains_partial_attachments_for_cancel() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = arena_owner(0x804000);
        // SAFETY: Isolated inactive memory; the ordinary circle policy must
        // reject the fourth mapping after allocation without losing ownership.
        assert_eq!(
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) },
            Err(launch_error::circle(circle_error::child_limit_exceeded))
        );
        assert_eq!(owner.mapping().retained_leases(), 5);
        assert_eq!(circles.accounting().address_spaces, 1);
        assert_eq!(circles.accounting().mappings, 0);
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(owner.arena_extent(), None);
        // SAFETY: No task was published and no hardware consumer exists.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn native_arenas_are_separate_zeroed_owned_regions_with_exact_lifecycle_counts() {
        for base in [0x804000, 0x814000] {
            for publish in [false, true] {
                let mut ledger = [0; frame_count / 4];
                let mut allocator = allocator(&mut ledger);
                let initial = allocator.accounting();
                let mut memory = fake_memory::new();
                let mut circles = circle_table::new_native_arenas();
                let mut tasks = task_table::new();
                let mut owner = arena_owner(base);
                assert_eq!(owner.arena_extent(), None);
                // SAFETY: All physical accesses are isolated modeled RAM; the
                // callback initializes/readbacks every declared code byte.
                unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }
                    .unwrap();
                assert_eq!(owner.arena_extent(), Some((base, 8 * page_size)));
                assert_eq!(owner.stack_region(), 3);
                assert_eq!(owner.mapping().retained_leases(), 5);
                assert_eq!(allocator.accounting().data_owned, 20);
                assert_eq!(allocator.accounting().table_owned, 9);
                assert_eq!(initial.free - allocator.accounting().free, 29);
                assert_eq!(circles.accounting().mappings, 4);
                let arena = owner.mapping().data_range(2).unwrap();
                let arena_words = memory.range(arena).unwrap();
                assert!(
                    memory.words[arena_words.clone()]
                        .iter()
                        .all(|word| *word == 0)
                );
                memory.words[arena_words.clone()].fill(0x517a_a11c_ffff_ffff);
                if publish {
                    commit(&mut owner, &mut tasks, &mut circles).unwrap();
                }
                // Mismatched children must fail before clearing dirty arena RAM.
                let circle = owner.circle().unwrap();
                circles
                    .detach_child(circle, circle_child_kind::mapping)
                    .unwrap();
                let before = allocator.accounting();
                assert_eq!(
                    unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) },
                    Err(launch_error::attachment_mismatch)
                );
                assert_eq!(allocator.accounting(), before);
                assert!(
                    memory.words[arena_words.clone()]
                        .iter()
                        .all(|word| *word == 0x517a_a11c_ffff_ffff)
                );
                circles
                    .attach_child(circle, circle_child_kind::mapping)
                    .unwrap();
                // SAFETY: No task was started; no context/root consumer exists.
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .unwrap();
                assert_released(&owner, &allocator, initial, &tasks, &circles);
                assert_eq!(owner.arena_extent(), None);
                assert!(memory.words[arena_words].iter().all(|word| *word == 0));
            }
        }
    }

    #[test]
    fn native_arena_profile_rejects_legacy_selection_wrong_base_permissions_and_missing_guards() {
        for variant in 0..7 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let permissions = match variant {
                2 => page_permissions {
                    writable: false,
                    ..rw
                },
                3 => rx,
                _ => rw,
            };
            let mapping = arena_mapping(0x804000, permissions, variant != 4);
            let admission =
                user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, stack_top).unwrap();
            let mut owner = if variant == 0 {
                prepared_launch::new_with_code_pages(mapping, admission, 8)
            } else {
                prepared_launch::new_with_native_arena(
                    mapping,
                    admission,
                    if variant == 1 {
                        0x814000
                    } else if variant == 5 {
                        u64::MAX
                    } else {
                        0x804000
                    },
                    super::super::arena_profile::arena_profile::native_application,
                )
            };
            if variant == 6 {
                // A profile may not use the arena itself as the return stack.
                owner.admission =
                    user_entry_plan::new(owner.mapping.plan(), 0, 4, code_base, 2, 0x80c000)
                        .unwrap();
            }
            // SAFETY: Invalid inert profile must reject before any access.
            assert!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid arena populated")
                    })
                }
                .is_err()
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(circles.accounting(), circle_table::new().accounting());
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn a_program_launch_reports_its_own_geometry_and_stays_cancellable() {
        for base in super::super::arena_profile::program_arena_bases {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = program_owner(base);
            // SAFETY: Modeled inactive memory. The closure only reports that
            // population was reached; what it would copy is the vessel's bytes,
            // which this fixture is not the place to assert.
            let prepared =
                unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| true) };
            assert!(prepared.is_ok(), "a program geometry must be admitted");
            // A 64 KiB arena, reported from the retained profile rather than a
            // literal. The length is the whole point: the same owner under the
            // native profile would report 32 KiB.
            assert_eq!(owner.arena_extent(), Some((base, 64 * 1024)));
            assert_eq!(owner.mapping_count(), 4);
            assert_eq!(owner.expected_code_pages, 2);
            assert_eq!(owner.mapping().retained_leases(), 5);
            assert_eq!(allocator.accounting().data_owned, 22);
            assert_eq!(allocator.accounting().table_owned, 9);
            assert_eq!(initial.free - allocator.accounting().free, 31);
            // SAFETY: Cancellation must reclaim everything prepare acquired.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn manifest_loader_owns_exact_memory_and_clears_dirty_arena_and_stack() {
        for base in [0x804000, 0x828000] {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = manifest_loader_owner(base);
            // SAFETY: Isolated modeled inactive memory, no hardware accesses.
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| true) }
                .unwrap();
            assert_eq!(owner.arena_extent(), Some((base, 128 * 1024)));
            assert_eq!(owner.mapping_count(), 4);
            assert_eq!(owner.expected_code_pages, 5);
            assert_eq!(owner.stack_pages(), 8);
            assert_eq!(owner.mapping().retained_leases(), 5);
            assert_eq!(allocator.accounting().data_owned, 47);
            assert_eq!(allocator.accounting().table_owned, 9);
            assert_eq!(initial.free - allocator.accounting().free, 56);
            assert!(program_residency(owner.mapping().plan(), 9).is_ok());
            assert!(matches!(
                program_residency(owner.mapping().plan(), 8),
                Err(resident_error::table_budget_exhausted)
            ));
            let arena = memory
                .range(owner.mapping().data_range(2).unwrap())
                .unwrap();
            let stack = memory
                .range(owner.mapping().data_range(3).unwrap())
                .unwrap();
            assert_eq!(arena.len() * 8, 128 * 1024);
            assert_eq!(stack.len() * 8, 32 * 1024);
            assert!(memory.words[arena.clone()].iter().all(|word| *word == 0));
            assert!(memory.words[stack.clone()].iter().all(|word| *word == 0));
            memory.words[arena.clone()].fill(0x4d41_4e49_4645_5354);
            memory.words[stack.clone()].fill(0x5354_4143_4b5f_4c44);
            // SAFETY: Cancellation retains and clears the complete owned spans.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
            assert!(memory.words[arena].iter().all(|word| *word == 0));
            assert!(memory.words[stack].iter().all(|word| *word == 0));
        }
    }

    #[test]
    fn manifest_loader_refuses_wrong_geometry_and_unselected_wide_profiles_before_allocation() {
        for variant in 0..22 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let initial_circles = circles.accounting();
            let mut tasks = task_table::new();
            let base = 0x804000;
            let widths = match variant {
                0 => [2, 32, 8],
                1 => [8, 32, 8],
                2 => [4, 32, 8],
                3 => [6, 32, 8],
                4 => [5, 31, 8],
                5 => [5, 33, 8],
                6 => [5, 32, 7],
                7 => [5, 32, 9],
                _ => [5, 32, 8],
            };
            let permissions = match variant {
                8 => page_permissions {
                    writable: false,
                    ..rw
                },
                9 => rx,
                _ => rw,
            };
            let mapping = manifest_loader_mapping(base, widths, permissions, variant != 10);
            let admission = if variant == 21 {
                let wider = manifest_loader_mapping(base, [6, 32, 8], rw, true);
                user_entry_plan::new(wider.plan(), 0, 5 * page_size + 1, code_base, 3, stack_top)
                    .unwrap()
            } else {
                let initial_stack = if variant == 20 {
                    stack_top - 16
                } else {
                    stack_top
                };
                user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, initial_stack).unwrap()
            };
            let mut owner = prepared_launch::new_with_native_arena(
                mapping,
                admission,
                base,
                super::super::arena_profile::arena_profile::manifest_loader,
            );
            match variant {
                11 => owner.expected_code_pages = 8,
                12 => owner.arena_kind = None,
                13 => owner.arena_base = None,
                14 => {
                    owner.arena_base = None;
                    owner.arena_kind = None;
                }
                15 => owner.arena_kind = Some(super::super::arena_profile::arena_profile::program),
                16 => {
                    owner.arena_kind =
                        Some(super::super::arena_profile::arena_profile::native_application)
                }
                17 => owner.workspace_profile = true,
                18 => owner.mixed_workspace_profile = true,
                19 => owner.arena_base = Some(0x818000),
                _ => {}
            }
            // SAFETY: Invalid inert geometry must reject before access or allocation.
            assert_eq!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid manifest loader populated")
                    })
                },
                Err(launch_error::invalid_profile),
                "variant {variant}"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(circles.accounting(), initial_circles);
            assert_eq!(tasks.active_count(), 0);
            // SAFETY: Rejected preparation published no execution consumer.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn a_program_arena_requires_exactly_nine_resident_tables() {
        // `resident_plan` takes the user plan's table count plus one. The user
        // plan needs eight for this geometry, so nine admits and eight is
        // refused. Bracketing pins the exact requirement without reading a
        // private field, and it fails if either count moves.
        let mapping =
            program_arena_mapping(super::super::arena_profile::program_arena_bases[0], rw);
        let plan = mapping.plan();
        assert_eq!(plan.tables().len(), 8);
        assert!(program_residency(plan, 9).is_ok());
        assert!(matches!(
            program_residency(plan, 8),
            Err(resident_error::table_budget_exhausted)
        ));
    }

    #[test]
    fn two_program_code_pages_require_exact_mapping_width_and_retained_kind() {
        for variant in 0..13 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let initial_circles = circles.accounting();
            let mut tasks = task_table::new();
            let base = 0x804000;
            let pages = match variant {
                0 => 1,
                1 => 3,
                2 => 8,
                _ => 2,
            };
            let mapping = program_arena_mapping_with_code_pages(base, rw, pages);
            let admission = if variant == 12 {
                let wider = program_arena_mapping_with_code_pages(base, rw, 3);
                user_entry_plan::new(wider.plan(), 0, 2 * page_size + 1, code_base, 3, stack_top)
                    .unwrap()
            } else {
                user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, stack_top).unwrap()
            };
            let mut owner = prepared_launch::new_with_native_arena(
                mapping,
                admission,
                base,
                super::super::arena_profile::arena_profile::program,
            );
            match variant {
                3 => owner.expected_code_pages = 1,
                4 => owner.expected_code_pages = 3,
                5 => owner.expected_code_pages = 8,
                6 => owner.arena_kind = None,
                7 => {
                    owner.arena_kind =
                        Some(super::super::arena_profile::arena_profile::native_application);
                }
                8 => owner.arena_base = None,
                9 => {
                    owner.arena_base = None;
                    owner.arena_kind = None;
                }
                10 => owner.workspace_profile = true,
                11 => owner.mixed_workspace_profile = true,
                _ => {}
            }
            // SAFETY: Wrong geometry or retained selection must fail before
            // memory access, allocation or publication of any lifecycle child.
            assert_eq!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid program code profile populated")
                    })
                },
                Err(launch_error::invalid_profile),
                "variant {variant}"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            assert_eq!(circles.accounting(), initial_circles);
            assert_eq!(tasks.active_count(), 0);
            // SAFETY: Rejected preparation acquired no execution consumers.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn a_program_profile_refuses_a_writable_executable_or_unguarded_arena() {
        // The same refusals the native profile gets, against the program
        // geometry: a non-writable arena, an executable one, and a base the
        // profile does not admit.
        for variant in 0..3 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let permissions = match variant {
                0 => page_permissions {
                    writable: false,
                    ..rw
                },
                1 => rx,
                _ => rw,
            };
            let base = super::super::arena_profile::program_arena_bases[0];
            let mapping = program_arena_mapping(base, permissions);
            let admission =
                user_entry_plan::new(mapping.plan(), 0, 4, code_base, 3, stack_top).unwrap();
            let mut owner = prepared_launch::new_with_native_arena(
                mapping,
                admission,
                if variant == 2 { 0x814000 } else { base },
                super::super::arena_profile::arena_profile::program,
            );
            // SAFETY: An inert invalid profile must reject before any access.
            assert!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid program arena populated")
                    })
                }
                .is_err(),
                "variant {variant} must be refused"
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!((memory.clears, memory.writes, memory.reads), (0, 0, 0));
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn every_native_arena_clear_and_partial_population_failure_remains_cancellable() {
        for fail_clear in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = arena_owner(0x804000);
            if fail_clear <= 5 {
                memory.fail_clear = Some(fail_clear);
            }
            // SAFETY: Modeled inactive memory; intentional failure retains all
            // acquired leases, including partially dirtied arena contents.
            let result = unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    |mapping, memory| {
                        let arena = memory.range(mapping.data_range(2).unwrap()).unwrap();
                        memory.words[arena].fill(u64::MAX);
                        false
                    },
                )
            };
            assert!(result.is_err());
            assert_eq!(owner.arena_extent(), None);
            assert_eq!(tasks.active_count(), 0);
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn two_native_arenas_with_private_transition_stacks_reclaim_exactly_sixty_six_frames() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new_native_arenas();
        let mut tasks = task_table::new();
        let mut owners = [arena_owner(0x804000), arena_owner(0x814000)];
        let mut stacks = [None, None];
        for (index, owner) in owners.iter_mut().enumerate() {
            // SAFETY: Isolated inactive RAM, including every transition stack.
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
            let lease = unsafe {
                allocator.allocate_with(4, 1, frame_purpose::data, |range| {
                    memory.clear_and_verify(range)
                })
            }
            .unwrap();
            allocator.bind(&lease).unwrap();
            stacks[index] = Some(lease);
            commit(owner, &mut tasks, &mut circles).unwrap();
        }
        let allocated = allocator.accounting();
        assert_eq!((allocated.data_owned, allocated.table_owned), (48, 18));
        assert_eq!((allocated.active_leases, allocated.bound_leases), (12, 12));
        assert_eq!(initial.free - allocated.free, 66);
        assert_eq!(circles.accounting().mappings, 8);
        assert_ne!(
            owners[0].mapping().data_range(2),
            owners[1].mapping().data_range(2)
        );
        for (owner, stack) in owners.iter_mut().zip(stacks.iter_mut()) {
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            let lease = stack.as_mut().unwrap();
            allocator.unbind(lease).unwrap();
            unsafe { allocator.release_with(lease, |range| memory.clear_and_verify(range)) }
                .unwrap();
        }
        assert_eq!(allocator.accounting(), initial);
        assert_eq!(circles.accounting(), circle_table::new().accounting());
        assert_eq!(tasks.active_count(), 0);
    }

    #[test]
    fn native_arena_cleanup_failures_retain_every_remaining_lease_for_retry() {
        // Table clear, four region releases, and final table release.
        for failure in 1..=6 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new_native_arenas();
            let mut tasks = task_table::new();
            let mut owner = arena_owner(0x814000);
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
            let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            assert!(
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .is_err()
            );
            assert_eq!(tasks.snapshot(task).unwrap().state(), task_state::exited);
            assert_eq!(owner.task(), Some(task));
            assert_eq!(owner.arena_extent(), None);
            assert_eq!(circles.accounting().mappings, 4);
            assert!(owner.mapping().retained_leases() > 0);
            memory.reset_faults();
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    fn populate(mapping: &staged_mapping, memory: &mut fake_memory) -> bool {
        let code = mapping.data_range(0).unwrap();
        let data = mapping.data_range(1).unwrap();
        let code_index = memory.range(code).unwrap().start;
        let data_index = memory.range(data).unwrap().start;
        memory.words[code_index] = 0x9090_9090;
        memory.words[data_index] = 0x517a;
        memory.words[code_index] == 0x9090_9090 && memory.words[data_index] == 0x517a
    }

    fn commit(
        owner: &mut prepared_launch,
        tasks: &mut task_table,
        circles: &mut circle_table,
    ) -> Result<task_identity, launch_error> {
        owner.commit(
            tasks,
            circles,
            fixture_principal_id::new(1).unwrap(),
            synthetic_origin_metadata::machine_role_fixture,
        )
    }

    fn assert_released(
        owner: &prepared_launch,
        allocator: &frame_allocator<'_>,
        initial: frame_accounting,
        tasks: &task_table,
        circles: &circle_table,
    ) {
        assert!(owner.is_released());
        assert_eq!(owner.mapping().retained_leases(), 0);
        assert!(owner.task().is_none());
        assert!(owner.circle().is_none());
        assert_eq!(allocator.accounting(), initial);
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(circles.accounting(), circle_table::new().accounting());
    }

    #[test]
    fn explicit_eight_page_profile_populates_commits_and_reclaims_every_owned_frame() {
        for publish in [false, true] {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mapping = mapping_with_code_pages(code_base, 8);
            let admission =
                user_entry_plan::new(mapping.plan(), 0, 8 * page_size, code_base, 2, stack_top)
                    .unwrap();
            let mut owner = prepared_launch::new_with_code_pages(mapping, admission, 8);
            // SAFETY: All RAM is isolated vector storage, the root stays
            // inactive, and every declared code byte is populated/read back.
            unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    |mapping, memory| {
                        let range = memory.range(mapping.data_range(0).unwrap()).unwrap();
                        assert_eq!(range.len() * 8, 8 * page_size as usize);
                        for (offset, word) in memory.words[range.clone()].iter_mut().enumerate() {
                            *word = 0x9090_9090_0000_0000 | offset as u64;
                        }
                        memory.words[range]
                            .iter()
                            .enumerate()
                            .all(|(offset, word)| *word == (0x9090_9090_0000_0000 | offset as u64))
                    },
                )
            }
            .unwrap();
            assert!(owner.is_prepared());
            assert_eq!(tasks.active_count(), 0);
            assert_eq!(allocator.accounting().data_owned, 12);
            assert_eq!(allocator.accounting().table_owned, 9);
            assert_eq!(initial.free - allocator.accounting().free, 21);
            assert_eq!(circles.accounting().mappings, 3);
            let retained = [
                owner.mapping().table_range().unwrap(),
                owner.mapping().data_range(0).unwrap(),
                owner.mapping().data_range(1).unwrap(),
                owner.mapping().data_range(2).unwrap(),
            ];
            let task = publish.then(|| commit(&mut owner, &mut tasks, &mut circles).unwrap());
            if let Some(task) = task {
                assert!(owner.is_committed());
                assert_eq!(tasks.snapshot(task).unwrap().state(), task_state::ready);
            }
            // SAFETY: No task was entered and no mapping consumer was attached.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
            for range in retained {
                assert!(
                    memory.words[memory.range(range).unwrap()]
                        .iter()
                        .all(|word| *word == 0)
                );
            }
            if let Some(task) = task {
                assert!(tasks.validate(task).is_err());
            }
        }
    }

    #[test]
    fn unsupported_mismatched_and_overlong_code_profiles_reject_before_resource_mutation() {
        // Admission may have been built from another inert mapping. Recheck
        // both explicit policy and initialized byte extent before acquiring RAM.
        for (mapped_pages, admitted_pages, length, selected_pages, use_default) in [
            (8, 8, 8 * page_size, 1, true),
            (8, 8, 4, 1, false),
            (1, 1, 4, 8, false),
            (2, 2, 2 * page_size, 2, false),
            (8, 8, 4, 0, false),
            (8, 8, 4, 9, false),
            (8, 8, 4, u64::MAX, false),
            (8, 9, 8 * page_size + 1, 8, false),
        ] {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let before = memory.words.clone();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let admitted = mapping_with_code_pages(code_base, admitted_pages);
            let admission =
                user_entry_plan::new(admitted.plan(), 0, length, code_base, 2, stack_top).unwrap();
            let mapping = mapping_with_code_pages(code_base, mapped_pages);
            let mut owner = if use_default {
                prepared_launch::new(mapping, admission)
            } else {
                prepared_launch::new_with_code_pages(mapping, admission, selected_pages)
            };
            // SAFETY: Invalid inert metadata is rejected before this callback,
            // any mapping memory access, or lifecycle-table mutation.
            assert_eq!(
                unsafe {
                    owner.prepare(&mut allocator, &mut memory, &mut circles, |_, _| {
                        panic!("invalid profile reached population")
                    })
                },
                Err(launch_error::invalid_profile)
            );
            assert_eq!(allocator.accounting(), initial);
            assert_eq!(memory.words, before);
            assert_eq!((memory.clears, memory.reads, memory.writes), (0, 0, 0));
            assert_eq!(circles.accounting(), circle_table::new().accounting());
            assert_eq!(tasks.active_count(), 0);
            assert_eq!(owner.mapping().retained_leases(), 0);
            assert_eq!(
                commit(&mut owner, &mut tasks, &mut circles),
                Err(launch_error::invalid_state)
            );
            // SAFETY: The rejected owner never acquired hardware consumers.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn preparation_publishes_no_task_and_commit_is_explicit_and_single_use() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = owner();
        assert_eq!(
            commit(&mut owner, &mut tasks, &mut circles),
            Err(launch_error::invalid_state)
        );
        // SAFETY: All modeled ownership is local and the callback verifies it.
        unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
        assert!(owner.is_prepared());
        assert_eq!(tasks.active_count(), 0);
        assert!(owner.task().is_none());
        assert_eq!(circles.accounting().mappings, 3);
        // SAFETY: This rejected query activates nothing.
        assert_eq!(
            unsafe { owner.committed_mapping_mut() }.err(),
            Some(launch_error::invalid_state)
        );
        let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
        let circle = owner.circle().unwrap();
        assert!(owner.is_committed());
        assert_eq!(tasks.snapshot(task).unwrap().state(), task_state::ready);
        assert_eq!(
            commit(&mut owner, &mut tasks, &mut circles),
            Err(launch_error::invalid_state)
        );
        // SAFETY: The modeled root has never been activated or borrowed.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert!(tasks.validate(task).is_err());
        assert!(circles.validate(circle).is_err());
        assert_released(&owner, &allocator, initial, &tasks, &circles);
        // SAFETY: Completed cancellation has no live consumers and is idempotent.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
    }

    #[test]
    fn every_allocation_failure_and_partial_population_retains_cancellable_ownership() {
        for failure in 0..8 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut owner = owner();
            match failure {
                0..=3 => memory.fail_clear = Some(failure + 1),
                4 => memory.fail_write = Some(1),
                5 => memory.fail_read = Some(1),
                _ => {}
            }
            // SAFETY: The deliberately failing callback writes only the owned
            // image; all failure variants retain the owner and inactive model.
            let result = unsafe {
                owner.prepare(
                    &mut allocator,
                    &mut memory,
                    &mut circles,
                    |mapping, memory| {
                        let copied = populate(mapping, memory);
                        if failure == 7 {
                            memory.fail_read = Some(memory.reads + 1);
                        }
                        copied && failure != 6
                    },
                )
            };
            assert!(result.is_err(), "failure {failure}");
            assert_eq!(tasks.active_count(), 0);
            assert!(!owner.is_prepared());
            assert_eq!(
                commit(&mut owner, &mut tasks, &mut circles),
                Err(launch_error::invalid_state)
            );
            memory.reset_faults();
            // SAFETY: Every failed construction remains inactive and retained.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn circle_exhaustion_retains_mapping_and_does_not_touch_other_circles() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let existing = core::array::from_fn::<_, circle_capacity, _>(|_| circles.create().unwrap());
        let accounting = circles.accounting();
        let mut tasks = task_table::new();
        let mut owner = owner();
        // SAFETY: Only the local vector and exclusively borrowed tables mutate.
        assert_eq!(
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) },
            Err(launch_error::circle(circle_error::capacity_exhausted))
        );
        assert!(owner.mapping().retained_leases() > 0);
        assert_eq!(tasks.active_count(), 0);
        // SAFETY: The incomplete owner has no consumers and no circle to detach.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_eq!(circles.accounting(), accounting);
        for circle in existing {
            circles.destroy(circle).unwrap();
        }
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn prepared_owner_without_task_can_be_cancelled() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = owner();
        // SAFETY: The vector models all retained RAM and no root is active.
        unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
        let circle = owner.circle().unwrap();
        // SAFETY: No task or root consumer was ever published.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert!(circles.validate(circle).is_err());
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn every_cleanup_clear_failure_cancels_ready_task_and_preserves_retryable_ownership() {
        // Table-arena clear, three data releases, and table-arena release.
        for failure in 1..=5 {
            let mut ledger = [0; frame_count / 4];
            let mut allocator = allocator(&mut ledger);
            let initial = allocator.accounting();
            let mut memory = fake_memory::new();
            let mut circles = circle_table::new();
            let mut tasks = task_table::new();
            let mut owner = owner();
            // SAFETY: Local modeled RAM/owners; no active root or continuation.
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
            let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
            let circle = owner.circle().unwrap();
            memory.fail_clear = Some(memory.clears + failure);
            // SAFETY: The ready task is unstarted and root has no consumers.
            assert!(
                unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }
                    .is_err()
            );
            assert_eq!(tasks.snapshot(task).unwrap().state(), task_state::exited);
            assert!(tasks.start(task).is_err());
            assert_eq!(owner.task(), Some(task));
            assert_eq!(owner.circle(), Some(circle));
            assert_eq!(circles.accounting().mappings, 3);
            assert!(owner.mapping().retained_leases() > 0);
            assert!(!owner.is_committed());
            memory.reset_faults();
            // SAFETY: The retained prefix is inactive and remains exclusively owned.
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
            assert_released(&owner, &allocator, initial, &tasks, &circles);
        }
    }

    #[test]
    fn running_task_cannot_be_cancelled_but_terminal_task_can_be_reclaimed() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = owner();
        // SAFETY: Local vector-backed ownership; no actual execution occurs.
        unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
        let task = commit(&mut owner, &mut tasks, &mut circles).unwrap();
        tasks.start(task).unwrap();
        let allocated = allocator.accounting();
        let children = circles.accounting();
        let clears = memory.clears;
        // SAFETY: No hardware execution exists; the lifecycle state deliberately
        // rejects this request before any memory operation.
        assert_eq!(
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) },
            Err(launch_error::task(task_error::invalid_state))
        );
        assert_eq!(allocator.accounting(), allocated);
        assert_eq!(circles.accounting(), children);
        assert_eq!(memory.clears, clears);
        assert!(owner.is_committed());
        tasks.exit(task).unwrap();
        // SAFETY: The modeled task is terminal with no consumers.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn foreign_children_block_commit_and_cleanup_without_mutation() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let mut owner = owner();
        // SAFETY: Only isolated vector-backed storage is accessed.
        unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.unwrap();
        let circle = owner.circle().unwrap();
        circles
            .attach_child(circle, circle_child_kind::endpoint)
            .unwrap();
        let allocated = allocator.accounting();
        let children = circles.accounting();
        let clears = memory.clears;
        assert_eq!(
            commit(&mut owner, &mut tasks, &mut circles),
            Err(launch_error::attachment_mismatch)
        );
        // SAFETY: No root consumers exist; unexpected bookkeeping is rejected.
        assert_eq!(
            unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) },
            Err(launch_error::attachment_mismatch)
        );
        assert!(owner.is_prepared());
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(allocator.accounting(), allocated);
        assert_eq!(circles.accounting(), children);
        assert_eq!(memory.clears, clears);
        circles
            .detach_child(circle, circle_child_kind::endpoint)
            .unwrap();
        commit(&mut owner, &mut tasks, &mut circles).unwrap();
        // SAFETY: Removed the foreign attachment; the ready task is unstarted.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }

    #[test]
    fn admission_for_a_different_mapping_is_rejected_before_allocation() {
        let mut ledger = [0; frame_count / 4];
        let mut allocator = allocator(&mut ledger);
        let initial = allocator.accounting();
        let mut memory = fake_memory::new();
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let original = mapping(code_base);
        let admission =
            user_entry_plan::new(original.plan(), 0, 4, code_base, 2, stack_top).unwrap();
        let mut owner = prepared_launch::new(mapping(code_base + page_size), admission);
        // SAFETY: Rejection occurs before the callback or any modeled allocation.
        assert!(
            unsafe { owner.prepare(&mut allocator, &mut memory, &mut circles, populate) }.is_err()
        );
        assert_eq!(allocator.accounting(), initial);
        assert_eq!(memory.clears, 0);
        assert_eq!(tasks.active_count(), 0);
        // SAFETY: Empty retained ownership has never acquired a consumer.
        unsafe { owner.cancel(&mut allocator, &mut memory, &mut tasks, &mut circles) }.unwrap();
        assert_released(&owner, &allocator, initial, &tasks, &circles);
    }
}

#[cfg(feature = "native_editor_workspace_test")]
impl prepared_launch {
    pub(super) const fn is_mixed_workspace_profile(&self) -> bool {
        self.mixed_workspace_profile
    }
}
