use anomalous_desktop_scene::{
    declared_scene_state, paint_color_token, paint_command_kind, paint_shape_token,
    prepared_render_preview, scene_candidate, scene_depth, scene_disclosure, scene_rect,
    scene_revision, scene_slot, standard_paint_command_count,
};

use crate::identity::surface_identity;
use crate::service::surface_error;

pub(crate) const maximum_snapshot_bytes: usize = 1_024;

const viewport_width: u32 = 1_024;
const viewport_height: u32 = 768;
const public_status: &str = "System ready";
const protected_status: &str = "Protected status";

const canonical_labels: [&str; 7] = [
    "Anomalous desktop",
    "Anomalous",
    "Navigation",
    "Open shell",
    "Inspect system",
    "Workspace",
    "System workspace",
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct render_rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl render_rect {
    fn from_scene(value: scene_rect) -> Result<Self, surface_error> {
        Ok(Self {
            x: u16::try_from(value.x()).map_err(|_| surface_error::unsupported_profile)?,
            y: u16::try_from(value.y()).map_err(|_| surface_error::unsupported_profile)?,
            width: u16::try_from(value.width()).map_err(|_| surface_error::unsupported_profile)?,
            height: u16::try_from(value.height())
                .map_err(|_| surface_error::unsupported_profile)?,
        })
    }

    #[must_use]
    pub const fn x(self) -> u16 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> u16 {
        self.y
    }

    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum render_label_selector {
    anomalous_desktop,
    anomalous,
    navigation,
    open_shell,
    inspect_system,
    workspace,
    system_workspace,
    system_ready,
    protected_status,
}

impl render_label_selector {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::anomalous_desktop => "Anomalous desktop",
            Self::anomalous => "Anomalous",
            Self::navigation => "Navigation",
            Self::open_shell => "Open shell",
            Self::inspect_system => "Inspect system",
            Self::workspace => "Workspace",
            Self::system_workspace => "System workspace",
            Self::system_ready => public_status,
            Self::protected_status => protected_status,
        }
    }

    #[must_use]
    pub const fn disclosure(self) -> scene_disclosure {
        match self {
            Self::protected_status => scene_disclosure::protected,
            _ => scene_disclosure::declared_public,
        }
    }
}

const label_selectors: [render_label_selector; 7] = [
    render_label_selector::anomalous_desktop,
    render_label_selector::anomalous,
    render_label_selector::navigation,
    render_label_selector::open_shell,
    render_label_selector::inspect_system,
    render_label_selector::workspace,
    render_label_selector::system_workspace,
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct render_command {
    kind: paint_command_kind,
    shape: paint_shape_token,
    color: paint_color_token,
    secondary_color: Option<paint_color_token>,
    slot: scene_slot,
    bounds: render_rect,
    state_active: bool,
}

impl render_command {
    const empty: Self = Self {
        kind: paint_command_kind::fill,
        shape: paint_shape_token::rectangle,
        color: paint_color_token::canvas,
        secondary_color: None,
        slot: scene_slot::root,
        bounds: render_rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        },
        state_active: false,
    };

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
    pub const fn bounds(self) -> render_rect {
        self.bounds
    }

    #[must_use]
    pub const fn state_active(self) -> bool {
        self.state_active
    }
}

pub(crate) struct render_snapshot {
    surface: surface_identity,
    scene: anomalous_desktop_scene::scene_identity,
    base: scene_revision,
    revision: scene_revision,
    state: declared_scene_state,
    viewport: render_rect,
    commands: [render_command; standard_paint_command_count],
    command_count: u8,
    labels: [render_label_selector; anomalous_desktop_scene::scene_node_count],
}

const _: () = assert!(core::mem::size_of::<render_snapshot>() <= maximum_snapshot_bytes);

impl render_snapshot {
    pub(crate) fn from_preview(
        surface: surface_identity,
        preview: &prepared_render_preview<'_>,
    ) -> Result<Self, surface_error> {
        let viewport = render_rect::from_scene(preview.viewport())?;
        if u32::from(viewport.x) != 0
            || u32::from(viewport.y) != 0
            || u32::from(viewport.width) != viewport_width
            || u32::from(viewport.height) != viewport_height
        {
            return Err(surface_error::unsupported_profile);
        }

        let expected_count = match preview.state().depth() {
            scene_depth::standard => standard_paint_command_count,
            scene_depth::flat => anomalous_desktop_scene::flat_paint_command_count,
        };
        if preview.command_count() != expected_count {
            return Err(surface_error::unsupported_profile);
        }

        let mut commands = [render_command::empty; standard_paint_command_count];
        for (index, destination) in commands.iter_mut().enumerate().take(expected_count) {
            let source = preview
                .command(index)
                .ok_or(surface_error::unsupported_profile)?;
            *destination = render_command {
                kind: source.kind(),
                shape: source.shape(),
                color: source.color(),
                secondary_color: source.secondary_color(),
                slot: source.slot(),
                bounds: render_rect::from_scene(source.bounds())?,
                state_active: source.state_active(),
            };
        }
        if preview.command(expected_count).is_some() {
            return Err(surface_error::unsupported_profile);
        }

        let mut labels =
            [render_label_selector::anomalous_desktop; anomalous_desktop_scene::scene_node_count];
        for (index, destination) in labels.iter_mut().enumerate() {
            let label = preview
                .label(index)
                .ok_or(surface_error::unsupported_profile)?;
            *destination = selector_for(index, label.disclosure(), label.expose_str())?;
        }
        if preview
            .label(anomalous_desktop_scene::scene_node_count)
            .is_some()
        {
            return Err(surface_error::unsupported_profile);
        }

        Ok(Self {
            surface,
            scene: preview.identity(),
            base: preview.base_revision(),
            revision: preview.next_revision(),
            state: preview.state(),
            viewport,
            commands,
            command_count: expected_count as u8,
            labels,
        })
    }

    pub(crate) const fn revision(&self) -> scene_revision {
        self.revision
    }

    pub(crate) const fn view(&self) -> render_snapshot_view<'_> {
        render_snapshot_view { snapshot: self }
    }
}

pub struct render_snapshot_view<'a> {
    snapshot: &'a render_snapshot,
}

impl<'a> render_snapshot_view<'a> {
    #[must_use]
    pub const fn surface(&self) -> surface_identity {
        self.snapshot.surface
    }

    #[must_use]
    pub const fn scene(&self) -> anomalous_desktop_scene::scene_identity {
        self.snapshot.scene
    }

    #[must_use]
    pub const fn base_revision(&self) -> scene_revision {
        self.snapshot.base
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.snapshot.revision
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.snapshot.state
    }

    #[must_use]
    pub const fn viewport(&self) -> render_rect {
        self.snapshot.viewport
    }

    #[must_use]
    pub const fn command_count(&self) -> usize {
        self.snapshot.command_count as usize
    }

    #[must_use]
    pub fn command(&self, index: usize) -> Option<render_command> {
        if index >= self.command_count() {
            return None;
        }
        Some(self.snapshot.commands[index])
    }

    #[must_use]
    pub fn label(&self, index: usize) -> Option<render_label_selector> {
        self.snapshot.labels.get(index).copied()
    }
}

pub(crate) fn validate_candidate_profile(candidate: &scene_candidate) -> Result<(), surface_error> {
    for (index, expected) in canonical_labels.iter().enumerate() {
        let node = candidate
            .node(index)
            .ok_or(surface_error::unsupported_profile)?;
        if node.slot().index() != index {
            return Err(surface_error::unsupported_profile);
        }
        if node.disclosure() != scene_disclosure::declared_public
            || node.label().expose_declared_public() != Some(*expected)
        {
            return Err(surface_error::unsupported_profile);
        }
    }
    let status = candidate
        .node(scene_slot::status.index())
        .ok_or(surface_error::unsupported_profile)?;
    match candidate.state().status_disclosure() {
        scene_disclosure::declared_public => {
            if status.disclosure() != scene_disclosure::declared_public
                || status.label().expose_declared_public() != Some(public_status)
            {
                return Err(surface_error::unsupported_profile);
            }
        }
        scene_disclosure::protected => {
            if status.disclosure() != scene_disclosure::protected {
                return Err(surface_error::unsupported_profile);
            }
        }
    }
    Ok(())
}

fn selector_for(
    index: usize,
    disclosure: scene_disclosure,
    text: &str,
) -> Result<render_label_selector, surface_error> {
    if index < canonical_labels.len() {
        if disclosure != scene_disclosure::declared_public || text != canonical_labels[index] {
            return Err(surface_error::unsupported_profile);
        }
        return Ok(label_selectors[index]);
    }
    if index != scene_slot::status.index() {
        return Err(surface_error::unsupported_profile);
    }
    match (disclosure, text) {
        (scene_disclosure::declared_public, public_status) => {
            Ok(render_label_selector::system_ready)
        }
        (scene_disclosure::protected, protected_status) => {
            Ok(render_label_selector::protected_status)
        }
        _ => Err(surface_error::unsupported_profile),
    }
}

#[cfg(test)]
pub(crate) const fn snapshot_size() -> usize {
    core::mem::size_of::<render_snapshot>()
}
