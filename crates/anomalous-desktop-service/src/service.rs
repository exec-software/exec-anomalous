use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use anomalous_desktop_scene::{
    action_resolution, desktop_scene, focus_direction, local_action_handle, local_focus_handle,
    scene_action_reference, scene_candidate, scene_control, scene_error, scene_focus,
    scene_resolution_source, scene_revision,
};

use crate::identity::{
    delivery_attempt, publication_sequence, renderer_identity, surface_identity,
};
use crate::presented::presented_scene_view;
use crate::snapshot::{render_snapshot, render_snapshot_view, validate_candidate_profile};

const first_instance: u64 = 1;
const first_sequence: u64 = 1;
const maximum_attempts: u8 = 2;
const maximum_restarts: u8 = 1;
static next_surface_instance: AtomicU64 = AtomicU64::new(first_instance);

#[derive(Clone, Copy, Eq, PartialEq)]
struct surface_instance(u64);

fn allocate_surface_instance() -> Result<surface_instance, surface_error> {
    next_surface_instance
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(surface_instance)
        .map_err(|_| surface_error::surface_instance_exhausted)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum publication_phase {
    steady,
    queued,
    leased,
    ready,
    presented,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum surface_error {
    surface_instance_exhausted,
    control_denied,
    renderer_denied,
    pending_full,
    no_pending_revision,
    wrong_phase,
    stale_delivery,
    attempt_limit_reached,
    restart_limit_reached,
    renderer_generation_exhausted,
    publication_sequence_exhausted,
    unsupported_profile,
    scene_failure(scene_error),
    invariant_violation,
}

impl surface_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::surface_instance_exhausted => "surface_instance_exhausted",
            Self::control_denied => "control_denied",
            Self::renderer_denied => "renderer_denied",
            Self::pending_full => "pending_full",
            Self::no_pending_revision => "no_pending_revision",
            Self::wrong_phase => "wrong_phase",
            Self::stale_delivery => "stale_delivery",
            Self::attempt_limit_reached => "attempt_limit_reached",
            Self::restart_limit_reached => "restart_limit_reached",
            Self::renderer_generation_exhausted => "renderer_generation_exhausted",
            Self::publication_sequence_exhausted => "publication_sequence_exhausted",
            Self::unsupported_profile => "unsupported_profile",
            Self::scene_failure(_) => "scene_failure",
            Self::invariant_violation => "invariant_violation",
        }
    }

    #[must_use]
    pub const fn scene_error(self) -> Option<scene_error> {
        match self {
            Self::scene_failure(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Debug for surface_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for surface_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// Move-only authority for mutation, renderer replacement, publication, and
/// retirement of one exact surface instance.
pub struct surface_control {
    instance: surface_instance,
    surface: surface_identity,
}

/// Move-only authority for the trusted external-presentation assertion.
pub struct presentation_control {
    instance: surface_instance,
    surface: surface_identity,
}

impl presentation_control {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }
}

impl surface_control {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }
}

/// Move-only endpoint for one exact renderer incarnation.
pub struct renderer_endpoint {
    instance: surface_instance,
    surface: surface_identity,
    renderer: renderer_identity,
}

impl renderer_endpoint {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.renderer
    }
}

pub struct surface_construction_rejection {
    error: surface_error,
    candidate: scene_candidate,
}

impl surface_construction_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
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
    error: surface_error,
    candidate: scene_candidate,
}

impl submission_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
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

/// Move-only receipt that one revision occupies the sole pending slot.
pub struct queued_revision {
    surface: surface_identity,
    base: scene_revision,
    revision: scene_revision,
}

impl queued_revision {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn base_revision(&self) -> scene_revision {
        self.base
    }

    #[must_use]
    pub fn revision(&self) -> scene_revision {
        self.revision
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct attempt_stamp {
    renderer: renderer_identity,
    attempt: delivery_attempt,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum pending_phase {
    queued,
    leased(attempt_stamp),
    ready(attempt_stamp),
    presented(attempt_stamp),
}

struct pending_update {
    prepared: anomalous_desktop_scene::prepared_revision,
    snapshot: render_snapshot,
    phase: pending_phase,
    attempts_started: u8,
}

/// Move-only authority to read one exact immutable renderer candidate.
pub struct render_lease {
    instance: surface_instance,
    surface: surface_identity,
    revision: scene_revision,
    stamp: attempt_stamp,
}

impl render_lease {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn revision(&self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.stamp.renderer
    }

    #[must_use]
    pub fn attempt(&self) -> delivery_attempt {
        self.stamp.attempt
    }
}

pub struct completion_rejection {
    error: surface_error,
    lease: render_lease,
}

impl completion_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    #[must_use]
    pub fn into_lease(self) -> render_lease {
        self.lease
    }
}

/// Move-only renderer completion awaiting a trusted host publication assertion.
pub struct prepared_presentation {
    instance: surface_instance,
    surface: surface_identity,
    revision: scene_revision,
    stamp: attempt_stamp,
}

impl prepared_presentation {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn revision(&self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.stamp.renderer
    }

    #[must_use]
    pub fn attempt(&self) -> delivery_attempt {
        self.stamp.attempt
    }
}

/// Move-only trusted-host assertion derived from one exact ready owner.
pub struct presentation_receipt {
    instance: surface_instance,
    surface: surface_identity,
    revision: scene_revision,
    stamp: attempt_stamp,
}

impl presentation_receipt {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn revision(&self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.stamp.renderer
    }

    #[must_use]
    pub fn attempt(&self) -> delivery_attempt {
        self.stamp.attempt
    }
}

pub struct presentation_rejection {
    error: surface_error,
    ready: prepared_presentation,
}

impl presentation_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    #[must_use]
    pub fn into_ready(self) -> prepared_presentation {
        self.ready
    }
}

pub struct publication_rejection {
    error: surface_error,
    receipt: presentation_receipt,
}

impl publication_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    #[must_use]
    pub fn into_receipt(self) -> presentation_receipt {
        self.receipt
    }
}

pub struct publication_receipt {
    surface: surface_identity,
    revision: scene_revision,
    sequence: publication_sequence,
    renderer: renderer_identity,
    attempt: delivery_attempt,
}

impl publication_receipt {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn revision(&self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub fn sequence(&self) -> publication_sequence {
        self.sequence
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.renderer
    }

    #[must_use]
    pub fn attempt(&self) -> delivery_attempt {
        self.attempt
    }
}

pub struct restart_receipt {
    previous: renderer_identity,
    endpoint: renderer_endpoint,
}

impl restart_receipt {
    #[must_use]
    pub fn previous_renderer(&self) -> renderer_identity {
        self.previous
    }

    #[must_use]
    pub fn renderer(&self) -> renderer_identity {
        self.endpoint.renderer
    }

    #[must_use]
    pub fn into_endpoint(self) -> renderer_endpoint {
        self.endpoint
    }
}

/// Service-private wrapper around one scene-local action capability.
pub struct surface_action_handle {
    instance: surface_instance,
    surface: surface_identity,
    handle: local_action_handle,
}

impl surface_action_handle {
    #[must_use]
    pub fn reference(&self) -> scene_action_reference {
        self.handle.reference()
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.handle.source()
    }
}

pub struct surface_action_rejection {
    error: surface_error,
    handle: surface_action_handle,
}

impl surface_action_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    #[must_use]
    pub fn into_handle(self) -> surface_action_handle {
        self.handle
    }
}

/// Service-private wrapper around one scene-local focus capability.
pub struct surface_focus_handle {
    instance: surface_instance,
    surface: surface_identity,
    handle: local_focus_handle,
}

impl surface_focus_handle {
    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.handle.source()
    }

    #[must_use]
    pub fn cursor(&self) -> scene_focus {
        self.handle.cursor()
    }
}

pub struct surface_focus_rejection {
    error: surface_error,
    handle: surface_focus_handle,
}

pub struct surface_focus_resolution {
    reference: anomalous_desktop_scene::scene_focus_reference,
    source: scene_resolution_source,
    changed: bool,
    handle: surface_focus_handle,
}

impl surface_focus_resolution {
    #[must_use]
    pub fn reference(&self) -> anomalous_desktop_scene::scene_focus_reference {
        self.reference
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.source
    }

    #[must_use]
    pub fn scene_focus_changed(&self) -> bool {
        self.changed
    }

    #[must_use]
    pub fn handle(&self) -> &surface_focus_handle {
        &self.handle
    }

    #[must_use]
    pub fn into_handle(self) -> surface_focus_handle {
        self.handle
    }
}

impl surface_focus_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    #[must_use]
    pub fn into_handle(self) -> surface_focus_handle {
        self.handle
    }
}

pub struct surface_retirement {
    surface: surface_identity,
    final_revision: scene_revision,
    final_sequence: publication_sequence,
    final_renderer: renderer_identity,
    discarded_pending_revision: bool,
}

impl surface_retirement {
    #[must_use]
    pub fn surface(&self) -> surface_identity {
        self.surface
    }

    #[must_use]
    pub fn final_revision(&self) -> scene_revision {
        self.final_revision
    }

    #[must_use]
    pub fn final_sequence(&self) -> publication_sequence {
        self.final_sequence
    }

    #[must_use]
    pub fn final_renderer(&self) -> renderer_identity {
        self.final_renderer
    }

    #[must_use]
    pub fn discarded_pending_revision(&self) -> bool {
        self.discarded_pending_revision
    }
}

pub struct surface_retirement_rejection {
    error: surface_error,
    surface: desktop_surface,
    control: surface_control,
}

impl surface_retirement_rejection {
    #[must_use]
    pub fn error(&self) -> surface_error {
        self.error
    }

    pub fn into_parts(self) -> (desktop_surface, surface_control) {
        (self.surface, self.control)
    }
}

#[must_use = "retire the live desktop surface explicitly"]
pub struct desktop_surface {
    instance: surface_instance,
    identity: surface_identity,
    scene: desktop_scene,
    scene_control: scene_control,
    renderer: renderer_identity,
    publication: publication_sequence,
    restarts: u8,
    pending: Option<pending_update>,
}

impl desktop_surface {
    /// Constructs a service around revision one, which the trusted host asserts
    /// is already fully presented together with its semantic projections.
    #[allow(clippy::result_large_err)]
    pub fn new_presented(
        identity: surface_identity,
        renderer: renderer_identity,
        candidate: scene_candidate,
    ) -> Result<
        (
            Self,
            surface_control,
            presentation_control,
            renderer_endpoint,
        ),
        surface_construction_rejection,
    > {
        let reject = |error, candidate| surface_construction_rejection { error, candidate };
        let instance = match allocate_surface_instance() {
            Ok(instance) => instance,
            Err(error) => return Err(reject(error, candidate)),
        };
        if let Err(error) = validate_candidate_profile(&candidate) {
            return Err(reject(error, candidate));
        }
        let (scene, scene_control) = match desktop_scene::new(candidate) {
            Ok(value) => value,
            Err(rejection) => {
                return Err(reject(
                    surface_error::scene_failure(rejection.error()),
                    rejection.into_candidate(),
                ));
            }
        };
        let publication = match publication_sequence::try_new(first_sequence) {
            Ok(sequence) => sequence,
            Err(_) => panic!("desktop surface sequence invariant violated"),
        };
        let surface = Self {
            instance,
            identity,
            scene,
            scene_control,
            renderer,
            publication,
            restarts: 0,
            pending: None,
        };
        let control = surface_control {
            instance,
            surface: identity,
        };
        let endpoint = renderer_endpoint {
            instance,
            surface: identity,
            renderer,
        };
        let presentation = presentation_control {
            instance,
            surface: identity,
        };
        Ok((surface, control, presentation, endpoint))
    }

    #[must_use]
    pub const fn identity(&self) -> surface_identity {
        self.identity
    }

    #[must_use]
    pub const fn scene_identity(&self) -> anomalous_desktop_scene::scene_identity {
        self.scene.identity()
    }

    #[must_use]
    pub const fn presented_revision(&self) -> scene_revision {
        self.scene.revision()
    }

    #[must_use]
    pub const fn publication_sequence(&self) -> publication_sequence {
        self.publication
    }

    #[must_use]
    pub const fn renderer(&self) -> renderer_identity {
        self.renderer
    }

    #[must_use]
    pub const fn restart_count(&self) -> u8 {
        self.restarts
    }

    #[must_use]
    pub fn phase(&self) -> publication_phase {
        match self.pending.as_ref().map(|pending| pending.phase) {
            None => publication_phase::steady,
            Some(pending_phase::queued) => publication_phase::queued,
            Some(pending_phase::leased(_)) => publication_phase::leased,
            Some(pending_phase::ready(_)) => publication_phase::ready,
            Some(pending_phase::presented(_)) => publication_phase::presented,
        }
    }

    #[must_use]
    pub fn pending_revision(&self) -> Option<scene_revision> {
        self.pending
            .as_ref()
            .map(|pending| pending.snapshot.revision())
    }

    #[must_use]
    pub const fn presented(&self) -> presented_scene_view<'_> {
        presented_scene_view::new(&self.scene)
    }

    #[allow(clippy::result_large_err)]
    pub fn submit_revision(
        &mut self,
        control: &surface_control,
        base: scene_revision,
        candidate: scene_candidate,
    ) -> Result<queued_revision, submission_rejection> {
        let reject = |error, candidate| submission_rejection { error, candidate };
        if let Err(error) = self.validate_control(control) {
            return Err(reject(error, candidate));
        }
        if self.pending.is_some() {
            return Err(reject(surface_error::pending_full, candidate));
        }
        if self.publication.next().is_err() {
            return Err(reject(
                surface_error::publication_sequence_exhausted,
                candidate,
            ));
        }
        if let Err(error) = validate_candidate_profile(&candidate) {
            return Err(reject(error, candidate));
        }
        let prepared = match self
            .scene
            .prepare_revision(&self.scene_control, base, candidate)
        {
            Ok(prepared) => prepared,
            Err(rejection) => {
                return Err(reject(
                    surface_error::scene_failure(rejection.error()),
                    rejection.into_candidate(),
                ));
            }
        };
        let snapshot = match render_snapshot::from_preview(self.identity, &prepared.preview()) {
            Ok(snapshot) => snapshot,
            Err(error) => return Err(reject(error, prepared.into_candidate())),
        };
        let revision = snapshot.revision();
        self.pending = Some(pending_update {
            prepared,
            snapshot,
            phase: pending_phase::queued,
            attempts_started: 0,
        });
        Ok(queued_revision {
            surface: self.identity,
            base,
            revision,
        })
    }

    pub fn acquire_render(
        &mut self,
        endpoint: &renderer_endpoint,
    ) -> Result<render_lease, surface_error> {
        self.validate_renderer(endpoint)?;
        let pending = self
            .pending
            .as_mut()
            .ok_or(surface_error::no_pending_revision)?;
        if pending.phase != pending_phase::queued {
            return Err(surface_error::wrong_phase);
        }
        if pending.attempts_started >= maximum_attempts {
            return Err(surface_error::attempt_limit_reached);
        }
        let attempt_value = u64::from(pending.attempts_started) + 1;
        let attempt = delivery_attempt::try_new(attempt_value)
            .map_err(|_| surface_error::invariant_violation)?;
        let stamp = attempt_stamp {
            renderer: self.renderer,
            attempt,
        };
        pending.attempts_started += 1;
        pending.phase = pending_phase::leased(stamp);
        Ok(render_lease {
            instance: self.instance,
            surface: self.identity,
            revision: pending.snapshot.revision(),
            stamp,
        })
    }

    pub fn render_candidate(
        &self,
        lease: &render_lease,
    ) -> Result<render_snapshot_view<'_>, surface_error> {
        self.validate_lease(lease)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(surface_error::no_pending_revision)?;
        Ok(pending.snapshot.view())
    }

    pub fn complete_render(
        &mut self,
        lease: render_lease,
    ) -> Result<prepared_presentation, completion_rejection> {
        if let Err(error) = self.validate_lease(&lease) {
            return Err(completion_rejection { error, lease });
        }
        let revision = lease.revision;
        let stamp = lease.stamp;
        let pending = self.pending.as_mut().expect("validated pending revision");
        pending.phase = pending_phase::ready(stamp);
        Ok(prepared_presentation {
            instance: self.instance,
            surface: self.identity,
            revision,
            stamp,
        })
    }

    pub fn restart_renderer(
        &mut self,
        control: &surface_control,
    ) -> Result<restart_receipt, surface_error> {
        self.validate_control(control)?;
        let pending = self
            .pending
            .as_mut()
            .ok_or(surface_error::no_pending_revision)?;
        if !matches!(
            pending.phase,
            pending_phase::leased(_) | pending_phase::ready(_)
        ) {
            return Err(surface_error::wrong_phase);
        }
        if self.restarts >= maximum_restarts {
            return Err(surface_error::restart_limit_reached);
        }
        let next = self
            .renderer
            .next_generation()
            .map_err(|_| surface_error::renderer_generation_exhausted)?;
        let previous = self.renderer;
        self.renderer = next;
        self.restarts += 1;
        pending.phase = pending_phase::queued;
        Ok(restart_receipt {
            previous,
            endpoint: renderer_endpoint {
                instance: self.instance,
                surface: self.identity,
                renderer: next,
            },
        })
    }

    /// Converts renderer readiness into the trusted host's assertion that the
    /// matching complete external presentation has finished. No pixel or
    /// scanout validation occurs in this host model.
    pub fn attest_presented(
        &mut self,
        control: &presentation_control,
        ready: prepared_presentation,
    ) -> Result<presentation_receipt, presentation_rejection> {
        if control.instance != self.instance || control.surface != self.identity {
            return Err(presentation_rejection {
                error: surface_error::control_denied,
                ready,
            });
        }
        if let Err(error) = self.validate_progression(
            ready.instance,
            ready.surface,
            ready.revision,
            ready.stamp,
            pending_phase::ready(ready.stamp),
        ) {
            return Err(presentation_rejection { error, ready });
        }
        self.pending
            .as_mut()
            .expect("validated pending revision")
            .phase = pending_phase::presented(ready.stamp);
        Ok(presentation_receipt {
            instance: ready.instance,
            surface: ready.surface,
            revision: ready.revision,
            stamp: ready.stamp,
        })
    }

    /// Commits the semantic revision only after consuming the exact trusted
    /// presentation receipt. This crate does not itself inspect pixels.
    pub fn publish(
        &mut self,
        control: &surface_control,
        receipt: presentation_receipt,
    ) -> Result<publication_receipt, publication_rejection> {
        if let Err(error) = self.validate_control(control) {
            return Err(publication_rejection { error, receipt });
        }
        if let Err(error) = self.validate_progression(
            receipt.instance,
            receipt.surface,
            receipt.revision,
            receipt.stamp,
            pending_phase::presented(receipt.stamp),
        ) {
            return Err(publication_rejection { error, receipt });
        }
        let next_sequence = match self.publication.next() {
            Ok(sequence) => sequence,
            Err(_) => {
                return Err(publication_rejection {
                    error: surface_error::publication_sequence_exhausted,
                    receipt,
                });
            }
        };
        let pending = self.pending.take().expect("validated pending revision");
        let pending_update {
            prepared,
            snapshot,
            phase,
            attempts_started,
        } = pending;
        let revision = match self.scene.commit_revision(prepared) {
            Ok(revision) => revision,
            Err(rejection) => {
                self.pending = Some(pending_update {
                    prepared: rejection.into_prepared(),
                    snapshot,
                    phase,
                    attempts_started,
                });
                return Err(publication_rejection {
                    error: surface_error::invariant_violation,
                    receipt,
                });
            }
        };
        self.publication = next_sequence;
        Ok(publication_receipt {
            surface: self.identity,
            revision,
            sequence: next_sequence,
            renderer: receipt.stamp.renderer,
            attempt: receipt.stamp.attempt,
        })
    }

    pub fn mint_accessible_action(
        &self,
        reference: scene_action_reference,
    ) -> Result<surface_action_handle, surface_error> {
        let handle = self
            .scene
            .projections()
            .render_plan()
            .mint_action_handle(reference)
            .map_err(surface_error::scene_failure)?;
        Ok(surface_action_handle {
            instance: self.instance,
            surface: self.identity,
            handle,
        })
    }

    pub fn mint_machine_action(
        &self,
        reference: scene_action_reference,
    ) -> Result<surface_action_handle, surface_error> {
        let handle = self
            .scene
            .projections()
            .machine_projection()
            .mint_action_handle(reference)
            .map_err(surface_error::scene_failure)?;
        Ok(surface_action_handle {
            instance: self.instance,
            surface: self.identity,
            handle,
        })
    }

    pub fn resolve_action(
        &self,
        handle: surface_action_handle,
    ) -> Result<action_resolution, surface_action_rejection> {
        if handle.instance != self.instance || handle.surface != self.identity {
            return Err(surface_action_rejection {
                error: surface_error::control_denied,
                handle,
            });
        }
        let surface_action_handle {
            instance,
            surface,
            handle,
        } = handle;
        match self.scene.resolve_action(handle) {
            Ok(resolution) => Ok(resolution),
            Err(rejection) => Err(surface_action_rejection {
                error: surface_error::scene_failure(rejection.error()),
                handle: surface_action_handle {
                    instance,
                    surface,
                    handle: rejection.into_handle(),
                },
            }),
        }
    }

    #[must_use]
    pub fn mint_accessible_focus(&self) -> surface_focus_handle {
        surface_focus_handle {
            instance: self.instance,
            surface: self.identity,
            handle: self.scene.projections().render_plan().mint_focus_handle(),
        }
    }

    #[must_use]
    pub fn mint_machine_focus(&self) -> surface_focus_handle {
        surface_focus_handle {
            instance: self.instance,
            surface: self.identity,
            handle: self
                .scene
                .projections()
                .machine_projection()
                .mint_focus_handle(),
        }
    }

    pub fn traverse_focus(
        &self,
        handle: surface_focus_handle,
        direction: focus_direction,
    ) -> Result<surface_focus_resolution, surface_focus_rejection> {
        if handle.instance != self.instance || handle.surface != self.identity {
            return Err(surface_focus_rejection {
                error: surface_error::control_denied,
                handle,
            });
        }
        let surface_focus_handle {
            instance,
            surface,
            handle,
        } = handle;
        match self.scene.traverse_focus(handle, direction) {
            Ok(resolution) => {
                let reference = resolution.reference();
                let source = resolution.source();
                let changed = resolution.scene_focus_changed();
                Ok(surface_focus_resolution {
                    reference,
                    source,
                    changed,
                    handle: surface_focus_handle {
                        instance,
                        surface,
                        handle: resolution.into_handle(),
                    },
                })
            }
            Err(rejection) => Err(surface_focus_rejection {
                error: surface_error::scene_failure(rejection.error()),
                handle: surface_focus_handle {
                    instance,
                    surface,
                    handle: rejection.into_handle(),
                },
            }),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn retire(
        self,
        control: surface_control,
    ) -> Result<surface_retirement, surface_retirement_rejection> {
        if control.instance != self.instance || control.surface != self.identity {
            return Err(surface_retirement_rejection {
                error: surface_error::control_denied,
                surface: self,
                control,
            });
        }
        let Self {
            identity,
            scene,
            scene_control,
            renderer,
            publication,
            pending,
            ..
        } = self;
        let discarded_pending_revision = pending.is_some();
        drop(pending);
        let scene_retirement = match scene.retire(scene_control) {
            Ok(retirement) => retirement,
            Err(_) => panic!("desktop service scene-control invariant violated"),
        };
        Ok(surface_retirement {
            surface: identity,
            final_revision: scene_retirement.final_revision(),
            final_sequence: publication,
            final_renderer: renderer,
            discarded_pending_revision,
        })
    }

    fn validate_control(&self, control: &surface_control) -> Result<(), surface_error> {
        if control.instance != self.instance || control.surface != self.identity {
            return Err(surface_error::control_denied);
        }
        Ok(())
    }

    fn validate_renderer(&self, endpoint: &renderer_endpoint) -> Result<(), surface_error> {
        if endpoint.instance != self.instance
            || endpoint.surface != self.identity
            || endpoint.renderer != self.renderer
        {
            return Err(surface_error::renderer_denied);
        }
        Ok(())
    }

    fn validate_lease(&self, lease: &render_lease) -> Result<(), surface_error> {
        if lease.instance != self.instance || lease.surface != self.identity {
            return Err(surface_error::stale_delivery);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(surface_error::no_pending_revision)?;
        if lease.revision != pending.snapshot.revision() || lease.stamp.renderer != self.renderer {
            return Err(surface_error::stale_delivery);
        }
        if pending.phase != pending_phase::leased(lease.stamp) {
            return Err(surface_error::stale_delivery);
        }
        Ok(())
    }

    fn validate_progression(
        &self,
        instance: surface_instance,
        surface: surface_identity,
        revision: scene_revision,
        stamp: attempt_stamp,
        expected_phase: pending_phase,
    ) -> Result<(), surface_error> {
        if instance != self.instance || surface != self.identity {
            return Err(surface_error::stale_delivery);
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or(surface_error::no_pending_revision)?;
        if revision != pending.snapshot.revision()
            || stamp.renderer != self.renderer
            || pending.phase != expected_phase
        {
            return Err(surface_error::stale_delivery);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_publication_sequence_for_test(&mut self, sequence: publication_sequence) {
        self.publication = sequence;
    }
}
