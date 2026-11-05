use crate::font::label_has_supported_glyphs;
use crate::raster::{frame_height, frame_width};
use crate::renderer_error;

pub const fixture_command_count: usize = 25;
pub const fixture_state_ordinal: u8 = 61;

const capsule_magic: [u8; 4] = *b"ANR1";
const capsule_header_byte_count: usize = 16;
const command_byte_count: usize = 16;
const fixture_label_count: u8 = 6;
const no_secondary_color: u8 = 0xff;
const no_label: u8 = 0xff;

pub const capsule_byte_count: usize =
    capsule_header_byte_count + fixture_command_count * command_byte_count;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum capsule_command_kind {
    fill = 0,
    outline = 1,
    label = 2,
    enabled_marker = 3,
    focus_outline = 4,
    shadow = 5,
    highlight = 6,
}

impl capsule_command_kind {
    fn try_from_byte(value: u8) -> Result<Self, renderer_error> {
        match value {
            0 => Ok(Self::fill),
            1 => Ok(Self::outline),
            2 => Ok(Self::label),
            3 => Ok(Self::enabled_marker),
            4 => Ok(Self::focus_outline),
            5 => Ok(Self::shadow),
            6 => Ok(Self::highlight),
            _ => Err(renderer_error::invalid_command_kind),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum capsule_shape {
    rectangle = 0,
    text = 1,
    enabled = 2,
    disabled = 3,
    focus_absent = 4,
    focus_double_outline = 5,
}

impl capsule_shape {
    fn try_from_byte(value: u8) -> Result<Self, renderer_error> {
        match value {
            0 => Ok(Self::rectangle),
            1 => Ok(Self::text),
            2 => Ok(Self::enabled),
            3 => Ok(Self::disabled),
            4 => Ok(Self::focus_absent),
            5 => Ok(Self::focus_double_outline),
            _ => Err(renderer_error::invalid_shape),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum capsule_color {
    canvas = 0,
    surface = 1,
    primary_fill = 2,
    secondary_fill = 3,
    disabled = 4,
    primary_text = 5,
    secondary_text = 6,
    on_accent = 7,
    outline = 8,
    focus_outer = 9,
    focus_inner = 10,
    public_status = 11,
    protected_status = 12,
    shadow = 13,
    highlight = 14,
}

impl capsule_color {
    fn try_from_byte(value: u8) -> Result<Self, renderer_error> {
        match value {
            0 => Ok(Self::canvas),
            1 => Ok(Self::surface),
            2 => Ok(Self::primary_fill),
            3 => Ok(Self::secondary_fill),
            4 => Ok(Self::disabled),
            5 => Ok(Self::primary_text),
            6 => Ok(Self::secondary_text),
            7 => Ok(Self::on_accent),
            8 => Ok(Self::outline),
            9 => Ok(Self::focus_outer),
            10 => Ok(Self::focus_inner),
            11 => Ok(Self::public_status),
            12 => Ok(Self::protected_status),
            13 => Ok(Self::shadow),
            14 => Ok(Self::highlight),
            _ => Err(renderer_error::invalid_color),
        }
    }

    #[must_use]
    pub(crate) const fn rgb(self) -> [u8; 3] {
        match self {
            Self::canvas => [0xe7, 0xec, 0xf3],
            Self::surface => [0xf5, 0xf7, 0xfb],
            Self::primary_fill => [0x1e, 0x40, 0xaf],
            Self::secondary_fill => [0xe2, 0xe8, 0xf0],
            Self::disabled => [0xcb, 0xd5, 0xe1],
            Self::primary_text => [0x17, 0x20, 0x33],
            Self::secondary_text => [0x40, 0x51, 0x68],
            Self::on_accent => [0xff, 0xff, 0xff],
            Self::outline => [0x52, 0x65, 0x7a],
            Self::focus_outer => [0x11, 0x18, 0x27],
            Self::focus_inner => [0xff, 0xff, 0xff],
            Self::public_status => [0x14, 0x53, 0x2d],
            Self::protected_status => [0x6b, 0x21, 0xa8],
            Self::shadow => [0xa9, 0xb4, 0xc4],
            Self::highlight => [0xff, 0xff, 0xff],
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum capsule_slot {
    root = 0,
    masthead = 1,
    navigation = 2,
    primary_action = 3,
    secondary_action = 4,
    content = 5,
    heading = 6,
    status = 7,
}

impl capsule_slot {
    fn try_from_byte(value: u8) -> Result<Self, renderer_error> {
        match value {
            0 => Ok(Self::root),
            1 => Ok(Self::masthead),
            2 => Ok(Self::navigation),
            3 => Ok(Self::primary_action),
            4 => Ok(Self::secondary_action),
            5 => Ok(Self::content),
            6 => Ok(Self::heading),
            7 => Ok(Self::status),
            _ => Err(renderer_error::invalid_slot),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct capsule_rect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl capsule_rect {
    pub const fn try_new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, renderer_error> {
        if width == 0 || height == 0 {
            return Err(renderer_error::invalid_geometry);
        }
        let Some(right) = x.checked_add(width) else {
            return Err(renderer_error::geometry_overflow);
        };
        let Some(bottom) = y.checked_add(height) else {
            return Err(renderer_error::geometry_overflow);
        };
        if right > frame_width || bottom > frame_height {
            return Err(renderer_error::outside_viewport);
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

#[derive(Clone, Copy)]
pub struct projection_command<'a> {
    kind: capsule_command_kind,
    shape: capsule_shape,
    color: capsule_color,
    secondary_color: Option<capsule_color>,
    slot: capsule_slot,
    bounds: capsule_rect,
    state_active: bool,
    label: Option<&'a str>,
}

impl<'a> projection_command<'a> {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        kind: capsule_command_kind,
        shape: capsule_shape,
        color: capsule_color,
        secondary_color: Option<capsule_color>,
        slot: capsule_slot,
        bounds: capsule_rect,
        state_active: bool,
        label: Option<&'a str>,
    ) -> Self {
        Self {
            kind,
            shape,
            color,
            secondary_color,
            slot,
            bounds,
            state_active,
            label,
        }
    }
}

#[derive(Clone, Copy)]
struct command_spec {
    kind: capsule_command_kind,
    shape: capsule_shape,
    color: capsule_color,
    secondary_color: Option<capsule_color>,
    slot: capsule_slot,
    label: Option<label_selector>,
    bounds: capsule_rect,
    state_active: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum label_selector {
    masthead = 0,
    navigation = 1,
    primary_action = 2,
    secondary_action = 3,
    heading = 4,
    protected_status = 5,
}

impl label_selector {
    fn try_from_byte(value: u8) -> Result<Self, renderer_error> {
        match value {
            0 => Ok(Self::masthead),
            1 => Ok(Self::navigation),
            2 => Ok(Self::primary_action),
            3 => Ok(Self::secondary_action),
            4 => Ok(Self::heading),
            5 => Ok(Self::protected_status),
            _ => Err(renderer_error::invalid_label_selector),
        }
    }

    #[must_use]
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::masthead => "Anomalous",
            Self::navigation => "Navigation",
            Self::primary_action => "Open shell",
            Self::secondary_action => "Inspect system",
            Self::heading => "System workspace",
            Self::protected_status => "Protected status",
        }
    }
}

const fn rect(x: u32, y: u32, width: u32, height: u32) -> capsule_rect {
    capsule_rect {
        x,
        y,
        width,
        height,
    }
}

const root_bounds: capsule_rect = rect(0, 0, 1024, 768);
const masthead_bounds: capsule_rect = rect(32, 24, 960, 88);
const navigation_bounds: capsule_rect = rect(32, 136, 256, 600);
const primary_bounds: capsule_rect = rect(56, 184, 208, 64);
const secondary_bounds: capsule_rect = rect(56, 272, 208, 64);
const content_bounds: capsule_rect = rect(312, 136, 680, 600);
const heading_bounds: capsule_rect = rect(352, 176, 600, 48);
const status_bounds: capsule_rect = rect(352, 248, 600, 96);

macro_rules! spec {
    ($kind:ident, $shape:ident, $color:ident, $secondary:expr, $slot:ident, $label:expr, $bounds:ident, $active:expr) => {
        command_spec {
            kind: capsule_command_kind::$kind,
            shape: capsule_shape::$shape,
            color: capsule_color::$color,
            secondary_color: $secondary,
            slot: capsule_slot::$slot,
            label: $label,
            bounds: $bounds,
            state_active: $active,
        }
    };
}

const fixture_specs: [command_spec; fixture_command_count] = [
    spec!(fill, rectangle, canvas, None, root, None, root_bounds, true),
    spec!(
        fill,
        rectangle,
        surface,
        None,
        masthead,
        None,
        masthead_bounds,
        true
    ),
    spec!(
        outline,
        rectangle,
        outline,
        None,
        masthead,
        None,
        masthead_bounds,
        true
    ),
    spec!(
        label,
        text,
        primary_text,
        None,
        masthead,
        Some(label_selector::masthead),
        masthead_bounds,
        true
    ),
    spec!(
        shadow,
        rectangle,
        shadow,
        None,
        navigation,
        None,
        navigation_bounds,
        true
    ),
    spec!(
        fill,
        rectangle,
        surface,
        None,
        navigation,
        None,
        navigation_bounds,
        true
    ),
    spec!(
        highlight,
        rectangle,
        highlight,
        None,
        navigation,
        None,
        navigation_bounds,
        true
    ),
    spec!(
        outline,
        rectangle,
        outline,
        None,
        navigation,
        None,
        navigation_bounds,
        true
    ),
    spec!(
        label,
        text,
        primary_text,
        None,
        navigation,
        Some(label_selector::navigation),
        navigation_bounds,
        true
    ),
    spec!(
        fill,
        rectangle,
        primary_fill,
        None,
        primary_action,
        None,
        primary_bounds,
        true
    ),
    spec!(
        outline,
        rectangle,
        outline,
        None,
        primary_action,
        None,
        primary_bounds,
        true
    ),
    spec!(
        label,
        text,
        on_accent,
        None,
        primary_action,
        Some(label_selector::primary_action),
        primary_bounds,
        true
    ),
    spec!(
        enabled_marker,
        enabled,
        on_accent,
        None,
        primary_action,
        None,
        primary_bounds,
        true
    ),
    spec!(
        focus_outline,
        focus_double_outline,
        focus_outer,
        Some(capsule_color::focus_inner),
        primary_action,
        None,
        primary_bounds,
        true
    ),
    spec!(
        fill,
        rectangle,
        disabled,
        None,
        secondary_action,
        None,
        secondary_bounds,
        true
    ),
    spec!(
        outline,
        rectangle,
        outline,
        None,
        secondary_action,
        None,
        secondary_bounds,
        true
    ),
    spec!(
        label,
        text,
        primary_text,
        None,
        secondary_action,
        Some(label_selector::secondary_action),
        secondary_bounds,
        true
    ),
    spec!(
        enabled_marker,
        disabled,
        outline,
        None,
        secondary_action,
        None,
        secondary_bounds,
        false
    ),
    spec!(
        focus_outline,
        focus_absent,
        outline,
        None,
        secondary_action,
        None,
        secondary_bounds,
        false
    ),
    spec!(
        shadow,
        rectangle,
        shadow,
        None,
        content,
        None,
        content_bounds,
        true
    ),
    spec!(
        fill,
        rectangle,
        surface,
        None,
        content,
        None,
        content_bounds,
        true
    ),
    spec!(
        highlight,
        rectangle,
        highlight,
        None,
        content,
        None,
        content_bounds,
        true
    ),
    spec!(
        outline,
        rectangle,
        outline,
        None,
        content,
        None,
        content_bounds,
        true
    ),
    spec!(
        label,
        text,
        primary_text,
        None,
        heading,
        Some(label_selector::heading),
        heading_bounds,
        true
    ),
    spec!(
        label,
        text,
        protected_status,
        None,
        status,
        Some(label_selector::protected_status),
        status_bounds,
        true
    ),
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct render_capsule {
    bytes: [u8; capsule_byte_count],
}

impl render_capsule {
    /// Returns the one accepted feature-private fixture capsule.
    ///
    /// Callers that have a semantic projection should use `try_encode` and
    /// compare the result instead of treating this as scene serialization.
    #[must_use]
    pub const fn fixture() -> Self {
        Self {
            bytes: canonical_capsule_bytes,
        }
    }

    pub fn try_encode(
        viewport: capsule_rect,
        state_ordinal: u8,
        commands: &[projection_command<'_>],
    ) -> Result<Self, renderer_error> {
        if viewport != root_bounds {
            return Err(renderer_error::wrong_viewport);
        }
        if state_ordinal != fixture_state_ordinal {
            return Err(renderer_error::wrong_state_ordinal);
        }
        if commands.len() != fixture_command_count {
            return Err(renderer_error::wrong_command_count);
        }
        for (index, command) in commands.iter().enumerate() {
            validate_projection_command(command, fixture_specs[index])?;
        }
        Ok(Self {
            bytes: canonical_capsule_bytes,
        })
    }

    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, renderer_error> {
        if bytes.len() != capsule_byte_count {
            return Err(renderer_error::wrong_capsule_length);
        }
        let mut owned = [0_u8; capsule_byte_count];
        owned.copy_from_slice(bytes);
        Self::try_from_bytes(owned)
    }

    pub fn try_from_bytes(bytes: [u8; capsule_byte_count]) -> Result<Self, renderer_error> {
        validate_encoded_capsule(&bytes)?;
        Ok(Self { bytes })
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; capsule_byte_count] {
        &self.bytes
    }

    pub(crate) fn command(&self, index: usize) -> encoded_command {
        debug_assert!(index < fixture_command_count);
        decode_command(&self.bytes, index).expect("validated capsule command")
    }
}

fn validate_projection_command(
    command: &projection_command<'_>,
    expected: command_spec,
) -> Result<(), renderer_error> {
    if let Some(label) = command.label
        && !label_has_supported_glyphs(label)
    {
        return Err(renderer_error::unsupported_glyph);
    }
    match (command.label, expected.label) {
        (None, Some(_)) => return Err(renderer_error::missing_label),
        (Some(_), None) => return Err(renderer_error::unexpected_label),
        (Some(label), Some(selector)) if label != selector.text() => {
            return Err(renderer_error::wrong_label);
        }
        _ => {}
    }
    if command.kind != expected.kind
        || command.shape != expected.shape
        || command.color != expected.color
        || command.secondary_color != expected.secondary_color
        || command.slot != expected.slot
        || command.bounds != expected.bounds
        || command.state_active != expected.state_active
    {
        return Err(renderer_error::capsule_mismatch);
    }
    Ok(())
}

#[cfg(feature = "transport")]
pub(crate) fn validate_fixture_projection_command(
    index: usize,
    command: &projection_command<'_>,
) -> Result<(), renderer_error> {
    let expected = fixture_specs
        .get(index)
        .copied()
        .ok_or(renderer_error::wrong_command_count)?;
    validate_projection_command(command, expected)
}

fn validate_encoded_capsule(bytes: &[u8; capsule_byte_count]) -> Result<(), renderer_error> {
    if bytes[..4] != capsule_magic {
        return Err(renderer_error::wrong_capsule_magic);
    }
    if read_u16(bytes, 4) as u32 != frame_width || read_u16(bytes, 6) as u32 != frame_height {
        return Err(renderer_error::wrong_viewport);
    }
    if bytes[8] as usize != fixture_command_count {
        return Err(renderer_error::wrong_command_count);
    }
    if bytes[9] != fixture_state_ordinal {
        return Err(renderer_error::wrong_state_ordinal);
    }
    if bytes[10] != fixture_label_count {
        return Err(renderer_error::wrong_label_count);
    }
    if bytes[11..capsule_header_byte_count]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(renderer_error::nonzero_reserved);
    }
    for (index, expected) in fixture_specs.iter().copied().enumerate() {
        let decoded = decode_command(bytes, index)?;
        if !decoded.matches(expected) {
            return Err(renderer_error::capsule_mismatch);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) struct encoded_command {
    pub(crate) kind: capsule_command_kind,
    pub(crate) shape: capsule_shape,
    pub(crate) color: capsule_color,
    pub(crate) secondary_color: Option<capsule_color>,
    pub(crate) slot: capsule_slot,
    pub(crate) label: Option<label_selector>,
    pub(crate) bounds: capsule_rect,
    pub(crate) state_active: bool,
}

impl encoded_command {
    fn matches(self, expected: command_spec) -> bool {
        self.kind == expected.kind
            && self.shape == expected.shape
            && self.color == expected.color
            && self.secondary_color == expected.secondary_color
            && self.slot == expected.slot
            && self.label == expected.label
            && self.bounds == expected.bounds
            && self.state_active == expected.state_active
    }
}

fn decode_command(
    bytes: &[u8; capsule_byte_count],
    index: usize,
) -> Result<encoded_command, renderer_error> {
    let offset = capsule_header_byte_count + index * command_byte_count;
    let kind = capsule_command_kind::try_from_byte(bytes[offset])?;
    let shape = capsule_shape::try_from_byte(bytes[offset + 1])?;
    let color = capsule_color::try_from_byte(bytes[offset + 2])?;
    let secondary_color = if bytes[offset + 3] == no_secondary_color {
        None
    } else {
        Some(
            capsule_color::try_from_byte(bytes[offset + 3])
                .map_err(|_| renderer_error::invalid_secondary_color)?,
        )
    };
    let slot = capsule_slot::try_from_byte(bytes[offset + 4])?;
    let label = if bytes[offset + 5] == no_label {
        None
    } else {
        Some(label_selector::try_from_byte(bytes[offset + 5])?)
    };
    let state_active = match bytes[offset + 6] {
        0 => false,
        1 => true,
        _ => return Err(renderer_error::invalid_state_flag),
    };
    if bytes[offset + 7] != 0 {
        return Err(renderer_error::nonzero_reserved);
    }
    let x = read_u16(bytes, offset + 8) as u32;
    let y = read_u16(bytes, offset + 10) as u32;
    let width = read_u16(bytes, offset + 12) as u32;
    let height = read_u16(bytes, offset + 14) as u32;
    let bounds = capsule_rect::try_new(x, y, width, height)?;
    Ok(encoded_command {
        kind,
        shape,
        color,
        secondary_color,
        slot,
        label,
        bounds,
        state_active,
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

const fn encode_canonical_capsule() -> [u8; capsule_byte_count] {
    let mut bytes = [0_u8; capsule_byte_count];
    bytes[0] = capsule_magic[0];
    bytes[1] = capsule_magic[1];
    bytes[2] = capsule_magic[2];
    bytes[3] = capsule_magic[3];
    write_u16(&mut bytes, 4, frame_width as u16);
    write_u16(&mut bytes, 6, frame_height as u16);
    bytes[8] = fixture_command_count as u8;
    bytes[9] = fixture_state_ordinal;
    bytes[10] = fixture_label_count;

    let mut index = 0;
    while index < fixture_command_count {
        let spec = fixture_specs[index];
        let offset = capsule_header_byte_count + index * command_byte_count;
        bytes[offset] = spec.kind as u8;
        bytes[offset + 1] = spec.shape as u8;
        bytes[offset + 2] = spec.color as u8;
        bytes[offset + 3] = match spec.secondary_color {
            Some(color) => color as u8,
            None => no_secondary_color,
        };
        bytes[offset + 4] = spec.slot as u8;
        bytes[offset + 5] = match spec.label {
            Some(label) => label as u8,
            None => no_label,
        };
        bytes[offset + 6] = spec.state_active as u8;
        write_u16(&mut bytes, offset + 8, spec.bounds.x as u16);
        write_u16(&mut bytes, offset + 10, spec.bounds.y as u16);
        write_u16(&mut bytes, offset + 12, spec.bounds.width as u16);
        write_u16(&mut bytes, offset + 14, spec.bounds.height as u16);
        index += 1;
    }
    bytes
}

const fn write_u16(bytes: &mut [u8; capsule_byte_count], offset: usize, value: u16) {
    let encoded = value.to_le_bytes();
    bytes[offset] = encoded[0];
    bytes[offset + 1] = encoded[1];
}

const canonical_capsule_bytes: [u8; capsule_byte_count] = encode_canonical_capsule();
