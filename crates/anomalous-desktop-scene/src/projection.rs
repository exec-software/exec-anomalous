use crate::identity::{action_identity, node_identity, scene_identity, scene_revision};
use crate::scene::{
    declared_scene_state, desktop_scene, local_action_handle, local_focus_handle,
    scene_action_kind, scene_candidate, scene_depth, scene_disclosure, scene_error, scene_focus,
    scene_node_count, scene_rect, scene_resolution_source, scene_role, scene_slot, viewport,
};

pub const standard_paint_command_count: usize = 25;
pub const flat_paint_command_count: usize = 21;
pub const accessibility_node_count: usize = 8;
pub const machine_node_count: usize = 8;
pub const machine_action_count: usize = 2;

const flat_removed_standard_indices: [usize; 4] = [4, 6, 19, 21];

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct rgb8 {
    red: u8,
    green: u8,
    blue: u8,
}

impl rgb8 {
    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum paint_color_token {
    canvas,
    surface,
    primary_fill,
    secondary_fill,
    disabled,
    primary_text,
    secondary_text,
    on_accent,
    outline,
    focus_outer,
    focus_inner,
    public_status,
    protected_status,
    shadow,
    highlight,
}

impl paint_color_token {
    #[must_use]
    pub const fn rgb(self) -> rgb8 {
        let (red, green, blue) = match self {
            Self::canvas => (0xe7, 0xec, 0xf3),
            Self::surface => (0xf5, 0xf7, 0xfb),
            Self::primary_fill => (0x1e, 0x40, 0xaf),
            Self::secondary_fill => (0xe2, 0xe8, 0xf0),
            Self::disabled => (0xcb, 0xd5, 0xe1),
            Self::primary_text => (0x17, 0x20, 0x33),
            Self::secondary_text => (0x40, 0x51, 0x68),
            Self::on_accent => (0xff, 0xff, 0xff),
            Self::outline => (0x52, 0x65, 0x7a),
            Self::focus_outer => (0x11, 0x18, 0x27),
            Self::focus_inner => (0xff, 0xff, 0xff),
            Self::public_status => (0x14, 0x53, 0x2d),
            Self::protected_status => (0x6b, 0x21, 0xa8),
            Self::shadow => (0xa9, 0xb4, 0xc4),
            Self::highlight => (0xff, 0xff, 0xff),
        };
        rgb8 { red, green, blue }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum paint_command_kind {
    fill,
    outline,
    label,
    enabled_marker,
    focus_outline,
    shadow,
    highlight,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum paint_shape_token {
    rectangle,
    text,
    enabled,
    disabled,
    focus_absent,
    focus_double_outline,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_action_reference {
    scene: scene_identity,
    revision: scene_revision,
    node: node_identity,
    action: action_identity,
    kind: scene_action_kind,
}

impl scene_action_reference {
    pub(crate) const fn new(
        scene: scene_identity,
        revision: scene_revision,
        node: node_identity,
        action: action_identity,
        kind: scene_action_kind,
    ) -> Self {
        Self {
            scene,
            revision,
            node,
            action,
            kind,
        }
    }

    #[must_use]
    pub const fn scene(self) -> scene_identity {
        self.scene
    }

    #[must_use]
    pub const fn revision(self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub const fn node(self) -> node_identity {
        self.node
    }

    #[must_use]
    pub const fn action(self) -> action_identity {
        self.action
    }

    #[must_use]
    pub const fn kind(self) -> scene_action_kind {
        self.kind
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct scene_focus_reference {
    scene: scene_identity,
    revision: scene_revision,
    node: node_identity,
}

impl scene_focus_reference {
    pub(crate) const fn new(
        scene: scene_identity,
        revision: scene_revision,
        node: node_identity,
    ) -> Self {
        Self {
            scene,
            revision,
            node,
        }
    }

    #[must_use]
    pub const fn scene(self) -> scene_identity {
        self.scene
    }

    #[must_use]
    pub const fn revision(self) -> scene_revision {
        self.revision
    }

    #[must_use]
    pub const fn node(self) -> node_identity {
        self.node
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct paint_command {
    kind: paint_command_kind,
    shape: paint_shape_token,
    color: paint_color_token,
    secondary_color: Option<paint_color_token>,
    slot: scene_slot,
    node: node_identity,
    bounds: scene_rect,
    state_active: bool,
    action: Option<scene_action_reference>,
}

impl paint_command {
    #[must_use]
    pub const fn kind(self) -> paint_command_kind {
        self.kind
    }

    #[must_use]
    pub const fn shape(self) -> paint_shape_token {
        self.shape
    }

    #[must_use]
    pub const fn color(self) -> paint_color_token {
        self.color
    }

    #[must_use]
    pub const fn secondary_color(self) -> Option<paint_color_token> {
        self.secondary_color
    }

    #[must_use]
    pub const fn slot(self) -> scene_slot {
        self.slot
    }

    #[must_use]
    pub const fn node(self) -> node_identity {
        self.node
    }

    #[must_use]
    pub const fn bounds(self) -> scene_rect {
        self.bounds
    }

    /// Reports the state asserted by a fixed state-command slot.
    ///
    /// This is never a command-presence or renderer-skip flag. In particular,
    /// a false enabled marker still renders its `disabled` shape.
    #[must_use]
    pub const fn state_active(self) -> bool {
        self.state_active
    }

    #[must_use]
    pub const fn action(self) -> Option<scene_action_reference> {
        self.action
    }
}

/// One authority-free paint command borrowed from an uncommitted revision.
///
/// Unlike [`paint_command`], this renderer-facing value deliberately omits the
/// revision-bound action reference. The scene identity and next revision are
/// available only from the containing [`prepared_render_preview`].
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct prepared_paint_command {
    kind: paint_command_kind,
    shape: paint_shape_token,
    color: paint_color_token,
    secondary_color: Option<paint_color_token>,
    slot: scene_slot,
    node: node_identity,
    bounds: scene_rect,
    state_active: bool,
}

impl prepared_paint_command {
    const fn from_paint(command: paint_command) -> Self {
        Self {
            kind: command.kind,
            shape: command.shape,
            color: command.color,
            secondary_color: command.secondary_color,
            slot: command.slot,
            node: command.node,
            bounds: command.bounds,
            state_active: command.state_active,
        }
    }

    #[must_use]
    pub const fn kind(self) -> paint_command_kind {
        self.kind
    }

    #[must_use]
    pub const fn shape(self) -> paint_shape_token {
        self.shape
    }

    #[must_use]
    pub const fn color(self) -> paint_color_token {
        self.color
    }

    #[must_use]
    pub const fn secondary_color(self) -> Option<paint_color_token> {
        self.secondary_color
    }

    #[must_use]
    pub const fn slot(self) -> scene_slot {
        self.slot
    }

    #[must_use]
    pub const fn node(self) -> node_identity {
        self.node
    }

    #[must_use]
    pub const fn bounds(self) -> scene_rect {
        self.bounds
    }

    /// Reports the state asserted by a fixed state-command slot.
    ///
    /// This is never a command-presence or renderer-skip flag.
    #[must_use]
    pub const fn state_active(self) -> bool {
        self.state_active
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct accessibility_node {
    slot: scene_slot,
    node: node_identity,
    parent: Option<node_identity>,
    role: scene_role,
    bounds: scene_rect,
    disclosure: scene_disclosure,
    enabled: bool,
    focused: bool,
    focus_order: Option<u8>,
    action: Option<scene_action_reference>,
}

macro_rules! semantic_node_accessors {
    () => {
        #[must_use]
        pub const fn slot(self) -> scene_slot {
            self.slot
        }

        #[must_use]
        pub const fn node(self) -> node_identity {
            self.node
        }

        #[must_use]
        pub const fn parent(self) -> Option<node_identity> {
            self.parent
        }

        #[must_use]
        pub const fn role(self) -> scene_role {
            self.role
        }

        #[must_use]
        pub const fn bounds(self) -> scene_rect {
            self.bounds
        }

        #[must_use]
        pub const fn disclosure(self) -> scene_disclosure {
            self.disclosure
        }

        #[must_use]
        pub const fn enabled(self) -> bool {
            self.enabled
        }

        #[must_use]
        pub const fn focused(self) -> bool {
            self.focused
        }

        #[must_use]
        pub const fn focus_order(self) -> Option<u8> {
            self.focus_order
        }

        #[must_use]
        pub const fn action(self) -> Option<scene_action_reference> {
            self.action
        }
    };
}

impl accessibility_node {
    semantic_node_accessors!();
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct machine_node {
    slot: scene_slot,
    node: node_identity,
    parent: Option<node_identity>,
    role: scene_role,
    bounds: scene_rect,
    disclosure: scene_disclosure,
    enabled: bool,
    focused: bool,
    focus_order: Option<u8>,
    action: Option<scene_action_reference>,
}

impl machine_node {
    semantic_node_accessors!();
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct machine_action_descriptor {
    reference: scene_action_reference,
    enabled: bool,
    focused: bool,
}

impl machine_action_descriptor {
    #[must_use]
    pub const fn reference(self) -> scene_action_reference {
        self.reference
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn focused(self) -> bool {
        self.focused
    }
}

pub struct projection_label<'a> {
    disclosure: scene_disclosure,
    text: &'a str,
}

impl<'a> projection_label<'a> {
    #[must_use]
    pub const fn disclosure(&self) -> scene_disclosure {
        self.disclosure
    }

    /// Exposes declared-public text or the fixed protected placeholder.
    #[must_use]
    pub const fn expose_str(&self) -> &'a str {
        self.text
    }
}

pub(crate) struct projection_storage {
    identity: scene_identity,
    revision: scene_revision,
    state: declared_scene_state,
    paints: [paint_command; standard_paint_command_count],
    paint_count: usize,
    accessibility: [accessibility_node; accessibility_node_count],
    machine: [machine_node; machine_node_count],
    actions: [machine_action_descriptor; machine_action_count],
}

impl projection_storage {
    pub(crate) fn actions(&self) -> &[machine_action_descriptor; machine_action_count] {
        &self.actions
    }

    pub(crate) fn parity_holds(
        &self,
        candidate: &scene_candidate,
        revision: scene_revision,
    ) -> bool {
        if self.identity != candidate.identity()
            || self.revision != revision
            || self.state != candidate.state()
            || self.paint_count
                != match candidate.state().depth() {
                    scene_depth::standard => standard_paint_command_count,
                    scene_depth::flat => flat_paint_command_count,
                }
        {
            return false;
        }
        for index in 0..scene_node_count {
            let accessibility = self.accessibility[index];
            let machine = self.machine[index];
            if accessibility.node != machine.node
                || accessibility.slot != machine.slot
                || accessibility.parent != machine.parent
                || accessibility.role != machine.role
                || accessibility.bounds != machine.bounds
                || accessibility.disclosure != machine.disclosure
                || accessibility.enabled != machine.enabled
                || accessibility.focused != machine.focused
                || accessibility.focus_order != machine.focus_order
                || accessibility.action != machine.action
            {
                return false;
            }
            let node = candidate.node_at(index);
            let expected_action = node.action().map(|action| {
                scene_action_reference::new(
                    candidate.identity(),
                    revision,
                    node.identity(),
                    action.identity(),
                    action.kind(),
                )
            });
            let (expected_enabled, expected_focused, expected_focus_order) = match node.slot() {
                scene_slot::primary_action => (
                    candidate.state().primary_enabled(),
                    candidate.state().focus() == scene_focus::primary,
                    Some(0),
                ),
                scene_slot::secondary_action => (
                    candidate.state().secondary_enabled(),
                    candidate.state().focus() == scene_focus::secondary,
                    Some(1),
                ),
                _ => (true, false, None),
            };
            if accessibility.node != node.identity()
                || accessibility.slot != node.slot()
                || accessibility.parent != node.parent()
                || accessibility.role != node.role()
                || accessibility.bounds != node.bounds()
                || accessibility.disclosure != node.disclosure()
                || accessibility.enabled != expected_enabled
                || accessibility.focused != expected_focused
                || accessibility.focus_order != expected_focus_order
                || accessibility.action != expected_action
            {
                return false;
            }
        }
        for (action_index, node_index) in [
            scene_slot::primary_action.index(),
            scene_slot::secondary_action.index(),
        ]
        .into_iter()
        .enumerate()
        {
            let descriptor = self.actions[action_index];
            let accessible = self.accessibility[node_index];
            let machine = self.machine[node_index];
            let expected_enabled = if action_index == 0 {
                candidate.state().primary_enabled()
            } else {
                candidate.state().secondary_enabled()
            };
            let expected_focused = candidate.state().focus()
                == if action_index == 0 {
                    scene_focus::primary
                } else {
                    scene_focus::secondary
                };
            let candidate_action = candidate.node_at(node_index).action().map(|action| {
                scene_action_reference::new(
                    candidate.identity(),
                    revision,
                    candidate.node_at(node_index).identity(),
                    action.identity(),
                    action.kind(),
                )
            });
            if accessible.action != Some(descriptor.reference)
                || machine.action != Some(descriptor.reference)
                || candidate_action != Some(descriptor.reference)
                || accessible.enabled != descriptor.enabled
                || machine.enabled != descriptor.enabled
                || descriptor.enabled != expected_enabled
                || accessible.focused != descriptor.focused
                || machine.focused != descriptor.focused
                || descriptor.focused != expected_focused
            {
                return false;
            }
        }
        for command in &self.paints[..self.paint_count] {
            let node_index = command.slot.index();
            let node = candidate.node_at(node_index);
            let semantic = self.machine[node_index];
            if command.slot != semantic.slot
                || command.node != node.identity()
                || command.node != semantic.node
                || command.bounds != node.bounds()
                || command.bounds != semantic.bounds
            {
                return false;
            }
            match command.slot {
                scene_slot::primary_action | scene_slot::secondary_action => {
                    if command.action != semantic.action {
                        return false;
                    }
                }
                _ if command.action.is_some() => return false,
                _ => {}
            }
        }
        true
    }
}

pub(crate) fn derive_projections(
    candidate: &scene_candidate,
    revision: scene_revision,
) -> Result<projection_storage, scene_error> {
    let identity = candidate.identity();
    let state = candidate.state();
    let first = semantic_record(candidate, revision, 0)?;
    let mut accessibility = [first.0; accessibility_node_count];
    let mut machine = [first.1; machine_node_count];
    for index in 0..scene_node_count {
        let (accessible, semantic) = semantic_record(candidate, revision, index)?;
        accessibility[index] = accessible;
        machine[index] = semantic;
    }

    let primary_reference = machine[scene_slot::primary_action.index()]
        .action
        .ok_or(scene_error::projection_mismatch)?;
    let secondary_reference = machine[scene_slot::secondary_action.index()]
        .action
        .ok_or(scene_error::projection_mismatch)?;
    let actions = [
        machine_action_descriptor {
            reference: primary_reference,
            enabled: state.primary_enabled(),
            focused: state.focus() == scene_focus::primary,
        },
        machine_action_descriptor {
            reference: secondary_reference,
            enabled: state.secondary_enabled(),
            focused: state.focus() == scene_focus::secondary,
        },
    ];

    let standard = standard_paint_plan(candidate, revision, primary_reference, secondary_reference);
    let (paints, paint_count) = match state.depth() {
        scene_depth::standard => (standard, standard_paint_command_count),
        scene_depth::flat => {
            let mut flat = [standard[0]; standard_paint_command_count];
            let mut destination = 0;
            for (index, command) in standard.into_iter().enumerate() {
                if flat_removed_standard_indices.contains(&index) {
                    continue;
                }
                flat[destination] = command;
                destination += 1;
            }
            if destination != flat_paint_command_count {
                return Err(scene_error::invariant_violation);
            }
            (flat, destination)
        }
    };

    let storage = projection_storage {
        identity,
        revision,
        state,
        paints,
        paint_count,
        accessibility,
        machine,
        actions,
    };
    if !storage.parity_holds(candidate, revision) {
        return Err(scene_error::projection_mismatch);
    }
    Ok(storage)
}

/// Borrowed renderer-facing view of one completely validated, uncommitted
/// scene revision.
///
/// This view is descriptive data only. It cannot commit the revision, mint an
/// action or focus handle, resolve an effect, or expose accessibility or
/// machine projection owners. Labels have already passed through the same
/// projection boundary as committed labels, so protected contents appear only
/// as the fixed redaction placeholder.
pub struct prepared_render_preview<'a> {
    identity: scene_identity,
    base: scene_revision,
    next: scene_revision,
    candidate: &'a scene_candidate,
    projections: &'a projection_storage,
}

impl<'a> prepared_render_preview<'a> {
    pub(crate) const fn new(
        identity: scene_identity,
        base: scene_revision,
        next: scene_revision,
        candidate: &'a scene_candidate,
        projections: &'a projection_storage,
    ) -> Self {
        Self {
            identity,
            base,
            next,
            candidate,
            projections,
        }
    }

    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.identity
    }

    #[must_use]
    pub const fn base_revision(&self) -> scene_revision {
        self.base
    }

    #[must_use]
    pub const fn next_revision(&self) -> scene_revision {
        self.next
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.projections.state
    }

    #[must_use]
    pub const fn viewport(&self) -> scene_rect {
        viewport
    }

    #[must_use]
    pub const fn command_count(&self) -> usize {
        self.projections.paint_count
    }

    #[must_use]
    pub fn command(&self, index: usize) -> Option<prepared_paint_command> {
        if index >= self.command_count() {
            return None;
        }
        Some(prepared_paint_command::from_paint(
            self.projections.paints[index],
        ))
    }

    #[must_use]
    pub fn label(&self, index: usize) -> Option<projection_label<'a>> {
        projected_label(self.candidate, index)
    }
}

fn semantic_record(
    candidate: &scene_candidate,
    revision: scene_revision,
    index: usize,
) -> Result<(accessibility_node, machine_node), scene_error> {
    let node = candidate.node_at(index);
    let slot = node.slot();
    let (enabled, focused, focus_order) = match slot {
        scene_slot::primary_action => (
            candidate.state().primary_enabled(),
            candidate.state().focus() == scene_focus::primary,
            Some(0),
        ),
        scene_slot::secondary_action => (
            candidate.state().secondary_enabled(),
            candidate.state().focus() == scene_focus::secondary,
            Some(1),
        ),
        _ => (true, false, None),
    };
    let action = node.action().map(|action| {
        scene_action_reference::new(
            candidate.identity(),
            revision,
            node.identity(),
            action.identity(),
            action.kind(),
        )
    });
    let accessible = accessibility_node {
        slot,
        node: node.identity(),
        parent: node.parent(),
        role: node.role(),
        bounds: node.bounds(),
        disclosure: node.disclosure(),
        enabled,
        focused,
        focus_order,
        action,
    };
    let machine = machine_node {
        slot: accessible.slot,
        node: accessible.node,
        parent: accessible.parent,
        role: accessible.role,
        bounds: accessible.bounds,
        disclosure: accessible.disclosure,
        enabled: accessible.enabled,
        focused: accessible.focused,
        focus_order: accessible.focus_order,
        action: accessible.action,
    };
    Ok((accessible, machine))
}

fn standard_paint_plan(
    candidate: &scene_candidate,
    _revision: scene_revision,
    primary_action: scene_action_reference,
    secondary_action: scene_action_reference,
) -> [paint_command; standard_paint_command_count] {
    let root = candidate.node_at(scene_slot::root.index());
    let masthead = candidate.node_at(scene_slot::masthead.index());
    let navigation = candidate.node_at(scene_slot::navigation.index());
    let primary = candidate.node_at(scene_slot::primary_action.index());
    let secondary = candidate.node_at(scene_slot::secondary_action.index());
    let content = candidate.node_at(scene_slot::content.index());
    let heading = candidate.node_at(scene_slot::heading.index());
    let status = candidate.node_at(scene_slot::status.index());
    let state = candidate.state();

    let plain = |kind, shape, color, node: &crate::scene::scene_node_definition| paint_command {
        kind,
        shape,
        color,
        secondary_color: None,
        slot: node.slot(),
        node: node.identity(),
        bounds: node.bounds(),
        state_active: true,
        action: None,
    };
    let control =
        |kind, shape, color, node: &crate::scene::scene_node_definition, action| paint_command {
            kind,
            shape,
            color,
            secondary_color: if matches!(shape, paint_shape_token::focus_double_outline) {
                Some(paint_color_token::focus_inner)
            } else {
                None
            },
            slot: node.slot(),
            node: node.identity(),
            bounds: node.bounds(),
            state_active: match kind {
                paint_command_kind::enabled_marker => {
                    matches!(shape, paint_shape_token::enabled)
                }
                paint_command_kind::focus_outline => {
                    matches!(shape, paint_shape_token::focus_double_outline)
                }
                _ => true,
            },
            action: Some(action),
        };

    let primary_fill_color = if state.primary_enabled() {
        paint_color_token::primary_fill
    } else {
        paint_color_token::disabled
    };
    let secondary_fill_color = if state.secondary_enabled() {
        paint_color_token::secondary_fill
    } else {
        paint_color_token::disabled
    };
    let primary_label = if state.primary_enabled() {
        paint_color_token::on_accent
    } else {
        paint_color_token::secondary_text
    };
    let primary_marker = if state.primary_enabled() {
        (paint_shape_token::enabled, paint_color_token::on_accent)
    } else {
        (paint_shape_token::disabled, paint_color_token::outline)
    };
    let secondary_marker = if state.secondary_enabled() {
        (paint_shape_token::enabled, paint_color_token::public_status)
    } else {
        (paint_shape_token::disabled, paint_color_token::outline)
    };
    let primary_focus = if state.focus() == scene_focus::primary {
        (
            paint_shape_token::focus_double_outline,
            paint_color_token::focus_outer,
        )
    } else {
        (paint_shape_token::focus_absent, paint_color_token::outline)
    };
    let secondary_focus = if state.focus() == scene_focus::secondary {
        (
            paint_shape_token::focus_double_outline,
            paint_color_token::focus_outer,
        )
    } else {
        (paint_shape_token::focus_absent, paint_color_token::outline)
    };
    let status_color = match state.status_disclosure() {
        scene_disclosure::declared_public => paint_color_token::public_status,
        scene_disclosure::protected => paint_color_token::protected_status,
    };

    [
        plain(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            paint_color_token::canvas,
            root,
        ),
        plain(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            paint_color_token::surface,
            masthead,
        ),
        plain(
            paint_command_kind::outline,
            paint_shape_token::rectangle,
            paint_color_token::outline,
            masthead,
        ),
        plain(
            paint_command_kind::label,
            paint_shape_token::text,
            paint_color_token::primary_text,
            masthead,
        ),
        plain(
            paint_command_kind::shadow,
            paint_shape_token::rectangle,
            paint_color_token::shadow,
            navigation,
        ),
        plain(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            paint_color_token::surface,
            navigation,
        ),
        plain(
            paint_command_kind::highlight,
            paint_shape_token::rectangle,
            paint_color_token::highlight,
            navigation,
        ),
        plain(
            paint_command_kind::outline,
            paint_shape_token::rectangle,
            paint_color_token::outline,
            navigation,
        ),
        plain(
            paint_command_kind::label,
            paint_shape_token::text,
            paint_color_token::primary_text,
            navigation,
        ),
        control(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            primary_fill_color,
            primary,
            primary_action,
        ),
        control(
            paint_command_kind::outline,
            paint_shape_token::rectangle,
            paint_color_token::outline,
            primary,
            primary_action,
        ),
        control(
            paint_command_kind::label,
            paint_shape_token::text,
            primary_label,
            primary,
            primary_action,
        ),
        control(
            paint_command_kind::enabled_marker,
            primary_marker.0,
            primary_marker.1,
            primary,
            primary_action,
        ),
        control(
            paint_command_kind::focus_outline,
            primary_focus.0,
            primary_focus.1,
            primary,
            primary_action,
        ),
        control(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            secondary_fill_color,
            secondary,
            secondary_action,
        ),
        control(
            paint_command_kind::outline,
            paint_shape_token::rectangle,
            paint_color_token::outline,
            secondary,
            secondary_action,
        ),
        control(
            paint_command_kind::label,
            paint_shape_token::text,
            paint_color_token::primary_text,
            secondary,
            secondary_action,
        ),
        control(
            paint_command_kind::enabled_marker,
            secondary_marker.0,
            secondary_marker.1,
            secondary,
            secondary_action,
        ),
        control(
            paint_command_kind::focus_outline,
            secondary_focus.0,
            secondary_focus.1,
            secondary,
            secondary_action,
        ),
        plain(
            paint_command_kind::shadow,
            paint_shape_token::rectangle,
            paint_color_token::shadow,
            content,
        ),
        plain(
            paint_command_kind::fill,
            paint_shape_token::rectangle,
            paint_color_token::surface,
            content,
        ),
        plain(
            paint_command_kind::highlight,
            paint_shape_token::rectangle,
            paint_color_token::highlight,
            content,
        ),
        plain(
            paint_command_kind::outline,
            paint_shape_token::rectangle,
            paint_color_token::outline,
            content,
        ),
        plain(
            paint_command_kind::label,
            paint_shape_token::text,
            paint_color_token::primary_text,
            heading,
        ),
        plain(
            paint_command_kind::label,
            paint_shape_token::text,
            status_color,
            status,
        ),
    ]
}

pub struct scene_projections<'a> {
    scene: &'a desktop_scene,
}

impl<'a> scene_projections<'a> {
    pub(crate) const fn new(scene: &'a desktop_scene) -> Self {
        Self { scene }
    }

    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.scene.identity()
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.scene.revision()
    }

    #[must_use]
    pub fn render_plan(&self) -> accessible_render_plan<'a> {
        accessible_render_plan { scene: self.scene }
    }

    #[must_use]
    pub fn machine_projection(&self) -> machine_scene_projection<'a> {
        machine_scene_projection { scene: self.scene }
    }
}

pub struct accessible_render_plan<'a> {
    scene: &'a desktop_scene,
}

impl<'a> accessible_render_plan<'a> {
    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.scene.identity()
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.scene.revision()
    }

    #[must_use]
    pub const fn viewport(&self) -> scene_rect {
        viewport
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.scene.state()
    }

    #[must_use]
    pub fn command_count(&self) -> usize {
        self.scene.projection_storage().paint_count
    }

    #[must_use]
    pub fn command(&self, index: usize) -> Option<paint_command> {
        if index >= self.command_count() {
            return None;
        }
        Some(self.scene.projection_storage().paints[index])
    }

    #[must_use]
    pub const fn accessibility_node_count(&self) -> usize {
        accessibility_node_count
    }

    #[must_use]
    pub fn accessibility_node(&self, index: usize) -> Option<accessibility_node> {
        self.scene
            .projection_storage()
            .accessibility
            .get(index)
            .copied()
    }

    #[must_use]
    pub fn label(&self, index: usize) -> Option<projection_label<'a>> {
        projected_label(self.scene.candidate(), index)
    }

    pub fn mint_action_handle(
        &self,
        reference: scene_action_reference,
    ) -> Result<local_action_handle, scene_error> {
        if !self
            .scene
            .projection_storage()
            .actions
            .iter()
            .any(|descriptor| descriptor.reference == reference)
        {
            return Err(scene_error::invalid_action_reference);
        }
        let enabled = self
            .scene
            .projection_storage()
            .actions
            .iter()
            .find(|descriptor| descriptor.reference == reference)
            .map(|descriptor| descriptor.enabled)
            .ok_or(scene_error::invalid_action_reference)?;
        Ok(local_action_handle::new(
            self.scene.instance(),
            reference,
            enabled,
            scene_resolution_source::accessible_projection,
        ))
    }

    #[must_use]
    pub fn mint_focus_handle(&self) -> local_focus_handle {
        local_focus_handle::new(
            self.scene.instance(),
            self.scene.identity(),
            self.scene.revision(),
            self.scene.state().focus(),
            scene_resolution_source::accessible_projection,
        )
    }
}

pub struct machine_scene_projection<'a> {
    scene: &'a desktop_scene,
}

impl<'a> machine_scene_projection<'a> {
    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.scene.identity()
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.scene.revision()
    }

    #[must_use]
    pub const fn viewport(&self) -> scene_rect {
        viewport
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.scene.state()
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        machine_node_count
    }

    #[must_use]
    pub fn node(&self, index: usize) -> Option<machine_node> {
        self.scene.projection_storage().machine.get(index).copied()
    }

    #[must_use]
    pub fn label(&self, index: usize) -> Option<projection_label<'a>> {
        projected_label(self.scene.candidate(), index)
    }

    #[must_use]
    pub const fn action_count(&self) -> usize {
        machine_action_count
    }

    #[must_use]
    pub fn action(&self, index: usize) -> Option<machine_action_descriptor> {
        self.scene.projection_storage().actions.get(index).copied()
    }

    pub fn mint_action_handle(
        &self,
        reference: scene_action_reference,
    ) -> Result<local_action_handle, scene_error> {
        if !self
            .scene
            .projection_storage()
            .actions
            .iter()
            .any(|descriptor| descriptor.reference == reference)
        {
            return Err(scene_error::invalid_action_reference);
        }
        let enabled = self
            .scene
            .projection_storage()
            .actions
            .iter()
            .find(|descriptor| descriptor.reference == reference)
            .map(|descriptor| descriptor.enabled)
            .ok_or(scene_error::invalid_action_reference)?;
        Ok(local_action_handle::new(
            self.scene.instance(),
            reference,
            enabled,
            scene_resolution_source::machine_projection,
        ))
    }

    #[must_use]
    pub fn mint_focus_handle(&self) -> local_focus_handle {
        local_focus_handle::new(
            self.scene.instance(),
            self.scene.identity(),
            self.scene.revision(),
            self.scene.state().focus(),
            scene_resolution_source::machine_projection,
        )
    }
}

fn projected_label(candidate: &scene_candidate, index: usize) -> Option<projection_label<'_>> {
    let node = candidate.node(index)?;
    Some(projection_label {
        disclosure: node.disclosure(),
        text: node.label().projected_str(),
    })
}
