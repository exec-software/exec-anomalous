#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Host-only bounded recovery around the real transactional desktop service.
//!
//! A surviving coordinator retains every publication owner and one immutable
//! redacted redraw candidate. Beginning external presentation closes its sole
//! semantic read facade before any trusted presenter may mutate pixels. Only
//! complete-copy assertion, semantic commit, and exact finalization reopen it.
//! One explicitly authorized presenter replacement invalidates old local
//! owners and requires a complete redraw, including after semantic commit.
//!
//! The presenter is trusted: this crate cannot inspect pixels, authenticate a
//! process, establish scanout, revoke external device access, or survive loss
//! of the coordinator. Tickets and endpoints are private local Rust authority,
//! never a wire capability. Existing observations cannot be recalled, and
//! dropping this coordinator does not clear an external display. Construction
//! preserves the service's trusted assertion that its initial scene is already
//! presented. Retirement reports local retirement, not display withdrawal.
#![doc = include_str!("privacy_compile_fail.md")]

use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use anomalous_desktop_scene::{scene_candidate, scene_revision};
use anomalous_desktop_service::{
    desktop_surface, prepared_presentation, presentation_control, presentation_receipt,
    presented_scene_view, publication_receipt, publication_sequence, queued_revision, render_lease,
    render_snapshot_view, renderer_endpoint, renderer_identity, surface_control, surface_error,
    surface_identity, surface_retirement,
};
use anomalous_desktop_transport::{
    surface_snapshot_v1, transport_error, try_encode_service_snapshot,
};

static next_recovery_instance: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Eq, PartialEq)]
struct recovery_instance(u64);

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum recovery_error {
    instance_exhausted,
    control_denied,
    presenter_denied,
    stale_ticket,
    wrong_phase,
    observation_unavailable,
    replacement_limit_reached,
    presenter_generation_exhausted,
    transaction_exhausted,
    service(surface_error),
    transport(transport_error),
    invariant_violation,
}

impl recovery_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::instance_exhausted => "instance_exhausted",
            Self::control_denied => "control_denied",
            Self::presenter_denied => "presenter_denied",
            Self::stale_ticket => "stale_ticket",
            Self::wrong_phase => "wrong_phase",
            Self::observation_unavailable => "observation_unavailable",
            Self::replacement_limit_reached => "replacement_limit_reached",
            Self::presenter_generation_exhausted => "presenter_generation_exhausted",
            Self::transaction_exhausted => "transaction_exhausted",
            Self::service(error) => error.code(),
            Self::transport(error) => error.code(),
            Self::invariant_violation => "invariant_violation",
        }
    }
}

impl fmt::Debug for recovery_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for recovery_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Content-free local lifecycle; phase alone grants no presentation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum recovery_phase {
    steady,
    queued,
    leased,
    ready,
    presenting,
    copied,
    published,
}

/// Descriptive local status. Committed revision is not an available read view.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct recovery_status {
    phase: recovery_phase,
    revision: scene_revision,
    sequence: publication_sequence,
    copy_confirmed: bool,
}

impl recovery_status {
    #[must_use]
    pub const fn phase(self) -> recovery_phase {
        self.phase
    }

    #[must_use]
    pub const fn committed_revision(self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub const fn publication_sequence(self) -> publication_sequence {
        self.sequence
    }

    #[must_use]
    pub const fn copy_confirmed(self) -> bool {
        self.copy_confirmed
    }
}

/// Separate move-only authority for one replacement and explicit retirement.
pub struct recovery_control {
    instance: recovery_instance,
}

/// Move-only trusted presenter authority for one coordinator incarnation.
pub struct presenter_endpoint {
    instance: recovery_instance,
    generation: u64,
}

/// Move-only authority for one exact transaction and presenter generation.
/// Dropping it never reopens reads or abandons the retained candidate.
#[must_use = "finish this ticket or explicitly replace the presenter"]
pub struct presentation_ticket {
    instance: recovery_instance,
    presenter_generation: u64,
    transaction: u64,
}

pub struct construction_rejection {
    error: recovery_error,
    candidate: scene_candidate,
}

impl construction_rejection {
    #[must_use]
    pub const fn error(&self) -> recovery_error {
        self.error
    }

    #[must_use]
    pub fn candidate(&self) -> &scene_candidate {
        &self.candidate
    }

    #[must_use]
    pub fn into_candidate(self) -> scene_candidate {
        self.candidate
    }
}

pub struct submission_rejection {
    error: recovery_error,
    candidate: scene_candidate,
}

impl submission_rejection {
    #[must_use]
    pub const fn error(&self) -> recovery_error {
        self.error
    }

    #[must_use]
    pub fn candidate(&self) -> &scene_candidate {
        &self.candidate
    }

    #[must_use]
    pub fn into_candidate(self) -> scene_candidate {
        self.candidate
    }
}

pub struct finish_rejection {
    error: recovery_error,
    ticket: presentation_ticket,
}

impl finish_rejection {
    #[must_use]
    pub const fn error(&self) -> recovery_error {
        self.error
    }

    pub fn into_ticket(self) -> presentation_ticket {
        self.ticket
    }
}

pub struct retirement_rejection {
    error: recovery_error,
    surface: recovery_surface,
    control: recovery_control,
}

impl retirement_rejection {
    #[must_use]
    pub const fn error(&self) -> recovery_error {
        self.error
    }

    pub fn into_parts(self) -> (recovery_surface, recovery_control) {
        (self.surface, self.control)
    }
}

/// Surviving local owner of one real semantic surface and its recovery state.
/// No raw service, service controls, action handles, or publication owners escape.
#[must_use = "retire the coordinator explicitly"]
pub struct recovery_surface {
    instance: recovery_instance,
    surface: desktop_surface,
    control: surface_control,
    presentation: presentation_control,
    renderer: renderer_endpoint,
    lease: Option<render_lease>,
    prepared: Option<prepared_presentation>,
    receipt: Option<presentation_receipt>,
    publication: Option<publication_receipt>,
    snapshot: Option<surface_snapshot_v1>,
    phase: recovery_phase,
    presenter_generation: u64,
    replacements: u8,
    transaction: u64,
    ticket_issued: bool,
    copy_confirmed: bool,
}

impl recovery_surface {
    /// Trusted bootstrap assertion, matching `desktop_surface::new_presented`.
    /// Neither this constructor nor its initial read view verifies pixels.
    #[allow(clippy::result_large_err)]
    pub fn new_presented(
        identity: surface_identity,
        renderer: renderer_identity,
        candidate: scene_candidate,
    ) -> Result<(Self, recovery_control, presenter_endpoint), construction_rejection> {
        let instance = match next_recovery_instance.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| current.checked_add(1),
        ) {
            Ok(instance) => recovery_instance(instance),
            Err(_) => {
                return Err(construction_rejection {
                    error: recovery_error::instance_exhausted,
                    candidate,
                });
            }
        };
        let (surface, control, presentation, renderer) =
            match desktop_surface::new_presented(identity, renderer, candidate) {
                Ok(owners) => owners,
                Err(rejection) => {
                    return Err(construction_rejection {
                        error: recovery_error::service(rejection.error()),
                        candidate: rejection.into_candidate(),
                    });
                }
            };
        Ok((
            Self {
                instance,
                surface,
                control,
                presentation,
                renderer,
                lease: None,
                prepared: None,
                receipt: None,
                publication: None,
                snapshot: None,
                phase: recovery_phase::steady,
                presenter_generation: 1,
                replacements: 0,
                transaction: 0,
                ticket_issued: false,
                copy_confirmed: false,
            },
            recovery_control { instance },
            presenter_endpoint {
                instance,
                generation: 1,
            },
        ))
    }

    /// Observe only the complete committed scene while no presentation is active.
    /// Previously copied observations are historical and cannot be recalled.
    pub fn presented(&self) -> Result<presented_scene_view<'_>, recovery_error> {
        if self.presentation_active() {
            return Err(recovery_error::observation_unavailable);
        }
        Ok(self.surface.presented())
    }

    /// Read descriptive state without acknowledging or advancing anything.
    #[must_use]
    pub fn status(&self) -> recovery_status {
        recovery_status {
            phase: self.phase,
            revision: self.surface.presented_revision(),
            sequence: self.surface.publication_sequence(),
            copy_confirmed: self.copy_confirmed,
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn submit_revision(
        &mut self,
        base: scene_revision,
        candidate: scene_candidate,
    ) -> Result<queued_revision, submission_rejection> {
        if self.presentation_active() {
            return Err(submission_rejection {
                error: recovery_error::wrong_phase,
                candidate,
            });
        }
        match self.surface.submit_revision(&self.control, base, candidate) {
            Ok(queued) => {
                self.phase = recovery_phase::queued;
                Ok(queued)
            }
            Err(rejection) => Err(submission_rejection {
                error: recovery_error::service(rejection.error()),
                candidate: rejection.into_candidate(),
            }),
        }
    }

    /// Acquire the real service lease and retain it within the coordinator.
    pub fn acquire_render(&mut self) -> Result<(), recovery_error> {
        if self.phase != recovery_phase::queued {
            return Err(recovery_error::wrong_phase);
        }
        let lease = self
            .surface
            .acquire_render(&self.renderer)
            .map_err(recovery_error::service)?;
        self.lease = Some(lease);
        self.phase = recovery_phase::leased;
        Ok(())
    }

    pub fn render_candidate(&self) -> Result<render_snapshot_view<'_>, recovery_error> {
        if self.phase != recovery_phase::leased {
            return Err(recovery_error::wrong_phase);
        }
        self.surface
            .render_candidate(
                self.lease
                    .as_ref()
                    .ok_or(recovery_error::invariant_violation)?,
            )
            .map_err(recovery_error::service)
    }

    /// Retain the exact redacted redraw body before consuming the service lease.
    /// Readiness alone neither closes the gate nor asserts external presentation.
    pub fn complete_render(&mut self) -> Result<(), recovery_error> {
        if self.phase != recovery_phase::leased {
            return Err(recovery_error::wrong_phase);
        }
        let lease = self
            .lease
            .as_ref()
            .ok_or(recovery_error::invariant_violation)?;
        let snapshot = self
            .surface
            .render_candidate(lease)
            .map_err(recovery_error::service)?;
        let encoded =
            try_encode_service_snapshot(&snapshot, lease).map_err(recovery_error::transport)?;
        let lease = self.lease.take().expect("validated retained lease");
        match self.surface.complete_render(lease) {
            Ok(prepared) => {
                self.prepared = Some(prepared);
                self.snapshot = Some(encoded);
                self.phase = recovery_phase::ready;
                Ok(())
            }
            Err(rejection) => {
                let error = recovery_error::service(rejection.error());
                self.lease = Some(rejection.into_lease());
                Err(error)
            }
        }
    }

    /// Close semantic reads before granting an external presentation ticket.
    /// After replacement, reissue only for the same retained transaction.
    pub fn begin(
        &mut self,
        presenter: &presenter_endpoint,
    ) -> Result<presentation_ticket, recovery_error> {
        self.validate_presenter(presenter)?;
        if self.ticket_issued {
            return Err(recovery_error::wrong_phase);
        }
        if self.phase == recovery_phase::ready {
            let transaction = self
                .transaction
                .checked_add(1)
                .ok_or(recovery_error::transaction_exhausted)?;
            if self.prepared.is_none() || self.snapshot.is_none() {
                return Err(recovery_error::invariant_violation);
            }
            self.transaction = transaction;
            self.phase = recovery_phase::presenting;
        } else if !self.presentation_active() {
            return Err(recovery_error::wrong_phase);
        }
        self.ticket_issued = true;
        Ok(presentation_ticket {
            instance: self.instance,
            presenter_generation: self.presenter_generation,
            transaction: self.transaction,
        })
    }

    /// Borrow only the retained, immutable, redacted body for complete redraw.
    /// Possession of the bytes is descriptive and cannot replace either owner.
    pub fn redraw_candidate(
        &self,
        presenter: &presenter_endpoint,
        ticket: &presentation_ticket,
    ) -> Result<&surface_snapshot_v1, recovery_error> {
        self.validate_ticket(presenter, ticket)?;
        self.snapshot
            .as_ref()
            .ok_or(recovery_error::invariant_violation)
    }

    /// Trusted assertion that this presenter completed a full matching redraw.
    /// This does not inspect pixels. Replacement always requires this assertion
    /// again, even when a service receipt or semantic commit already exists.
    pub fn attest_copied(
        &mut self,
        presenter: &presenter_endpoint,
        ticket: &presentation_ticket,
    ) -> Result<(), recovery_error> {
        self.validate_ticket(presenter, ticket)?;
        if self.copy_confirmed {
            return Err(recovery_error::wrong_phase);
        }
        if self.phase == recovery_phase::published {
            self.copy_confirmed = true;
            return Ok(());
        }
        if self.phase != recovery_phase::presenting {
            return Err(recovery_error::wrong_phase);
        }
        if self.receipt.is_none() {
            let ready = self
                .prepared
                .take()
                .ok_or(recovery_error::invariant_violation)?;
            match self.surface.attest_presented(&self.presentation, ready) {
                Ok(receipt) => self.receipt = Some(receipt),
                Err(rejection) => {
                    let error = recovery_error::service(rejection.error());
                    self.prepared = Some(rejection.into_ready());
                    return Err(error);
                }
            }
        }
        self.copy_confirmed = true;
        self.phase = recovery_phase::copied;
        Ok(())
    }

    /// Consume the retained service receipt exactly once while reads stay shut.
    pub fn publish_semantics(
        &mut self,
        presenter: &presenter_endpoint,
        ticket: &presentation_ticket,
    ) -> Result<(), recovery_error> {
        self.validate_ticket(presenter, ticket)?;
        if self.phase != recovery_phase::copied || !self.copy_confirmed {
            return Err(recovery_error::wrong_phase);
        }
        let receipt = self
            .receipt
            .take()
            .ok_or(recovery_error::invariant_violation)?;
        match self.surface.publish(&self.control, receipt) {
            Ok(publication) => {
                self.publication = Some(publication);
                self.phase = recovery_phase::published;
                Ok(())
            }
            Err(rejection) => {
                let error = recovery_error::service(rejection.error());
                self.receipt = Some(rejection.into_receipt());
                Err(error)
            }
        }
    }

    /// Reopen reads only for the exact finalized transaction and current copy.
    /// Rejection returns the original ticket; no rejected operation opens reads.
    pub fn finish(
        &mut self,
        presenter: &presenter_endpoint,
        ticket: presentation_ticket,
    ) -> Result<(), finish_rejection> {
        if let Err(error) = self.validate_ticket(presenter, &ticket) {
            return Err(finish_rejection { error, ticket });
        }
        if self.phase != recovery_phase::published || !self.copy_confirmed {
            return Err(finish_rejection {
                error: recovery_error::wrong_phase,
                ticket,
            });
        }
        if !self.publication_is_exact() {
            return Err(finish_rejection {
                error: recovery_error::invariant_violation,
                ticket,
            });
        }
        self.publication = None;
        self.snapshot = None;
        self.ticket_issued = false;
        self.copy_confirmed = false;
        self.phase = recovery_phase::steady;
        Ok(())
    }

    /// Authorize at most one presenter replacement in this coordinator lifetime.
    /// The trusted caller must stop the old external presenter before invoking
    /// this operation; local generation invalidation cannot revoke device access.
    /// The new presenter must redraw even after semantic publication.
    pub fn replace_presenter(
        &mut self,
        control: &recovery_control,
    ) -> Result<presenter_endpoint, recovery_error> {
        if control.instance != self.instance {
            return Err(recovery_error::control_denied);
        }
        if !self.presentation_active() {
            return Err(recovery_error::wrong_phase);
        }
        if self.replacements != 0 {
            return Err(recovery_error::replacement_limit_reached);
        }
        let generation = self
            .presenter_generation
            .checked_add(1)
            .ok_or(recovery_error::presenter_generation_exhausted)?;
        self.presenter_generation = generation;
        self.replacements += 1;
        self.ticket_issued = false;
        self.copy_confirmed = false;
        if self.phase == recovery_phase::copied {
            self.phase = recovery_phase::presenting;
        }
        Ok(presenter_endpoint {
            instance: self.instance,
            generation,
        })
    }

    /// Explicitly retire from any phase. This clears local ownership only and
    /// never claims to clear, restore, or withdraw pixels on an external display.
    #[allow(clippy::result_large_err)]
    pub fn retire(
        self,
        recovery: recovery_control,
    ) -> Result<surface_retirement, retirement_rejection> {
        if recovery.instance != self.instance {
            return Err(retirement_rejection {
                error: recovery_error::control_denied,
                surface: self,
                control: recovery,
            });
        }
        let Self {
            instance,
            surface,
            control,
            presentation,
            renderer,
            lease,
            prepared,
            receipt,
            publication,
            snapshot,
            phase,
            presenter_generation,
            replacements,
            transaction,
            ticket_issued,
            copy_confirmed,
        } = self;
        match surface.retire(control) {
            Ok(retirement) => Ok(retirement),
            Err(rejection) => {
                let error = recovery_error::service(rejection.error());
                let (surface, control) = rejection.into_parts();
                Err(retirement_rejection {
                    error,
                    surface: Self {
                        instance,
                        surface,
                        control,
                        presentation,
                        renderer,
                        lease,
                        prepared,
                        receipt,
                        publication,
                        snapshot,
                        phase,
                        presenter_generation,
                        replacements,
                        transaction,
                        ticket_issued,
                        copy_confirmed,
                    },
                    control: recovery,
                })
            }
        }
    }

    fn presentation_active(&self) -> bool {
        matches!(
            self.phase,
            recovery_phase::presenting | recovery_phase::copied | recovery_phase::published
        )
    }

    fn validate_presenter(&self, presenter: &presenter_endpoint) -> Result<(), recovery_error> {
        if presenter.instance != self.instance || presenter.generation != self.presenter_generation
        {
            return Err(recovery_error::presenter_denied);
        }
        Ok(())
    }

    fn validate_ticket(
        &self,
        presenter: &presenter_endpoint,
        ticket: &presentation_ticket,
    ) -> Result<(), recovery_error> {
        self.validate_presenter(presenter)?;
        if ticket.instance != self.instance
            || ticket.presenter_generation != self.presenter_generation
            || ticket.transaction != self.transaction
            || !self.ticket_issued
            || !self.presentation_active()
        {
            return Err(recovery_error::stale_ticket);
        }
        Ok(())
    }

    fn publication_is_exact(&self) -> bool {
        let (Some(publication), Some(snapshot)) = (&self.publication, &self.snapshot) else {
            return false;
        };
        let binding = snapshot.binding();
        publication.surface() == self.surface.identity()
            && publication.revision().get() == binding.revision()
            && publication.revision() == self.surface.presented_revision()
            && publication.sequence() == self.surface.publication_sequence()
            && publication.renderer().slot() == binding.renderer().slot()
            && publication.renderer().generation() == binding.renderer().generation()
            && publication.attempt().get() == binding.attempt()
            && self.surface.pending_revision().is_none()
            && self.lease.is_none()
            && self.prepared.is_none()
            && self.receipt.is_none()
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
