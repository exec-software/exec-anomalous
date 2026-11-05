#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Portable, bounded window/workspace policy for explicitly public built-ins.
//!
//! Human and machine clients submit identical typed commands through privately
//! branded local endpoints. Origin is attribution, never authority. Only the
//! trusted local controller binds/revokes clients and asserts presentation.
//! These owners authenticate no OS task, channel, user, or remote connection.
//!
//! Construction returns an unpublished initial candidate. Readers see no view
//! until its complete presentation has been asserted and published. Later
//! preparation preserves the committed view; beginning presentation gates both
//! observations and actions until publication. A dropped ticket does not commit
//! or reopen reads. Native crash recovery and scanout verification are outside
//! this crate. Built-in titles, local scratch text and admitted graphics metadata
//! are public. The scratch editor is memory-only and has no document service or
//! save authority. Scoped
//! local producers can attach immutable RGB8 frame descriptors; pixels remain
//! borrowed by the presenter, outside this copyable policy snapshot. No private
//! application disclosure, task authentication, or native client IPC is implied.

#![doc = include_str!("ownership_compile_fail.md")]

use core::sync::atomic::{AtomicU64, Ordering};

pub use anomalous_desktop_theme::{
    cursor_style, density_mode, font_family, theme_definition, theme_mode, theme_settings,
};

pub use anomalous_editor_contract::{caret_motion, editor_error};
pub use anomalous_window_surface::surface_descriptor;

mod editor;
use editor::editor_content;
pub use editor::{editor_action, editor_feedback, editor_view};

#[cfg(feature = "native_editor_bridge")]
pub mod native_bridge;

pub const window_capacity: usize = 4;
pub const workspace_capacity: usize = 2;
pub const client_capacity: usize = 4;
pub const workarea_top: u32 = 64;
pub const workarea_bottom_inset: u32 = 80;
pub const workarea_margin: u32 = 16;
pub const minimum_window_width: u32 = 120;
pub const minimum_window_height: u32 = 96;

static next_instance: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum desktop_error {
    instance_exhausted,
    invalid_viewport,
    invalid_geometry,
    invalid_pointer_buttons,
    invalid_pointer_target,
    invalid_theme,
    invalid_batch,
    launcher_closed,
    invalid_launcher_graphics,
    not_appearance_window,
    not_graphics_window,
    not_editor_window,
    #[cfg(feature = "native_editor_bridge")]
    not_native_editor_window,
    #[cfg(feature = "native_editor_bridge")]
    native_editor_open_required,
    #[cfg(feature = "native_editor_bridge")]
    native_editor_close_required,
    editor_not_focused,
    appearance_window_exists,
    window_not_floating,
    window_capacity_exhausted,
    client_capacity_exhausted,
    client_generation_exhausted,
    window_generation_exhausted,
    sequence_exhausted,
    control_denied,
    client_denied,
    grant_denied,
    stale_revision,
    stale_window,
    window_not_visible,
    layout_too_small,
    backpressure,
    unavailable,
    stale_ticket,
    wrong_phase,
}

impl desktop_error {
    pub const fn code(self) -> &'static str {
        match self {
            Self::instance_exhausted => "instance_exhausted",
            Self::invalid_viewport => "invalid_viewport",
            Self::invalid_geometry => "invalid_geometry",
            Self::invalid_pointer_buttons => "invalid_pointer_buttons",
            Self::invalid_pointer_target => "invalid_pointer_target",
            Self::invalid_theme => "invalid_theme",
            Self::invalid_batch => "invalid_batch",
            Self::launcher_closed => "launcher_closed",
            Self::invalid_launcher_graphics => "invalid_launcher_graphics",
            Self::not_appearance_window => "not_appearance_window",
            Self::not_graphics_window => "not_graphics_window",
            Self::not_editor_window => "not_editor_window",
            #[cfg(feature = "native_editor_bridge")]
            Self::not_native_editor_window => "not_native_editor_window",
            #[cfg(feature = "native_editor_bridge")]
            Self::native_editor_open_required => "native_editor_open_required",
            #[cfg(feature = "native_editor_bridge")]
            Self::native_editor_close_required => "native_editor_close_required",
            Self::editor_not_focused => "editor_not_focused",
            Self::appearance_window_exists => "appearance_window_exists",
            Self::window_not_floating => "window_not_floating",
            Self::window_capacity_exhausted => "window_capacity_exhausted",
            Self::client_capacity_exhausted => "client_capacity_exhausted",
            Self::client_generation_exhausted => "client_generation_exhausted",
            Self::window_generation_exhausted => "window_generation_exhausted",
            Self::sequence_exhausted => "sequence_exhausted",
            Self::control_denied => "control_denied",
            Self::client_denied => "client_denied",
            Self::grant_denied => "grant_denied",
            Self::stale_revision => "stale_revision",
            Self::stale_window => "stale_window",
            Self::window_not_visible => "window_not_visible",
            Self::layout_too_small => "layout_too_small",
            Self::backpressure => "backpressure",
            Self::unavailable => "unavailable",
            Self::stale_ticket => "stale_ticket",
            Self::wrong_phase => "wrong_phase",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum workspace_id {
    first,
    second,
}
impl workspace_id {
    pub const fn index(self) -> usize {
        match self {
            Self::first => 0,
            Self::second => 1,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum layout_mode {
    floating,
    horizontal,
    vertical,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum client_origin {
    human,
    machine,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum demo_content {
    welcome,
    inspector,
    activity,
    appearance,
    graphics,
    editor,
    #[cfg(feature = "native_editor_bridge")]
    native_editor,
}
impl demo_content {
    pub const fn title(self) -> &'static str {
        match self {
            Self::welcome => "Welcome",
            Self::inspector => "System Inspector",
            Self::activity => "Activity",
            Self::appearance => "Appearance",
            Self::graphics => "Graphics Lab",
            Self::editor => "Editor Lab",
            #[cfg(feature = "native_editor_bridge")]
            Self::native_editor => "Native Editor",
        }
    }
}

/// The complete catalogue of public built-ins that this workspace can open.
/// Selection and labels describe an action; they grant no client authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum launcher_choice {
    welcome,
    inspector,
    activity,
    appearance,
    graphics,
    editor,
}

impl launcher_choice {
    pub const all: [Self; 6] = [
        Self::welcome,
        Self::inspector,
        Self::activity,
        Self::appearance,
        Self::graphics,
        Self::editor,
    ];

    pub const fn title(self) -> &'static str {
        self.content().title()
    }

    pub const fn content(self) -> demo_content {
        match self {
            Self::welcome => demo_content::welcome,
            Self::inspector => demo_content::inspector,
            Self::activity => demo_content::activity,
            Self::appearance => demo_content::appearance,
            Self::graphics => demo_content::graphics,
            Self::editor => demo_content::editor,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::welcome => Self::inspector,
            Self::inspector => Self::activity,
            Self::activity => Self::appearance,
            Self::appearance => Self::graphics,
            Self::graphics => Self::editor,
            Self::editor => Self::welcome,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::welcome => Self::editor,
            Self::inspector => Self::welcome,
            Self::activity => Self::inspector,
            Self::appearance => Self::activity,
            Self::graphics => Self::appearance,
            Self::editor => Self::graphics,
        }
    }
}

/// Fixed public appearance actions, shared by pointer, keyboard and machine
/// clients. A choice is a preference value, never authority to change it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum appearance_choice {
    light,
    dark,
    high_contrast,
    comfortable,
    compact,
    regular_text,
    large_text,
    arrow_cursor,
    crosshair_cursor,
    rounded_frames,
    square_frames,
    depth_on,
    depth_off,
    reduced_motion_on,
    reduced_motion_off,
    reset,
}
impl appearance_choice {
    pub const all: [Self; 16] = [
        Self::light,
        Self::dark,
        Self::high_contrast,
        Self::comfortable,
        Self::compact,
        Self::regular_text,
        Self::large_text,
        Self::arrow_cursor,
        Self::crosshair_cursor,
        Self::rounded_frames,
        Self::square_frames,
        Self::depth_on,
        Self::depth_off,
        Self::reduced_motion_on,
        Self::reduced_motion_off,
        Self::reset,
    ];
    pub fn next(self) -> Self {
        Self::all[(self.index() + 1) % Self::all.len()]
    }
    pub fn previous(self) -> Self {
        Self::all[(self.index() + Self::all.len() - 1) % Self::all.len()]
    }
    fn index(self) -> usize {
        Self::all.iter().position(|choice| *choice == self).unwrap()
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::light => "Light",
            Self::dark => "Dark",
            Self::high_contrast => "High Contrast",
            Self::comfortable => "Comfortable",
            Self::compact => "Compact",
            Self::regular_text => "Regular",
            Self::large_text => "Large",
            Self::arrow_cursor => "Arrow",
            Self::crosshair_cursor => "Crosshair",
            Self::rounded_frames => "Rounded",
            Self::square_frames => "Square",
            Self::depth_on => "Shadows on",
            Self::depth_off => "Shadows off",
            Self::reduced_motion_on => "Reduce motion",
            Self::reduced_motion_off => "Allow motion",
            Self::reset => "Reset appearance",
        }
    }
}

/// Canonical software cursor state, published with its matching desktop frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct pointer_position {
    x: u32,
    y: u32,
    visible: bool,
}
impl pointer_position {
    pub const fn x(self) -> u32 {
        self.x
    }
    pub const fn y(self) -> u32 {
        self.y
    }
    pub const fn visible(self) -> bool {
        self.visible
    }
}

/// Public correlation, never authority. Reusing a slot advances its generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct window_id {
    slot: u8,
    generation: u64,
}
impl window_id {
    pub const fn slot(self) -> usize {
        self.slot as usize
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct window_rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
impl window_rect {
    pub const fn try_new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, desktop_error> {
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(desktop_error::invalid_geometry);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
    pub const fn x(self) -> u32 {
        self.x
    }
    pub const fn y(self) -> u32 {
        self.y
    }
    pub const fn width(self) -> u32 {
        self.width
    }
    pub const fn height(self) -> u32 {
        self.height
    }
    pub const fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
    const fn within(self, outer: Self) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.x + self.width <= outer.x + outer.width
            && self.y + self.height <= outer.y + outer.height
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct client_grants(u8);
impl client_grants {
    pub const observe: Self = Self(1);
    pub const arrange: Self = Self(2);
    pub const manage: Self = Self(4);
    pub const customize: Self = Self(8);
    pub const present: Self = Self(16);
    pub const all: Self = Self(31);
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

/// Public semantic identity shared by control picking and gesture feedback.
/// A target is observation, never authority to perform its corresponding action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum desktop_target {
    desktop,
    window(window_id),
    titlebar(window_id),
    resize(window_id),
    minimize(window_id),
    maximize(window_id),
    close(window_id),
    task(window_id),
    launcher,
    launcher_item(launcher_choice),
    launcher_background,
    appearance_launcher,
    appearance_choice(window_id, appearance_choice),
    theme,
    density,
    font,
    cursor,
    workspace(workspace_id),
    layout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum desktop_command {
    toggle_launcher,
    dismiss_launcher,
    select_launcher(launcher_choice),
    activate_launcher {
        choice: launcher_choice,
        graphics: Option<surface_descriptor>,
    },
    open_appearance,
    open_graphics(surface_descriptor),
    #[cfg(feature = "native_editor_bridge")]
    open_native_editor(surface_descriptor),
    open_editor,
    edit_editor {
        window: window_id,
        action: editor_action,
    },
    select_appearance {
        window: window_id,
        choice: appearance_choice,
    },
    activate_appearance {
        window: window_id,
        choice: appearance_choice,
    },
    create {
        content: demo_content,
        workspace: workspace_id,
        bounds: window_rect,
    },
    close(window_id),
    focus(window_id),
    raise(window_id),
    move_window {
        window: window_id,
        x: u32,
        y: u32,
    },
    resize_window {
        window: window_id,
        width: u32,
        height: u32,
    },
    maximize(window_id),
    restore_geometry(window_id),
    set_pointer {
        x: u32,
        y: u32,
    },
    set_pointer_buttons(u8),
    arm_pointer(desktop_target),
    cancel_pointer,
    minimize(window_id),
    restore(window_id),
    switch_workspace(workspace_id),
    move_to_workspace {
        window: window_id,
        workspace: workspace_id,
    },
    set_layout(layout_mode),
    set_theme(theme_mode),
    set_theme_settings(theme_settings),
    set_density(density_mode),
    set_depth(bool),
    set_reduced_motion(bool),
}
impl desktop_command {
    const fn grant(self) -> client_grants {
        match self {
            Self::activate_launcher { choice, .. } => {
                let grants = client_grants::manage.union(client_grants::arrange);
                if matches!(choice, launcher_choice::graphics) {
                    grants.union(client_grants::present)
                } else {
                    grants
                }
            }
            Self::open_appearance | Self::open_editor => {
                client_grants::manage.union(client_grants::arrange)
            }
            Self::edit_editor { .. } => client_grants::present,
            Self::open_graphics(_) => client_grants::manage
                .union(client_grants::arrange)
                .union(client_grants::present),
            #[cfg(feature = "native_editor_bridge")]
            Self::open_native_editor(_) => client_grants::manage
                .union(client_grants::arrange)
                .union(client_grants::present),
            Self::activate_appearance { .. } => {
                client_grants::arrange.union(client_grants::customize)
            }
            Self::create { .. } | Self::close(_) => client_grants::manage,
            Self::set_theme_settings(_)
            | Self::set_theme(_)
            | Self::set_density(_)
            | Self::set_depth(_)
            | Self::set_reduced_motion(_) => client_grants::customize,
            _ => client_grants::arrange,
        }
    }
}

pub struct desktop_control {
    instance: u64,
}
/// Scoped local producer authority. Public surface/window identifiers cannot
/// manufacture this owner. Revoking its client or closing its window retires it.
pub struct surface_owner {
    instance: u64,
    client_slot: usize,
    client_generation: u64,
    window: window_id,
}

pub struct client_endpoint {
    instance: u64,
    slot: usize,
    generation: u64,
}
#[must_use = "publish or explicitly cancel the retained update"]
pub struct update_ticket {
    instance: u64,
    transaction: u64,
}

pub struct publication_rejection {
    error: desktop_error,
    ticket: update_ticket,
}
impl publication_rejection {
    pub const fn error(&self) -> desktop_error {
        self.error
    }
    pub fn into_ticket(self) -> update_ticket {
        self.ticket
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum presentation_phase {
    steady,
    prepared,
    presenting,
    copied,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct publication {
    revision: u64,
    created: Option<window_id>,
    origin: Option<client_origin>,
}
impl publication {
    pub const fn revision(self) -> u64 {
        self.revision
    }
    pub const fn created_window(self) -> Option<window_id> {
        self.created
    }
    pub const fn origin(self) -> Option<client_origin> {
        self.origin
    }
}

#[derive(Clone, Copy)]
struct window_state {
    id: window_id,
    workspace: workspace_id,
    floating: window_rect,
    effective: window_rect,
    minimized: bool,
    maximized: bool,
    content: demo_content,
    appearance_selection: Option<appearance_choice>,
    surface: Option<surface_descriptor>,
    surface_sequence: u64,
    editor: Option<editor_content>,
    tile_order: u64,
}
#[derive(Clone, Copy)]
struct workspace_state {
    layout: layout_mode,
    focus: Option<window_id>,
}
#[derive(Clone, Copy)]
struct snapshot {
    revision: u64,
    width: u32,
    height: u32,
    windows: [Option<window_state>; window_capacity],
    generations: [u64; window_capacity],
    exhausted: [bool; window_capacity],
    order: [Option<window_id>; window_capacity],
    workspaces: [workspace_state; workspace_capacity],
    active: workspace_id,
    next_tile_order: u64,
    appearance: theme_settings,
    definition: theme_definition,
    pointer: pointer_position,
    pointer_buttons: u8,
    pointer_armed: Option<desktop_target>,
    launcher_open: bool,
    launcher_selection: launcher_choice,
}
impl snapshot {
    /// Only candidate state is changed. A caller must normalize it and publish
    /// through the ordinary transaction before this action becomes observable.
    fn launcher_window(
        &mut self,
        choice: launcher_choice,
    ) -> Result<(window_id, Option<window_id>), desktop_error> {
        let existing = self
            .windows
            .iter()
            .flatten()
            .find(|window| window.content == choice.content())
            .map(|window| window.id);
        let (id, created) = if let Some(id) = existing {
            let active = self.active;
            let window = self.window_mut(id)?;
            window.workspace = active;
            window.minimized = false;
            (id, None)
        } else {
            let area = self.work_area();
            let (width, height) = match choice {
                launcher_choice::appearance => (840, 568),
                launcher_choice::graphics => (640, 448),
                _ => (480, 320),
            };
            let width = area.width.min(width);
            let height = area.height.min(height);
            let bounds = window_rect::try_new(
                area.x + (area.width - width) / 2,
                area.y + (area.height - height) / 2,
                width,
                height,
            )?;
            let id = self.create(choice.content(), self.active, bounds)?;
            (id, Some(id))
        };
        self.workspaces[self.active.index()].focus = Some(id);
        self.raise(id);
        Ok((id, created))
    }

    fn launcher_enabled(&self, choice: launcher_choice) -> bool {
        // This checks window policy only. The trusted service still supplies
        // the initial Graphics descriptor and the presenter binds its bytes.
        let mut candidate = *self;
        candidate.launcher_window(choice).is_ok() && candidate.normalize().is_ok()
    }

    const fn work_area(&self) -> window_rect {
        window_rect {
            x: self.definition.metrics.workarea_margin,
            y: self.definition.metrics.workarea_top,
            width: self.width - 2 * self.definition.metrics.workarea_margin,
            height: self.height
                - self.definition.metrics.workarea_top
                - self.definition.metrics.workarea_bottom_inset,
        }
    }
    fn window(&self, id: window_id) -> Result<&window_state, desktop_error> {
        self.windows
            .get(id.slot())
            .and_then(Option::as_ref)
            .filter(|window| window.id == id)
            .ok_or(desktop_error::stale_window)
    }
    fn window_mut(&mut self, id: window_id) -> Result<&mut window_state, desktop_error> {
        self.windows
            .get_mut(id.slot())
            .and_then(Option::as_mut)
            .filter(|window| window.id == id)
            .ok_or(desktop_error::stale_window)
    }
    fn validate_bounds(&self, bounds: window_rect) -> Result<(), desktop_error> {
        if bounds.width < minimum_window_width
            || bounds.height < minimum_window_height
            || !bounds.within(self.work_area())
        {
            Err(desktop_error::invalid_geometry)
        } else {
            Ok(())
        }
    }
    fn create(
        &mut self,
        content: demo_content,
        workspace: workspace_id,
        bounds: window_rect,
    ) -> Result<window_id, desktop_error> {
        self.validate_bounds(bounds)?;
        if content == demo_content::appearance
            && self
                .windows
                .iter()
                .flatten()
                .any(|window| window.content == content)
        {
            return Err(desktop_error::appearance_window_exists);
        }
        let slot = self
            .windows
            .iter()
            .enumerate()
            .position(|(slot, window)| window.is_none() && !self.exhausted[slot])
            .ok_or_else(|| {
                if self.windows.iter().any(Option::is_none) {
                    desktop_error::window_generation_exhausted
                } else {
                    desktop_error::window_capacity_exhausted
                }
            })?;
        let next = self
            .next_tile_order
            .checked_add(1)
            .ok_or(desktop_error::sequence_exhausted)?;
        let id = window_id {
            slot: slot as u8,
            generation: self.generations[slot],
        };
        let order_slot = self
            .order
            .iter()
            .position(Option::is_none)
            .ok_or(desktop_error::window_capacity_exhausted)?;
        self.windows[slot] = Some(window_state {
            id,
            workspace,
            floating: bounds,
            effective: bounds,
            minimized: false,
            maximized: false,
            content,
            appearance_selection: (content == demo_content::appearance)
                .then_some(appearance_choice::light),
            surface: None,
            surface_sequence: 0,
            editor: (content == demo_content::editor).then(editor_content::new),
            tile_order: self.next_tile_order,
        });
        self.order[order_slot] = Some(id);
        self.next_tile_order = next;
        self.workspaces[workspace.index()].focus = Some(id);
        Ok(id)
    }
    /// The scoped bridge may retire this window after a closed observation or
    /// an explicit controller terminal assertion. Call inside prepare_with;
    /// normalization and complete
    /// presentation still precede publication of the resulting candidate.
    #[cfg(feature = "native_editor_bridge")]
    fn remove_native_editor(&mut self, id: window_id) -> Result<(), desktop_error> {
        if self.window(id)?.content != demo_content::native_editor {
            return Err(desktop_error::not_native_editor_window);
        }
        self.windows[id.slot()] = None;
        self.remove_order(id);
        self.pointer_armed = None;
        if let Some(generation) = self.generations[id.slot()].checked_add(1) {
            self.generations[id.slot()] = generation;
        } else {
            self.exhausted[id.slot()] = true;
        }
        Ok(())
    }
    fn remove_order(&mut self, id: window_id) {
        if let Some(position) = self.order.iter().position(|entry| *entry == Some(id)) {
            for index in position..window_capacity - 1 {
                self.order[index] = self.order[index + 1];
            }
            self.order[window_capacity - 1] = None;
        }
    }
    fn raise(&mut self, id: window_id) {
        self.remove_order(id);
        if let Some(slot) = self.order.iter().position(Option::is_none) {
            self.order[slot] = Some(id);
        }
    }
    fn visible(&self, id: window_id) -> Result<(), desktop_error> {
        let window = self.window(id)?;
        if window.minimized || window.workspace != self.active {
            Err(desktop_error::window_not_visible)
        } else {
            Ok(())
        }
    }
    fn apply(&mut self, command: desktop_command) -> Result<Option<window_id>, desktop_error> {
        // Any non-pointer effect in a batch invalidates feedback from its old
        // scene. Explicit rearming must come after those effects and validate
        // against their resulting state.
        if !matches!(
            command,
            desktop_command::set_pointer { .. }
                | desktop_command::set_pointer_buttons(_)
                | desktop_command::arm_pointer(_)
        ) {
            self.pointer_armed = None;
        }
        let mut created = None;
        match command {
            desktop_command::toggle_launcher => {
                self.launcher_open = !self.launcher_open;
                if self.launcher_open {
                    self.launcher_selection = launcher_choice::welcome;
                }
            }
            desktop_command::dismiss_launcher => {
                self.launcher_open = false;
            }
            desktop_command::select_launcher(choice) => {
                if !self.launcher_open {
                    return Err(desktop_error::launcher_closed);
                }
                self.launcher_selection = choice;
            }
            desktop_command::activate_launcher { choice, graphics } => {
                if !self.launcher_open {
                    return Err(desktop_error::launcher_closed);
                }
                if choice != launcher_choice::graphics && graphics.is_some() {
                    return Err(desktop_error::invalid_launcher_graphics);
                }
                let (id, new_window) = self.launcher_window(choice)?;
                if choice == launcher_choice::graphics {
                    let window = self.window_mut(id)?;
                    if window.surface.is_none() {
                        window.surface =
                            Some(graphics.ok_or(desktop_error::invalid_launcher_graphics)?);
                        window.surface_sequence = 1;
                    }
                }
                created = new_window;
                self.launcher_selection = choice;
                self.launcher_open = false;
            }
            desktop_command::open_graphics(frame) => {
                let existing = self
                    .windows
                    .iter()
                    .flatten()
                    .find(|window| window.content == demo_content::graphics)
                    .map(|window| window.id);
                let id = if let Some(id) = existing {
                    let active = self.active;
                    let window = self.window_mut(id)?;
                    window.workspace = active;
                    window.minimized = false;
                    if window.surface.is_none() {
                        window.surface = Some(frame);
                        window.surface_sequence = 1;
                    }
                    id
                } else {
                    let area = self.work_area();
                    let width = area.width.min(640);
                    let height = area.height.min(448);
                    let bounds = window_rect::try_new(
                        area.x + (area.width - width) / 2,
                        area.y + (area.height - height) / 2,
                        width,
                        height,
                    )?;
                    let id = self.create(demo_content::graphics, self.active, bounds)?;
                    let window = self.window_mut(id)?;
                    window.surface = Some(frame);
                    window.surface_sequence = 1;
                    created = Some(id);
                    id
                };
                self.workspaces[self.active.index()].focus = Some(id);
                self.raise(id);
            }
            #[cfg(feature = "native_editor_bridge")]
            desktop_command::open_native_editor(frame) => {
                let existing = self
                    .windows
                    .iter()
                    .flatten()
                    .find(|window| window.content == demo_content::native_editor)
                    .map(|window| window.id);
                let id = if let Some(id) = existing {
                    let active = self.active;
                    let window = self.window_mut(id)?;
                    window.workspace = active;
                    window.minimized = false;
                    // Restoring a window never replaces its owner's observation.
                    id
                } else {
                    let area = self.work_area();
                    let width = area.width.min(640);
                    let height = area.height.min(448);
                    let bounds = window_rect::try_new(
                        area.x + (area.width - width) / 2,
                        area.y + (area.height - height) / 2,
                        width,
                        height,
                    )?;
                    let id = self.create(demo_content::native_editor, self.active, bounds)?;
                    let window = self.window_mut(id)?;
                    window.surface = Some(frame);
                    window.surface_sequence = 1;
                    created = Some(id);
                    id
                };
                self.workspaces[self.active.index()].focus = Some(id);
                self.raise(id);
            }
            desktop_command::open_editor => {
                let (_, new_window) = self.launcher_window(launcher_choice::editor)?;
                created = new_window;
            }
            desktop_command::edit_editor { window, action } => {
                self.visible(window)?;
                if self.window(window)?.content != demo_content::editor {
                    return Err(desktop_error::not_editor_window);
                }
                if self.workspaces[self.active.index()].focus != Some(window) {
                    return Err(desktop_error::editor_not_focused);
                }
                self.window_mut(window)?
                    .editor
                    .as_mut()
                    .ok_or(desktop_error::not_editor_window)?
                    .apply(action);
            }
            desktop_command::open_appearance => {
                let existing = self
                    .windows
                    .iter()
                    .flatten()
                    .find(|window| window.content == demo_content::appearance)
                    .map(|window| window.id);
                let id = if let Some(id) = existing {
                    let active = self.active;
                    let window = self.window_mut(id)?;
                    window.workspace = active;
                    window.minimized = false;
                    id
                } else {
                    let area = self.work_area();
                    let width = area.width.min(840);
                    let height = area.height.min(568);
                    let bounds = window_rect::try_new(
                        area.x + (area.width - width) / 2,
                        area.y + (area.height - height) / 2,
                        width,
                        height,
                    )?;
                    let id = self.create(demo_content::appearance, self.active, bounds)?;
                    created = Some(id);
                    id
                };
                self.workspaces[self.active.index()].focus = Some(id);
                self.raise(id);
            }
            desktop_command::select_appearance { window, choice }
            | desktop_command::activate_appearance { window, choice } => {
                self.visible(window)?;
                if self.window(window)?.content != demo_content::appearance {
                    return Err(desktop_error::not_appearance_window);
                }
                self.window_mut(window)?.appearance_selection = Some(choice);
                self.workspaces[self.active.index()].focus = Some(window);
                self.raise(window);
                if matches!(command, desktop_command::activate_appearance { .. }) {
                    self.activate_appearance(choice);
                }
            }
            desktop_command::create {
                content,
                workspace,
                bounds,
            } => {
                #[cfg(feature = "native_editor_bridge")]
                if content == demo_content::native_editor {
                    return Err(desktop_error::native_editor_open_required);
                }
                created = Some(self.create(content, workspace, bounds)?);
            }
            desktop_command::close(id) => {
                self.window(id)?;
                #[cfg(feature = "native_editor_bridge")]
                if self.window(id)?.content == demo_content::native_editor {
                    return Err(desktop_error::native_editor_close_required);
                }
                self.windows[id.slot()] = None;
                self.remove_order(id);
                if let Some(generation) = self.generations[id.slot()].checked_add(1) {
                    self.generations[id.slot()] = generation;
                } else {
                    self.exhausted[id.slot()] = true;
                }
            }
            desktop_command::focus(id) => {
                self.visible(id)?;
                self.workspaces[self.active.index()].focus = Some(id);
                self.raise(id);
            }
            desktop_command::raise(id) => {
                self.visible(id)?;
                self.raise(id);
            }
            desktop_command::move_window { window, x, y } => {
                let old = self.window(window)?.floating;
                let bounds = window_rect::try_new(x, y, old.width, old.height)?;
                self.validate_bounds(bounds)?;
                self.window_mut(window)?.floating = bounds;
            }
            desktop_command::resize_window {
                window,
                width,
                height,
            } => {
                let old = self.window(window)?.floating;
                let bounds = window_rect::try_new(old.x, old.y, width, height)?;
                self.validate_bounds(bounds)?;
                self.window_mut(window)?.floating = bounds;
            }
            desktop_command::maximize(id) => {
                self.visible(id)?;
                if self.workspaces[self.active.index()].layout != layout_mode::floating {
                    return Err(desktop_error::window_not_floating);
                }
                self.window_mut(id)?.maximized = true;
                self.workspaces[self.active.index()].focus = Some(id);
                self.raise(id);
            }
            desktop_command::restore_geometry(id) => {
                self.window_mut(id)?.maximized = false;
            }
            desktop_command::set_pointer { x, y } => {
                self.pointer = pointer_position {
                    x: x.min(self.width - 1),
                    y: y.min(self.height - 1),
                    visible: true,
                };
            }
            desktop_command::set_pointer_buttons(buttons) => {
                if buttons > 7 {
                    return Err(desktop_error::invalid_pointer_buttons);
                }
                self.pointer_buttons = buttons;
                if buttons != 1 {
                    self.pointer_armed = None;
                }
            }
            desktop_command::arm_pointer(target) => {
                self.validate_pointer_target(target)?;
                self.pointer_armed = Some(target);
            }
            desktop_command::cancel_pointer => {}
            desktop_command::minimize(id) => {
                self.window_mut(id)?.minimized = true;
            }
            desktop_command::restore(id) => {
                let workspace = self.window(id)?.workspace;
                self.window_mut(id)?.minimized = false;
                self.workspaces[workspace.index()].focus = Some(id);
                self.raise(id);
            }
            desktop_command::switch_workspace(workspace) => {
                self.active = workspace;
            }
            desktop_command::move_to_workspace { window, workspace } => {
                self.window_mut(window)?.workspace = workspace;
            }
            desktop_command::set_layout(layout) => {
                self.workspaces[self.active.index()].layout = layout;
            }
            desktop_command::set_theme_settings(settings) => {
                self.appearance = settings;
            }
            desktop_command::set_theme(theme) => {
                self.appearance.theme = theme;
            }
            desktop_command::set_density(density) => {
                self.appearance.density = density;
            }
            desktop_command::set_depth(depth) => {
                self.appearance.depth = depth;
            }
            desktop_command::set_reduced_motion(reduced) => {
                self.appearance.reduced_motion = reduced;
            }
        }
        self.normalize()?;
        Ok(created)
    }
    fn validate_pointer_target(&self, target: desktop_target) -> Result<(), desktop_error> {
        if self.pointer_buttons != 1 || !self.pointer.visible {
            return Err(desktop_error::invalid_pointer_target);
        }
        use desktop_target::*;
        if self.launcher_open && !matches!(target, launcher | launcher_item(_)) {
            return Err(desktop_error::invalid_pointer_target);
        }
        match target {
            desktop | window(_) | titlebar(_) | resize(_) | launcher_background => {
                Err(desktop_error::invalid_pointer_target)
            }
            launcher_item(choice) => {
                if self.launcher_open && self.launcher_enabled(choice) {
                    Ok(())
                } else {
                    Err(desktop_error::invalid_pointer_target)
                }
            }
            minimize(id) | close(id) => self.visible(id),
            maximize(id) => {
                self.visible(id)?;
                if self.workspaces[self.active.index()].layout != layout_mode::floating {
                    return Err(desktop_error::window_not_floating);
                }
                Ok(())
            }
            task(id) => {
                if self.window(id)?.workspace != self.active {
                    return Err(desktop_error::window_not_visible);
                }
                Ok(())
            }
            appearance_choice(id, choice) => {
                self.visible(id)?;
                if self.window(id)?.content != demo_content::appearance {
                    return Err(desktop_error::not_appearance_window);
                }
                let mut candidate = *self;
                candidate.activate_appearance(choice);
                candidate.normalize()
            }
            launcher | appearance_launcher | theme | density | font | cursor | workspace(_)
            | layout => Ok(()),
        }
    }
    fn activate_appearance(&mut self, choice: appearance_choice) {
        self.appearance = self.appearance_settings(choice);
    }
    fn appearance_settings(&self, choice: appearance_choice) -> theme_settings {
        use appearance_choice::*;
        let mut settings = self.appearance;
        match choice {
            light => settings.theme = theme_mode::light,
            dark => settings.theme = theme_mode::dark,
            high_contrast => settings.theme = theme_mode::high_contrast,
            comfortable => settings.density = density_mode::comfortable,
            compact => settings.density = density_mode::compact,
            regular_text => settings.font_scale = 1,
            large_text => settings.font_scale = 2,
            arrow_cursor => settings.cursor = cursor_style::arrow,
            crosshair_cursor => settings.cursor = cursor_style::crosshair,
            rounded_frames | square_frames => {
                settings.overrides.corner_radius =
                    Some(if choice == rounded_frames { 6 } else { 0 });
            }
            depth_on => settings.depth = true,
            depth_off => settings.depth = false,
            reduced_motion_on => settings.reduced_motion = true,
            reduced_motion_off => settings.reduced_motion = false,
            reset => settings = theme_settings::initial,
        }
        settings
    }
    fn normalize(&mut self) -> Result<(), desktop_error> {
        self.definition = self
            .appearance
            .validate()
            .map_err(|_| desktop_error::invalid_theme)?;
        for window in self.windows.iter().flatten() {
            self.validate_bounds(window.floating)?;
        }
        for workspace in [workspace_id::first, workspace_id::second] {
            let focus_valid = self.workspaces[workspace.index()].focus.is_some_and(|id| {
                self.window(id)
                    .is_ok_and(|window| window.workspace == workspace && !window.minimized)
            });
            if !focus_valid {
                self.workspaces[workspace.index()].focus =
                    self.order.iter().rev().flatten().copied().find(|id| {
                        self.window(*id)
                            .is_ok_and(|window| window.workspace == workspace && !window.minimized)
                    });
            }
            let layout = self.workspaces[workspace.index()].layout;
            let mut ids = [None; window_capacity];
            let mut count = 0;
            for window in self
                .windows
                .iter()
                .flatten()
                .filter(|window| window.workspace == workspace && !window.minimized)
            {
                ids[count] = Some(window.id);
                count += 1;
            }
            // Stable insertion sort on creation order, independent of stacking.
            for index in 1..count {
                let mut cursor = index;
                while cursor > 0 {
                    let left = ids[cursor - 1].expect("occupied tile slot");
                    let right = ids[cursor].expect("occupied tile slot");
                    if self.window(left)?.tile_order <= self.window(right)?.tile_order {
                        break;
                    }
                    ids.swap(cursor - 1, cursor);
                    cursor -= 1;
                }
            }
            let area = self.work_area();
            let gap = self.definition.metrics.tile_gap;
            for (index, id) in ids[..count].iter().flatten().copied().enumerate() {
                let bounds = match layout {
                    layout_mode::floating => {
                        let window = self.window(id)?;
                        if window.maximized {
                            area
                        } else {
                            window.floating
                        }
                    }
                    layout_mode::horizontal | layout_mode::vertical => {
                        let extent = if layout == layout_mode::horizontal {
                            area.width
                        } else {
                            area.height
                        };
                        let available = extent
                            .checked_sub(gap * (count as u32 - 1))
                            .ok_or(desktop_error::layout_too_small)?;
                        let start = available * index as u32 / count as u32;
                        let end = available * (index as u32 + 1) / count as u32;
                        let offset = start + gap * index as u32;
                        let bounds = if layout == layout_mode::horizontal {
                            window_rect {
                                x: area.x + offset,
                                y: area.y,
                                width: end - start,
                                height: area.height,
                            }
                        } else {
                            window_rect {
                                x: area.x,
                                y: area.y + offset,
                                width: area.width,
                                height: end - start,
                            }
                        };
                        if bounds.width < minimum_window_width
                            || bounds.height < minimum_window_height
                        {
                            return Err(desktop_error::layout_too_small);
                        }
                        bounds
                    }
                };
                self.window_mut(id)?.effective = bounds;
            }
        }
        Ok(())
    }
}

/// All renderer, accessibility and machine facts borrow one canonical revision.
/// Copying this view never copies authority or a mutation/publication owner.
#[derive(Clone, Copy)]
pub struct render_view<'a> {
    snapshot: &'a snapshot,
}
impl render_view<'_> {
    pub const fn launcher_open(&self) -> bool {
        self.snapshot.launcher_open
    }

    pub const fn launcher_selection(&self) -> launcher_choice {
        self.snapshot.launcher_selection
    }

    pub fn launcher_enabled(&self, choice: launcher_choice) -> bool {
        self.snapshot.launcher_enabled(choice)
    }

    /// Whether these preferences and all existing window geometry can form a
    /// valid successor. This bounded public-state copy changes no owner state.
    /// Visibility, grants and revision are still checked on submission; this
    /// predicate confers no authority and does not reserve a transaction.
    pub fn appearance_enabled(&self, choice: appearance_choice) -> bool {
        let mut candidate = *self.snapshot;
        candidate.activate_appearance(choice);
        candidate.normalize().is_ok()
    }
    /// Whether a choice matches the stored preference. Depth may still be
    /// suppressed by High Contrast; use `depth` for its effective behavior.
    /// Reset is an action and never a persistent selected value.
    pub fn appearance_selected(&self, choice: appearance_choice) -> bool {
        use appearance_choice::*;
        let settings = self.theme_settings();
        match choice {
            light => settings.theme == theme_mode::light,
            dark => settings.theme == theme_mode::dark,
            high_contrast => settings.theme == theme_mode::high_contrast,
            comfortable => settings.density == density_mode::comfortable,
            compact => settings.density == density_mode::compact,
            regular_text => settings.font_scale == 1,
            large_text => settings.font_scale == 2,
            arrow_cursor => settings.cursor == cursor_style::arrow,
            crosshair_cursor => settings.cursor == cursor_style::crosshair,
            rounded_frames => self.snapshot.definition.frame.radius != 0,
            square_frames => self.snapshot.definition.frame.radius == 0,
            depth_on => settings.depth,
            depth_off => !settings.depth,
            reduced_motion_on => settings.reduced_motion,
            reduced_motion_off => !settings.reduced_motion,
            reset => false,
        }
    }
    pub const fn width(&self) -> u32 {
        self.snapshot.width
    }
    pub const fn height(&self) -> u32 {
        self.snapshot.height
    }
    pub const fn revision(&self) -> u64 {
        self.snapshot.revision
    }
    pub const fn theme(&self) -> theme_mode {
        self.snapshot.appearance.theme
    }
    pub const fn density(&self) -> density_mode {
        self.snapshot.appearance.density
    }
    pub const fn depth(&self) -> bool {
        self.snapshot.definition.depth
    }
    pub const fn reduced_motion(&self) -> bool {
        self.snapshot.appearance.reduced_motion
    }
    pub const fn theme_settings(&self) -> theme_settings {
        self.snapshot.appearance
    }
    pub const fn theme_definition(&self) -> theme_definition {
        self.snapshot.definition
    }
    pub const fn pointer(&self) -> pointer_position {
        self.snapshot.pointer
    }
    /// Published physical button bits: left, right, and middle in bits 0..=2.
    pub const fn pointer_buttons(&self) -> u8 {
        self.snapshot.pointer_buttons
    }
    /// The control armed in this revision. It conveys no activation authority;
    /// the input adapter still checks release identity, picking and continuity.
    pub const fn pointer_armed(&self) -> Option<desktop_target> {
        self.snapshot.pointer_armed
    }
    pub const fn active_workspace(&self) -> workspace_id {
        self.snapshot.active
    }
    pub const fn layout(&self) -> layout_mode {
        self.snapshot.workspaces[self.snapshot.active.index()].layout
    }
    pub const fn work_area(&self) -> window_rect {
        self.snapshot.work_area()
    }
    pub const fn accessibility(&self) -> Self {
        *self
    }
    pub const fn machine(&self) -> Self {
        *self
    }
    pub fn windows(&self) -> impl Iterator<Item = window_view> + '_ {
        self.snapshot
            .order
            .iter()
            .flatten()
            .filter_map(|id| self.window(*id))
            .filter(|window| !window.minimized && window.workspace == self.snapshot.active)
    }
    pub fn all_windows(&self) -> impl Iterator<Item = window_view> + '_ {
        self.snapshot
            .order
            .iter()
            .flatten()
            .filter_map(|id| self.window(*id))
    }
    pub fn window(&self, id: window_id) -> Option<window_view> {
        self.snapshot.window(id).ok().map(|window| window_view {
            id: window.id,
            workspace: window.workspace,
            bounds: window.effective,
            floating: window.floating,
            focused: !window.minimized
                && window.workspace == self.snapshot.active
                && self.snapshot.workspaces[window.workspace.index()].focus == Some(id),
            minimized: window.minimized,
            maximized: window.maximized
                && self.snapshot.workspaces[window.workspace.index()].layout
                    == layout_mode::floating,
            content: window.content,
            appearance_selection: window.appearance_selection,
            surface: window.surface,
            surface_sequence: window.surface_sequence,
            editor: window.editor.as_ref().map(editor_content::view),
        })
    }
    pub fn focused_window(&self) -> Option<window_view> {
        self.snapshot.workspaces[self.snapshot.active.index()]
            .focus
            .and_then(|id| self.window(id))
    }
    pub fn next_focus(&self) -> Option<window_id> {
        // Stable slot traversal avoids repeatedly raising only the top pair.
        let focused = self.focused_window().map(|window| window.id.slot());
        let start = focused.map_or(0, |slot| (slot + 1) % window_capacity);
        (0..window_capacity)
            .map(|offset| (start + offset) % window_capacity)
            .filter_map(|slot| self.snapshot.windows[slot])
            .find(|window| !window.minimized && window.workspace == self.snapshot.active)
            .map(|window| window.id)
    }
    pub fn hit_test(&self, x: u32, y: u32) -> Option<window_id> {
        self.windows()
            .filter(|window| window.bounds.contains(x, y))
            .last()
            .map(|window| window.id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct window_view {
    id: window_id,
    workspace: workspace_id,
    bounds: window_rect,
    floating: window_rect,
    focused: bool,
    minimized: bool,
    maximized: bool,
    content: demo_content,
    appearance_selection: Option<appearance_choice>,
    surface: Option<surface_descriptor>,
    surface_sequence: u64,
    editor: Option<editor_view>,
}
impl window_view {
    /// Explicitly public local scratch content, not a private file or authority.
    pub const fn editor(self) -> Option<editor_view> {
        self.editor
    }
    /// Public content metadata; this descriptor carries no producer authority.
    pub const fn surface(self) -> Option<surface_descriptor> {
        self.surface
    }
    pub const fn surface_sequence(self) -> u64 {
        self.surface_sequence
    }
    pub const fn appearance_selection(self) -> Option<appearance_choice> {
        self.appearance_selection
    }
    pub const fn id(self) -> window_id {
        self.id
    }
    pub const fn bounds(self) -> window_rect {
        self.bounds
    }
    pub const fn floating_bounds(self) -> window_rect {
        self.floating
    }
    pub const fn focused(self) -> bool {
        self.focused
    }
    pub const fn minimized(self) -> bool {
        self.minimized
    }
    pub const fn maximized(self) -> bool {
        self.maximized
    }
    pub const fn workspace(self) -> workspace_id {
        self.workspace
    }
    pub const fn content(self) -> demo_content {
        self.content
    }
    pub const fn title(self) -> &'static str {
        self.content.title()
    }
}

#[derive(Clone, Copy)]
struct client_slot {
    generation: u64,
    live: bool,
    exhausted: bool,
    grants: client_grants,
    origin: client_origin,
}
impl client_slot {
    const empty: Self = Self {
        generation: 1,
        live: false,
        exhausted: false,
        grants: client_grants(0),
        origin: client_origin::human,
    };
}
#[derive(Clone, Copy)]
struct pending_source {
    slot: usize,
    generation: u64,
    grant: client_grants,
    origin: client_origin,
}
struct pending_update {
    transaction: u64,
    candidate: snapshot,
    phase: presentation_phase,
    source: Option<pending_source>,
    created: Option<window_id>,
}

pub struct desktop {
    instance: u64,
    committed: snapshot,
    pending: Option<pending_update>,
    clients: [client_slot; client_capacity],
    next_transaction: u64,
    #[cfg(feature = "native_editor_bridge")]
    native_editor_binding: u64,
    // A submitted remote action freezes publication and grant changes, while
    // leaving committed observations and reply pumping available. Only its
    // exact resolution or trusted terminal withdrawal releases this fence.
    #[cfg(feature = "native_editor_bridge")]
    native_action_fence: Option<(u64, u64)>,
}
impl desktop {
    /// Construct an empty unpublished desktop. No initial pixels are asserted.
    pub fn new(
        width: u32,
        height: u32,
    ) -> Result<(Self, desktop_control, update_ticket), desktop_error> {
        Self::construct(width, height, false)
    }
    /// Construct two overlapping public windows as the unpublished first frame.
    pub fn new_demo(
        width: u32,
        height: u32,
    ) -> Result<(Self, desktop_control, update_ticket), desktop_error> {
        Self::construct(width, height, true)
    }
    fn construct(
        width: u32,
        height: u32,
        demo: bool,
    ) -> Result<(Self, desktop_control, update_ticket), desktop_error> {
        if !(640..=4096).contains(&width) || !(480..=2160).contains(&height) {
            return Err(desktop_error::invalid_viewport);
        }
        let instance = next_instance
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| desktop_error::instance_exhausted)?;
        let committed = snapshot {
            revision: 0,
            width,
            height,
            windows: [None; window_capacity],
            generations: [1; window_capacity],
            exhausted: [false; window_capacity],
            order: [None; window_capacity],
            workspaces: [workspace_state {
                layout: layout_mode::floating,
                focus: None,
            }; workspace_capacity],
            active: workspace_id::first,
            next_tile_order: 1,
            appearance: theme_settings::initial,
            definition: theme_settings::initial
                .validate()
                .map_err(|_| desktop_error::invalid_theme)?,
            pointer: pointer_position {
                x: width / 2,
                y: height / 2,
                visible: false,
            },
            pointer_buttons: 0,
            pointer_armed: None,
            launcher_open: false,
            launcher_selection: launcher_choice::welcome,
        };
        let mut candidate = committed;
        candidate.revision = 1;
        if demo {
            let area = candidate.work_area();
            let w = area.width * 3 / 5;
            let h = area.height * 3 / 5;
            candidate.create(
                demo_content::welcome,
                workspace_id::first,
                window_rect::try_new(area.x + 24, area.y + 24, w, h)?,
            )?;
            candidate.create(
                demo_content::inspector,
                workspace_id::first,
                window_rect::try_new(
                    area.x + area.width - w - 24,
                    area.y + area.height - h - 24,
                    w,
                    h,
                )?,
            )?;
            candidate.normalize()?;
        }
        Ok((
            Self {
                instance,
                committed,
                pending: Some(pending_update {
                    transaction: 1,
                    candidate,
                    phase: presentation_phase::prepared,
                    source: None,
                    created: None,
                }),
                clients: [client_slot::empty; client_capacity],
                next_transaction: 2,
                #[cfg(feature = "native_editor_bridge")]
                native_editor_binding: 0,
                #[cfg(feature = "native_editor_bridge")]
                native_action_fence: None,
            },
            desktop_control { instance },
            update_ticket {
                instance,
                transaction: 1,
            },
        ))
    }
    pub fn bind_client(
        &mut self,
        control: &desktop_control,
        origin: client_origin,
        grants: client_grants,
    ) -> Result<client_endpoint, desktop_error> {
        self.check_control(control)?;
        self.require_action_resolved()?;
        let slot = self
            .clients
            .iter()
            .position(|client| !client.live && !client.exhausted)
            .ok_or_else(|| {
                if self.clients.iter().any(|client| !client.live) {
                    desktop_error::client_generation_exhausted
                } else {
                    desktop_error::client_capacity_exhausted
                }
            })?;
        self.clients[slot].live = true;
        self.clients[slot].origin = origin;
        self.clients[slot].grants = grants;
        Ok(client_endpoint {
            instance: self.instance,
            slot,
            generation: self.clients[slot].generation,
        })
    }
    pub fn revoke_client(
        &mut self,
        control: &desktop_control,
        endpoint: &client_endpoint,
    ) -> Result<(), desktop_error> {
        self.check_control(control)?;
        self.check_client(endpoint)?;
        self.require_action_resolved()?;
        let client = &mut self.clients[endpoint.slot];
        client.live = false;
        client.grants = client_grants(0);
        if let Some(generation) = client.generation.checked_add(1) {
            client.generation = generation;
        } else {
            client.exhausted = true;
        }
        Ok(())
    }
    pub fn observe(&self, endpoint: &client_endpoint) -> Result<render_view<'_>, desktop_error> {
        let client = self.check_client(endpoint)?;
        if !client.grants.contains(client_grants::observe) {
            return Err(desktop_error::grant_denied);
        }
        self.require_available()?;
        Ok(render_view {
            snapshot: &self.committed,
        })
    }
    pub fn prepare(
        &mut self,
        endpoint: &client_endpoint,
        expected_revision: u64,
        command: desktop_command,
    ) -> Result<update_ticket, desktop_error> {
        self.prepare_batch(endpoint, expected_revision, &[command])
    }
    /// Prepare up to four commands as one candidate and one publication. Every
    /// required grant is checked before mutation. At most one create is allowed
    /// so the publication's optional created-window result remains unambiguous.
    /// Any invalid command leaves the committed state and transaction unchanged.
    pub fn prepare_batch(
        &mut self,
        endpoint: &client_endpoint,
        expected_revision: u64,
        commands: &[desktop_command],
    ) -> Result<update_ticket, desktop_error> {
        self.require_available()?;
        self.check_client(endpoint)?;
        if commands.is_empty()
            || commands.len() > 4
            || commands
                .iter()
                .filter(|c| {
                    #[cfg(feature = "native_editor_bridge")]
                    if matches!(c, desktop_command::open_native_editor(_)) {
                        return true;
                    }
                    matches!(
                        c,
                        desktop_command::activate_launcher { .. }
                            | desktop_command::create { .. }
                            | desktop_command::open_appearance
                            | desktop_command::open_graphics(_)
                            | desktop_command::open_editor
                    )
                })
                .count()
                > 1
        {
            return Err(desktop_error::invalid_batch);
        }
        let grant = commands
            .iter()
            .fold(client_grants::observe, |grants, command| {
                grants.union(command.grant())
            });
        self.prepare_with(endpoint, expected_revision, grant, |candidate| {
            let mut created = None;
            for command in commands {
                if let Some(window) = candidate.apply(*command)? {
                    created = Some(window);
                }
            }
            Ok(created)
        })
    }
    fn prepare_with(
        &mut self,
        endpoint: &client_endpoint,
        expected_revision: u64,
        grant: client_grants,
        apply: impl FnOnce(&mut snapshot) -> Result<Option<window_id>, desktop_error>,
    ) -> Result<update_ticket, desktop_error> {
        self.require_available()?;
        self.require_action_resolved()?;
        let client = *self.check_client(endpoint)?;
        // A candidate render view requires observation as well as action grants.
        if !client.grants.contains(grant) {
            return Err(desktop_error::grant_denied);
        }
        if self.pending.is_some() {
            return Err(desktop_error::backpressure);
        }
        if expected_revision != self.committed.revision {
            return Err(desktop_error::stale_revision);
        }
        let transaction = self.next_transaction;
        let next = transaction
            .checked_add(1)
            .ok_or(desktop_error::sequence_exhausted)?;
        let revision = self
            .committed
            .revision
            .checked_add(1)
            .ok_or(desktop_error::sequence_exhausted)?;
        let mut candidate = self.committed;
        candidate.revision = revision;
        // Armed feedback belongs to one exact scene revision. Only a validated
        // pointer continuation explicitly carries it into a new publication.
        candidate.pointer_armed = None;
        let created = apply(&mut candidate)?;
        self.pending = Some(pending_update {
            transaction,
            candidate,
            phase: presentation_phase::prepared,
            source: Some(pending_source {
                slot: endpoint.slot,
                generation: endpoint.generation,
                grant,
                origin: client.origin,
            }),
            created,
        });
        self.next_transaction = next;
        Ok(update_ticket {
            instance: self.instance,
            transaction,
        })
    }
    /// Bind one already-published graphics window to a granted local producer.
    /// This is trusted in-process authority, not task authentication or IPC.
    pub fn bind_surface(
        &self,
        control: &desktop_control,
        endpoint: &client_endpoint,
        window: window_id,
    ) -> Result<surface_owner, desktop_error> {
        self.check_control(control)?;
        self.require_available()?;
        let client = self.check_client(endpoint)?;
        if !client
            .grants
            .contains(client_grants::observe.union(client_grants::present))
        {
            return Err(desktop_error::grant_denied);
        }
        if self.committed.window(window)?.content != demo_content::graphics {
            return Err(desktop_error::not_graphics_window);
        }
        Ok(surface_owner {
            instance: self.instance,
            client_slot: endpoint.slot,
            client_generation: endpoint.generation,
            window,
        })
    }
    /// Reserve an immutable content replacement. The presenter must retain the
    /// frame matching the descriptor through publication and subsequent redraws.
    pub fn prepare_surface(
        &mut self,
        owner: &surface_owner,
        expected_revision: u64,
        frame: surface_descriptor,
    ) -> Result<update_ticket, desktop_error> {
        if owner.instance != self.instance {
            return Err(desktop_error::client_denied);
        }
        let endpoint = client_endpoint {
            instance: owner.instance,
            slot: owner.client_slot,
            generation: owner.client_generation,
        };
        self.prepare_with(
            &endpoint,
            expected_revision,
            client_grants::observe.union(client_grants::present),
            |candidate| {
                let window = candidate.window_mut(owner.window)?;
                if window.content != demo_content::graphics {
                    return Err(desktop_error::not_graphics_window);
                }
                window.surface_sequence = window
                    .surface_sequence
                    .checked_add(1)
                    .ok_or(desktop_error::sequence_exhausted)?;
                window.surface = Some(frame);
                Ok(None)
            },
        )
    }
    pub fn candidate(&self, ticket: &update_ticket) -> Result<render_view<'_>, desktop_error> {
        let pending = self.check_ticket(ticket)?;
        Ok(render_view {
            snapshot: &pending.candidate,
        })
    }
    pub fn begin(
        &mut self,
        control: &desktop_control,
        ticket: &update_ticket,
    ) -> Result<(), desktop_error> {
        self.check_control(control)?;
        self.require_action_resolved()?;
        let pending = self.check_ticket(ticket)?;
        if pending.phase != presentation_phase::prepared {
            return Err(desktop_error::wrong_phase);
        }
        self.check_source(pending.source)?;
        self.pending
            .as_mut()
            .expect("validated pending owner")
            .phase = presentation_phase::presenting;
        Ok(())
    }
    /// Trusted local assertion of a complete matching frame; no pixel inspection.
    pub fn attest_presented(
        &mut self,
        control: &desktop_control,
        ticket: &update_ticket,
    ) -> Result<(), desktop_error> {
        self.check_control(control)?;
        self.require_action_resolved()?;
        if self.check_ticket(ticket)?.phase != presentation_phase::presenting {
            return Err(desktop_error::wrong_phase);
        }
        self.pending
            .as_mut()
            .expect("validated pending owner")
            .phase = presentation_phase::copied;
        Ok(())
    }
    pub fn publish(
        &mut self,
        control: &desktop_control,
        ticket: update_ticket,
    ) -> Result<publication, publication_rejection> {
        if let Err(error) = self.publish_preflight(control, &ticket) {
            return Err(publication_rejection { error, ticket });
        }
        let pending = self.pending.take().expect("validated pending owner");
        let result = publication {
            revision: pending.candidate.revision,
            created: pending.created,
            origin: pending.source.map(|source| source.origin),
        };
        self.committed = pending.candidate;
        Ok(result)
    }
    /// Cancel only before external pixels may have changed. A canceled bootstrap
    /// still has no presented view; production adapters may retire/reconstruct it.
    pub fn cancel(
        &mut self,
        control: &desktop_control,
        ticket: update_ticket,
    ) -> Result<(), publication_rejection> {
        let check = self
            .check_control(control)
            .and_then(|()| self.check_ticket(&ticket))
            .and_then(|pending| {
                if pending.phase == presentation_phase::prepared {
                    Ok(())
                } else {
                    Err(desktop_error::wrong_phase)
                }
            });
        if let Err(error) = check {
            return Err(publication_rejection { error, ticket });
        }
        self.pending = None;
        Ok(())
    }
    pub const fn phase(&self) -> presentation_phase {
        match &self.pending {
            Some(pending) => pending.phase,
            None => presentation_phase::steady,
        }
    }
    fn require_action_resolved(&self) -> Result<(), desktop_error> {
        #[cfg(feature = "native_editor_bridge")]
        if self.native_action_fence.is_some() {
            return Err(desktop_error::backpressure);
        }
        Ok(())
    }
    fn require_available(&self) -> Result<(), desktop_error> {
        if self.committed.revision == 0
            || matches!(
                self.phase(),
                presentation_phase::presenting | presentation_phase::copied
            )
        {
            Err(desktop_error::unavailable)
        } else {
            Ok(())
        }
    }
    fn check_control(&self, control: &desktop_control) -> Result<(), desktop_error> {
        if control.instance == self.instance {
            Ok(())
        } else {
            Err(desktop_error::control_denied)
        }
    }
    fn check_client(&self, endpoint: &client_endpoint) -> Result<&client_slot, desktop_error> {
        if endpoint.instance != self.instance {
            return Err(desktop_error::client_denied);
        }
        self.clients
            .get(endpoint.slot)
            .filter(|client| client.live && client.generation == endpoint.generation)
            .ok_or(desktop_error::client_denied)
    }
    fn check_ticket(&self, ticket: &update_ticket) -> Result<&pending_update, desktop_error> {
        if ticket.instance != self.instance {
            return Err(desktop_error::stale_ticket);
        }
        self.pending
            .as_ref()
            .filter(|pending| pending.transaction == ticket.transaction)
            .ok_or(desktop_error::stale_ticket)
    }
    fn check_source(&self, source: Option<pending_source>) -> Result<(), desktop_error> {
        if let Some(source) = source {
            let client = &self.clients[source.slot];
            if !client.live
                || client.generation != source.generation
                || client.origin != source.origin
            {
                return Err(desktop_error::client_denied);
            }
            if !client.grants.contains(source.grant) {
                return Err(desktop_error::grant_denied);
            }
        }
        Ok(())
    }
    fn publish_preflight(
        &self,
        control: &desktop_control,
        ticket: &update_ticket,
    ) -> Result<(), desktop_error> {
        self.check_control(control)?;
        self.require_action_resolved()?;
        let pending = self.check_ticket(ticket)?;
        if pending.phase != presentation_phase::copied {
            return Err(desktop_error::wrong_phase);
        }
        self.check_source(pending.source)
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod launcher_tests;
#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "native_editor_bridge"))]
mod native_window_tests;
