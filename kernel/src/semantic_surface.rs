#![cfg_attr(not(test), allow(dead_code))]

//! Architecture-neutral owner for one copied semantic-surface snapshot.
//!
//! This module is the bounded kernel state machine from ADR 0028. It owns one
//! exact source/consumer route and at most one complete version-1 transport
//! body. It performs no Rite admission, user copy, scheduling, rendering,
//! presentation, service completion, or semantic publication.
//!
//! Numeric identities and body fields are correlation only. The trusted
//! kernel integration must authenticate the current task, select the unique
//! endpoint owner, and retain serialized exclusion around every prepare/copy/
//! commit sequence. Possession of a matching [`semantic_surface_binding`]
//! does not confer authority.

use core::fmt;

use anomalous_desktop_transport::{
    surface_snapshot_binding, surface_snapshot_body_v1, surface_snapshot_v1_byte_count,
};

use crate::identity::{endpoint_id, next_generation, task_identity};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum semantic_surface_error {
    zero_endpoint_generation,
    endpoint_generation_exhausted,
    source_is_consumer,
    wrong_endpoint,
    wrong_source,
    wrong_consumer,
    wrong_surface,
    wrong_scene,
    stale_revision,
    wrong_renderer,
    stale_attempt,
    endpoint_full,
    endpoint_empty,
    endpoint_closed,
    submission_replayed,
    stale_preparation,
    invariant_violation,
}

impl semantic_surface_error {
    #[must_use]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::zero_endpoint_generation => "zero_endpoint_generation",
            Self::endpoint_generation_exhausted => "endpoint_generation_exhausted",
            Self::source_is_consumer => "source_is_consumer",
            Self::wrong_endpoint => "wrong_endpoint",
            Self::wrong_source => "wrong_source",
            Self::wrong_consumer => "wrong_consumer",
            Self::wrong_surface => "wrong_surface",
            Self::wrong_scene => "wrong_scene",
            Self::stale_revision => "stale_revision",
            Self::wrong_renderer => "wrong_renderer",
            Self::stale_attempt => "stale_attempt",
            Self::endpoint_full => "endpoint_full",
            Self::endpoint_empty => "endpoint_empty",
            Self::endpoint_closed => "endpoint_closed",
            Self::submission_replayed => "submission_replayed",
            Self::stale_preparation => "stale_preparation",
            Self::invariant_violation => "invariant_violation",
        }
    }
}

impl fmt::Debug for semantic_surface_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for semantic_surface_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Private generational name for one kernel-owned endpoint slot.
///
/// This value is freshness data, not authority. The table or fixed proof host
/// that owns the corresponding [`semantic_surface_endpoint`] remains the
/// authority boundary.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct semantic_surface_endpoint_identity(endpoint_id);

impl semantic_surface_endpoint_identity {
    pub(crate) const fn try_new(
        slot: usize,
        generation: u64,
    ) -> Result<Self, semantic_surface_error> {
        let Some(identity) = endpoint_id::try_from_parts(slot, generation) else {
            return Err(semantic_surface_error::zero_endpoint_generation);
        };
        Ok(Self(identity))
    }

    #[must_use]
    pub(crate) const fn kernel_identity(self) -> endpoint_id {
        self.0
    }

    #[must_use]
    pub(crate) const fn slot(self) -> usize {
        self.0.slot()
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.0.generation_value()
    }

    pub(crate) const fn next_generation(self) -> Result<Self, semantic_surface_error> {
        let Some(generation) = next_generation(self.generation()) else {
            return Err(semantic_surface_error::endpoint_generation_exhausted);
        };
        Ok(Self(endpoint_id::from_parts(self.slot(), generation)))
    }
}

/// Exact immutable route and snapshot correlation for one endpoint lifetime.
///
/// It is deliberately copyable because copying public correlation must never
/// copy authority. A trusted owner must compare every field against the live
/// endpoint before inspecting private state.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct semantic_surface_binding {
    endpoint: semantic_surface_endpoint_identity,
    source: task_identity,
    consumer: task_identity,
    snapshot: surface_snapshot_binding,
}

impl semantic_surface_binding {
    pub(crate) fn try_new(
        endpoint: semantic_surface_endpoint_identity,
        source: task_identity,
        consumer: task_identity,
        snapshot: surface_snapshot_binding,
    ) -> Result<Self, semantic_surface_error> {
        if source == consumer {
            return Err(semantic_surface_error::source_is_consumer);
        }
        Ok(Self {
            endpoint,
            source,
            consumer,
            snapshot,
        })
    }

    #[must_use]
    pub(crate) const fn endpoint(self) -> semantic_surface_endpoint_identity {
        self.endpoint
    }

    #[must_use]
    pub(crate) const fn source(self) -> task_identity {
        self.source
    }

    #[must_use]
    pub(crate) const fn consumer(self) -> task_identity {
        self.consumer
    }

    #[must_use]
    pub(crate) const fn snapshot(self) -> surface_snapshot_binding {
        self.snapshot
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum semantic_surface_phase {
    empty,
    queued,
    closed,
}

struct semantic_surface_slot {
    occupied: bool,
    bytes: [u8; surface_snapshot_v1_byte_count],
}

impl semantic_surface_slot {
    const fn empty() -> Self {
        Self {
            occupied: false,
            bytes: [0; surface_snapshot_v1_byte_count],
        }
    }

    fn install(&mut self, snapshot: surface_snapshot_body_v1) {
        debug_assert!(self.storage_is_clear());
        self.bytes = snapshot.into_bytes();
        self.occupied = true;
    }

    fn clear(&mut self) {
        self.bytes.fill(0);
        self.occupied = false;
        debug_assert!(self.storage_is_clear());
    }

    fn storage_is_clear(&self) -> bool {
        !self.occupied && self.bytes.iter().all(|byte| *byte == 0)
    }
}

/// A rejected submit that returns the complete validated body unchanged.
///
/// Dropping the rejection instead of recovering the body clears its owned byte
/// array. It does not and cannot clear the caller's original user mapping.
pub(crate) struct semantic_surface_submit_rejection {
    error: semantic_surface_error,
    snapshot: Option<surface_snapshot_body_v1>,
}

impl semantic_surface_submit_rejection {
    #[must_use]
    pub(crate) const fn error(&self) -> semantic_surface_error {
        self.error
    }

    #[must_use]
    pub(crate) fn snapshot(&self) -> &surface_snapshot_body_v1 {
        self.snapshot
            .as_ref()
            .expect("semantic-surface rejection lost its snapshot")
    }

    pub(crate) fn into_snapshot(mut self) -> surface_snapshot_body_v1 {
        self.snapshot
            .take()
            .expect("semantic-surface rejection lost its snapshot")
    }
}

impl Drop for semantic_surface_submit_rejection {
    fn drop(&mut self) {
        clear_owned_snapshot(&mut self.snapshot);
    }
}

/// Move-only submit preparation retaining the complete validated body.
///
/// Preparation never mutates the endpoint. Dropping this value clears its
/// kernel-owned body; committing it either installs all 528 bytes or returns
/// the same owner in a [`semantic_surface_submit_rejection`].
#[must_use = "a prepared semantic-surface submit must be committed or rejected"]
pub(crate) struct semantic_surface_prepared_submit {
    binding: semantic_surface_binding,
    phase: semantic_surface_phase,
    submission_committed: bool,
    snapshot: Option<surface_snapshot_body_v1>,
}

impl Drop for semantic_surface_prepared_submit {
    fn drop(&mut self) {
        clear_owned_snapshot(&mut self.snapshot);
    }
}

/// Move-only proof that one complete body now belongs to the endpoint.
pub(crate) struct semantic_surface_submit_receipt {
    binding: semantic_surface_binding,
}

impl semantic_surface_submit_receipt {
    /// Returns correlation only, never endpoint or presentation authority.
    #[must_use]
    pub(crate) const fn binding(&self) -> semantic_surface_binding {
        self.binding
    }
}

/// Frozen borrow token for one queued source body.
///
/// The token contains no bytes and no authority outside the endpoint owner.
/// [`Self::body`] revalidates the live endpoint before exposing the fixed body
/// to the one architecture copy path. Dropping the token leaves the source
/// slot intact.
#[must_use = "a prepared semantic-surface delivery must be committed or discarded"]
pub(crate) struct semantic_surface_prepared_delivery {
    binding: semantic_surface_binding,
    phase: semantic_surface_phase,
    submission_committed: bool,
}

impl semantic_surface_prepared_delivery {
    pub(crate) fn body<'a>(
        &self,
        endpoint: &'a semantic_surface_endpoint,
    ) -> Result<&'a [u8; surface_snapshot_v1_byte_count], semantic_surface_error> {
        endpoint.validate_prepared_delivery(self)?;
        Ok(&endpoint.slot.bytes)
    }

    /// Returns correlation only, never authority.
    #[must_use]
    pub(crate) const fn binding(&self) -> semantic_surface_binding {
        self.binding
    }
}

/// Move-only proof that one exact body was copied and consumed.
pub(crate) struct semantic_surface_delivery_receipt {
    binding: semantic_surface_binding,
}

impl semantic_surface_delivery_receipt {
    /// Returns correlation only. It proves no render completion, presentation,
    /// pixels, scanout, or semantic publication.
    #[must_use]
    pub(crate) const fn binding(&self) -> semantic_surface_binding {
        self.binding
    }
}

/// Result of an explicit endpoint close.
pub(crate) struct semantic_surface_close_receipt {
    first_close: bool,
    discarded: bool,
}

impl semantic_surface_close_receipt {
    #[must_use]
    pub(crate) const fn first_close(&self) -> bool {
        self.first_close
    }

    #[must_use]
    pub(crate) const fn discarded(&self) -> bool {
        self.discarded
    }
}

/// One fixed, single-delivery semantic-surface endpoint.
///
/// The endpoint can accept exactly one body during its lifetime. After a
/// successful delivery it is empty for polling purposes but remembers that
/// its one submission was spent, preventing replay of the same fixed
/// revision. Explicit close clears a queued body. Drop also clears as a
/// fail-safe, but callers should close before orderly retirement.
#[must_use = "explicitly close a semantic-surface endpoint before retirement"]
pub(crate) struct semantic_surface_endpoint {
    binding: semantic_surface_binding,
    phase: semantic_surface_phase,
    submission_committed: bool,
    slot: semantic_surface_slot,
}

impl semantic_surface_endpoint {
    pub(crate) const fn new(binding: semantic_surface_binding) -> Self {
        Self {
            binding,
            phase: semantic_surface_phase::empty,
            submission_committed: false,
            slot: semantic_surface_slot::empty(),
        }
    }

    /// Returns freshness correlation only, never submit or read authority.
    #[must_use]
    pub(crate) const fn binding(&self) -> semantic_surface_binding {
        self.binding
    }

    /// Reports only whether the complete mutable body slot is logically clear.
    ///
    /// This is bounded teardown evidence, not secure-erasure evidence, and it
    /// intentionally exposes neither phase nor body content.
    #[must_use]
    pub(crate) fn storage_is_clear(&self) -> bool {
        self.slot.storage_is_clear()
    }

    /// Validates one complete body and freezes its exact empty-slot commit.
    ///
    /// The body is already a validated core-codec owner; the endpoint checks
    /// every trusted correlation field independently. All rejection paths
    /// return the exact owner without changing endpoint state.
    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_submit(
        &self,
        binding: semantic_surface_binding,
        snapshot: surface_snapshot_body_v1,
    ) -> Result<semantic_surface_prepared_submit, semantic_surface_submit_rejection> {
        let reject = |error, snapshot| semantic_surface_submit_rejection {
            error,
            snapshot: Some(snapshot),
        };
        if let Err(error) = self.validate_binding(binding) {
            return Err(reject(error, snapshot));
        }
        if let Err(error) = self.require_invariants() {
            return Err(reject(error, snapshot));
        }
        if let Err(error) = self.validate_snapshot_binding(snapshot.binding()) {
            return Err(reject(error, snapshot));
        }
        match self.phase {
            semantic_surface_phase::closed => {
                return Err(reject(semantic_surface_error::endpoint_closed, snapshot));
            }
            semantic_surface_phase::queued => {
                return Err(reject(semantic_surface_error::endpoint_full, snapshot));
            }
            semantic_surface_phase::empty => {}
        }
        if self.submission_committed {
            return Err(reject(
                semantic_surface_error::submission_replayed,
                snapshot,
            ));
        }

        Ok(semantic_surface_prepared_submit {
            binding: self.binding,
            phase: self.phase,
            submission_committed: self.submission_committed,
            snapshot: Some(snapshot),
        })
    }

    /// Atomically transfers all 528 bytes into the endpoint.
    ///
    /// A stale preparation, intervening close, or competing commit returns the
    /// exact body owner. No rejection overwrites or clears the queued slot.
    #[allow(clippy::result_large_err)]
    pub(crate) fn commit_submit(
        &mut self,
        mut prepared: semantic_surface_prepared_submit,
    ) -> Result<semantic_surface_submit_receipt, semantic_surface_submit_rejection> {
        let error = if let Err(error) = self.validate_binding(prepared.binding) {
            Some(error)
        } else if self.require_invariants().is_err() {
            Some(semantic_surface_error::invariant_violation)
        } else if self.phase != prepared.phase
            || self.submission_committed != prepared.submission_committed
        {
            Some(semantic_surface_error::stale_preparation)
        } else if self.phase == semantic_surface_phase::closed {
            Some(semantic_surface_error::endpoint_closed)
        } else if self.phase == semantic_surface_phase::queued {
            Some(semantic_surface_error::endpoint_full)
        } else if self.submission_committed {
            Some(semantic_surface_error::submission_replayed)
        } else {
            let snapshot = prepared
                .snapshot
                .as_ref()
                .expect("prepared semantic-surface submit lost its snapshot");
            self.validate_snapshot_binding(snapshot.binding()).err()
        };
        if let Some(error) = error {
            return Err(semantic_surface_submit_rejection {
                error,
                snapshot: prepared.snapshot.take(),
            });
        }

        let snapshot = prepared
            .snapshot
            .take()
            .expect("validated semantic-surface submit lost its snapshot");
        self.slot.install(snapshot);
        self.submission_committed = true;
        self.phase = semantic_surface_phase::queued;
        debug_assert!(self.invariants_hold());
        Ok(semantic_surface_submit_receipt {
            binding: self.binding,
        })
    }

    /// Freezes one queued body without copying or consuming it.
    pub(crate) fn prepare_delivery(
        &self,
        binding: semantic_surface_binding,
    ) -> Result<semantic_surface_prepared_delivery, semantic_surface_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        match self.phase {
            semantic_surface_phase::closed => {
                return Err(semantic_surface_error::endpoint_closed);
            }
            semantic_surface_phase::empty => {
                return Err(semantic_surface_error::endpoint_empty);
            }
            semantic_surface_phase::queued => {}
        }
        Ok(semantic_surface_prepared_delivery {
            binding: self.binding,
            phase: self.phase,
            submission_committed: self.submission_committed,
        })
    }

    /// Consumes and clears a fully copied source body.
    ///
    /// The architecture path must perform every fallible destination and copy
    /// check through [`semantic_surface_prepared_delivery::body`] before this
    /// call while holding serialized exclusion. A changed endpoint after that
    /// selection is a kernel integration defect and fails stop.
    pub(crate) fn commit_prepared_delivery(
        &mut self,
        prepared: semantic_surface_prepared_delivery,
    ) -> semantic_surface_delivery_receipt {
        self.validate_prepared_delivery(&prepared)
            .expect("prepared semantic-surface delivery changed after selection");
        self.slot.clear();
        self.phase = semantic_surface_phase::empty;
        debug_assert!(self.invariants_hold());
        semantic_surface_delivery_receipt {
            binding: self.binding,
        }
    }

    /// Closes this exact endpoint and clears any queued body.
    ///
    /// Close is idempotent for the same complete binding. A foreign or stale
    /// binding rejects before private state inspection or clearing.
    pub(crate) fn close(
        &mut self,
        binding: semantic_surface_binding,
    ) -> Result<semantic_surface_close_receipt, semantic_surface_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        if self.phase == semantic_surface_phase::closed {
            return Ok(semantic_surface_close_receipt {
                first_close: false,
                discarded: false,
            });
        }
        let discarded = self.phase == semantic_surface_phase::queued;
        self.slot.clear();
        self.phase = semantic_surface_phase::closed;
        debug_assert!(self.invariants_hold());
        Ok(semantic_surface_close_receipt {
            first_close: true,
            discarded,
        })
    }

    fn validate_binding(
        &self,
        candidate: semantic_surface_binding,
    ) -> Result<(), semantic_surface_error> {
        if candidate.endpoint != self.binding.endpoint {
            return Err(semantic_surface_error::wrong_endpoint);
        }
        if candidate.source != self.binding.source {
            return Err(semantic_surface_error::wrong_source);
        }
        if candidate.consumer != self.binding.consumer {
            return Err(semantic_surface_error::wrong_consumer);
        }
        self.validate_snapshot_binding(candidate.snapshot)
    }

    fn validate_snapshot_binding(
        &self,
        candidate: surface_snapshot_binding,
    ) -> Result<(), semantic_surface_error> {
        let expected = self.binding.snapshot;
        if candidate.surface() != expected.surface() {
            return Err(semantic_surface_error::wrong_surface);
        }
        if candidate.scene() != expected.scene() {
            return Err(semantic_surface_error::wrong_scene);
        }
        if candidate.base_revision() != expected.base_revision()
            || candidate.revision() != expected.revision()
        {
            return Err(semantic_surface_error::stale_revision);
        }
        if candidate.renderer() != expected.renderer() {
            return Err(semantic_surface_error::wrong_renderer);
        }
        if candidate.attempt() != expected.attempt() {
            return Err(semantic_surface_error::stale_attempt);
        }
        Ok(())
    }

    fn validate_prepared_delivery(
        &self,
        prepared: &semantic_surface_prepared_delivery,
    ) -> Result<(), semantic_surface_error> {
        self.validate_binding(prepared.binding)?;
        self.require_invariants()?;
        if prepared.phase != semantic_surface_phase::queued
            || self.phase != prepared.phase
            || !prepared.submission_committed
            || self.submission_committed != prepared.submission_committed
            || !self.slot.occupied
        {
            return Err(semantic_surface_error::stale_preparation);
        }
        Ok(())
    }

    fn require_invariants(&self) -> Result<(), semantic_surface_error> {
        if self.invariants_hold() {
            Ok(())
        } else {
            Err(semantic_surface_error::invariant_violation)
        }
    }

    fn invariants_hold(&self) -> bool {
        match self.phase {
            semantic_surface_phase::empty => !self.slot.occupied && self.slot.storage_is_clear(),
            semantic_surface_phase::queued => {
                self.submission_committed
                    && self.slot.occupied
                    && !self.slot.bytes.iter().all(|byte| *byte == 0)
            }
            semantic_surface_phase::closed => !self.slot.occupied && self.slot.storage_is_clear(),
        }
    }

    #[cfg(test)]
    fn test_snapshot(&self) -> semantic_surface_endpoint_test_snapshot {
        semantic_surface_endpoint_test_snapshot {
            binding: self.binding,
            phase: self.phase,
            submission_committed: self.submission_committed,
            occupied: self.slot.occupied,
            bytes: self.slot.bytes,
        }
    }
}

impl Drop for semantic_surface_endpoint {
    fn drop(&mut self) {
        self.slot.clear();
        self.phase = semantic_surface_phase::closed;
    }
}

fn clear_owned_snapshot(snapshot: &mut Option<surface_snapshot_body_v1>) {
    if let Some(snapshot) = snapshot.take() {
        let mut bytes = snapshot.into_bytes();
        bytes.fill(0);
        debug_assert!(bytes.iter().all(|byte| *byte == 0));
    }
}

#[cfg(test)]
#[derive(Eq, PartialEq)]
struct semantic_surface_endpoint_test_snapshot {
    binding: semantic_surface_binding,
    phase: semantic_surface_phase,
    submission_committed: bool,
    occupied: bool,
    bytes: [u8; surface_snapshot_v1_byte_count],
}

#[cfg(test)]
impl Drop for semantic_surface_endpoint_test_snapshot {
    fn drop(&mut self) {
        self.bytes.fill(0);
        self.occupied = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::circle_table;
    use crate::identity::{circle_id, fixture_principal_id, task_id};
    use crate::rite::{copy_bounded_body_bytes, rite_channel, rite_decision};
    use crate::task::{
        synthetic_origin_metadata, task_binding, task_bounds, task_table, task_table_test_snapshot,
    };
    use anomalous_abi::{id_128, rite_call, rite_operation, rite_payload_kind, rite_rights};
    use anomalous_desktop_transport::{
        surface_snapshot_v1, transport_color, transport_command_kind, transport_depth,
        transport_disclosure, transport_focus, transport_identity, transport_label_selector,
        transport_motion, transport_rect, transport_render_command, transport_scene_state,
        transport_shape, transport_slot,
    };

    const surface_slot: u64 = 11;
    const surface_generation: u64 = 12;
    const scene_slot: u64 = 21;
    const scene_generation: u64 = 22;
    const base_revision: u64 = 31;
    const revision: u64 = 32;
    const renderer_slot: u64 = 41;
    const renderer_generation: u64 = 42;
    const attempt: u64 = 1;

    #[derive(Eq, PartialEq)]
    struct source_copy_state {
        endpoint: semantic_surface_endpoint_test_snapshot,
        channel: crate::rite::rite_channel_test_snapshot,
        tasks: task_table_test_snapshot,
        binding: semantic_surface_binding,
    }

    fn source_copy_fixture() -> (circle_table, task_table, task_identity) {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("source circle");
        let bounds = task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000)
            .expect("source bounds");
        let mut tasks = task_table::new();
        let source = tasks
            .create(
                &mut circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(1).expect("source principal"),
                    synthetic_origin_metadata::human_role_fixture,
                    bounds,
                ),
            )
            .expect("source task");
        tasks.start(source).expect("source running");
        (circles, tasks, source)
    }

    fn model_source_body_copy(
        source: &[u8; surface_snapshot_v1_byte_count],
        scratch: &mut [u8; surface_snapshot_v1_byte_count],
        copy_count: &mut usize,
        bytes_copied: &mut usize,
        failure_at: Option<usize>,
    ) -> Result<(), &'static str> {
        if scratch.iter().any(|byte| *byte != 0) {
            return Err("semantic_surface_source_scratch_not_clear");
        }
        let next_copy_count = copy_count
            .checked_add(1)
            .ok_or("rite_body_copy_count_overflow")?;
        let next_bytes_copied = bytes_copied
            .checked_add(source.len())
            .ok_or("rite_body_byte_count_overflow")?;
        if let Err(reason) = copy_bounded_body_bytes(scratch, |index| {
            if failure_at == Some(index) {
                Err("injected_semantic_surface_source_copy_failure")
            } else {
                Ok(source[index])
            }
        }) {
            scratch.fill(0);
            return Err(reason);
        }
        *copy_count = next_copy_count;
        *bytes_copied = next_bytes_copied;
        Ok(())
    }

    fn transport_id(slot: u64, generation: u64) -> transport_identity {
        transport_identity::try_new(slot, generation).expect("transport identity")
    }

    fn snapshot_binding(
        surface: (u64, u64),
        scene: (u64, u64),
        revisions: (u64, u64),
        renderer: (u64, u64),
        delivery_attempt: u64,
    ) -> surface_snapshot_binding {
        surface_snapshot_binding::try_new(
            transport_id(surface.0, surface.1),
            transport_id(scene.0, scene.1),
            revisions.0,
            revisions.1,
            transport_id(renderer.0, renderer.1),
            delivery_attempt,
        )
        .expect("snapshot binding")
    }

    fn expected_snapshot_binding() -> surface_snapshot_binding {
        snapshot_binding(
            (surface_slot, surface_generation),
            (scene_slot, scene_generation),
            (base_revision, revision),
            (renderer_slot, renderer_generation),
            attempt,
        )
    }

    fn task(
        task_slot: usize,
        task_generation: u64,
        circle_slot: usize,
        circle_generation: u64,
    ) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(task_slot, task_generation).expect("task identity"),
            circle_id::try_from_parts(circle_slot, circle_generation).expect("circle identity"),
        )
    }

    fn source() -> task_identity {
        task(1, 2, 3, 4)
    }

    fn consumer() -> task_identity {
        task(5, 6, 7, 8)
    }

    fn endpoint_identity(generation: u64) -> semantic_surface_endpoint_identity {
        semantic_surface_endpoint_identity::try_new(9, generation).expect("endpoint identity")
    }

    fn binding_with(
        endpoint: semantic_surface_endpoint_identity,
        source: task_identity,
        consumer: task_identity,
        snapshot: surface_snapshot_binding,
    ) -> semantic_surface_binding {
        semantic_surface_binding::try_new(endpoint, source, consumer, snapshot)
            .expect("semantic-surface binding")
    }

    fn binding() -> semantic_surface_binding {
        binding_with(
            endpoint_identity(10),
            source(),
            consumer(),
            expected_snapshot_binding(),
        )
    }

    fn viewport() -> transport_rect {
        transport_rect::try_new(0, 0, 1_024, 768).expect("fixed viewport")
    }

    fn fill_command() -> transport_render_command {
        transport_render_command::new(
            transport_command_kind::fill,
            transport_shape::rectangle,
            transport_color::canvas,
            None,
            transport_slot::root,
            true,
            viewport(),
        )
    }

    fn state() -> transport_scene_state {
        transport_scene_state::try_new(
            false,
            false,
            transport_focus::none,
            transport_disclosure::declared_public,
            transport_depth::standard,
            transport_motion::standard,
        )
        .expect("fixed scene state")
    }

    fn snapshot_for(binding: surface_snapshot_binding) -> surface_snapshot_body_v1 {
        let labels = [
            transport_label_selector::anomalous_desktop,
            transport_label_selector::anomalous,
            transport_label_selector::navigation,
            transport_label_selector::open_shell,
            transport_label_selector::inspect_system,
            transport_label_selector::workspace,
            transport_label_selector::system_workspace,
            transport_label_selector::system_ready,
        ];
        let commands = [fill_command(); 25];
        surface_snapshot_v1::try_encode(binding, state(), viewport(), labels, &commands)
            .expect("fixed snapshot")
            .into_body()
    }

    fn snapshot() -> surface_snapshot_body_v1 {
        snapshot_for(expected_snapshot_binding())
    }

    fn expect_prepared_submit(
        result: Result<semantic_surface_prepared_submit, semantic_surface_submit_rejection>,
    ) -> semantic_surface_prepared_submit {
        match result {
            Ok(prepared) => prepared,
            Err(rejection) => panic!("unexpected submit rejection: {}", rejection.error().code()),
        }
    }

    fn expect_submit_receipt(
        result: Result<semantic_surface_submit_receipt, semantic_surface_submit_rejection>,
    ) -> semantic_surface_submit_receipt {
        match result {
            Ok(receipt) => receipt,
            Err(rejection) => panic!("unexpected submit rejection: {}", rejection.error().code()),
        }
    }

    fn expect_submit_rejection(
        result: Result<semantic_surface_prepared_submit, semantic_surface_submit_rejection>,
    ) -> semantic_surface_submit_rejection {
        match result {
            Ok(prepared) => {
                drop(prepared);
                panic!("expected submit rejection")
            }
            Err(rejection) => rejection,
        }
    }

    fn expect_commit_rejection(
        result: Result<semantic_surface_submit_receipt, semantic_surface_submit_rejection>,
    ) -> semantic_surface_submit_rejection {
        match result {
            Ok(receipt) => {
                drop(receipt);
                panic!("expected commit rejection")
            }
            Err(rejection) => rejection,
        }
    }

    fn submit(endpoint: &mut semantic_surface_endpoint) -> semantic_surface_submit_receipt {
        let prepared = expect_prepared_submit(endpoint.prepare_submit(binding(), snapshot()));
        expect_submit_receipt(endpoint.commit_submit(prepared))
    }

    #[test]
    fn endpoint_generation_is_nonzero_and_advances_without_wrapping() {
        assert!(matches!(
            semantic_surface_endpoint_identity::try_new(4, 0),
            Err(semantic_surface_error::zero_endpoint_generation)
        ));

        let current = semantic_surface_endpoint_identity::try_new(4, 7).expect("identity");
        let next = current.next_generation().expect("next generation");
        assert!(current.slot() == 4);
        assert!(current.generation() == 7);
        assert!(current.kernel_identity().generation_value() == 7);
        assert!(next.slot() == 4);
        assert!(next.generation() == 8);

        let exhausted =
            semantic_surface_endpoint_identity::try_new(4, u64::MAX).expect("maximum generation");
        assert!(matches!(
            exhausted.next_generation(),
            Err(semantic_surface_error::endpoint_generation_exhausted)
        ));
        assert!(exhausted.generation() == u64::MAX);
    }

    #[test]
    fn binding_requires_distinct_complete_task_incarnations() {
        assert!(matches!(
            semantic_surface_binding::try_new(
                endpoint_identity(10),
                source(),
                source(),
                expected_snapshot_binding(),
            ),
            Err(semantic_surface_error::source_is_consumer)
        ));

        let exact = binding();
        assert!(exact.endpoint() == endpoint_identity(10));
        assert!(exact.source() == source());
        assert!(exact.consumer() == consumer());
        assert!(exact.snapshot() == expected_snapshot_binding());
    }

    #[test]
    fn empty_delivery_rejects_without_mutation() {
        let endpoint = semantic_surface_endpoint::new(binding());
        let before = endpoint.test_snapshot();
        assert!(matches!(
            endpoint.prepare_delivery(binding()),
            Err(semantic_surface_error::endpoint_empty)
        ));
        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn every_endpoint_and_task_incarnation_field_is_checked_first() {
        let endpoint = semantic_surface_endpoint::new(binding());
        let cases = [
            (
                binding_with(
                    endpoint_identity(11),
                    source(),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_endpoint,
            ),
            (
                binding_with(
                    semantic_surface_endpoint_identity::try_new(8, 10).expect("foreign slot"),
                    source(),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_endpoint,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    task(2, 2, 3, 4),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_source,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    task(1, 3, 3, 4),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_source,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    task(1, 2, 4, 4),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_source,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    task(1, 2, 3, 5),
                    consumer(),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_source,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    source(),
                    task(6, 6, 7, 8),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_consumer,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    source(),
                    task(5, 7, 7, 8),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_consumer,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    source(),
                    task(5, 6, 8, 8),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_consumer,
            ),
            (
                binding_with(
                    endpoint_identity(10),
                    source(),
                    task(5, 6, 7, 9),
                    expected_snapshot_binding(),
                ),
                semantic_surface_error::wrong_consumer,
            ),
        ];

        for (candidate, expected) in cases {
            let before = endpoint.test_snapshot();
            let rejection = expect_submit_rejection(endpoint.prepare_submit(candidate, snapshot()));
            assert!(rejection.error() == expected);
            assert!(rejection.snapshot().as_bytes() == snapshot().as_bytes());
            assert!(before == endpoint.test_snapshot());
        }
    }

    #[test]
    fn every_snapshot_correlation_component_is_checked_independently() {
        let endpoint = semantic_surface_endpoint::new(binding());
        let cases = [
            (
                snapshot_binding(
                    (surface_slot + 1, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_surface,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation + 1),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_surface,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot + 1, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_scene,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation + 1),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_scene,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision + 1, revision + 1),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::stale_revision,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot + 1, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_renderer,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation + 1),
                    attempt,
                ),
                semantic_surface_error::wrong_renderer,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    2,
                ),
                semantic_surface_error::stale_attempt,
            ),
        ];

        for (foreign, expected) in cases {
            let before = endpoint.test_snapshot();
            let rejection =
                expect_submit_rejection(endpoint.prepare_submit(binding(), snapshot_for(foreign)));
            assert!(rejection.error() == expected);
            assert!(rejection.snapshot().binding() == foreign);
            assert!(before == endpoint.test_snapshot());
        }
    }

    #[test]
    fn candidate_binding_checks_every_snapshot_component_before_state() {
        let endpoint = semantic_surface_endpoint::new(binding());
        let cases = [
            (
                snapshot_binding(
                    (surface_slot + 1, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_surface,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot + 1, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_scene,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision + 1, revision + 1),
                    (renderer_slot, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::stale_revision,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot + 1, renderer_generation),
                    attempt,
                ),
                semantic_surface_error::wrong_renderer,
            ),
            (
                snapshot_binding(
                    (surface_slot, surface_generation),
                    (scene_slot, scene_generation),
                    (base_revision, revision),
                    (renderer_slot, renderer_generation),
                    2,
                ),
                semantic_surface_error::stale_attempt,
            ),
        ];

        for (foreign, expected) in cases {
            let candidate = binding_with(endpoint_identity(10), source(), consumer(), foreign);
            assert!(
                matches!(endpoint.prepare_delivery(candidate), Err(error) if error == expected)
            );
        }
    }

    #[test]
    fn submit_commit_moves_all_bytes_and_returns_exact_correlation() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        let expected = snapshot().into_bytes();
        let prepared = expect_prepared_submit(endpoint.prepare_submit(binding(), snapshot()));
        assert!(endpoint.phase == semantic_surface_phase::empty);
        assert!(endpoint.slot.storage_is_clear());

        let receipt = expect_submit_receipt(endpoint.commit_submit(prepared));
        assert!(receipt.binding() == binding());
        assert!(endpoint.binding() == binding());
        assert!(endpoint.phase == semantic_surface_phase::queued);
        assert!(endpoint.submission_committed);
        assert!(endpoint.slot.occupied);
        assert!(endpoint.slot.bytes == expected);
    }

    #[test]
    fn full_backpressure_returns_second_body_without_touching_first() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let before = endpoint.test_snapshot();
        let second = snapshot();
        let expected_second = snapshot().into_bytes();

        let rejection = expect_submit_rejection(endpoint.prepare_submit(binding(), second));
        assert!(rejection.error() == semantic_surface_error::endpoint_full);
        assert!(rejection.snapshot().as_bytes() == &expected_second);
        let returned = rejection.into_snapshot();
        assert!(returned.as_bytes() == &expected_second);
        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn competing_prepared_submit_cannot_overwrite_committed_body() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        let first = expect_prepared_submit(endpoint.prepare_submit(binding(), snapshot()));
        let second = expect_prepared_submit(endpoint.prepare_submit(binding(), snapshot()));
        drop(expect_submit_receipt(endpoint.commit_submit(first)));
        let before = endpoint.test_snapshot();

        let rejection = expect_commit_rejection(endpoint.commit_submit(second));
        assert!(rejection.error() == semantic_surface_error::stale_preparation);
        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn prepared_delivery_borrows_without_consuming_and_drop_preserves_slot() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        let expected = snapshot().into_bytes();
        drop(submit(&mut endpoint));
        let before = endpoint.test_snapshot();

        let prepared = endpoint
            .prepare_delivery(binding())
            .expect("prepare delivery");
        assert!(prepared.binding() == binding());
        assert!(prepared.body(&endpoint).expect("borrow body") == &expected);
        drop(prepared);

        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn delivery_consumes_once_and_clears_the_complete_slot() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let prepared = endpoint
            .prepare_delivery(binding())
            .expect("prepare delivery");
        let expected = snapshot().into_bytes();
        assert!(prepared.body(&endpoint).expect("body") == &expected);

        let receipt = endpoint.commit_prepared_delivery(prepared);
        assert!(receipt.binding() == binding());
        assert!(endpoint.phase == semantic_surface_phase::empty);
        assert!(endpoint.submission_committed);
        assert!(endpoint.slot.storage_is_clear());
        assert!(endpoint.slot.bytes.iter().all(|byte| *byte == 0));
        assert!(matches!(
            endpoint.prepare_delivery(binding()),
            Err(semantic_surface_error::endpoint_empty)
        ));
    }

    #[test]
    fn stale_delivery_preparation_cannot_borrow_after_another_delivery() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let first = endpoint
            .prepare_delivery(binding())
            .expect("first preparation");
        let replay = endpoint
            .prepare_delivery(binding())
            .expect("second preparation");
        drop(endpoint.commit_prepared_delivery(first));

        assert!(matches!(
            replay.body(&endpoint),
            Err(semantic_surface_error::stale_preparation)
        ));
        assert!(endpoint.slot.storage_is_clear());
    }

    #[test]
    fn successful_delivery_permanently_rejects_submit_replay() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let prepared = endpoint
            .prepare_delivery(binding())
            .expect("prepare delivery");
        drop(endpoint.commit_prepared_delivery(prepared));
        let before = endpoint.test_snapshot();

        let rejection = expect_submit_rejection(endpoint.prepare_submit(binding(), snapshot()));
        assert!(rejection.error() == semantic_surface_error::submission_replayed);
        assert!(rejection.snapshot().binding() == expected_snapshot_binding());
        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn close_clears_a_queued_body_and_is_idempotent() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));

        let receipt = endpoint.close(binding()).expect("close endpoint");
        assert!(receipt.first_close());
        assert!(receipt.discarded());
        assert!(endpoint.phase == semantic_surface_phase::closed);
        assert!(endpoint.storage_is_clear());
        assert!(endpoint.slot.bytes.iter().all(|byte| *byte == 0));

        let repeated = endpoint.close(binding()).expect("repeat close");
        assert!(!repeated.first_close());
        assert!(!repeated.discarded());
        assert!(matches!(
            endpoint.prepare_delivery(binding()),
            Err(semantic_surface_error::endpoint_closed)
        ));
        let rejection = expect_submit_rejection(endpoint.prepare_submit(binding(), snapshot()));
        assert!(rejection.error() == semantic_surface_error::endpoint_closed);
    }

    #[test]
    fn foreign_close_cannot_clear_the_queued_body() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let before = endpoint.test_snapshot();
        let foreign = binding_with(
            endpoint_identity(11),
            source(),
            consumer(),
            expected_snapshot_binding(),
        );

        assert!(matches!(
            endpoint.close(foreign),
            Err(semantic_surface_error::wrong_endpoint)
        ));
        assert!(before == endpoint.test_snapshot());
    }

    #[test]
    fn close_of_never_used_or_delivered_endpoint_reports_no_discard() {
        let mut unused = semantic_surface_endpoint::new(binding());
        let unused_receipt = unused.close(binding()).expect("close unused");
        assert!(unused_receipt.first_close());
        assert!(!unused_receipt.discarded());

        let mut delivered = semantic_surface_endpoint::new(binding());
        drop(submit(&mut delivered));
        let prepared = delivered
            .prepare_delivery(binding())
            .expect("prepare delivery");
        drop(delivered.commit_prepared_delivery(prepared));
        let delivered_receipt = delivered.close(binding()).expect("close delivered");
        assert!(delivered_receipt.first_close());
        assert!(!delivered_receipt.discarded());
    }

    #[test]
    fn prepared_delivery_is_invalidated_by_close_without_disclosure() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        drop(submit(&mut endpoint));
        let prepared = endpoint
            .prepare_delivery(binding())
            .expect("prepare delivery");
        let receipt = endpoint.close(binding()).expect("close");
        assert!(receipt.discarded());

        assert!(matches!(
            prepared.body(&endpoint),
            Err(semantic_surface_error::stale_preparation)
        ));
        assert!(endpoint.slot.storage_is_clear());
    }

    #[test]
    fn endpoint_retirement_rejects_equal_public_values_under_old_generation() {
        let old_binding = binding();
        let mut retired = semantic_surface_endpoint::new(old_binding);
        assert!(
            retired
                .close(old_binding)
                .expect("retire old")
                .first_close()
        );

        let replacement_identity = old_binding
            .endpoint()
            .next_generation()
            .expect("replacement generation");
        let replacement_binding = binding_with(
            replacement_identity,
            source(),
            consumer(),
            expected_snapshot_binding(),
        );
        let replacement = semantic_surface_endpoint::new(replacement_binding);
        let rejection =
            expect_submit_rejection(replacement.prepare_submit(old_binding, snapshot()));
        assert!(rejection.error() == semantic_surface_error::wrong_endpoint);
        assert!(replacement.phase == semantic_surface_phase::empty);
        assert!(replacement.slot.storage_is_clear());
    }

    #[test]
    fn invariant_failure_is_atomic_and_returns_submit_owner() {
        let mut endpoint = semantic_surface_endpoint::new(binding());
        endpoint.slot.bytes[17] = 0x5a;
        let before = endpoint.test_snapshot();

        let rejection = expect_submit_rejection(endpoint.prepare_submit(binding(), snapshot()));
        assert!(rejection.error() == semantic_surface_error::invariant_violation);
        assert!(rejection.snapshot().binding() == expected_snapshot_binding());
        assert!(before == endpoint.test_snapshot());

        endpoint.slot.clear();
    }

    #[test]
    fn every_source_copy_fault_leaves_precommit_authority_unchanged_and_unpublished() {
        let (_circles, tasks, source) = source_copy_fixture();
        let binding = semantic_surface_binding::try_new(
            endpoint_identity(10),
            source,
            consumer(),
            expected_snapshot_binding(),
        )
        .expect("source binding");
        let body = snapshot().into_bytes();
        let request = rite_call::new(
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            surface_snapshot_v1_byte_count as u32,
            surface_snapshot_v1_byte_count as u32,
            id_128::from_bytes([0x61; 16]),
        );

        let admitted_channel = {
            let mut channel = rite_channel::try_new(source, rite_rights::semantic_surface_submit)
                .expect("source channel");
            let admission = match channel.assess(&tasks, source, request).expect("admission") {
                rite_decision::admitted(admission) => admission,
                rite_decision::rejected(_) => panic!("source submit rejected"),
            };
            // A production source-mapping fault is fail-stop, so no completion
            // or drop path runs. Capture the exact post-admission baseline.
            core::mem::forget(admission);
            channel.test_snapshot()
        };

        for failure_at in 0..surface_snapshot_v1_byte_count {
            let endpoint = semantic_surface_endpoint::new(binding);
            let mut channel = rite_channel::try_new(source, rite_rights::semantic_surface_submit)
                .expect("source channel");
            let admission = match channel.assess(&tasks, source, request).expect("admission") {
                rite_decision::admitted(admission) => admission,
                rite_decision::rejected(_) => panic!("source submit rejected"),
            };
            let before = source_copy_state {
                endpoint: endpoint.test_snapshot(),
                channel: admitted_channel,
                tasks: tasks.test_snapshot(),
                binding,
            };
            let mut scratch = [0_u8; surface_snapshot_v1_byte_count];
            let mut copy_count = 3;
            let mut bytes_copied = 31;
            let copy_count_before = copy_count;
            let bytes_copied_before = bytes_copied;
            let mailbox = [0x7b_u8; 40 + surface_snapshot_v1_byte_count];
            let mailbox_before = mailbox;

            assert_eq!(
                model_source_body_copy(
                    &body,
                    &mut scratch,
                    &mut copy_count,
                    &mut bytes_copied,
                    Some(failure_at),
                ),
                Err("injected_semantic_surface_source_copy_failure")
            );
            assert!(scratch.iter().all(|byte| *byte == 0));
            assert_eq!(copy_count, copy_count_before);
            assert_eq!(bytes_copied, bytes_copied_before);
            assert_eq!(
                mailbox, mailbox_before,
                "no reply publication at {failure_at}"
            );

            // No parse, endpoint preparation, channel settlement, or reply
            // publication follows the injected source-copy fault.
            core::mem::forget(admission);
            let after = source_copy_state {
                endpoint: endpoint.test_snapshot(),
                channel: channel.test_snapshot(),
                tasks: tasks.test_snapshot(),
                binding,
            };
            assert!(before == after, "source copy fault at {failure_at}");
        }
    }

    #[test]
    fn source_copy_accounting_overflow_precedes_any_source_or_scratch_mutation() {
        let source = snapshot().into_bytes();
        for (copy_count, bytes_copied, expected) in [
            (usize::MAX, 0, "rite_body_copy_count_overflow"),
            (
                0,
                usize::MAX - (surface_snapshot_v1_byte_count - 1),
                "rite_body_byte_count_overflow",
            ),
        ] {
            let mut scratch = [0_u8; surface_snapshot_v1_byte_count];
            let mut copies = copy_count;
            let mut bytes = bytes_copied;

            assert_eq!(
                model_source_body_copy(&source, &mut scratch, &mut copies, &mut bytes, None),
                Err(expected)
            );
            assert!(scratch.iter().all(|byte| *byte == 0));
            assert_eq!(copies, copy_count);
            assert_eq!(bytes, bytes_copied);
        }
    }
}
