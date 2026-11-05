#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Exact copied-byte codec for the fixed ADR 0028 semantic-surface profile.
//!
//! The 528-byte version-1 body is correlation and redacted drawing data, not
//! authority or evidence of rendering, presentation, scanout, or publication.

use core::fmt;

pub const surface_snapshot_v1_header_byte_count: usize = 112;
pub const surface_snapshot_v1_label_count: usize = 8;
pub const surface_snapshot_v1_command_area_offset: usize = 128;
pub const surface_snapshot_v1_command_byte_count: usize = 16;
pub const surface_snapshot_v1_command_capacity: usize = 25;
pub const surface_snapshot_v1_byte_count: usize = 528;
pub const surface_snapshot_v1_version: u16 = 1;
pub const surface_snapshot_v1_profile: u16 = 1;
pub const surface_snapshot_v1_standard_command_count: usize = 25;
pub const surface_snapshot_v1_flat_command_count: usize = 21;
pub const surface_snapshot_v1_maximum_attempt: u64 = 2;
pub const surface_snapshot_v1_viewport_width: u16 = 1_024;
pub const surface_snapshot_v1_viewport_height: u16 = 768;

const header_size_offset: usize = 0;
const version_offset: usize = 2;
const total_size_offset: usize = 4;
const flags_offset: usize = 8;
const profile_offset: usize = 12;
const command_count_offset: usize = 14;
const label_count_offset: usize = 16;
const command_size_offset: usize = 18;
const viewport_width_offset: usize = 20;
const viewport_height_offset: usize = 22;
const primary_enabled_offset: usize = 24;
const secondary_enabled_offset: usize = 25;
const focus_offset: usize = 26;
const disclosure_offset: usize = 27;
const depth_offset: usize = 28;
const motion_offset: usize = 29;
const reserved_offset: usize = 30;
const surface_slot_offset: usize = 32;
const surface_generation_offset: usize = 40;
const scene_slot_offset: usize = 48;
const scene_generation_offset: usize = 56;
const base_revision_offset: usize = 64;
const revision_offset: usize = 72;
const renderer_slot_offset: usize = 80;
const renderer_generation_offset: usize = 88;
const attempt_offset: usize = 96;
const reserved_2_offset: usize = 104;
const labels_offset: usize = 112;
const label_reserved_offset: usize = 120;

const _: () = {
    assert!(surface_snapshot_v1_header_byte_count == 112);
    assert!(surface_snapshot_v1_command_area_offset == 128);
    assert!(surface_snapshot_v1_command_byte_count == 16);
    assert!(surface_snapshot_v1_command_capacity == 25);
    assert!(
        surface_snapshot_v1_command_area_offset
            + surface_snapshot_v1_command_capacity * surface_snapshot_v1_command_byte_count
            == surface_snapshot_v1_byte_count
    );
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum transport_error {
    wrong_length,
    wrong_header_size,
    wrong_version,
    wrong_total_size,
    unsupported_flags,
    wrong_profile,
    wrong_command_count,
    wrong_label_count,
    wrong_command_size,
    wrong_viewport,
    invalid_state,
    invalid_binding,
    invalid_revision,
    invalid_attempt,
    invalid_label_selector,
    label_profile_mismatch,
    invalid_command_kind,
    invalid_shape,
    invalid_color,
    invalid_secondary_color,
    invalid_slot,
    invalid_state_flag,
    invalid_geometry,
    geometry_overflow,
    outside_viewport,
    nonzero_reserved,
    nonzero_trailing_command,
    binding_mismatch,
}

impl transport_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::wrong_length => "wrong_length",
            Self::wrong_header_size => "wrong_header_size",
            Self::wrong_version => "wrong_version",
            Self::wrong_total_size => "wrong_total_size",
            Self::unsupported_flags => "unsupported_flags",
            Self::wrong_profile => "wrong_profile",
            Self::wrong_command_count => "wrong_command_count",
            Self::wrong_label_count => "wrong_label_count",
            Self::wrong_command_size => "wrong_command_size",
            Self::wrong_viewport => "wrong_viewport",
            Self::invalid_state => "invalid_state",
            Self::invalid_binding => "invalid_binding",
            Self::invalid_revision => "invalid_revision",
            Self::invalid_attempt => "invalid_attempt",
            Self::invalid_label_selector => "invalid_label_selector",
            Self::label_profile_mismatch => "label_profile_mismatch",
            Self::invalid_command_kind => "invalid_command_kind",
            Self::invalid_shape => "invalid_shape",
            Self::invalid_color => "invalid_color",
            Self::invalid_secondary_color => "invalid_secondary_color",
            Self::invalid_slot => "invalid_slot",
            Self::invalid_state_flag => "invalid_state_flag",
            Self::invalid_geometry => "invalid_geometry",
            Self::geometry_overflow => "geometry_overflow",
            Self::outside_viewport => "outside_viewport",
            Self::nonzero_reserved => "nonzero_reserved",
            Self::nonzero_trailing_command => "nonzero_trailing_command",
            Self::binding_mismatch => "binding_mismatch",
        }
    }
}

impl fmt::Debug for transport_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for transport_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct transport_identity {
    slot: u64,
    generation: u64,
}

impl transport_identity {
    #[must_use]
    pub const fn try_new(slot: u64, generation: u64) -> Option<Self> {
        if slot == 0 || generation == 0 {
            None
        } else {
            Some(Self { slot, generation })
        }
    }

    #[must_use]
    pub const fn slot(self) -> u64 {
        self.slot
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct surface_snapshot_binding {
    surface: transport_identity,
    scene: transport_identity,
    base_revision: u64,
    revision: u64,
    renderer: transport_identity,
    attempt: u64,
}

impl surface_snapshot_binding {
    pub const fn try_new(
        surface: transport_identity,
        scene: transport_identity,
        base_revision: u64,
        revision: u64,
        renderer: transport_identity,
        attempt: u64,
    ) -> Result<Self, transport_error> {
        if base_revision == 0
            || revision == 0
            || !matches!(base_revision.checked_add(1), Some(next) if next == revision)
        {
            return Err(transport_error::invalid_revision);
        }
        if attempt == 0 || attempt > surface_snapshot_v1_maximum_attempt {
            return Err(transport_error::invalid_attempt);
        }
        Ok(Self {
            surface,
            scene,
            base_revision,
            revision,
            renderer,
            attempt,
        })
    }

    #[must_use]
    pub const fn surface(self) -> transport_identity {
        self.surface
    }
    #[must_use]
    pub const fn scene(self) -> transport_identity {
        self.scene
    }
    #[must_use]
    pub const fn base_revision(self) -> u64 {
        self.base_revision
    }
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    #[must_use]
    pub const fn renderer(self) -> transport_identity {
        self.renderer
    }
    #[must_use]
    pub const fn attempt(self) -> u64 {
        self.attempt
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct transport_rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl transport_rect {
    pub const fn try_new(x: u16, y: u16, width: u16, height: u16) -> Result<Self, transport_error> {
        if width == 0 || height == 0 {
            return Err(transport_error::invalid_geometry);
        }
        if x.checked_add(width).is_none() || y.checked_add(height).is_none() {
            return Err(transport_error::geometry_overflow);
        }
        Ok(Self {
            x,
            y,
            width,
            height,
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

macro_rules! transport_enum {
    ($name:ident { $($variant:ident = $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Eq, PartialEq)]
        #[repr(u8)]
        pub enum $name { $($variant = $value),+ }

        impl $name {
            #[must_use]
            pub const fn from_raw(value: u8) -> Option<Self> {
                match value { $($value => Some(Self::$variant),)+ _ => None }
            }
            #[must_use]
            pub const fn raw(self) -> u8 {
                match self { $(Self::$variant => $value),+ }
            }
        }
    };
}

transport_enum!(transport_focus { none = 0, primary = 1, secondary = 2 });
transport_enum!(transport_disclosure { declared_public = 0, protected = 1 });
transport_enum!(transport_depth { standard = 0, flat = 1 });
transport_enum!(transport_motion { standard = 0, reduced = 1 });
transport_enum!(transport_command_kind {
    fill = 1, outline = 2, label = 3, enabled_marker = 4,
    focus_outline = 5, shadow = 6, highlight = 7
});
transport_enum!(transport_shape {
    rectangle = 1, text = 2, enabled = 3, disabled = 4,
    focus_absent = 5, focus_double_outline = 6
});
transport_enum!(transport_color {
    canvas = 1, surface = 2, primary_fill = 3, secondary_fill = 4,
    disabled = 5, primary_text = 6, secondary_text = 7, on_accent = 8,
    outline = 9, focus_outer = 10, focus_inner = 11, public_status = 12,
    protected_status = 13, shadow = 14, highlight = 15
});
transport_enum!(transport_slot {
    root = 1, masthead = 2, navigation = 3, primary_action = 4,
    secondary_action = 5, content = 6, heading = 7, status = 8
});
transport_enum!(transport_label_selector {
    anomalous_desktop = 1, anomalous = 2, navigation = 3, open_shell = 4,
    inspect_system = 5, workspace = 6, system_workspace = 7,
    system_ready = 8, protected_status = 9
});

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct transport_scene_state {
    primary_enabled: bool,
    secondary_enabled: bool,
    focus: transport_focus,
    disclosure: transport_disclosure,
    depth: transport_depth,
    motion: transport_motion,
}

impl transport_scene_state {
    pub const fn try_new(
        primary_enabled: bool,
        secondary_enabled: bool,
        focus: transport_focus,
        disclosure: transport_disclosure,
        depth: transport_depth,
        motion: transport_motion,
    ) -> Result<Self, transport_error> {
        if (matches!(focus, transport_focus::primary) && !primary_enabled)
            || (matches!(focus, transport_focus::secondary) && !secondary_enabled)
        {
            return Err(transport_error::invalid_state);
        }
        Ok(Self {
            primary_enabled,
            secondary_enabled,
            focus,
            disclosure,
            depth,
            motion,
        })
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
    pub const fn focus(self) -> transport_focus {
        self.focus
    }
    #[must_use]
    pub const fn disclosure(self) -> transport_disclosure {
        self.disclosure
    }
    #[must_use]
    pub const fn depth(self) -> transport_depth {
        self.depth
    }
    #[must_use]
    pub const fn motion(self) -> transport_motion {
        self.motion
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct transport_render_command {
    kind: transport_command_kind,
    shape: transport_shape,
    color: transport_color,
    secondary_color: Option<transport_color>,
    slot: transport_slot,
    state_active: bool,
    bounds: transport_rect,
}

impl transport_render_command {
    #[must_use]
    pub const fn new(
        kind: transport_command_kind,
        shape: transport_shape,
        color: transport_color,
        secondary_color: Option<transport_color>,
        slot: transport_slot,
        state_active: bool,
        bounds: transport_rect,
    ) -> Self {
        Self {
            kind,
            shape,
            color,
            secondary_color,
            slot,
            state_active,
            bounds,
        }
    }

    #[must_use]
    pub const fn kind(self) -> transport_command_kind {
        self.kind
    }
    #[must_use]
    pub const fn shape(self) -> transport_shape {
        self.shape
    }
    #[must_use]
    pub const fn color(self) -> transport_color {
        self.color
    }
    #[must_use]
    pub const fn secondary_color(self) -> Option<transport_color> {
        self.secondary_color
    }
    #[must_use]
    pub const fn slot(self) -> transport_slot {
        self.slot
    }
    #[must_use]
    pub const fn state_active(self) -> bool {
        self.state_active
    }
    #[must_use]
    pub const fn bounds(self) -> transport_rect {
        self.bounds
    }
}

/// Structurally validated owner of one complete version-1 transport body.
///
/// This is the kernel-facing boundary. It validates fixed sizes, versions,
/// reservations, known tags, canonical depth/count/tail rules, checked
/// geometry, and public correlation. It deliberately does not enforce the
/// focus-versus-enablement or disclosure-versus-label profile policies.
///
/// No formatting, hashing, or general serialization implementation is
/// provided. `into_bytes` moves the copied body to its storage owner, which
/// remains responsible for its lifecycle and explicit clearing.
pub struct surface_snapshot_body_v1 {
    bytes: [u8; surface_snapshot_v1_byte_count],
}

impl surface_snapshot_body_v1 {
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, transport_error> {
        if bytes.len() != surface_snapshot_v1_byte_count {
            return Err(transport_error::wrong_length);
        }
        let mut owned = [0; surface_snapshot_v1_byte_count];
        owned.copy_from_slice(bytes);
        Self::try_from_bytes(owned)
    }

    pub fn try_from_bytes(
        bytes: [u8; surface_snapshot_v1_byte_count],
    ) -> Result<Self, transport_error> {
        validate_structure(&bytes)?;
        Ok(Self { bytes })
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; surface_snapshot_v1_byte_count] {
        &self.bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> [u8; surface_snapshot_v1_byte_count] {
        self.bytes
    }

    #[must_use]
    pub fn binding(&self) -> surface_snapshot_binding {
        decode_binding(&self.bytes).expect("structurally validated binding")
    }

    pub fn try_into_snapshot(self) -> Result<surface_snapshot_v1, transport_error> {
        surface_snapshot_v1::try_from_body(self)
    }
}

/// Fully fixed-profile validated owner of one version-1 body.
///
/// This is the consumer-facing typed view. In addition to structural safety,
/// it enforces focus enablement and the canonical disclosure-sensitive label
/// profile. Like the structural body, it has no formatting, hashing, or
/// general serialization implementation.
pub struct surface_snapshot_v1 {
    bytes: [u8; surface_snapshot_v1_byte_count],
}

const _: () = {
    assert!(core::mem::size_of::<surface_snapshot_body_v1>() == surface_snapshot_v1_byte_count);
    assert!(core::mem::size_of::<surface_snapshot_v1>() == surface_snapshot_v1_byte_count);
};

impl surface_snapshot_v1 {
    pub fn try_encode(
        binding: surface_snapshot_binding,
        state: transport_scene_state,
        viewport: transport_rect,
        labels: [transport_label_selector; surface_snapshot_v1_label_count],
        commands: &[transport_render_command],
    ) -> Result<Self, transport_error> {
        validate_viewport(viewport)?;
        if commands.len() != command_count_for_depth(state.depth()) {
            return Err(transport_error::wrong_command_count);
        }
        validate_label_profile(state.disclosure(), &labels)?;
        for command in commands {
            validate_command(*command)?;
        }

        let mut bytes = [0; surface_snapshot_v1_byte_count];
        write_u16(&mut bytes, header_size_offset, 112);
        write_u16(&mut bytes, version_offset, surface_snapshot_v1_version);
        write_u32(&mut bytes, total_size_offset, 528);
        write_u16(&mut bytes, profile_offset, surface_snapshot_v1_profile);
        write_u16(&mut bytes, command_count_offset, commands.len() as u16);
        write_u16(&mut bytes, label_count_offset, 8);
        write_u16(&mut bytes, command_size_offset, 16);
        write_u16(&mut bytes, viewport_width_offset, 1_024);
        write_u16(&mut bytes, viewport_height_offset, 768);
        bytes[primary_enabled_offset] = u8::from(state.primary_enabled());
        bytes[secondary_enabled_offset] = u8::from(state.secondary_enabled());
        bytes[focus_offset] = state.focus().raw();
        bytes[disclosure_offset] = state.disclosure().raw();
        bytes[depth_offset] = state.depth().raw();
        bytes[motion_offset] = state.motion().raw();
        write_u64(&mut bytes, surface_slot_offset, binding.surface.slot);
        write_u64(
            &mut bytes,
            surface_generation_offset,
            binding.surface.generation,
        );
        write_u64(&mut bytes, scene_slot_offset, binding.scene.slot);
        write_u64(
            &mut bytes,
            scene_generation_offset,
            binding.scene.generation,
        );
        write_u64(&mut bytes, base_revision_offset, binding.base_revision);
        write_u64(&mut bytes, revision_offset, binding.revision);
        write_u64(&mut bytes, renderer_slot_offset, binding.renderer.slot);
        write_u64(
            &mut bytes,
            renderer_generation_offset,
            binding.renderer.generation,
        );
        write_u64(&mut bytes, attempt_offset, binding.attempt);
        for (index, label) in labels.iter().enumerate() {
            bytes[labels_offset + index] = label.raw();
        }
        for (index, command) in commands.iter().enumerate() {
            encode_command(&mut bytes, index, *command);
        }
        debug_assert!(validate_complete_profile(&bytes).is_ok());
        Ok(Self { bytes })
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, transport_error> {
        surface_snapshot_body_v1::try_from_slice(bytes)?.try_into_snapshot()
    }

    pub fn try_from_bytes(
        bytes: [u8; surface_snapshot_v1_byte_count],
    ) -> Result<Self, transport_error> {
        surface_snapshot_body_v1::try_from_bytes(bytes)?.try_into_snapshot()
    }

    pub fn try_from_body(body: surface_snapshot_body_v1) -> Result<Self, transport_error> {
        validate_complete_profile(body.as_bytes())?;
        Ok(Self {
            bytes: body.into_bytes(),
        })
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; surface_snapshot_v1_byte_count] {
        &self.bytes
    }
    #[must_use]
    pub fn into_bytes(self) -> [u8; surface_snapshot_v1_byte_count] {
        self.bytes
    }
    #[must_use]
    pub fn into_body(self) -> surface_snapshot_body_v1 {
        surface_snapshot_body_v1 { bytes: self.bytes }
    }
    #[must_use]
    pub fn binding(&self) -> surface_snapshot_binding {
        decode_binding(&self.bytes).expect("validated binding")
    }
    #[must_use]
    pub fn state(&self) -> transport_scene_state {
        decode_state(&self.bytes).expect("validated state")
    }
    #[must_use]
    pub fn command_count(&self) -> usize {
        usize::from(read_u16(&self.bytes, command_count_offset))
    }
    #[must_use]
    pub const fn viewport(&self) -> transport_rect {
        transport_rect {
            x: 0,
            y: 0,
            width: 1_024,
            height: 768,
        }
    }
    #[must_use]
    pub fn label(&self, index: usize) -> Option<transport_label_selector> {
        (index < 8)
            .then(|| transport_label_selector::from_raw(self.bytes[labels_offset + index]))
            .flatten()
    }
    #[must_use]
    pub fn command(&self, index: usize) -> Option<transport_render_command> {
        if index >= self.command_count() {
            None
        } else {
            decode_command(&self.bytes, index).ok()
        }
    }
}

fn validate_structure(bytes: &[u8; surface_snapshot_v1_byte_count]) -> Result<(), transport_error> {
    if read_u16(bytes, header_size_offset) != 112 {
        return Err(transport_error::wrong_header_size);
    }
    if read_u16(bytes, version_offset) != 1 {
        return Err(transport_error::wrong_version);
    }
    if read_u32(bytes, total_size_offset) != 528 {
        return Err(transport_error::wrong_total_size);
    }
    if read_u32(bytes, flags_offset) != 0 {
        return Err(transport_error::unsupported_flags);
    }
    if read_u16(bytes, profile_offset) != 1 {
        return Err(transport_error::wrong_profile);
    }
    if read_u16(bytes, label_count_offset) != 8 {
        return Err(transport_error::wrong_label_count);
    }
    if read_u16(bytes, command_size_offset) != 16 {
        return Err(transport_error::wrong_command_size);
    }
    if read_u16(bytes, viewport_width_offset) != 1_024
        || read_u16(bytes, viewport_height_offset) != 768
    {
        return Err(transport_error::wrong_viewport);
    }
    let (_, _, _, _, depth, _) = decode_state_fields(bytes)?;
    let count = usize::from(read_u16(bytes, command_count_offset));
    if count != command_count_for_depth(depth) {
        return Err(transport_error::wrong_command_count);
    }
    decode_binding(bytes)?;
    if bytes[reserved_offset..surface_slot_offset]
        .iter()
        .chain(bytes[reserved_2_offset..labels_offset].iter())
        .chain(bytes[label_reserved_offset..surface_snapshot_v1_command_area_offset].iter())
        .any(|byte| *byte != 0)
    {
        return Err(transport_error::nonzero_reserved);
    }
    decode_labels(bytes)?;
    for index in 0..count {
        validate_command(decode_command(bytes, index)?)?;
    }
    let tail =
        surface_snapshot_v1_command_area_offset + count * surface_snapshot_v1_command_byte_count;
    if bytes[tail..].iter().any(|byte| *byte != 0) {
        return Err(transport_error::nonzero_trailing_command);
    }
    Ok(())
}

fn validate_complete_profile(
    bytes: &[u8; surface_snapshot_v1_byte_count],
) -> Result<(), transport_error> {
    validate_structure(bytes)?;
    let state = decode_state(bytes)?;
    let labels = decode_labels(bytes)?;
    validate_label_profile(state.disclosure(), &labels)
}

fn decode_binding(
    bytes: &[u8; surface_snapshot_v1_byte_count],
) -> Result<surface_snapshot_binding, transport_error> {
    let identity = |slot, generation| {
        transport_identity::try_new(read_u64(bytes, slot), read_u64(bytes, generation))
            .ok_or(transport_error::invalid_binding)
    };
    surface_snapshot_binding::try_new(
        identity(surface_slot_offset, surface_generation_offset)?,
        identity(scene_slot_offset, scene_generation_offset)?,
        read_u64(bytes, base_revision_offset),
        read_u64(bytes, revision_offset),
        identity(renderer_slot_offset, renderer_generation_offset)?,
        read_u64(bytes, attempt_offset),
    )
}

fn decode_state_fields(
    bytes: &[u8; surface_snapshot_v1_byte_count],
) -> Result<
    (
        bool,
        bool,
        transport_focus,
        transport_disclosure,
        transport_depth,
        transport_motion,
    ),
    transport_error,
> {
    Ok((
        decode_bool(bytes[primary_enabled_offset])?,
        decode_bool(bytes[secondary_enabled_offset])?,
        transport_focus::from_raw(bytes[focus_offset]).ok_or(transport_error::invalid_state)?,
        transport_disclosure::from_raw(bytes[disclosure_offset])
            .ok_or(transport_error::invalid_state)?,
        transport_depth::from_raw(bytes[depth_offset]).ok_or(transport_error::invalid_state)?,
        transport_motion::from_raw(bytes[motion_offset]).ok_or(transport_error::invalid_state)?,
    ))
}

fn decode_state(
    bytes: &[u8; surface_snapshot_v1_byte_count],
) -> Result<transport_scene_state, transport_error> {
    let (primary, secondary, focus, disclosure, depth, motion) = decode_state_fields(bytes)?;
    transport_scene_state::try_new(primary, secondary, focus, disclosure, depth, motion)
}

fn decode_labels(
    bytes: &[u8; surface_snapshot_v1_byte_count],
) -> Result<[transport_label_selector; 8], transport_error> {
    let mut labels = [transport_label_selector::anomalous_desktop; 8];
    for (index, label) in labels.iter_mut().enumerate() {
        *label = transport_label_selector::from_raw(bytes[labels_offset + index])
            .ok_or(transport_error::invalid_label_selector)?;
    }
    Ok(labels)
}

fn validate_label_profile(
    disclosure: transport_disclosure,
    labels: &[transport_label_selector; 8],
) -> Result<(), transport_error> {
    let fixed = [
        transport_label_selector::anomalous_desktop,
        transport_label_selector::anomalous,
        transport_label_selector::navigation,
        transport_label_selector::open_shell,
        transport_label_selector::inspect_system,
        transport_label_selector::workspace,
        transport_label_selector::system_workspace,
    ];
    if labels[..7] != fixed {
        return Err(transport_error::label_profile_mismatch);
    }
    let status = match disclosure {
        transport_disclosure::declared_public => transport_label_selector::system_ready,
        transport_disclosure::protected => transport_label_selector::protected_status,
    };
    if labels[7] != status {
        return Err(transport_error::label_profile_mismatch);
    }
    Ok(())
}

fn validate_viewport(viewport: transport_rect) -> Result<(), transport_error> {
    if viewport
        != (transport_rect {
            x: 0,
            y: 0,
            width: 1_024,
            height: 768,
        })
    {
        return Err(transport_error::wrong_viewport);
    }
    Ok(())
}

fn validate_command(command: transport_render_command) -> Result<(), transport_error> {
    let right = command
        .bounds
        .x
        .checked_add(command.bounds.width)
        .ok_or(transport_error::geometry_overflow)?;
    let bottom = command
        .bounds
        .y
        .checked_add(command.bounds.height)
        .ok_or(transport_error::geometry_overflow)?;
    if right > 1_024 || bottom > 768 {
        Err(transport_error::outside_viewport)
    } else {
        Ok(())
    }
}

const fn command_count_for_depth(depth: transport_depth) -> usize {
    match depth {
        transport_depth::standard => 25,
        transport_depth::flat => 21,
    }
}

fn encode_command(
    bytes: &mut [u8; surface_snapshot_v1_byte_count],
    index: usize,
    command: transport_render_command,
) {
    let offset = 128 + index * 16;
    bytes[offset] = command.kind.raw();
    bytes[offset + 1] = command.shape.raw();
    bytes[offset + 2] = command.color.raw();
    bytes[offset + 3] = command.secondary_color.map_or(0, transport_color::raw);
    bytes[offset + 4] = command.slot.raw();
    bytes[offset + 5] = u8::from(command.state_active);
    write_u16(bytes, offset + 8, command.bounds.x);
    write_u16(bytes, offset + 10, command.bounds.y);
    write_u16(bytes, offset + 12, command.bounds.width);
    write_u16(bytes, offset + 14, command.bounds.height);
}

fn decode_command(
    bytes: &[u8; surface_snapshot_v1_byte_count],
    index: usize,
) -> Result<transport_render_command, transport_error> {
    let offset = 128 + index * 16;
    let secondary = if bytes[offset + 3] == 0 {
        None
    } else {
        Some(
            transport_color::from_raw(bytes[offset + 3])
                .ok_or(transport_error::invalid_secondary_color)?,
        )
    };
    if read_u16(bytes, offset + 6) != 0 {
        return Err(transport_error::nonzero_reserved);
    }
    Ok(transport_render_command::new(
        transport_command_kind::from_raw(bytes[offset])
            .ok_or(transport_error::invalid_command_kind)?,
        transport_shape::from_raw(bytes[offset + 1]).ok_or(transport_error::invalid_shape)?,
        transport_color::from_raw(bytes[offset + 2]).ok_or(transport_error::invalid_color)?,
        secondary,
        transport_slot::from_raw(bytes[offset + 4]).ok_or(transport_error::invalid_slot)?,
        decode_bool(bytes[offset + 5])?,
        transport_rect::try_new(
            read_u16(bytes, offset + 8),
            read_u16(bytes, offset + 10),
            read_u16(bytes, offset + 12),
            read_u16(bytes, offset + 14),
        )?,
    ))
}

fn decode_bool(value: u8) -> Result<bool, transport_error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(transport_error::invalid_state_flag),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("four-byte field"),
    )
}
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("eight-byte field"),
    )
}
fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "service_adapter")]
mod service_adapter {
    use anomalous_desktop_scene::{
        action_identity, declared_public_text, declared_scene_state, node_identity,
        paint_color_token, paint_command_kind, paint_shape_token, protected_text,
        scene_action_definition, scene_action_kind, scene_action_reference, scene_candidate,
        scene_depth, scene_disclosure, scene_focus, scene_identity, scene_label, scene_motion,
        scene_node_count, scene_node_definition, scene_rect, scene_role, scene_slot,
    };
    use anomalous_desktop_service::{
        desktop_surface, presentation_control, publication_phase, render_label_selector,
        render_lease, render_snapshot_view, renderer_identity, surface_control, surface_identity,
    };

    use super::*;

    const fixture_slots: [scene_slot; scene_node_count] = [
        scene_slot::root,
        scene_slot::masthead,
        scene_slot::navigation,
        scene_slot::primary_action,
        scene_slot::secondary_action,
        scene_slot::content,
        scene_slot::heading,
        scene_slot::status,
    ];
    const fixture_roles: [scene_role; scene_node_count] = [
        scene_role::desktop,
        scene_role::region,
        scene_role::region,
        scene_role::control,
        scene_role::control,
        scene_role::region,
        scene_role::heading,
        scene_role::status,
    ];
    const fixture_parent_indices: [Option<usize>; scene_node_count] = [
        None,
        Some(0),
        Some(0),
        Some(2),
        Some(2),
        Some(0),
        Some(5),
        Some(5),
    ];
    const fixture_bounds: [(u32, u32, u32, u32); scene_node_count] = [
        (0, 0, 1_024, 768),
        (32, 24, 960, 88),
        (32, 136, 256, 600),
        (56, 184, 208, 64),
        (56, 272, 208, 64),
        (312, 136, 680, 600),
        (352, 176, 600, 48),
        (352, 248, 600, 96),
    ];
    const fixture_public_labels: [&str; scene_node_count] = [
        "Anomalous desktop",
        "Anomalous",
        "Navigation",
        "Open shell",
        "Inspect system",
        "Workspace",
        "System workspace",
        "System ready",
    ];
    const fixture_action_slots: [scene_slot; 2] =
        [scene_slot::primary_action, scene_slot::secondary_action];
    const fixture_protected_label: &str = "Protected status";

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub enum service_fixture_error {
        invalid_fixture,
        service_rejected,
        transport(transport_error),
    }

    impl service_fixture_error {
        #[must_use]
        pub const fn code(self) -> &'static str {
            match self {
                Self::invalid_fixture => "invalid_fixture",
                Self::service_rejected => "service_rejected",
                Self::transport(error) => error.code(),
            }
        }
    }

    impl fmt::Debug for service_fixture_error {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.code())
        }
    }

    impl fmt::Display for service_fixture_error {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.code())
        }
    }

    /// Host-only owner of one exact ADR 0027 successor transaction.
    ///
    /// The fixture retains the real move-only surface, mutation control,
    /// presentation control, and render lease while native presentation is
    /// exercised elsewhere. It is deliberately fixed to state ordinal 61 so
    /// its reduced-motion semantic state matches the bounded ADR 0026 renderer
    /// fixture. Encoding borrows these owners and cannot publish the successor.
    ///
    /// This is conformance support, not a native service, presentation receipt,
    /// or authority that may cross the byte transport.
    pub struct retained_host_transaction_fixture {
        surface: desktop_surface,
        control: surface_control,
        presentation: presentation_control,
        lease: render_lease,
    }

    /// Content-free result of consuming the fixed transaction successfully.
    #[derive(Clone, Copy, Eq, PartialEq)]
    pub struct retained_host_transaction_success {
        revision: u64,
        publication_sequence: u64,
    }

    impl retained_host_transaction_success {
        #[must_use]
        pub const fn revision(self) -> u64 {
            self.revision
        }

        #[must_use]
        pub const fn publication_sequence(self) -> u64 {
            self.publication_sequence
        }
    }

    impl retained_host_transaction_fixture {
        /// Construct the exact ordinal-61 protected successor and retain every
        /// ADR 0027 owner needed to publish it later.
        pub fn try_new(protected_status: &str) -> Result<Self, service_fixture_error> {
            try_new_retained_fixture(61, protected_status)
        }

        /// Encode the exact immutable body borrowed from this fixture's real
        /// service lease. No service phase or public projection is changed.
        pub fn try_encode_exact_body(&self) -> Result<surface_snapshot_v1, service_fixture_error> {
            let snapshot = self
                .surface
                .render_candidate(&self.lease)
                .map_err(|_| service_fixture_error::service_rejected)?;
            try_encode_service_snapshot(&snapshot, &self.lease)
                .map_err(service_fixture_error::transport)
        }

        /// Check that the successor remains private while revision one is the
        /// sole public human, accessibility, and machine observation.
        #[must_use]
        pub fn revision_one_remains_public(&self) -> bool {
            revision_one_remains_public(&self.surface)
        }

        /// Consume the retained owners after the caller has independently
        /// verified the matching native presentation. This is the trusted-host
        /// boundary: the method itself does not inspect pixels or a transcript.
        pub fn complete_after_verified_presentation(
            self,
        ) -> Result<retained_host_transaction_success, service_fixture_error> {
            let Self {
                mut surface,
                control,
                presentation,
                lease,
            } = self;
            if !revision_one_remains_public(&surface) {
                return Err(service_fixture_error::service_rejected);
            }
            let ready = surface
                .complete_render(lease)
                .map_err(|_| service_fixture_error::service_rejected)?;
            if surface.phase() != publication_phase::ready
                || !revision_one_projection_is_exact(&surface)
            {
                return Err(service_fixture_error::service_rejected);
            }
            let receipt = surface
                .attest_presented(&presentation, ready)
                .map_err(|_| service_fixture_error::service_rejected)?;
            if surface.phase() != publication_phase::presented
                || !revision_one_projection_is_exact(&surface)
            {
                return Err(service_fixture_error::service_rejected);
            }
            let publication = surface
                .publish(&control, receipt)
                .map_err(|_| service_fixture_error::service_rejected)?;
            let revision = publication.revision().get();
            let publication_sequence = publication.sequence().get();
            let publication_is_exact = revision == 2
                && publication_sequence == 2
                && publication.renderer().slot() == 61
                && publication.renderer().generation() == 1
                && publication.attempt().get() == 1
                && surface.presented_revision().get() == 2
                && surface.publication_sequence().get() == 2
                && surface.pending_revision().is_none()
                && surface.phase() == publication_phase::steady
                && fixture_state_from_ordinal(61)
                    .is_ok_and(|state| fixture_projection_is_exact(&surface, 2, state));
            let retirement = surface
                .retire(control)
                .map_err(|_| service_fixture_error::service_rejected)?;
            if !publication_is_exact
                || retirement.final_revision().get() != 2
                || retirement.final_sequence().get() != 2
                || retirement.final_renderer().slot() != 61
                || retirement.final_renderer().generation() != 1
                || retirement.discarded_pending_revision()
            {
                return Err(service_fixture_error::service_rejected);
            }
            Ok(retained_host_transaction_success {
                revision,
                publication_sequence,
            })
        }

        /// Consume a failed or unverified run without attesting presentation.
        /// The pending successor must be discarded while revision and sequence
        /// one remain the final public observation.
        pub fn retire_without_publication(self) -> Result<(), service_fixture_error> {
            let Self {
                surface, control, ..
            } = self;
            if !revision_one_remains_public(&surface) {
                return Err(service_fixture_error::service_rejected);
            }
            let retirement = surface
                .retire(control)
                .map_err(|_| service_fixture_error::service_rejected)?;
            if retirement.final_revision().get() != 1
                || retirement.final_sequence().get() != 1
                || retirement.final_renderer().slot() != 61
                || retirement.final_renderer().generation() != 1
                || !retirement.discarded_pending_revision()
            {
                return Err(service_fixture_error::service_rejected);
            }
            Ok(())
        }
    }

    fn revision_one_remains_public(surface: &desktop_surface) -> bool {
        surface.phase() == publication_phase::leased && revision_one_projection_is_exact(surface)
    }

    fn revision_one_projection_is_exact(surface: &desktop_surface) -> bool {
        surface.presented_revision().get() == 1
            && surface.publication_sequence().get() == 1
            && surface
                .pending_revision()
                .is_some_and(|revision| revision.get() == 2)
            && fixture_projection_is_exact(surface, 1, initial_fixture_state())
    }

    fn fixture_projection_is_exact(
        surface: &desktop_surface,
        revision: u64,
        state: declared_scene_state,
    ) -> bool {
        let Ok(scene) = fixture_scene_identity() else {
            return false;
        };
        let Some(viewport) = fixture_rect(0) else {
            return false;
        };
        let presented = surface.presented();
        if presented.identity() != scene
            || presented.revision().get() != revision
            || presented.state() != state
            || presented.viewport() != viewport
            || presented.render_command_count() != 25
            || presented.accessibility_node_count() != scene_node_count
            || presented.machine_node_count() != scene_node_count
            || presented.machine_action_count() != fixture_action_slots.len()
        {
            return false;
        }

        for (index, slot) in fixture_slots.into_iter().enumerate() {
            let Some(accessible) = presented.accessibility_node(index) else {
                return false;
            };
            let Some(machine) = presented.machine_node(index) else {
                return false;
            };
            let Some(accessibility_label) = presented.accessibility_label(index) else {
                return false;
            };
            let Some(machine_label) = presented.machine_label(index) else {
                return false;
            };
            let Ok(node) = fixture_node_identity(scene, slot) else {
                return false;
            };
            let parent = match fixture_parent_indices[index] {
                Some(parent_index) => {
                    let Ok(parent) = fixture_node_identity(scene, fixture_slots[parent_index])
                    else {
                        return false;
                    };
                    Some(parent)
                }
                None => None,
            };
            let Some(bounds) = fixture_rect(index) else {
                return false;
            };
            let disclosure = if slot == scene_slot::status {
                state.status_disclosure()
            } else {
                scene_disclosure::declared_public
            };
            let enabled = match slot {
                scene_slot::primary_action => state.primary_enabled(),
                scene_slot::secondary_action => state.secondary_enabled(),
                _ => true,
            };
            let focused = match slot {
                scene_slot::primary_action => state.focus() == scene_focus::primary,
                scene_slot::secondary_action => state.focus() == scene_focus::secondary,
                _ => false,
            };
            let focus_order = match slot {
                scene_slot::primary_action => Some(0),
                scene_slot::secondary_action => Some(1),
                _ => None,
            };
            let label = if disclosure == scene_disclosure::protected {
                fixture_protected_label
            } else {
                fixture_public_labels[index]
            };

            if accessible.slot() != slot
                || accessible.node() != node
                || accessible.parent() != parent
                || accessible.role() != fixture_roles[index]
                || accessible.bounds() != bounds
                || accessible.disclosure() != disclosure
                || accessible.enabled() != enabled
                || accessible.focused() != focused
                || accessible.focus_order() != focus_order
                || !fixture_action_reference_is_exact(accessible.action(), scene, revision, slot)
                || machine.slot() != slot
                || machine.node() != node
                || machine.parent() != parent
                || machine.role() != fixture_roles[index]
                || machine.bounds() != bounds
                || machine.disclosure() != disclosure
                || machine.enabled() != enabled
                || machine.focused() != focused
                || machine.focus_order() != focus_order
                || !fixture_action_reference_is_exact(machine.action(), scene, revision, slot)
                || accessibility_label.disclosure() != disclosure
                || accessibility_label.expose_str() != label
                || machine_label.disclosure() != disclosure
                || machine_label.expose_str() != label
            {
                return false;
            }
        }

        if presented.accessibility_node(scene_node_count).is_some()
            || presented.accessibility_label(scene_node_count).is_some()
            || presented.machine_node(scene_node_count).is_some()
            || presented.machine_label(scene_node_count).is_some()
        {
            return false;
        }

        for (index, slot) in fixture_action_slots.into_iter().enumerate() {
            let Some(action) = presented.machine_action(index) else {
                return false;
            };
            let enabled = if slot == scene_slot::primary_action {
                state.primary_enabled()
            } else {
                state.secondary_enabled()
            };
            let focused = state.focus()
                == if slot == scene_slot::primary_action {
                    scene_focus::primary
                } else {
                    scene_focus::secondary
                };
            if !fixture_action_reference_is_exact(Some(action.reference()), scene, revision, slot)
                || action.enabled() != enabled
                || action.focused() != focused
            {
                return false;
            }
        }

        presented
            .machine_action(fixture_action_slots.len())
            .is_none()
    }

    fn fixture_action_reference_is_exact(
        reference: Option<scene_action_reference>,
        scene: scene_identity,
        revision: u64,
        slot: scene_slot,
    ) -> bool {
        if !matches!(
            slot,
            scene_slot::primary_action | scene_slot::secondary_action
        ) {
            return reference.is_none();
        }
        let Some(reference) = reference else {
            return false;
        };
        let Ok(node) = fixture_node_identity(scene, slot) else {
            return false;
        };
        let Ok(action) = fixture_action_identity(scene, slot) else {
            return false;
        };
        reference.scene() == scene
            && reference.revision().get() == revision
            && reference.node() == node
            && reference.action() == action
            && reference.kind() == scene_action_kind::activate
    }

    fn fixture_rect(index: usize) -> Option<scene_rect> {
        let (x, y, width, height) = *fixture_bounds.get(index)?;
        scene_rect::try_new(x, y, width, height).ok()
    }

    /// Explicitly map one already-authorized ADR 0027 view and matching lease.
    ///
    /// Public equality here is only correlation. The caller must have obtained
    /// the view by validating this exact move-only lease with its owning
    /// service; this function does not recreate that authority from scalars.
    pub fn try_encode_service_snapshot(
        snapshot: &render_snapshot_view<'_>,
        lease: &render_lease,
    ) -> Result<surface_snapshot_v1, transport_error> {
        if snapshot.surface() != lease.surface() || snapshot.revision() != lease.revision() {
            return Err(transport_error::binding_mismatch);
        }
        let binding = surface_snapshot_binding::try_new(
            transport_identity::try_new(snapshot.surface().slot(), snapshot.surface().generation())
                .ok_or(transport_error::invalid_binding)?,
            transport_identity::try_new(snapshot.scene().slot(), snapshot.scene().generation())
                .ok_or(transport_error::invalid_binding)?,
            snapshot.base_revision().get(),
            snapshot.revision().get(),
            transport_identity::try_new(lease.renderer().slot(), lease.renderer().generation())
                .ok_or(transport_error::invalid_binding)?,
            lease.attempt().get(),
        )?;
        let source_state = snapshot.state();
        let state = transport_scene_state::try_new(
            source_state.primary_enabled(),
            source_state.secondary_enabled(),
            map_focus(source_state.focus()),
            map_disclosure(source_state.status_disclosure()),
            map_depth(source_state.depth()),
            map_motion(source_state.motion()),
        )?;
        let source_viewport = snapshot.viewport();
        let viewport = transport_rect::try_new(
            source_viewport.x(),
            source_viewport.y(),
            source_viewport.width(),
            source_viewport.height(),
        )?;

        let mut labels = [transport_label_selector::anomalous_desktop; 8];
        for (index, label) in labels.iter_mut().enumerate() {
            *label = map_label(
                snapshot
                    .label(index)
                    .ok_or(transport_error::wrong_label_count)?,
            );
        }
        if snapshot.label(8).is_some() {
            return Err(transport_error::wrong_label_count);
        }

        let count = snapshot.command_count();
        if count > 25 {
            return Err(transport_error::wrong_command_count);
        }
        let placeholder = transport_render_command::new(
            transport_command_kind::fill,
            transport_shape::rectangle,
            transport_color::canvas,
            None,
            transport_slot::root,
            false,
            transport_rect::try_new(0, 0, 1, 1)?,
        );
        let mut commands = [placeholder; 25];
        for (index, command) in commands.iter_mut().enumerate().take(count) {
            let source = snapshot
                .command(index)
                .ok_or(transport_error::wrong_command_count)?;
            let bounds = source.bounds();
            *command = transport_render_command::new(
                map_kind(source.kind()),
                map_shape(source.shape()),
                map_color(source.color()),
                source.secondary_color().map(map_color),
                map_slot(source.slot()),
                source.state_active(),
                transport_rect::try_new(bounds.x(), bounds.y(), bounds.width(), bounds.height())?,
            );
        }
        if snapshot.command(count).is_some() {
            return Err(transport_error::wrong_command_count);
        }
        surface_snapshot_v1::try_encode(binding, state, viewport, labels, &commands[..count])
    }

    /// Build the exact host-only ADR 0028 conformance fixture at state ordinal
    /// 60 and encode its leased successor. The supplied protected bytes affect
    /// only the private scene source and must not affect the returned body.
    ///
    /// This helper is absent unless `service_adapter` is enabled. It is test
    /// and build conformance support, not a deployed scene service.
    pub fn try_encode_fixed_protected_fixture(
        protected_status: &str,
    ) -> Result<surface_snapshot_v1, service_fixture_error> {
        try_encode_fixture_ordinal(60, protected_status)
    }

    pub(super) fn try_encode_fixture_ordinal(
        ordinal: u8,
        protected_status: &str,
    ) -> Result<surface_snapshot_v1, service_fixture_error> {
        let fixture = try_new_retained_fixture(ordinal, protected_status)?;
        let encoded = fixture.try_encode_exact_body()?;
        fixture.retire_without_publication()?;
        Ok(encoded)
    }

    fn try_new_retained_fixture(
        ordinal: u8,
        protected_status: &str,
    ) -> Result<retained_host_transaction_fixture, service_fixture_error> {
        let successor_state = fixture_state_from_ordinal(ordinal)?;
        let scene = fixture_scene_identity()?;
        let surface_identity =
            surface_identity::try_new(51, 3).map_err(|_| service_fixture_error::invalid_fixture)?;
        let renderer = renderer_identity::try_new(61, 1)
            .map_err(|_| service_fixture_error::invalid_fixture)?;
        let (mut surface, control, presentation, endpoint) = desktop_surface::new_presented(
            surface_identity,
            renderer,
            fixture_candidate(scene, initial_fixture_state(), protected_status)?,
        )
        .map_err(|_| service_fixture_error::service_rejected)?;
        let base = surface.presented_revision();
        surface
            .submit_revision(
                &control,
                base,
                fixture_candidate(scene, successor_state, protected_status)?,
            )
            .map_err(|_| service_fixture_error::service_rejected)?;
        let lease = surface
            .acquire_render(&endpoint)
            .map_err(|_| service_fixture_error::service_rejected)?;
        let fixture = retained_host_transaction_fixture {
            surface,
            control,
            presentation,
            lease,
        };
        if !fixture.revision_one_remains_public() {
            return Err(service_fixture_error::service_rejected);
        }
        Ok(fixture)
    }

    const fn initial_fixture_state() -> declared_scene_state {
        declared_scene_state::new(
            false,
            false,
            scene_focus::none,
            scene_disclosure::declared_public,
            scene_depth::standard,
            scene_motion::standard,
        )
    }

    fn fixture_state_from_ordinal(
        ordinal: u8,
    ) -> Result<declared_scene_state, service_fixture_error> {
        if ordinal >= 96 {
            return Err(service_fixture_error::invalid_fixture);
        }
        let motion = if ordinal.is_multiple_of(2) {
            scene_motion::standard
        } else {
            scene_motion::reduced
        };
        let depth = if (ordinal / 2).is_multiple_of(2) {
            scene_depth::standard
        } else {
            scene_depth::flat
        };
        let disclosure = if (ordinal / 4).is_multiple_of(2) {
            scene_disclosure::declared_public
        } else {
            scene_disclosure::protected
        };
        let packed = ordinal / 8;
        let focus = match packed % 3 {
            0 => scene_focus::none,
            1 => scene_focus::primary,
            2 => scene_focus::secondary,
            _ => return Err(service_fixture_error::invalid_fixture),
        };
        let enabled = packed / 3;
        let primary_enabled = enabled & 2 != 0;
        let secondary_enabled = enabled & 1 != 0;
        if (focus == scene_focus::primary && !primary_enabled)
            || (focus == scene_focus::secondary && !secondary_enabled)
        {
            return Err(service_fixture_error::invalid_fixture);
        }
        Ok(declared_scene_state::new(
            primary_enabled,
            secondary_enabled,
            focus,
            disclosure,
            depth,
            motion,
        ))
    }

    fn fixture_scene_identity() -> Result<scene_identity, service_fixture_error> {
        scene_identity::try_new(41, 7).map_err(|_| service_fixture_error::invalid_fixture)
    }

    fn fixture_node_identity(
        scene: scene_identity,
        slot: scene_slot,
    ) -> Result<node_identity, service_fixture_error> {
        node_identity::try_new(slot.identity_slot(), scene.generation())
            .map_err(|_| service_fixture_error::invalid_fixture)
    }

    fn fixture_action_identity(
        scene: scene_identity,
        slot: scene_slot,
    ) -> Result<action_identity, service_fixture_error> {
        let value = match slot {
            scene_slot::primary_action => 1,
            scene_slot::secondary_action => 2,
            _ => return Err(service_fixture_error::invalid_fixture),
        };
        action_identity::try_new(value, scene.generation())
            .map_err(|_| service_fixture_error::invalid_fixture)
    }

    fn fixture_candidate(
        scene: scene_identity,
        state: declared_scene_state,
        protected_status: &str,
    ) -> Result<scene_candidate, service_fixture_error> {
        let mut failed = false;
        let nodes = core::array::from_fn(|index| {
            let slot = fixture_slots[index];
            let parent = fixture_parent_indices[index].and_then(|parent| {
                fixture_node_identity(scene, fixture_slots[parent])
                    .map_err(|_| failed = true)
                    .ok()
            });
            let action = match slot {
                scene_slot::primary_action | scene_slot::secondary_action => {
                    fixture_action_identity(scene, slot)
                        .map(|identity| {
                            scene_action_definition::new(identity, scene_action_kind::activate)
                        })
                        .map_err(|_| failed = true)
                        .ok()
                }
                _ => None,
            };
            let label = if slot == scene_slot::status
                && state.status_disclosure() == scene_disclosure::protected
            {
                protected_text::try_from_str(protected_status)
                    .map(scene_label::protected)
                    .map_err(|_| failed = true)
                    .unwrap_or_else(|_| {
                        scene_label::declared_public(
                            declared_public_text::try_from_str("System ready")
                                .unwrap_or_else(|_| unreachable!()),
                        )
                    })
            } else {
                scene_label::declared_public(
                    declared_public_text::try_from_str(fixture_public_labels[index])
                        .map_err(|_| failed = true)
                        .unwrap_or_else(|_| unreachable!()),
                )
            };
            let (x, y, width, height) = fixture_bounds[index];
            let node_bounds = scene_rect::try_new(x, y, width, height)
                .map_err(|_| failed = true)
                .unwrap_or_else(|_| unreachable!());
            let identity = fixture_node_identity(scene, slot)
                .map_err(|_| failed = true)
                .unwrap_or_else(|_| fixture_node_identity(scene, scene_slot::root).unwrap());
            scene_node_definition::new(
                slot,
                identity,
                fixture_roles[index],
                parent,
                node_bounds,
                label,
                action,
            )
        });
        if failed {
            return Err(service_fixture_error::invalid_fixture);
        }
        Ok(scene_candidate::new(scene, nodes, state))
    }

    const fn map_focus(value: scene_focus) -> transport_focus {
        match value {
            scene_focus::none => transport_focus::none,
            scene_focus::primary => transport_focus::primary,
            scene_focus::secondary => transport_focus::secondary,
        }
    }
    const fn map_disclosure(value: scene_disclosure) -> transport_disclosure {
        match value {
            scene_disclosure::declared_public => transport_disclosure::declared_public,
            scene_disclosure::protected => transport_disclosure::protected,
        }
    }
    const fn map_depth(value: scene_depth) -> transport_depth {
        match value {
            scene_depth::standard => transport_depth::standard,
            scene_depth::flat => transport_depth::flat,
        }
    }
    const fn map_motion(value: scene_motion) -> transport_motion {
        match value {
            scene_motion::standard => transport_motion::standard,
            scene_motion::reduced => transport_motion::reduced,
        }
    }
    const fn map_kind(value: paint_command_kind) -> transport_command_kind {
        match value {
            paint_command_kind::fill => transport_command_kind::fill,
            paint_command_kind::outline => transport_command_kind::outline,
            paint_command_kind::label => transport_command_kind::label,
            paint_command_kind::enabled_marker => transport_command_kind::enabled_marker,
            paint_command_kind::focus_outline => transport_command_kind::focus_outline,
            paint_command_kind::shadow => transport_command_kind::shadow,
            paint_command_kind::highlight => transport_command_kind::highlight,
        }
    }
    const fn map_shape(value: paint_shape_token) -> transport_shape {
        match value {
            paint_shape_token::rectangle => transport_shape::rectangle,
            paint_shape_token::text => transport_shape::text,
            paint_shape_token::enabled => transport_shape::enabled,
            paint_shape_token::disabled => transport_shape::disabled,
            paint_shape_token::focus_absent => transport_shape::focus_absent,
            paint_shape_token::focus_double_outline => transport_shape::focus_double_outline,
        }
    }
    const fn map_color(value: paint_color_token) -> transport_color {
        match value {
            paint_color_token::canvas => transport_color::canvas,
            paint_color_token::surface => transport_color::surface,
            paint_color_token::primary_fill => transport_color::primary_fill,
            paint_color_token::secondary_fill => transport_color::secondary_fill,
            paint_color_token::disabled => transport_color::disabled,
            paint_color_token::primary_text => transport_color::primary_text,
            paint_color_token::secondary_text => transport_color::secondary_text,
            paint_color_token::on_accent => transport_color::on_accent,
            paint_color_token::outline => transport_color::outline,
            paint_color_token::focus_outer => transport_color::focus_outer,
            paint_color_token::focus_inner => transport_color::focus_inner,
            paint_color_token::public_status => transport_color::public_status,
            paint_color_token::protected_status => transport_color::protected_status,
            paint_color_token::shadow => transport_color::shadow,
            paint_color_token::highlight => transport_color::highlight,
        }
    }
    const fn map_slot(value: scene_slot) -> transport_slot {
        match value {
            scene_slot::root => transport_slot::root,
            scene_slot::masthead => transport_slot::masthead,
            scene_slot::navigation => transport_slot::navigation,
            scene_slot::primary_action => transport_slot::primary_action,
            scene_slot::secondary_action => transport_slot::secondary_action,
            scene_slot::content => transport_slot::content,
            scene_slot::heading => transport_slot::heading,
            scene_slot::status => transport_slot::status,
        }
    }
    const fn map_label(value: render_label_selector) -> transport_label_selector {
        match value {
            render_label_selector::anomalous_desktop => transport_label_selector::anomalous_desktop,
            render_label_selector::anomalous => transport_label_selector::anomalous,
            render_label_selector::navigation => transport_label_selector::navigation,
            render_label_selector::open_shell => transport_label_selector::open_shell,
            render_label_selector::inspect_system => transport_label_selector::inspect_system,
            render_label_selector::workspace => transport_label_selector::workspace,
            render_label_selector::system_workspace => transport_label_selector::system_workspace,
            render_label_selector::system_ready => transport_label_selector::system_ready,
            render_label_selector::protected_status => transport_label_selector::protected_status,
        }
    }

    #[cfg(test)]
    mod adapter_tests {
        use super::*;

        fn service_ok<T, E>(result: Result<T, E>, context: &'static str) -> T {
            match result {
                Ok(value) => value,
                Err(_) => panic!("{context}"),
            }
        }

        fn initial_state() -> declared_scene_state {
            declared_scene_state::new(
                false,
                false,
                scene_focus::none,
                scene_disclosure::declared_public,
                scene_depth::standard,
                scene_motion::standard,
            )
        }

        fn surface_id(slot: u64) -> surface_identity {
            surface_identity::try_new(slot, 3).unwrap_or_else(|_| panic!("surface identity"))
        }

        fn renderer_id() -> renderer_identity {
            renderer_identity::try_new(61, 1).unwrap_or_else(|_| panic!("renderer identity"))
        }

        const fn focus_code(value: scene_focus) -> u8 {
            match value {
                scene_focus::none => 0,
                scene_focus::primary => 1,
                scene_focus::secondary => 2,
            }
        }

        const fn disclosure_code(value: scene_disclosure) -> u8 {
            match value {
                scene_disclosure::declared_public => 0,
                scene_disclosure::protected => 1,
            }
        }

        const fn depth_code(value: scene_depth) -> u8 {
            match value {
                scene_depth::standard => 0,
                scene_depth::flat => 1,
            }
        }

        const fn motion_code(value: scene_motion) -> u8 {
            match value {
                scene_motion::standard => 0,
                scene_motion::reduced => 1,
            }
        }

        const fn kind_code(value: paint_command_kind) -> u8 {
            match value {
                paint_command_kind::fill => 1,
                paint_command_kind::outline => 2,
                paint_command_kind::label => 3,
                paint_command_kind::enabled_marker => 4,
                paint_command_kind::focus_outline => 5,
                paint_command_kind::shadow => 6,
                paint_command_kind::highlight => 7,
            }
        }

        const fn shape_code(value: paint_shape_token) -> u8 {
            match value {
                paint_shape_token::rectangle => 1,
                paint_shape_token::text => 2,
                paint_shape_token::enabled => 3,
                paint_shape_token::disabled => 4,
                paint_shape_token::focus_absent => 5,
                paint_shape_token::focus_double_outline => 6,
            }
        }

        const fn color_code(value: paint_color_token) -> u8 {
            match value {
                paint_color_token::canvas => 1,
                paint_color_token::surface => 2,
                paint_color_token::primary_fill => 3,
                paint_color_token::secondary_fill => 4,
                paint_color_token::disabled => 5,
                paint_color_token::primary_text => 6,
                paint_color_token::secondary_text => 7,
                paint_color_token::on_accent => 8,
                paint_color_token::outline => 9,
                paint_color_token::focus_outer => 10,
                paint_color_token::focus_inner => 11,
                paint_color_token::public_status => 12,
                paint_color_token::protected_status => 13,
                paint_color_token::shadow => 14,
                paint_color_token::highlight => 15,
            }
        }

        const fn slot_code(value: scene_slot) -> u8 {
            match value {
                scene_slot::root => 1,
                scene_slot::masthead => 2,
                scene_slot::navigation => 3,
                scene_slot::primary_action => 4,
                scene_slot::secondary_action => 5,
                scene_slot::content => 6,
                scene_slot::heading => 7,
                scene_slot::status => 8,
            }
        }

        const fn label_code(value: render_label_selector) -> u8 {
            match value {
                render_label_selector::anomalous_desktop => 1,
                render_label_selector::anomalous => 2,
                render_label_selector::navigation => 3,
                render_label_selector::open_shell => 4,
                render_label_selector::inspect_system => 5,
                render_label_selector::workspace => 6,
                render_label_selector::system_workspace => 7,
                render_label_selector::system_ready => 8,
                render_label_selector::protected_status => 9,
            }
        }

        fn assert_complete_mapping(
            source: &render_snapshot_view<'_>,
            lease: &render_lease,
            encoded: &surface_snapshot_v1,
        ) {
            assert_eq!(encoded.as_bytes().len(), surface_snapshot_v1_byte_count);
            let binding = encoded.binding();
            assert_eq!(binding.surface().slot(), source.surface().slot());
            assert_eq!(
                binding.surface().generation(),
                source.surface().generation()
            );
            assert_eq!(binding.scene().slot(), source.scene().slot());
            assert_eq!(binding.scene().generation(), source.scene().generation());
            assert_eq!(binding.base_revision(), source.base_revision().get());
            assert_eq!(binding.revision(), source.revision().get());
            assert_eq!(binding.renderer().slot(), lease.renderer().slot());
            assert_eq!(
                binding.renderer().generation(),
                lease.renderer().generation()
            );
            assert_eq!(binding.attempt(), lease.attempt().get());

            let source_state = source.state();
            let encoded_state = encoded.state();
            assert_eq!(
                encoded_state.primary_enabled(),
                source_state.primary_enabled()
            );
            assert_eq!(
                encoded_state.secondary_enabled(),
                source_state.secondary_enabled()
            );
            assert_eq!(
                encoded_state.focus().raw(),
                focus_code(source_state.focus())
            );
            assert_eq!(
                encoded_state.disclosure().raw(),
                disclosure_code(source_state.status_disclosure())
            );
            assert_eq!(
                encoded_state.depth().raw(),
                depth_code(source_state.depth())
            );
            assert_eq!(
                encoded_state.motion().raw(),
                motion_code(source_state.motion())
            );

            let source_viewport = source.viewport();
            let encoded_viewport = encoded.viewport();
            assert_eq!(encoded_viewport.x(), source_viewport.x());
            assert_eq!(encoded_viewport.y(), source_viewport.y());
            assert_eq!(encoded_viewport.width(), source_viewport.width());
            assert_eq!(encoded_viewport.height(), source_viewport.height());

            for index in 0..surface_snapshot_v1_label_count {
                let expected = label_code(source.label(index).expect("source label"));
                assert_eq!(encoded.label(index).expect("encoded label").raw(), expected);
            }
            assert!(source.label(surface_snapshot_v1_label_count).is_none());
            assert!(encoded.label(surface_snapshot_v1_label_count).is_none());

            assert_eq!(encoded.command_count(), source.command_count());
            for index in 0..source.command_count() {
                let expected = source.command(index).expect("source command");
                let actual = encoded.command(index).expect("encoded command");
                assert_eq!(actual.kind().raw(), kind_code(expected.kind()));
                assert_eq!(actual.shape().raw(), shape_code(expected.shape()));
                assert_eq!(actual.color().raw(), color_code(expected.color()));
                assert_eq!(
                    actual.secondary_color().map(transport_color::raw),
                    expected.secondary_color().map(color_code)
                );
                assert_eq!(actual.slot().raw(), slot_code(expected.slot()));
                assert_eq!(actual.state_active(), expected.state_active());
                let expected_bounds = expected.bounds();
                let actual_bounds = actual.bounds();
                assert_eq!(actual_bounds.x(), expected_bounds.x());
                assert_eq!(actual_bounds.y(), expected_bounds.y());
                assert_eq!(actual_bounds.width(), expected_bounds.width());
                assert_eq!(actual_bounds.height(), expected_bounds.height());
            }
            assert!(source.command(source.command_count()).is_none());
            assert!(encoded.command(encoded.command_count()).is_none());
        }

        #[test]
        fn adapter_maps_every_field_of_all_sixty_four_distinct_successor_states() {
            // ADR 0027's service suite proves all 64 x 64 accepted transitions.
            // No initial-state field enters a render_snapshot_view, so the 64
            // successor projections below exhaust the adapter's distinct state
            // inputs while still acquiring each view through a real lease.
            let mut accepted = 0;
            for ordinal in 0..96_u8 {
                let Ok(successor) = fixture_state_from_ordinal(ordinal) else {
                    continue;
                };
                accepted += 1;
                let scene = fixture_scene_identity().expect("scene identity");
                let (mut surface, control, _presentation, endpoint) = service_ok(
                    desktop_surface::new_presented(
                        surface_id(51),
                        renderer_id(),
                        fixture_candidate(scene, initial_state(), "matrix private source")
                            .expect("initial fixture"),
                    ),
                    "presented surface",
                );
                let base = surface.presented_revision();
                let _ = service_ok(
                    surface.submit_revision(
                        &control,
                        base,
                        fixture_candidate(scene, successor, "matrix private source")
                            .expect("successor fixture"),
                    ),
                    "submit successor",
                );
                let lease = surface.acquire_render(&endpoint).expect("acquire lease");
                {
                    let source = surface.render_candidate(&lease).expect("render view");
                    let encoded =
                        try_encode_service_snapshot(&source, &lease).expect("encode view");
                    assert_complete_mapping(&source, &lease, &encoded);
                }
                assert!(
                    service_ok(surface.retire(control), "retire surface")
                        .discarded_pending_revision()
                );
            }
            assert_eq!(accepted, 64);
        }

        #[test]
        fn retained_ordinal_61_transactions_are_private_and_source_independent() {
            let first_source = "short protected value";
            let second_source =
                "a distinct protected source whose length is intentionally much greater";
            assert_ne!(first_source.len(), second_source.len());

            let first = retained_host_transaction_fixture::try_new(first_source)
                .unwrap_or_else(|error| panic!("{}", error.code()));
            let second = retained_host_transaction_fixture::try_new(second_source)
                .unwrap_or_else(|error| panic!("{}", error.code()));
            assert!(first.revision_one_remains_public());
            assert!(second.revision_one_remains_public());

            let first_body = first
                .try_encode_exact_body()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            let repeated_first_body = first
                .try_encode_exact_body()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            let second_body = second
                .try_encode_exact_body()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            assert_eq!(first_body.as_bytes(), repeated_first_body.as_bytes());
            assert_eq!(first_body.as_bytes(), second_body.as_bytes());
            assert!(first_body.binding().surface() == transport_identity::try_new(51, 3).unwrap());
            assert!(first_body.binding().scene() == transport_identity::try_new(41, 7).unwrap());
            assert_eq!(first_body.binding().base_revision(), 1);
            assert_eq!(first_body.binding().revision(), 2);
            assert!(first_body.binding().renderer() == transport_identity::try_new(61, 1).unwrap());
            assert_eq!(first_body.binding().attempt(), 1);
            assert!(first_body.state().primary_enabled());
            assert!(!first_body.state().secondary_enabled());
            assert!(first_body.state().focus() == transport_focus::primary);
            assert!(first_body.state().disclosure() == transport_disclosure::protected);
            assert!(first_body.state().depth() == transport_depth::standard);
            assert!(first_body.state().motion() == transport_motion::reduced);
            assert_eq!(first_body.command_count(), 25);
            assert!(first_body.label(7) == Some(transport_label_selector::protected_status));
            assert!(first.revision_one_remains_public());
            assert!(second.revision_one_remains_public());

            first
                .retire_without_publication()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            second
                .retire_without_publication()
                .unwrap_or_else(|error| panic!("{}", error.code()));
        }

        #[test]
        fn retained_transaction_publishes_only_on_consuming_success() {
            let fixture = retained_host_transaction_fixture::try_new(
                "protected value held through native presentation",
            )
            .unwrap_or_else(|error| panic!("{}", error.code()));
            assert!(fixture.revision_one_remains_public());
            let _ = fixture
                .try_encode_exact_body()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            assert!(fixture.revision_one_remains_public());

            let success = fixture
                .complete_after_verified_presentation()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            assert_eq!(success.revision(), 2);
            assert_eq!(success.publication_sequence(), 2);
        }

        #[test]
        fn retained_transaction_checks_exact_projections_across_publication() {
            for protected_status in [
                "brief private status",
                "different private status with deliberately distinct bytes",
            ] {
                let fixture = retained_host_transaction_fixture::try_new(protected_status)
                    .unwrap_or_else(|error| panic!("{}", error.code()));
                let retained_host_transaction_fixture {
                    mut surface,
                    control,
                    presentation,
                    lease,
                } = fixture;
                let successor = fixture_state_from_ordinal(61).expect("ordinal 61 state");

                assert!(revision_one_projection_is_exact(&surface));
                assert!(!fixture_projection_is_exact(&surface, 1, successor));

                let ready = surface
                    .complete_render(lease)
                    .unwrap_or_else(|_| panic!("complete render"));
                assert!(surface.phase() == publication_phase::ready);
                assert!(revision_one_projection_is_exact(&surface));

                let receipt = surface
                    .attest_presented(&presentation, ready)
                    .unwrap_or_else(|_| panic!("attest presentation"));
                assert!(surface.phase() == publication_phase::presented);
                assert!(revision_one_projection_is_exact(&surface));

                let publication = surface
                    .publish(&control, receipt)
                    .unwrap_or_else(|_| panic!("publish successor"));
                assert_eq!(publication.revision().get(), 2);
                assert_eq!(publication.sequence().get(), 2);
                assert!(fixture_projection_is_exact(&surface, 2, successor));
                assert!(!fixture_projection_is_exact(
                    &surface,
                    2,
                    initial_fixture_state(),
                ));

                let retirement = surface
                    .retire(control)
                    .unwrap_or_else(|_| panic!("retire published surface"));
                assert_eq!(retirement.final_revision().get(), 2);
                assert_eq!(retirement.final_sequence().get(), 2);
                assert!(!retirement.discarded_pending_revision());
            }
        }

        #[test]
        fn retained_transaction_failure_never_attests_or_publishes() {
            let fixture = retained_host_transaction_fixture::try_new(
                "protected value for rejected native evidence",
            )
            .unwrap_or_else(|error| panic!("{}", error.code()));
            let _ = fixture
                .try_encode_exact_body()
                .unwrap_or_else(|error| panic!("{}", error.code()));
            assert!(fixture.revision_one_remains_public());
            fixture
                .retire_without_publication()
                .unwrap_or_else(|error| panic!("{}", error.code()));

            match retained_host_transaction_fixture::try_new("") {
                Err(error) => assert!(error == service_fixture_error::invalid_fixture),
                Ok(unexpected) => {
                    let _ = unexpected.retire_without_publication();
                    panic!("empty protected source was accepted")
                }
            }
        }

        #[test]
        fn adapter_rejects_wrong_surface_and_revision_lease_pairings() {
            let scene = fixture_scene_identity().expect("scene identity");
            let successor = fixture_state_from_ordinal(60).expect("successor state");

            let (mut first, first_control, _first_presentation, first_endpoint) = service_ok(
                desktop_surface::new_presented(
                    surface_id(51),
                    renderer_id(),
                    fixture_candidate(scene, initial_state(), "first private source")
                        .expect("first initial fixture"),
                ),
                "first surface",
            );
            let _ = service_ok(
                first.submit_revision(
                    &first_control,
                    first.presented_revision(),
                    fixture_candidate(scene, successor, "first private source")
                        .expect("first successor fixture"),
                ),
                "first submit",
            );
            let first_lease = first.acquire_render(&first_endpoint).expect("first lease");

            let (mut foreign, foreign_control, _foreign_presentation, foreign_endpoint) =
                service_ok(
                    desktop_surface::new_presented(
                        surface_id(52),
                        renderer_id(),
                        fixture_candidate(scene, initial_state(), "foreign private source")
                            .expect("foreign initial fixture"),
                    ),
                    "foreign surface",
                );
            let _ = service_ok(
                foreign.submit_revision(
                    &foreign_control,
                    foreign.presented_revision(),
                    fixture_candidate(scene, successor, "foreign private source")
                        .expect("foreign successor fixture"),
                ),
                "foreign submit",
            );
            let foreign_lease = foreign
                .acquire_render(&foreign_endpoint)
                .expect("foreign lease");
            {
                let source = first.render_candidate(&first_lease).expect("first view");
                assert!(
                    try_encode_service_snapshot(&source, &foreign_lease).err()
                        == Some(transport_error::binding_mismatch)
                );
            }

            let (mut later, later_control, later_presentation, later_endpoint) = service_ok(
                desktop_surface::new_presented(
                    surface_id(51),
                    renderer_id(),
                    fixture_candidate(scene, initial_state(), "later private source")
                        .expect("later initial fixture"),
                ),
                "later surface",
            );
            let _ = service_ok(
                later.submit_revision(
                    &later_control,
                    later.presented_revision(),
                    fixture_candidate(scene, successor, "later private source")
                        .expect("later revision two fixture"),
                ),
                "later revision two submit",
            );
            let later_lease = later
                .acquire_render(&later_endpoint)
                .expect("later revision two lease");
            let ready = later
                .complete_render(later_lease)
                .unwrap_or_else(|_| panic!("complete later revision two"));
            let receipt = service_ok(
                later.attest_presented(&later_presentation, ready),
                "attest later revision two",
            );
            service_ok(
                later.publish(&later_control, receipt),
                "publish later revision two",
            );
            let _ = service_ok(
                later.submit_revision(
                    &later_control,
                    later.presented_revision(),
                    fixture_candidate(scene, successor, "later private source")
                        .expect("later revision three fixture"),
                ),
                "later revision three submit",
            );
            let later_lease = later
                .acquire_render(&later_endpoint)
                .expect("later revision three lease");
            {
                let source = first.render_candidate(&first_lease).expect("first view");
                assert!(source.surface() == later_lease.surface());
                assert!(source.revision() != later_lease.revision());
                assert!(
                    try_encode_service_snapshot(&source, &later_lease).err()
                        == Some(transport_error::binding_mismatch)
                );
            }

            assert!(
                service_ok(first.retire(first_control), "retire first")
                    .discarded_pending_revision()
            );
            assert!(
                service_ok(foreign.retire(foreign_control), "retire foreign")
                    .discarded_pending_revision()
            );
            assert!(
                service_ok(later.retire(later_control), "retire later")
                    .discarded_pending_revision()
            );
        }
    }
}

#[cfg(feature = "service_adapter")]
pub use service_adapter::{
    retained_host_transaction_fixture, retained_host_transaction_success, service_fixture_error,
    try_encode_fixed_protected_fixture, try_encode_service_snapshot,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(value: Result<T, transport_error>) -> T {
        value.unwrap_or_else(|error| panic!("{}", error.code()))
    }

    fn identity(slot: u64, generation: u64) -> transport_identity {
        transport_identity::try_new(slot, generation).unwrap_or_else(|| panic!("identity"))
    }

    fn binding() -> surface_snapshot_binding {
        ok(surface_snapshot_binding::try_new(
            identity(0x0102_0304_0506_0708, 0x1112_1314_1516_1718),
            identity(0x2122_2324_2526_2728, 0x3132_3334_3536_3738),
            0x4142_4344_4546_4748,
            0x4142_4344_4546_4749,
            identity(0x5152_5354_5556_5758, 0x6162_6364_6566_6768),
            1,
        ))
    }

    fn viewport() -> transport_rect {
        ok(transport_rect::try_new(0, 0, 1_024, 768))
    }

    fn state(
        primary: bool,
        secondary: bool,
        focus: transport_focus,
        disclosure: transport_disclosure,
        depth: transport_depth,
        motion: transport_motion,
    ) -> transport_scene_state {
        ok(transport_scene_state::try_new(
            primary, secondary, focus, disclosure, depth, motion,
        ))
    }

    fn labels(disclosure: transport_disclosure) -> [transport_label_selector; 8] {
        [
            transport_label_selector::anomalous_desktop,
            transport_label_selector::anomalous,
            transport_label_selector::navigation,
            transport_label_selector::open_shell,
            transport_label_selector::inspect_system,
            transport_label_selector::workspace,
            transport_label_selector::system_workspace,
            match disclosure {
                transport_disclosure::declared_public => transport_label_selector::system_ready,
                transport_disclosure::protected => transport_label_selector::protected_status,
            },
        ]
    }

    fn command() -> transport_render_command {
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

    fn snapshot(state: transport_scene_state) -> surface_snapshot_v1 {
        let commands = [command(); 25];
        ok(surface_snapshot_v1::try_encode(
            binding(),
            state,
            viewport(),
            labels(state.disclosure()),
            &commands[..command_count_for_depth(state.depth())],
        ))
    }

    fn basic(depth: transport_depth) -> transport_scene_state {
        state(
            false,
            false,
            transport_focus::none,
            transport_disclosure::declared_public,
            depth,
            transport_motion::standard,
        )
    }

    fn expect_u8_rejection(
        valid: &[u8; surface_snapshot_v1_byte_count],
        offset: usize,
        value: u8,
        expected: transport_error,
    ) {
        let mut malformed = *valid;
        malformed[offset] = value;
        assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(expected));
    }

    fn expect_u16_rejection(
        valid: &[u8; surface_snapshot_v1_byte_count],
        offset: usize,
        value: u16,
        expected: transport_error,
    ) {
        let mut malformed = *valid;
        write_u16(&mut malformed, offset, value);
        assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(expected));
    }

    fn expect_u32_rejection(
        valid: &[u8; surface_snapshot_v1_byte_count],
        offset: usize,
        value: u32,
        expected: transport_error,
    ) {
        let mut malformed = *valid;
        write_u32(&mut malformed, offset, value);
        assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(expected));
    }

    fn expect_u64_rejection(
        valid: &[u8; surface_snapshot_v1_byte_count],
        offset: usize,
        value: u64,
        expected: transport_error,
    ) {
        let mut malformed = *valid;
        write_u64(&mut malformed, offset, value);
        assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(expected));
    }

    #[test]
    fn exact_layout_and_little_endian_fields_round_trip() {
        let encoded = snapshot(basic(transport_depth::standard));
        let bytes = encoded.as_bytes();
        assert_eq!(&bytes[0..2], &112_u16.to_le_bytes());
        assert_eq!(&bytes[2..4], &1_u16.to_le_bytes());
        assert_eq!(&bytes[4..8], &528_u32.to_le_bytes());
        assert_eq!(&bytes[8..12], &[0; 4]);
        assert_eq!(&bytes[12..14], &1_u16.to_le_bytes());
        assert_eq!(&bytes[14..16], &25_u16.to_le_bytes());
        assert_eq!(&bytes[16..18], &8_u16.to_le_bytes());
        assert_eq!(&bytes[18..20], &16_u16.to_le_bytes());
        assert_eq!(&bytes[20..22], &1_024_u16.to_le_bytes());
        assert_eq!(&bytes[22..24], &768_u16.to_le_bytes());
        assert_eq!(&bytes[24..30], &[0; 6]);
        assert_eq!(&bytes[30..32], &[0; 2]);
        assert_eq!(&bytes[32..40], &0x0102_0304_0506_0708_u64.to_le_bytes());
        assert_eq!(&bytes[40..48], &0x1112_1314_1516_1718_u64.to_le_bytes());
        assert_eq!(&bytes[48..56], &0x2122_2324_2526_2728_u64.to_le_bytes());
        assert_eq!(&bytes[56..64], &0x3132_3334_3536_3738_u64.to_le_bytes());
        assert_eq!(&bytes[64..72], &0x4142_4344_4546_4748_u64.to_le_bytes());
        assert_eq!(&bytes[72..80], &0x4142_4344_4546_4749_u64.to_le_bytes());
        assert_eq!(&bytes[80..88], &0x5152_5354_5556_5758_u64.to_le_bytes());
        assert_eq!(&bytes[88..96], &0x6162_6364_6566_6768_u64.to_le_bytes());
        assert_eq!(&bytes[96..104], &1_u64.to_le_bytes());
        assert_eq!(&bytes[104..112], &[0; 8]);
        assert_eq!(&bytes[112..120], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&bytes[120..128], &[0; 8]);
        assert_eq!(&bytes[128..136], &[1, 1, 1, 0, 1, 1, 0, 0]);
        assert_eq!(&bytes[136..144], &[0, 0, 0, 0, 0, 4, 0, 3]);
        let decoded = ok(surface_snapshot_v1::try_from_slice(bytes));
        assert_eq!(decoded.as_bytes(), bytes);
        assert!(decoded.binding() == binding());
        assert!(decoded.state() == basic(transport_depth::standard));
        assert!(decoded.command(0) == Some(command()));
        assert!(decoded.command(25).is_none());
    }

    #[test]
    fn all_sixty_four_valid_and_thirty_two_invalid_states_are_exhaustive() {
        let focuses = [
            transport_focus::none,
            transport_focus::primary,
            transport_focus::secondary,
        ];
        let disclosures = [
            transport_disclosure::declared_public,
            transport_disclosure::protected,
        ];
        let depths = [transport_depth::standard, transport_depth::flat];
        let motions = [transport_motion::standard, transport_motion::reduced];
        let mut valid = 0;
        let mut invalid = 0;
        for primary in [false, true] {
            for secondary in [false, true] {
                for focus in focuses {
                    for disclosure in disclosures {
                        for depth in depths {
                            for motion in motions {
                                match transport_scene_state::try_new(
                                    primary, secondary, focus, disclosure, depth, motion,
                                ) {
                                    Ok(state) => {
                                        valid += 1;
                                        let decoded = ok(surface_snapshot_v1::try_from_bytes(
                                            snapshot(state).into_bytes(),
                                        ));
                                        assert!(decoded.state() == state);
                                        assert_eq!(
                                            decoded.command_count(),
                                            command_count_for_depth(depth)
                                        );
                                    }
                                    Err(error) => {
                                        invalid += 1;
                                        assert!(error == transport_error::invalid_state);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(valid, 64);
        assert_eq!(invalid, 32);
    }

    #[test]
    fn wire_enum_domains_are_exact_and_present_tags_are_nonzero() {
        assert_eq!(
            [
                transport_focus::none.raw(),
                transport_focus::primary.raw(),
                transport_focus::secondary.raw(),
            ],
            [0, 1, 2]
        );
        assert_eq!(
            [
                transport_disclosure::declared_public.raw(),
                transport_disclosure::protected.raw(),
            ],
            [0, 1]
        );
        assert_eq!(
            [transport_depth::standard.raw(), transport_depth::flat.raw(),],
            [0, 1]
        );
        assert_eq!(
            [
                transport_motion::standard.raw(),
                transport_motion::reduced.raw(),
            ],
            [0, 1]
        );
        assert_eq!(
            [
                transport_command_kind::fill.raw(),
                transport_command_kind::outline.raw(),
                transport_command_kind::label.raw(),
                transport_command_kind::enabled_marker.raw(),
                transport_command_kind::focus_outline.raw(),
                transport_command_kind::shadow.raw(),
                transport_command_kind::highlight.raw(),
            ],
            [1, 2, 3, 4, 5, 6, 7]
        );
        assert_eq!(
            [
                transport_shape::rectangle.raw(),
                transport_shape::text.raw(),
                transport_shape::enabled.raw(),
                transport_shape::disabled.raw(),
                transport_shape::focus_absent.raw(),
                transport_shape::focus_double_outline.raw(),
            ],
            [1, 2, 3, 4, 5, 6]
        );
        assert_eq!(
            [
                transport_color::canvas.raw(),
                transport_color::surface.raw(),
                transport_color::primary_fill.raw(),
                transport_color::secondary_fill.raw(),
                transport_color::disabled.raw(),
                transport_color::primary_text.raw(),
                transport_color::secondary_text.raw(),
                transport_color::on_accent.raw(),
                transport_color::outline.raw(),
                transport_color::focus_outer.raw(),
                transport_color::focus_inner.raw(),
                transport_color::public_status.raw(),
                transport_color::protected_status.raw(),
                transport_color::shadow.raw(),
                transport_color::highlight.raw(),
            ],
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        assert_eq!(
            [
                transport_slot::root.raw(),
                transport_slot::masthead.raw(),
                transport_slot::navigation.raw(),
                transport_slot::primary_action.raw(),
                transport_slot::secondary_action.raw(),
                transport_slot::content.raw(),
                transport_slot::heading.raw(),
                transport_slot::status.raw(),
            ],
            [1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(
            [
                transport_label_selector::anomalous_desktop.raw(),
                transport_label_selector::anomalous.raw(),
                transport_label_selector::navigation.raw(),
                transport_label_selector::open_shell.raw(),
                transport_label_selector::inspect_system.raw(),
                transport_label_selector::workspace.raw(),
                transport_label_selector::system_workspace.raw(),
                transport_label_selector::system_ready.raw(),
                transport_label_selector::protected_status.raw(),
            ],
            [1, 2, 3, 4, 5, 6, 7, 8, 9]
        );
        let mut counts = [0_usize; 9];
        for raw in 0..=u8::MAX {
            counts[0] += usize::from(transport_focus::from_raw(raw).is_some());
            counts[1] += usize::from(transport_disclosure::from_raw(raw).is_some());
            counts[2] += usize::from(transport_depth::from_raw(raw).is_some());
            counts[3] += usize::from(transport_motion::from_raw(raw).is_some());
            counts[4] += usize::from(transport_command_kind::from_raw(raw).is_some());
            counts[5] += usize::from(transport_shape::from_raw(raw).is_some());
            counts[6] += usize::from(transport_color::from_raw(raw).is_some());
            counts[7] += usize::from(transport_slot::from_raw(raw).is_some());
            counts[8] += usize::from(transport_label_selector::from_raw(raw).is_some());
        }
        assert_eq!(counts, [3, 2, 2, 2, 7, 6, 15, 8, 9]);
        assert!(transport_command_kind::from_raw(0).is_none());
        assert!(transport_shape::from_raw(0).is_none());
        assert!(transport_color::from_raw(0).is_none());
        assert!(transport_slot::from_raw(0).is_none());
        assert!(transport_label_selector::from_raw(0).is_none());
    }

    #[test]
    fn every_present_command_tag_and_secondary_absence_round_trips() {
        let kinds = [
            transport_command_kind::fill,
            transport_command_kind::outline,
            transport_command_kind::label,
            transport_command_kind::enabled_marker,
            transport_command_kind::focus_outline,
            transport_command_kind::shadow,
            transport_command_kind::highlight,
        ];
        let shapes = [
            transport_shape::rectangle,
            transport_shape::text,
            transport_shape::enabled,
            transport_shape::disabled,
            transport_shape::focus_absent,
            transport_shape::focus_double_outline,
        ];
        let colors = [
            transport_color::canvas,
            transport_color::surface,
            transport_color::primary_fill,
            transport_color::secondary_fill,
            transport_color::disabled,
            transport_color::primary_text,
            transport_color::secondary_text,
            transport_color::on_accent,
            transport_color::outline,
            transport_color::focus_outer,
            transport_color::focus_inner,
            transport_color::public_status,
            transport_color::protected_status,
            transport_color::shadow,
            transport_color::highlight,
        ];
        let slots = [
            transport_slot::root,
            transport_slot::masthead,
            transport_slot::navigation,
            transport_slot::primary_action,
            transport_slot::secondary_action,
            transport_slot::content,
            transport_slot::heading,
            transport_slot::status,
        ];
        for index in 0..=colors.len() {
            let expected = transport_render_command::new(
                kinds[index % kinds.len()],
                shapes[index % shapes.len()],
                colors[index % colors.len()],
                if index == 0 {
                    None
                } else {
                    Some(colors[index - 1])
                },
                slots[index % slots.len()],
                index.is_multiple_of(2),
                ok(transport_rect::try_new(index as u16, index as u16, 1, 1)),
            );
            let mut commands = [command(); 25];
            commands[0] = expected;
            let encoded = ok(surface_snapshot_v1::try_encode(
                binding(),
                basic(transport_depth::standard),
                viewport(),
                labels(transport_disclosure::declared_public),
                &commands,
            ));
            assert!(encoded.command(0) == Some(expected));
        }
    }

    #[test]
    fn structural_body_does_not_import_consumer_scene_policy() {
        let valid = snapshot(basic(transport_depth::standard)).into_bytes();

        let mut disabled_focus = valid;
        disabled_focus[focus_offset] = transport_focus::primary.raw();
        let body = surface_snapshot_body_v1::try_from_bytes(disabled_focus)
            .unwrap_or_else(|error| panic!("{}", error.code()));
        assert!(body.binding() == binding());
        assert!(body.try_into_snapshot().err() == Some(transport_error::invalid_state));

        let mut noncanonical_labels = valid;
        noncanonical_labels[labels_offset] = transport_label_selector::workspace.raw();
        let body = surface_snapshot_body_v1::try_from_bytes(noncanonical_labels)
            .unwrap_or_else(|error| panic!("{}", error.code()));
        assert!(body.try_into_snapshot().err() == Some(transport_error::label_profile_mismatch));

        let body = snapshot(basic(transport_depth::standard)).into_body();
        assert_eq!(body.as_bytes().len(), surface_snapshot_v1_byte_count);
        assert!(body.binding() == binding());
    }

    #[test]
    fn every_reserved_byte_and_every_flat_tail_byte_rejects() {
        let standard = snapshot(basic(transport_depth::standard)).into_bytes();
        for offset in (30..32).chain(104..112).chain(120..128) {
            let mut malformed = standard;
            malformed[offset] = 1;
            assert!(
                surface_snapshot_v1::try_from_bytes(malformed).err()
                    == Some(transport_error::nonzero_reserved)
            );
        }
        for index in 0..25 {
            for field in 6..8 {
                let mut malformed = standard;
                malformed[128 + index * 16 + field] = 1;
                assert!(
                    surface_snapshot_v1::try_from_bytes(malformed).err()
                        == Some(transport_error::nonzero_reserved)
                );
            }
        }
        let flat = snapshot(basic(transport_depth::flat)).into_bytes();
        for offset in (128 + 21 * 16)..528 {
            let mut malformed = flat;
            malformed[offset] = 1;
            assert!(
                surface_snapshot_v1::try_from_bytes(malformed).err()
                    == Some(transport_error::nonzero_trailing_command)
            );
        }
    }

    #[test]
    fn malformed_headers_state_binding_labels_commands_and_geometry_reject() {
        let valid = snapshot(basic(transport_depth::standard)).into_bytes();
        let mut cases = [
            (0, transport_error::wrong_header_size),
            (2, transport_error::wrong_version),
            (4, transport_error::wrong_total_size),
            (12, transport_error::wrong_profile),
            (14, transport_error::wrong_command_count),
            (16, transport_error::wrong_label_count),
            (18, transport_error::wrong_command_size),
            (20, transport_error::wrong_viewport),
            (22, transport_error::wrong_viewport),
            (32, transport_error::invalid_binding),
            (40, transport_error::invalid_binding),
            (48, transport_error::invalid_binding),
            (56, transport_error::invalid_binding),
            (64, transport_error::invalid_revision),
            (80, transport_error::invalid_binding),
            (88, transport_error::invalid_binding),
            (96, transport_error::invalid_attempt),
        ];
        for (offset, expected) in cases.iter_mut() {
            let mut malformed = valid;
            let width = if *offset == 4 {
                4
            } else if *offset >= 32 {
                8
            } else {
                2
            };
            malformed[*offset..*offset + width].fill(0);
            assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(*expected));
        }
        let mut flags = valid;
        flags[8] = 1;
        assert!(
            surface_snapshot_v1::try_from_bytes(flags).err()
                == Some(transport_error::unsupported_flags)
        );
        for offset in [24, 25] {
            let mut malformed = valid;
            malformed[offset] = 2;
            assert!(
                surface_snapshot_v1::try_from_bytes(malformed).err()
                    == Some(transport_error::invalid_state_flag)
            );
        }
        for offset in [26, 27, 28, 29] {
            let mut malformed = valid;
            malformed[offset] = u8::MAX;
            assert!(
                surface_snapshot_v1::try_from_bytes(malformed).err()
                    == Some(transport_error::invalid_state)
            );
        }
        let command_cases = [
            (128, 0, transport_error::invalid_command_kind),
            (129, 0, transport_error::invalid_shape),
            (130, 0, transport_error::invalid_color),
            (131, 16, transport_error::invalid_secondary_color),
            (132, 0, transport_error::invalid_slot),
            (133, 2, transport_error::invalid_state_flag),
        ];
        for (offset, value, expected) in command_cases {
            let mut malformed = valid;
            malformed[offset] = value;
            assert!(surface_snapshot_v1::try_from_bytes(malformed).err() == Some(expected));
        }
        let mut selector = valid;
        selector[112] = 0;
        assert!(
            surface_snapshot_v1::try_from_bytes(selector).err()
                == Some(transport_error::invalid_label_selector)
        );
        let mut profile = valid;
        profile[112] = 6;
        assert!(
            surface_snapshot_v1::try_from_bytes(profile).err()
                == Some(transport_error::label_profile_mismatch)
        );
        let mut zero_width = valid;
        write_u16(&mut zero_width, 140, 0);
        assert!(
            surface_snapshot_v1::try_from_bytes(zero_width).err()
                == Some(transport_error::invalid_geometry)
        );
        let mut overflow = valid;
        write_u16(&mut overflow, 136, u16::MAX);
        assert!(
            surface_snapshot_v1::try_from_bytes(overflow).err()
                == Some(transport_error::geometry_overflow)
        );
        let mut outside = valid;
        write_u16(&mut outside, 136, 1_024);
        write_u16(&mut outside, 140, 1);
        assert!(
            surface_snapshot_v1::try_from_bytes(outside).err()
                == Some(transport_error::outside_viewport)
        );
    }

    #[test]
    fn every_header_and_record_field_class_covers_boundary_mutations() {
        let standard = snapshot(basic(transport_depth::standard)).into_bytes();
        for value in [0, 111, 113, u16::MAX] {
            expect_u16_rejection(
                &standard,
                header_size_offset,
                value,
                transport_error::wrong_header_size,
            );
        }
        for value in [0, 2, u16::MAX] {
            expect_u16_rejection(
                &standard,
                version_offset,
                value,
                transport_error::wrong_version,
            );
        }
        for value in [0, 527, 529, u32::MAX] {
            expect_u32_rejection(
                &standard,
                total_size_offset,
                value,
                transport_error::wrong_total_size,
            );
        }
        for value in [1, u32::MAX] {
            expect_u32_rejection(
                &standard,
                flags_offset,
                value,
                transport_error::unsupported_flags,
            );
        }
        for value in [0, 2, u16::MAX] {
            expect_u16_rejection(
                &standard,
                profile_offset,
                value,
                transport_error::wrong_profile,
            );
        }
        for value in [0, 21, 24, 26, u16::MAX] {
            expect_u16_rejection(
                &standard,
                command_count_offset,
                value,
                transport_error::wrong_command_count,
            );
        }
        let flat = snapshot(basic(transport_depth::flat)).into_bytes();
        expect_u16_rejection(
            &flat,
            command_count_offset,
            25,
            transport_error::wrong_command_count,
        );
        for value in [0, 7, 9, u16::MAX] {
            expect_u16_rejection(
                &standard,
                label_count_offset,
                value,
                transport_error::wrong_label_count,
            );
        }
        for value in [0, 15, 17, u16::MAX] {
            expect_u16_rejection(
                &standard,
                command_size_offset,
                value,
                transport_error::wrong_command_size,
            );
        }
        for value in [0, 1_023, 1_025, u16::MAX] {
            expect_u16_rejection(
                &standard,
                viewport_width_offset,
                value,
                transport_error::wrong_viewport,
            );
        }
        for value in [0, 767, 769, u16::MAX] {
            expect_u16_rejection(
                &standard,
                viewport_height_offset,
                value,
                transport_error::wrong_viewport,
            );
        }

        for offset in [primary_enabled_offset, secondary_enabled_offset] {
            for value in [2, u8::MAX] {
                expect_u8_rejection(
                    &standard,
                    offset,
                    value,
                    transport_error::invalid_state_flag,
                );
            }
        }
        for (offset, values) in [
            (focus_offset, [3, u8::MAX]),
            (disclosure_offset, [2, u8::MAX]),
            (depth_offset, [2, u8::MAX]),
            (motion_offset, [2, u8::MAX]),
        ] {
            for value in values {
                expect_u8_rejection(&standard, offset, value, transport_error::invalid_state);
            }
        }

        for offset in [
            surface_slot_offset,
            surface_generation_offset,
            scene_slot_offset,
            scene_generation_offset,
            renderer_slot_offset,
            renderer_generation_offset,
        ] {
            expect_u64_rejection(&standard, offset, 0, transport_error::invalid_binding);
            let mut maximum = standard;
            write_u64(&mut maximum, offset, u64::MAX);
            assert!(surface_snapshot_v1::try_from_bytes(maximum).is_ok());
        }
        for (base, revision) in [
            (0, 1),
            (1, 0),
            (1, 1),
            (1, 3),
            (u64::MAX, 1),
            (u64::MAX, u64::MAX),
        ] {
            let mut malformed = standard;
            write_u64(&mut malformed, base_revision_offset, base);
            write_u64(&mut malformed, revision_offset, revision);
            assert!(
                surface_snapshot_v1::try_from_bytes(malformed).err()
                    == Some(transport_error::invalid_revision)
            );
        }
        let mut maximum_revision = standard;
        write_u64(&mut maximum_revision, base_revision_offset, u64::MAX - 1);
        write_u64(&mut maximum_revision, revision_offset, u64::MAX);
        assert!(surface_snapshot_v1::try_from_bytes(maximum_revision).is_ok());
        for value in [0, 3, u64::MAX] {
            expect_u64_rejection(
                &standard,
                attempt_offset,
                value,
                transport_error::invalid_attempt,
            );
        }
        let mut second_attempt = standard;
        write_u64(&mut second_attempt, attempt_offset, 2);
        assert!(surface_snapshot_v1::try_from_bytes(second_attempt).is_ok());

        for offset in labels_offset..label_reserved_offset {
            for value in [0, 10, u8::MAX] {
                expect_u8_rejection(
                    &standard,
                    offset,
                    value,
                    transport_error::invalid_label_selector,
                );
            }
        }
        for (index, expected) in [1_u8, 2, 3, 4, 5, 6, 7].into_iter().enumerate() {
            let replacement = if expected == 1 { 2 } else { 1 };
            expect_u8_rejection(
                &standard,
                labels_offset + index,
                replacement,
                transport_error::label_profile_mismatch,
            );
        }
        expect_u8_rejection(
            &standard,
            labels_offset + 7,
            transport_label_selector::protected_status.raw(),
            transport_error::label_profile_mismatch,
        );
        let protected = snapshot(state(
            false,
            false,
            transport_focus::none,
            transport_disclosure::protected,
            transport_depth::standard,
            transport_motion::standard,
        ))
        .into_bytes();
        expect_u8_rejection(
            &protected,
            labels_offset + 7,
            transport_label_selector::system_ready.raw(),
            transport_error::label_profile_mismatch,
        );

        for (offset, values, expected) in [
            (
                surface_snapshot_v1_command_area_offset,
                [0, 8, u8::MAX],
                transport_error::invalid_command_kind,
            ),
            (
                surface_snapshot_v1_command_area_offset + 1,
                [0, 7, u8::MAX],
                transport_error::invalid_shape,
            ),
            (
                surface_snapshot_v1_command_area_offset + 2,
                [0, 16, u8::MAX],
                transport_error::invalid_color,
            ),
            (
                surface_snapshot_v1_command_area_offset + 3,
                [16, 17, u8::MAX],
                transport_error::invalid_secondary_color,
            ),
            (
                surface_snapshot_v1_command_area_offset + 4,
                [0, 9, u8::MAX],
                transport_error::invalid_slot,
            ),
            (
                surface_snapshot_v1_command_area_offset + 5,
                [2, 3, u8::MAX],
                transport_error::invalid_state_flag,
            ),
        ] {
            for value in values {
                expect_u8_rejection(&standard, offset, value, expected);
            }
        }

        for offset in [
            surface_snapshot_v1_command_area_offset + 12,
            surface_snapshot_v1_command_area_offset + 14,
        ] {
            expect_u16_rejection(&standard, offset, 0, transport_error::invalid_geometry);
        }
        for (coordinate, extent) in [(136, 140), (138, 142)] {
            let mut overflow = standard;
            write_u16(&mut overflow, coordinate, u16::MAX);
            write_u16(&mut overflow, extent, 2);
            assert!(
                surface_snapshot_v1::try_from_bytes(overflow).err()
                    == Some(transport_error::geometry_overflow)
            );
        }
        for (coordinate, extent, edge) in [(136, 140, 1_024), (138, 142, 768)] {
            let mut outside = standard;
            write_u16(&mut outside, coordinate, edge);
            write_u16(&mut outside, extent, 1);
            assert!(
                surface_snapshot_v1::try_from_bytes(outside).err()
                    == Some(transport_error::outside_viewport)
            );
        }
        let mut edge = standard;
        write_u16(&mut edge, 136, 1_023);
        write_u16(&mut edge, 138, 767);
        write_u16(&mut edge, 140, 1);
        write_u16(&mut edge, 142, 1);
        assert!(surface_snapshot_v1::try_from_bytes(edge).is_ok());
    }

    #[test]
    fn constructors_revision_wrap_and_exact_slice_length_reject() {
        assert!(transport_identity::try_new(0, 1).is_none());
        assert!(transport_identity::try_new(1, 0).is_none());
        assert!(
            surface_snapshot_binding::try_new(
                identity(1, 1),
                identity(2, 1),
                u64::MAX,
                1,
                identity(3, 1),
                1,
            )
            .err()
                == Some(transport_error::invalid_revision)
        );
        assert!(
            transport_rect::try_new(0, 0, 0, 1).err() == Some(transport_error::invalid_geometry)
        );
        assert!(
            transport_rect::try_new(u16::MAX, 0, 1, 1).err()
                == Some(transport_error::geometry_overflow)
        );
        let bytes = snapshot(basic(transport_depth::standard)).into_bytes();
        assert!(
            surface_snapshot_v1::try_from_slice(&bytes[..527]).err()
                == Some(transport_error::wrong_length)
        );
        let mut oversized = [0; 529];
        oversized[..528].copy_from_slice(&bytes);
        assert!(
            surface_snapshot_v1::try_from_slice(&oversized).err()
                == Some(transport_error::wrong_length)
        );
    }

    #[cfg(feature = "service_adapter")]
    #[test]
    fn fixed_service_fixture_is_canonical_and_protected_source_independent() {
        let first = try_encode_fixed_protected_fixture("short private value")
            .unwrap_or_else(|error| panic!("{}", error.code()));
        let second = try_encode_fixed_protected_fixture(
            "a different and substantially longer private fixture value",
        )
        .unwrap_or_else(|error| panic!("{}", error.code()));
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert!(first.binding().surface() == identity(51, 3));
        assert!(first.binding().scene() == identity(41, 7));
        assert_eq!(first.binding().base_revision(), 1);
        assert_eq!(first.binding().revision(), 2);
        assert!(first.binding().renderer() == identity(61, 1));
        assert_eq!(first.binding().attempt(), 1);
        assert!(first.state().primary_enabled());
        assert!(!first.state().secondary_enabled());
        assert!(first.state().focus() == transport_focus::primary);
        assert!(first.state().disclosure() == transport_disclosure::protected);
        assert!(first.state().depth() == transport_depth::standard);
        assert!(first.state().motion() == transport_motion::standard);
        assert!(first.label(7) == Some(transport_label_selector::protected_status));
    }

    #[cfg(feature = "service_adapter")]
    #[test]
    fn adapter_maps_all_sixty_four_valid_service_states() {
        let mut valid = 0;
        let mut invalid = 0;
        for ordinal in 0..96_u8 {
            let packed = ordinal / 8;
            let focus = packed % 3;
            let enabled = packed / 3;
            let primary = enabled & 2 != 0;
            let secondary = enabled & 1 != 0;
            let is_valid = (focus != 1 || primary) && (focus != 2 || secondary);
            let result = service_adapter::try_encode_fixture_ordinal(
                ordinal,
                "private adapter matrix fixture",
            );
            if is_valid {
                valid += 1;
                let encoded = result.unwrap_or_else(|error| panic!("{}", error.code()));
                assert_eq!(encoded.state().primary_enabled(), primary);
                assert_eq!(encoded.state().secondary_enabled(), secondary);
                assert_eq!(encoded.state().focus().raw(), focus);
                assert_eq!(encoded.state().disclosure().raw(), (ordinal / 4) % 2);
                assert_eq!(encoded.state().depth().raw(), (ordinal / 2) % 2);
                assert_eq!(encoded.state().motion().raw(), ordinal % 2);
                assert_eq!(
                    encoded.command_count(),
                    command_count_for_depth(encoded.state().depth())
                );
            } else {
                invalid += 1;
                assert!(result.err() == Some(service_fixture_error::invalid_fixture));
            }
        }
        assert_eq!(valid, 64);
        assert_eq!(invalid, 32);
    }
}
