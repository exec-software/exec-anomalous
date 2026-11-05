use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::identity::{action_identity, node_identity, scene_identity, scene_revision};
use crate::projection::{
    derive_projections, prepared_render_preview, projection_storage, scene_action_reference,
    scene_focus_reference,
};
use crate::text::scene_label;

pub const scene_node_count: usize = 8;
pub const scene_action_count: usize = 2;

const first_revision: u64 = 1;
const first_instance: u64 = 1;
static next_scene_instance: AtomicU64 = AtomicU64::new(first_instance);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct scene_instance(u64);

fn allocate_scene_instance() -> Result<scene_instance, scene_error> {
    next_scene_instance
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(scene_instance)
        .map_err(|_| scene_error::scene_instance_exhausted)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_error {
    scene_instance_exhausted,
    control_denied,
    scene_not_live,
    wrong_scene_identity,
    stale_revision,
    revision_exhausted,
    invalid_geometry,
    geometry_overflow,
    wrong_slot,
    duplicate_node_identity,
    wrong_node_identity,
    duplicate_action_identity,
    wrong_action_identity,
    wrong_identity_generation,
    wrong_role,
    wrong_parent,
    wrong_geometry,
    wrong_action,
    wrong_disclosure,
    empty_label,
    protected_label_not_allowed,
    focus_disabled,
    projection_mismatch,
    invalid_action_reference,
    invalid_action_handle,
    disabled_action,
    invalid_focus_handle,
    no_enabled_focus,
    invariant_violation,
}

impl scene_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::scene_instance_exhausted => "scene_instance_exhausted",
            Self::control_denied => "control_denied",
            Self::scene_not_live => "scene_not_live",
            Self::wrong_scene_identity => "wrong_scene_identity",
            Self::stale_revision => "stale_revision",
            Self::revision_exhausted => "revision_exhausted",
            Self::invalid_geometry => "invalid_geometry",
            Self::geometry_overflow => "geometry_overflow",
            Self::wrong_slot => "wrong_slot",
            Self::duplicate_node_identity => "duplicate_node_identity",
            Self::wrong_node_identity => "wrong_node_identity",
            Self::duplicate_action_identity => "duplicate_action_identity",
            Self::wrong_action_identity => "wrong_action_identity",
            Self::wrong_identity_generation => "wrong_identity_generation",
            Self::wrong_role => "wrong_role",
            Self::wrong_parent => "wrong_parent",
            Self::wrong_geometry => "wrong_geometry",
            Self::wrong_action => "wrong_action",
            Self::wrong_disclosure => "wrong_disclosure",
            Self::empty_label => "empty_label",
            Self::protected_label_not_allowed => "protected_label_not_allowed",
            Self::focus_disabled => "focus_disabled",
            Self::projection_mismatch => "projection_mismatch",
            Self::invalid_action_reference => "invalid_action_reference",
            Self::invalid_action_handle => "invalid_action_handle",
            Self::disabled_action => "disabled_action",
            Self::invalid_focus_handle => "invalid_focus_handle",
            Self::no_enabled_focus => "no_enabled_focus",
            Self::invariant_violation => "invariant_violation",
        }
    }
}

impl fmt::Debug for scene_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for scene_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum scene_slot {
    root = 0,
    masthead = 1,
    navigation = 2,
    primary_action = 3,
    secondary_action = 4,
    content = 5,
    heading = 6,
    status = 7,
}

impl scene_slot {
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn identity_slot(self) -> u64 {
        self as u64 + 1
    }

    #[must_use]
    pub const fn canonical_bounds(self) -> scene_rect {
        canonical_geometry[self.index()]
    }

    #[must_use]
    pub const fn canonical_role(self) -> scene_role {
        canonical_roles[self.index()]
    }

    #[must_use]
    pub const fn canonical_parent(self) -> Option<Self> {
        match self {
            Self::root => None,
            Self::masthead | Self::navigation | Self::content => Some(Self::root),
            Self::primary_action | Self::secondary_action => Some(Self::navigation),
            Self::heading | Self::status => Some(Self::content),
        }
    }

    pub(crate) const canonical: [Self; scene_node_count] = [
        Self::root,
        Self::masthead,
        Self::navigation,
        Self::primary_action,
        Self::secondary_action,
        Self::content,
        Self::heading,
        Self::status,
    ];
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_role {
    desktop,
    region,
    control,
    heading,
    status,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_action_kind {
    activate,
    dismiss,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_disclosure {
    declared_public,
    protected,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum scene_focus {
    none = 0,
    primary = 1,
    secondary = 2,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum scene_depth {
    standard = 0,
    flat = 1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum scene_motion {
    standard = 0,
    reduced = 1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum focus_direction {
    forward,
    reverse,
}

#[derive(Clone, Copy, Eq, PartialEq)]
/// Records which committed projection minted a local handle.
///
/// This is not caller provenance, a principal, human presence, authentication,
/// or authority, and policy must not treat it as any of those properties.
pub enum scene_resolution_source {
    accessible_projection,
    machine_projection,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl scene_rect {
    pub const fn try_new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, scene_error> {
        if width == 0 || height == 0 {
            return Err(scene_error::invalid_geometry);
        }
        if x.checked_add(width).is_none() || y.checked_add(height).is_none() {
            return Err(scene_error::geometry_overflow);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }

    #[must_use]
    pub const fn x(self) -> u32 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> u32 {
        self.y
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

pub(crate) const viewport: scene_rect = scene_rect {
    x: 0,
    y: 0,
    width: 1024,
    height: 768,
};

const canonical_geometry: [scene_rect; scene_node_count] = [
    viewport,
    scene_rect {
        x: 32,
        y: 24,
        width: 960,
        height: 88,
    },
    scene_rect {
        x: 32,
        y: 136,
        width: 256,
        height: 600,
    },
    scene_rect {
        x: 56,
        y: 184,
        width: 208,
        height: 64,
    },
    scene_rect {
        x: 56,
        y: 272,
        width: 208,
        height: 64,
    },
    scene_rect {
        x: 312,
        y: 136,
        width: 680,
        height: 600,
    },
    scene_rect {
        x: 352,
        y: 176,
        width: 600,
        height: 48,
    },
    scene_rect {
        x: 352,
        y: 248,
        width: 600,
        height: 96,
    },
];

const canonical_roles: [scene_role; scene_node_count] = [
    scene_role::desktop,
    scene_role::region,
    scene_role::region,
    scene_role::control,
    scene_role::control,
    scene_role::region,
    scene_role::heading,
    scene_role::status,
];

const canonical_parent_indices: [Option<usize>; scene_node_count] = [
    None,
    Some(0),
    Some(0),
    Some(2),
    Some(2),
    Some(0),
    Some(5),
    Some(5),
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_action_definition {
    identity: action_identity,
    kind: scene_action_kind,
}

impl scene_action_definition {
    #[must_use]
    pub const fn new(identity: action_identity, kind: scene_action_kind) -> Self {
        Self { identity, kind }
    }

    #[must_use]
    pub const fn identity(self) -> action_identity {
        self.identity
    }

    #[must_use]
    pub const fn kind(self) -> scene_action_kind {
        self.kind
    }
}

pub struct scene_node_definition {
    slot: scene_slot,
    identity: node_identity,
    role: scene_role,
    parent: Option<node_identity>,
    bounds: scene_rect,
    label: scene_label,
    action: Option<scene_action_definition>,
}

impl scene_node_definition {
    #[must_use]
    pub fn new(
        slot: scene_slot,
        identity: node_identity,
        role: scene_role,
        parent: Option<node_identity>,
        bounds: scene_rect,
        label: scene_label,
        action: Option<scene_action_definition>,
    ) -> Self {
        Self {
            slot,
            identity,
            role,
            parent,
            bounds,
            label,
            action,
        }
    }

    #[must_use]
    pub const fn slot(&self) -> scene_slot {
        self.slot
    }

    #[must_use]
    pub const fn identity(&self) -> node_identity {
        self.identity
    }

    #[must_use]
    pub const fn role(&self) -> scene_role {
        self.role
    }

    #[must_use]
    pub const fn parent(&self) -> Option<node_identity> {
        self.parent
    }

    #[must_use]
    pub const fn bounds(&self) -> scene_rect {
        self.bounds
    }

    #[must_use]
    pub const fn action(&self) -> Option<scene_action_definition> {
        self.action
    }

    #[must_use]
    pub const fn disclosure(&self) -> scene_disclosure {
        if self.label.is_declared_public() {
            scene_disclosure::declared_public
        } else {
            scene_disclosure::protected
        }
    }

    #[must_use]
    pub fn label(&self) -> &scene_label {
        &self.label
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct declared_scene_state {
    primary_enabled: bool,
    secondary_enabled: bool,
    focus: scene_focus,
    status_disclosure: scene_disclosure,
    depth: scene_depth,
    motion: scene_motion,
}

impl declared_scene_state {
    #[must_use]
    pub const fn new(
        primary_enabled: bool,
        secondary_enabled: bool,
        focus: scene_focus,
        status_disclosure: scene_disclosure,
        depth: scene_depth,
        motion: scene_motion,
    ) -> Self {
        Self {
            primary_enabled,
            secondary_enabled,
            focus,
            status_disclosure,
            depth,
            motion,
        }
    }

    #[must_use]
    pub const fn primary_enabled(self) -> bool {
        self.primary_enabled
    }

    #[must_use]
    pub const fn secondary_enabled(self) -> bool {
        self.secondary_enabled
    }

    #[must_use]
    pub const fn focus(self) -> scene_focus {
        self.focus
    }

    #[must_use]
    pub const fn status_disclosure(self) -> scene_disclosure {
        self.status_disclosure
    }

    #[must_use]
    pub const fn depth(self) -> scene_depth {
        self.depth
    }

    #[must_use]
    pub const fn motion(self) -> scene_motion {
        self.motion
    }

    /// Frozen ordinal of the complete 96-candidate state matrix.
    #[must_use]
    pub const fn candidate_ordinal(self) -> u8 {
        let enabled_mask = (self.primary_enabled as u8) * 2 + self.secondary_enabled as u8;
        let disclosure = match self.status_disclosure {
            scene_disclosure::declared_public => 0,
            scene_disclosure::protected => 1,
        };
        (enabled_mask * 3 + self.focus as u8) * 8
            + disclosure * 4
            + self.depth as u8 * 2
            + self.motion as u8
    }
}

pub struct scene_candidate {
    identity: scene_identity,
    nodes: [scene_node_definition; scene_node_count],
    state: declared_scene_state,
}

impl scene_candidate {
    #[must_use]
    pub fn new(
        identity: scene_identity,
        nodes: [scene_node_definition; scene_node_count],
        state: declared_scene_state,
    ) -> Self {
        Self {
            identity,
            nodes,
            state,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.identity
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.state
    }

    #[must_use]
    pub fn node(&self, index: usize) -> Option<&scene_node_definition> {
        self.nodes.get(index)
    }

    #[must_use]
    pub fn into_nodes(self) -> [scene_node_definition; scene_node_count] {
        self.nodes
    }

    pub(crate) fn node_at(&self, index: usize) -> &scene_node_definition {
        &self.nodes[index]
    }
}

pub struct scene_control {
    instance: scene_instance,
    identity: scene_identity,
}

pub struct scene_construction_rejection {
    error: scene_error,
    candidate: scene_candidate,
}

impl scene_construction_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
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

pub struct prepared_revision {
    instance: scene_instance,
    identity: scene_identity,
    base: scene_revision,
    next: scene_revision,
    candidate: scene_candidate,
    projections: projection_storage,
}

impl prepared_revision {
    /// Borrows the renderer-facing portion of this uncommitted revision.
    ///
    /// The preview is not a committed scene projection or an authority token.
    /// It exposes no private scene brand, source candidate, machine or
    /// accessibility owner, or local action/focus handle. Protected label
    /// contents remain replaced by the fixed projection placeholder.
    #[must_use]
    pub fn preview(&self) -> prepared_render_preview<'_> {
        prepared_render_preview::new(
            self.identity,
            self.base,
            self.next,
            &self.candidate,
            &self.projections,
        )
    }

    /// Recovers the complete candidate from a still caller-owned preparation.
    ///
    /// This does not cancel a revision already installed in another service or
    /// mutate the live scene.
    #[must_use]
    pub fn into_candidate(self) -> scene_candidate {
        self.candidate
    }
}

pub struct prepare_rejection {
    error: scene_error,
    candidate: scene_candidate,
}

impl prepare_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
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

pub struct commit_rejection {
    error: scene_error,
    prepared: prepared_revision,
}

impl commit_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
        self.error
    }

    #[must_use]
    pub fn into_prepared(self) -> prepared_revision {
        self.prepared
    }
}

#[must_use = "retire the live scene owner explicitly"]
pub struct desktop_scene {
    instance: scene_instance,
    identity: scene_identity,
    revision: scene_revision,
    candidate: scene_candidate,
    projections: projection_storage,
    live: bool,
}

impl desktop_scene {
    // Rejections return the allocation-free candidate owner intact.
    #[allow(clippy::result_large_err)]
    pub fn new(
        candidate: scene_candidate,
    ) -> Result<(Self, scene_control), scene_construction_rejection> {
        let revision = match scene_revision::try_new(first_revision) {
            Ok(revision) => revision,
            Err(_) => {
                return Err(scene_construction_rejection {
                    error: scene_error::invariant_violation,
                    candidate,
                });
            }
        };
        Self::new_at_revision(candidate, revision)
    }

    #[allow(clippy::result_large_err)]
    fn new_at_revision(
        candidate: scene_candidate,
        revision: scene_revision,
    ) -> Result<(Self, scene_control), scene_construction_rejection> {
        if let Err(error) = validate_candidate(&candidate, None) {
            return Err(scene_construction_rejection { error, candidate });
        }
        let projections = match derive_projections(&candidate, revision) {
            Ok(projections) => projections,
            Err(error) => {
                return Err(scene_construction_rejection { error, candidate });
            }
        };
        let instance = match allocate_scene_instance() {
            Ok(instance) => instance,
            Err(error) => {
                return Err(scene_construction_rejection { error, candidate });
            }
        };
        let identity = candidate.identity;
        let scene = Self {
            instance,
            identity,
            revision,
            candidate,
            projections,
            live: true,
        };
        let control = scene_control { instance, identity };
        Ok((scene, control))
    }

    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn new_at_revision_for_test(
        candidate: scene_candidate,
        revision: scene_revision,
    ) -> Result<(Self, scene_control), scene_construction_rejection> {
        Self::new_at_revision(candidate, revision)
    }

    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.identity
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.candidate.state
    }

    // Rejection preserves the complete move-only candidate for retry.
    #[allow(clippy::result_large_err)]
    pub fn prepare_revision(
        &self,
        control: &scene_control,
        base: scene_revision,
        candidate: scene_candidate,
    ) -> Result<prepared_revision, prepare_rejection> {
        let reject = |error, candidate| prepare_rejection { error, candidate };
        if !self.live {
            return Err(reject(scene_error::scene_not_live, candidate));
        }
        if control.instance != self.instance || control.identity != self.identity {
            return Err(reject(scene_error::control_denied, candidate));
        }
        if base != self.revision {
            return Err(reject(scene_error::stale_revision, candidate));
        }
        let next = match self.revision.next() {
            Ok(next) => next,
            Err(_) => return Err(reject(scene_error::revision_exhausted, candidate)),
        };
        if let Err(error) = validate_candidate(&candidate, Some(&self.candidate)) {
            return Err(reject(error, candidate));
        }
        let projections = match derive_projections(&candidate, next) {
            Ok(projections) => projections,
            Err(error) => return Err(reject(error, candidate)),
        };
        Ok(prepared_revision {
            instance: self.instance,
            identity: self.identity,
            base,
            next,
            candidate,
            projections,
        })
    }

    // Rejection preserves the complete unreadable prepared owner for retry.
    #[allow(clippy::result_large_err)]
    pub fn commit_revision(
        &mut self,
        prepared: prepared_revision,
    ) -> Result<scene_revision, commit_rejection> {
        let error = if !self.live {
            Some(scene_error::scene_not_live)
        } else if prepared.instance != self.instance || prepared.identity != self.identity {
            Some(scene_error::control_denied)
        } else if prepared.base != self.revision || self.revision.next().ok() != Some(prepared.next)
        {
            Some(scene_error::stale_revision)
        } else if prepared.candidate.identity != self.identity {
            Some(scene_error::wrong_scene_identity)
        } else if !prepared
            .projections
            .parity_holds(&prepared.candidate, prepared.next)
        {
            Some(scene_error::projection_mismatch)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(commit_rejection { error, prepared });
        }

        let prepared_revision {
            next,
            candidate,
            projections,
            ..
        } = prepared;
        self.candidate = candidate;
        self.projections = projections;
        self.revision = next;
        Ok(next)
    }

    #[must_use]
    pub fn projections(&self) -> crate::projection::scene_projections<'_> {
        crate::projection::scene_projections::new(self)
    }

    pub fn resolve_action(
        &self,
        handle: local_action_handle,
    ) -> Result<action_resolution, action_resolution_rejection> {
        let error = if !self.live {
            Some(scene_error::scene_not_live)
        } else if handle.instance != self.instance {
            Some(scene_error::control_denied)
        } else if handle.reference.scene() != self.identity {
            Some(scene_error::wrong_scene_identity)
        } else if handle.reference.revision() != self.revision {
            Some(scene_error::stale_revision)
        } else {
            match self.action_enabled(handle.reference) {
                None => Some(scene_error::invalid_action_reference),
                Some(enabled) if enabled != handle.enabled => {
                    Some(scene_error::invalid_action_handle)
                }
                Some(false) => Some(scene_error::disabled_action),
                Some(true) => None,
            }
        };
        if let Some(error) = error {
            return Err(action_resolution_rejection { error, handle });
        }
        Ok(action_resolution {
            reference: handle.reference,
            source: handle.source,
        })
    }

    pub fn traverse_focus(
        &self,
        handle: local_focus_handle,
        direction: focus_direction,
    ) -> Result<focus_resolution, focus_resolution_rejection> {
        let error = if !self.live {
            Some(scene_error::scene_not_live)
        } else if handle.instance != self.instance {
            Some(scene_error::control_denied)
        } else if handle.scene != self.identity {
            Some(scene_error::wrong_scene_identity)
        } else if handle.revision != self.revision {
            Some(scene_error::stale_revision)
        } else if !valid_focus_cursor(self.candidate.state, handle.cursor) {
            Some(scene_error::invalid_focus_handle)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(focus_resolution_rejection { error, handle });
        }

        let destination = match next_focus(self.candidate.state, handle.cursor, direction) {
            Some(destination) => destination,
            None => {
                return Err(focus_resolution_rejection {
                    error: scene_error::no_enabled_focus,
                    handle,
                });
            }
        };
        let node = match destination {
            scene_focus::primary => {
                self.candidate.nodes[scene_slot::primary_action.index()].identity
            }
            scene_focus::secondary => {
                self.candidate.nodes[scene_slot::secondary_action.index()].identity
            }
            scene_focus::none => {
                return Err(focus_resolution_rejection {
                    error: scene_error::invariant_violation,
                    handle,
                });
            }
        };
        let reference = scene_focus_reference::new(self.identity, self.revision, node);
        let next_handle = local_focus_handle {
            instance: handle.instance,
            scene: handle.scene,
            revision: handle.revision,
            cursor: destination,
            source: handle.source,
        };
        Ok(focus_resolution {
            reference,
            source: handle.source,
            handle: next_handle,
        })
    }

    // Rejection returns both large move-only owners without allocation.
    #[allow(clippy::result_large_err)]
    pub fn retire(
        mut self,
        control: scene_control,
    ) -> Result<scene_retirement, retirement_rejection> {
        if !self.live {
            return Err(retirement_rejection {
                error: scene_error::scene_not_live,
                scene: self,
                control,
            });
        }
        if control.instance != self.instance || control.identity != self.identity {
            return Err(retirement_rejection {
                error: scene_error::control_denied,
                scene: self,
                control,
            });
        }
        self.live = false;
        Ok(scene_retirement {
            identity: self.identity,
            revision: self.revision,
        })
    }

    fn action_enabled(&self, reference: scene_action_reference) -> Option<bool> {
        for descriptor in self.projections.actions() {
            if descriptor.reference() == reference {
                return match reference.node() {
                    node if node
                        == self.candidate.nodes[scene_slot::primary_action.index()].identity =>
                    {
                        Some(self.candidate.state.primary_enabled)
                    }
                    node if node
                        == self.candidate.nodes[scene_slot::secondary_action.index()].identity =>
                    {
                        Some(self.candidate.state.secondary_enabled)
                    }
                    _ => None,
                };
            }
        }
        None
    }

    pub(crate) const fn instance(&self) -> scene_instance {
        self.instance
    }

    pub(crate) fn candidate(&self) -> &scene_candidate {
        &self.candidate
    }

    pub(crate) fn projection_storage(&self) -> &projection_storage {
        &self.projections
    }
}

pub struct local_action_handle {
    instance: scene_instance,
    reference: scene_action_reference,
    enabled: bool,
    source: scene_resolution_source,
}

impl local_action_handle {
    pub(crate) const fn new(
        instance: scene_instance,
        reference: scene_action_reference,
        enabled: bool,
        source: scene_resolution_source,
    ) -> Self {
        Self {
            instance,
            reference,
            enabled,
            source,
        }
    }

    #[must_use]
    pub fn reference(&self) -> scene_action_reference {
        self.reference
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.source
    }
}

pub struct action_resolution {
    reference: scene_action_reference,
    source: scene_resolution_source,
}

impl action_resolution {
    #[must_use]
    pub fn reference(&self) -> scene_action_reference {
        self.reference
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.source
    }

    #[must_use]
    pub const fn effect_performed(&self) -> bool {
        false
    }
}

pub struct action_resolution_rejection {
    error: scene_error,
    handle: local_action_handle,
}

impl action_resolution_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
        self.error
    }

    #[must_use]
    pub fn into_handle(self) -> local_action_handle {
        self.handle
    }
}

pub struct local_focus_handle {
    instance: scene_instance,
    scene: scene_identity,
    revision: scene_revision,
    cursor: scene_focus,
    source: scene_resolution_source,
}

impl local_focus_handle {
    pub(crate) const fn new(
        instance: scene_instance,
        scene: scene_identity,
        revision: scene_revision,
        cursor: scene_focus,
        source: scene_resolution_source,
    ) -> Self {
        Self {
            instance,
            scene,
            revision,
            cursor,
            source,
        }
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.source
    }

    #[must_use]
    pub fn cursor(&self) -> scene_focus {
        self.cursor
    }
}

pub struct focus_resolution {
    reference: scene_focus_reference,
    source: scene_resolution_source,
    handle: local_focus_handle,
}

impl focus_resolution {
    #[must_use]
    pub fn reference(&self) -> scene_focus_reference {
        self.reference
    }

    #[must_use]
    pub fn source(&self) -> scene_resolution_source {
        self.source
    }

    #[must_use]
    pub const fn scene_focus_changed(&self) -> bool {
        false
    }

    #[must_use]
    pub fn handle(&self) -> &local_focus_handle {
        &self.handle
    }

    #[must_use]
    pub fn into_handle(self) -> local_focus_handle {
        self.handle
    }
}

pub struct focus_resolution_rejection {
    error: scene_error,
    handle: local_focus_handle,
}

impl focus_resolution_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
        self.error
    }

    #[must_use]
    pub fn into_handle(self) -> local_focus_handle {
        self.handle
    }
}

pub struct scene_retirement {
    identity: scene_identity,
    revision: scene_revision,
}

impl scene_retirement {
    #[must_use]
    pub fn identity(&self) -> scene_identity {
        self.identity
    }

    #[must_use]
    pub fn final_revision(&self) -> scene_revision {
        self.revision
    }
}

pub struct retirement_rejection {
    error: scene_error,
    scene: desktop_scene,
    control: scene_control,
}

impl retirement_rejection {
    #[must_use]
    pub fn error(&self) -> scene_error {
        self.error
    }

    pub fn into_parts(self) -> (desktop_scene, scene_control) {
        (self.scene, self.control)
    }
}

fn validate_candidate(
    candidate: &scene_candidate,
    current: Option<&scene_candidate>,
) -> Result<(), scene_error> {
    if current.is_some_and(|owner| candidate.identity != owner.identity) {
        return Err(scene_error::wrong_scene_identity);
    }

    match (
        candidate.nodes[scene_slot::primary_action.index()].action,
        candidate.nodes[scene_slot::secondary_action.index()].action,
    ) {
        (Some(primary), Some(secondary)) if primary.identity == secondary.identity => {
            return Err(scene_error::duplicate_action_identity);
        }
        _ => {}
    }

    for index in 0..scene_node_count {
        let node = &candidate.nodes[index];
        let expected_slot = scene_slot::canonical[index];
        if node.slot != expected_slot {
            return Err(scene_error::wrong_slot);
        }
        for previous in 0..index {
            if candidate.nodes[previous].identity == node.identity {
                return Err(scene_error::duplicate_node_identity);
            }
        }
        if node.identity.slot() != expected_slot.identity_slot() {
            return Err(scene_error::wrong_node_identity);
        }
        if node.identity.generation() != candidate.identity.generation() {
            return Err(scene_error::wrong_identity_generation);
        }
        if node.role != canonical_roles[index] {
            return Err(scene_error::wrong_role);
        }
        let expected_parent = canonical_parent_indices[index]
            .map(|parent_index| candidate.nodes[parent_index].identity);
        if node.parent != expected_parent {
            return Err(scene_error::wrong_parent);
        }
        if node.bounds != canonical_geometry[index] {
            return Err(scene_error::wrong_geometry);
        }
        if node.bounds.x.checked_add(node.bounds.width).is_none()
            || node.bounds.y.checked_add(node.bounds.height).is_none()
            || node.bounds.x + node.bounds.width > viewport.width
            || node.bounds.y + node.bounds.height > viewport.height
        {
            return Err(scene_error::wrong_geometry);
        }
        if node.label.is_empty() {
            return Err(scene_error::empty_label);
        }

        let expected_action_slot = match expected_slot {
            scene_slot::primary_action => Some(1),
            scene_slot::secondary_action => Some(2),
            _ => None,
        };
        match (node.action, expected_action_slot) {
            (Some(action), Some(slot)) => {
                if action.identity.slot() != slot {
                    return Err(scene_error::wrong_action_identity);
                }
                if action.identity.generation() != candidate.identity.generation() {
                    return Err(scene_error::wrong_identity_generation);
                }
                if action.kind != scene_action_kind::activate {
                    return Err(scene_error::wrong_action);
                }
            }
            (None, None) => {}
            _ => return Err(scene_error::wrong_action),
        }

        if expected_slot != scene_slot::status && node.label.is_protected() {
            return Err(scene_error::protected_label_not_allowed);
        }
        if let Some(current) = current {
            let old = &current.nodes[index];
            if node.identity != old.identity {
                return Err(scene_error::wrong_node_identity);
            }
            if node.action.map(|action| action.identity) != old.action.map(|action| action.identity)
            {
                return Err(scene_error::wrong_action_identity);
            }
        }
    }

    let primary_action = candidate.nodes[scene_slot::primary_action.index()]
        .action
        .ok_or(scene_error::invariant_violation)?;
    let secondary_action = candidate.nodes[scene_slot::secondary_action.index()]
        .action
        .ok_or(scene_error::invariant_violation)?;
    if primary_action.identity == secondary_action.identity {
        return Err(scene_error::duplicate_action_identity);
    }

    let status = &candidate.nodes[scene_slot::status.index()];
    if status.disclosure() != candidate.state.status_disclosure {
        return Err(scene_error::wrong_disclosure);
    }
    match candidate.state.focus {
        scene_focus::none => {}
        scene_focus::primary if candidate.state.primary_enabled => {}
        scene_focus::secondary if candidate.state.secondary_enabled => {}
        scene_focus::primary | scene_focus::secondary => return Err(scene_error::focus_disabled),
    }
    Ok(())
}

fn valid_focus_cursor(state: declared_scene_state, cursor: scene_focus) -> bool {
    match cursor {
        scene_focus::none => true,
        scene_focus::primary => state.primary_enabled,
        scene_focus::secondary => state.secondary_enabled,
    }
}

fn next_focus(
    state: declared_scene_state,
    cursor: scene_focus,
    direction: focus_direction,
) -> Option<scene_focus> {
    let primary = state.primary_enabled;
    let secondary = state.secondary_enabled;
    if !primary && !secondary {
        return None;
    }
    match (cursor, direction) {
        (scene_focus::none, focus_direction::forward) => Some(if primary {
            scene_focus::primary
        } else {
            scene_focus::secondary
        }),
        (scene_focus::none, focus_direction::reverse) => Some(if secondary {
            scene_focus::secondary
        } else {
            scene_focus::primary
        }),
        (scene_focus::primary, focus_direction::forward | focus_direction::reverse) => {
            Some(if secondary {
                scene_focus::secondary
            } else {
                scene_focus::primary
            })
        }
        (scene_focus::secondary, focus_direction::forward | focus_direction::reverse) => {
            Some(if primary {
                scene_focus::primary
            } else {
                scene_focus::secondary
            })
        }
    }
}
