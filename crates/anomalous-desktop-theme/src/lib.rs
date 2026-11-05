#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Versioned, allocation-free declarative desktop appearance. The initial catalog
//! is compiled data, not an external package format or executable plugin boundary.
//! Only the embedded bitmap font, static depth, and two software cursors are
//! rendered. Login and sound roles are explicitly unavailable runtime components.

pub const schema_version: u16 = 1;
pub type rgb = [u8; 3];

pub mod assets;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum theme_mode {
    light,
    dark,
    high_contrast,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum density_mode {
    compact,
    comfortable,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum font_family {
    embedded_5x7,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum cursor_style {
    arrow,
    crosshair,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum component_support {
    unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct semantic_colors {
    pub background: rgb,
    pub surface: rgb,
    pub panel: rgb,
    pub inactive_title: rgb,
    pub active_title: rgb,
    pub text: rgb,
    pub secondary_text: rgb,
    pub active_text: rgb,
    pub border: rgb,
    pub shadow: rgb,
    pub accent: rgb,
    pub focus_outer: rgb,
    pub focus_inner: rgb,
    pub cursor_fill: rgb,
    pub cursor_outline: rgb,
}

/// Frame interaction colors are independently replaceable but validated with
/// the semantic surfaces they are painted on. Separator borders are decorative;
/// the primary outline and control glyphs carry the accessible state cues.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct decoration_palette {
    pub primary: rgb,
    pub primary_hover: rgb,
    pub primary_pressed: rgb,
    pub primary_ink: rgb,
    pub control_hover: rgb,
    pub control_pressed: rgb,
    pub control_ink: rgb,
    pub danger_hover: rgb,
    pub danger_pressed: rgb,
    pub danger_ink: rgb,
    pub inactive_border: rgb,
}

/// Bounded native frame geometry. Shadows blend over the existing destination;
/// opacity is an 8-bit coverage value, not an extra opaque background color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct frame_style {
    pub radius: u8,
    pub border_width: u8,
    pub title_separator: u8,
    pub control_radius: u8,
    pub shadow_spread: u8,
    pub shadow_opacity: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct chrome_metrics {
    pub header_height: u32,
    pub panel_height: u32,
    pub workarea_top: u32,
    pub workarea_bottom_inset: u32,
    pub workarea_margin: u32,
    pub title_height: u32,
    pub control_size: u32,
    pub resize_grip: u32,
    pub tile_gap: u32,
    pub shadow_offset: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct theme_overrides {
    pub colors: Option<semantic_colors>,
    pub metrics: Option<chrome_metrics>,
    pub decorations: Option<decoration_palette>,
    pub frame: Option<frame_style>,
    /// A narrow frame-corner override layered after the complete frame profile.
    /// This leaves theme-dependent shadow and control geometry unconstrained.
    pub corner_radius: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct theme_settings {
    pub theme: theme_mode,
    pub density: density_mode,
    pub font: font_family,
    /// Multiplier of the embedded bitmap typography, bounded to one or two.
    pub font_scale: u8,
    pub cursor: cursor_style,
    pub cursor_size: u8,
    pub depth: bool,
    pub reduced_motion: bool,
    pub overrides: theme_overrides,
}
impl Default for theme_settings {
    fn default() -> Self {
        Self::initial
    }
}
impl theme_settings {
    pub const initial: Self = Self {
        theme: theme_mode::light,
        density: density_mode::comfortable,
        font: font_family::embedded_5x7,
        font_scale: 1,
        cursor: cursor_style::arrow,
        cursor_size: 1,
        depth: true,
        reduced_motion: true,
        overrides: theme_overrides {
            colors: None,
            metrics: None,
            decorations: None,
            frame: None,
            corner_radius: None,
        },
    };
    pub fn validate(self) -> Result<theme_definition, theme_error> {
        if !(1..=2).contains(&self.font_scale) {
            return Err(theme_error::invalid_font_scale);
        }
        if !(1..=3).contains(&self.cursor_size) {
            return Err(theme_error::invalid_cursor_size);
        }
        let mut definition = builtin(self.theme, self.density);
        if let Some(colors) = self.overrides.colors {
            definition.colors = colors;
        }
        if let Some(metrics) = self.overrides.metrics {
            definition.metrics = metrics;
        }
        if let Some(decorations) = self.overrides.decorations {
            definition.decorations = decorations;
        }
        if let Some(frame) = self.overrides.frame {
            definition.frame = frame;
        }
        if let Some(radius) = self.overrides.corner_radius {
            definition.frame.radius = radius;
        }
        validate_colors(definition.colors)?;
        validate_metrics(definition.metrics)?;
        validate_decorations(definition.colors, definition.decorations)?;
        validate_frame(definition.frame)?;
        definition.font = self.font;
        definition.font_scale = self.font_scale;
        definition.cursor = self.cursor;
        definition.cursor_size = self.cursor_size;
        definition.depth = self.depth && self.theme != theme_mode::high_contrast;
        definition.reduced_motion = self.reduced_motion;
        // There is deliberately no animation backend; requesting motion cannot
        // enable one, and reduced motion always disables any future transition.
        definition.animation_duration_ms = 0;
        Ok(definition)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum theme_error {
    invalid_font_scale,
    invalid_cursor_size,
    invalid_metrics,
    invalid_frame,
    insufficient_contrast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct theme_definition {
    pub schema: u16,
    pub name: &'static str,
    pub colors: semantic_colors,
    pub metrics: chrome_metrics,
    pub decorations: decoration_palette,
    pub frame: frame_style,
    pub font: font_family,
    pub font_scale: u8,
    pub cursor: cursor_style,
    pub cursor_size: u8,
    pub depth: bool,
    pub reduced_motion: bool,
    pub animation_duration_ms: u16,
    pub login: component_support,
    pub sounds: component_support,
}

pub const fn builtin(theme: theme_mode, density: density_mode) -> theme_definition {
    // Dark follows Antimatter's Clay palette (vendored Displacer 1.0.1).
    // Light interprets the same warm neutrals, orange focus and teal secondary
    // role, with darker chromatic inks to preserve small-text contrast.
    let (name, colors, decorations) = match theme {
        theme_mode::light => (
            "Anomalous Light",
            semantic_colors {
                background: [221, 215, 206],
                surface: [250, 247, 242],
                panel: [234, 229, 221],
                inactive_title: [232, 226, 217],
                active_title: [242, 238, 231],
                text: [41, 38, 32],
                secondary_text: [101, 92, 83],
                active_text: [41, 38, 32],
                border: [170, 160, 145],
                shadow: [30, 24, 18],
                accent: [17, 105, 95],
                focus_outer: [166, 59, 8],
                focus_inner: [250, 247, 242],
                cursor_fill: [250, 247, 242],
                cursor_outline: [41, 38, 32],
            },
            decoration_palette {
                primary: [166, 59, 8],
                primary_hover: [150, 53, 8],
                primary_pressed: [125, 44, 7],
                primary_ink: [250, 247, 242],
                control_hover: [223, 215, 203],
                control_pressed: [206, 194, 177],
                control_ink: [41, 38, 32],
                danger_hover: [244, 215, 210],
                danger_pressed: [230, 185, 177],
                danger_ink: [127, 41, 37],
                inactive_border: [170, 160, 145],
            },
        ),
        theme_mode::dark => (
            "Anomalous Dark",
            semantic_colors {
                background: [20, 20, 20],
                surface: [26, 26, 26],
                panel: [26, 26, 26],
                inactive_title: [26, 26, 26],
                active_title: [34, 34, 34],
                text: [236, 233, 229],
                secondary_text: [156, 152, 147],
                active_text: [236, 233, 229],
                border: [42, 42, 42],
                shadow: [0, 0, 0],
                accent: [51, 183, 166],
                focus_outer: [242, 102, 26],
                focus_inner: [15, 15, 15],
                cursor_fill: [236, 233, 229],
                cursor_outline: [15, 15, 15],
            },
            decoration_palette {
                primary: [242, 102, 26],
                primary_hover: [255, 127, 51],
                primary_pressed: [255, 154, 92],
                primary_ink: [15, 15, 15],
                control_hover: [51, 51, 51],
                control_pressed: [65, 65, 65],
                control_ink: [236, 233, 229],
                danger_hover: [73, 35, 35],
                danger_pressed: [101, 48, 48],
                danger_ink: [255, 216, 216],
                inactive_border: [66, 66, 66],
            },
        ),
        theme_mode::high_contrast => (
            "Anomalous High Contrast",
            semantic_colors {
                background: [0, 0, 0],
                surface: [0, 0, 0],
                panel: [0, 0, 0],
                inactive_title: [0, 0, 0],
                active_title: [255, 255, 255],
                text: [255, 255, 255],
                secondary_text: [255, 255, 255],
                active_text: [0, 0, 0],
                border: [255, 255, 255],
                shadow: [0, 0, 0],
                accent: [255, 255, 0],
                focus_outer: [255, 255, 255],
                focus_inner: [0, 0, 0],
                cursor_fill: [255, 255, 255],
                cursor_outline: [0, 0, 0],
            },
            decoration_palette {
                primary: [255, 255, 0],
                primary_hover: [255, 255, 255],
                primary_pressed: [255, 255, 0],
                primary_ink: [0, 0, 0],
                control_hover: [255, 255, 0],
                control_pressed: [255, 255, 255],
                control_ink: [0, 0, 0],
                danger_hover: [255, 255, 0],
                danger_pressed: [255, 255, 255],
                danger_ink: [0, 0, 0],
                inactive_border: [255, 255, 255],
            },
        ),
    };
    let compact = matches!(density, density_mode::compact);
    theme_definition {
        schema: schema_version,
        name,
        colors,
        decorations,
        frame: frame_style {
            radius: if matches!(theme, theme_mode::high_contrast) {
                0
            } else {
                6
            },
            border_width: 1,
            title_separator: 1,
            control_radius: if matches!(theme, theme_mode::high_contrast) {
                0
            } else {
                4
            },
            shadow_spread: if matches!(theme, theme_mode::high_contrast) {
                0
            } else {
                8
            },
            shadow_opacity: if matches!(theme, theme_mode::high_contrast) {
                0
            } else {
                96
            },
        },
        metrics: chrome_metrics {
            header_height: 48,
            panel_height: 56,
            workarea_top: 64,
            workarea_bottom_inset: 80,
            workarea_margin: 16,
            title_height: if compact { 32 } else { 40 },
            control_size: if compact { 24 } else { 28 },
            resize_grip: 16,
            tile_gap: if compact { 8 } else { 12 },
            shadow_offset: 6,
        },
        font: font_family::embedded_5x7,
        font_scale: 1,
        cursor: cursor_style::arrow,
        cursor_size: 1,
        depth: !matches!(theme, theme_mode::high_contrast),
        reduced_motion: true,
        animation_duration_ms: 0,
        login: component_support::unavailable,
        sounds: component_support::unavailable,
    }
}

fn validate_metrics(m: chrome_metrics) -> Result<(), theme_error> {
    if !(32..=64).contains(&m.header_height)
        || !(40..=64).contains(&m.panel_height)
        || !(48..=96).contains(&m.workarea_top)
        || !(64..=128).contains(&m.workarea_bottom_inset)
        || !(8..=24).contains(&m.workarea_margin)
        || m.workarea_top < m.header_height + 8
        || m.workarea_bottom_inset < m.panel_height + 16
        || !(28..=56).contains(&m.title_height)
        || !(20..=36).contains(&m.control_size)
        || m.control_size + 8 > m.title_height
        || !(8..=24).contains(&m.resize_grip)
        || !(4..=20).contains(&m.tile_gap)
        || m.shadow_offset > 12
    {
        return Err(theme_error::invalid_metrics);
    }
    Ok(())
}

fn validate_frame(frame: frame_style) -> Result<(), theme_error> {
    if frame.radius > 12
        || !(1..=3).contains(&frame.border_width)
        || frame.title_separator > 2
        || frame.control_radius > 10
        || frame.shadow_spread > 16
        || frame.shadow_opacity > 160
    {
        return Err(theme_error::invalid_frame);
    }
    Ok(())
}

fn validate_decorations(c: semantic_colors, d: decoration_palette) -> Result<(), theme_error> {
    for (foreground, background) in [
        (d.primary_ink, d.primary),
        (d.primary_ink, d.primary_hover),
        (d.primary_ink, d.primary_pressed),
        (d.control_ink, d.control_hover),
        (d.control_ink, d.control_pressed),
        (d.danger_ink, d.danger_hover),
        (d.danger_ink, d.danger_pressed),
    ] {
        if !contrast_at_least(foreground, background, 45) {
            return Err(theme_error::insufficient_contrast);
        }
    }
    for background in [c.background, c.surface, c.panel, c.inactive_title] {
        if !contrast_at_least(d.primary, background, 30) {
            return Err(theme_error::insufficient_contrast);
        }
    }
    // High Contrast has a white active title on a black desktop. Its frame
    // renderer uses the inverse primary ink when the primary cannot contrast
    // the title. That same bounded choice is available to custom palettes.
    if !contrast_at_least(d.primary, c.active_title, 30)
        && !contrast_at_least(d.primary_ink, c.active_title, 30)
    {
        return Err(theme_error::insufficient_contrast);
    }
    Ok(())
}

fn validate_colors(c: semantic_colors) -> Result<(), theme_error> {
    for (foreground, background) in [
        (c.text, c.surface),
        (c.text, c.panel),
        (c.text, c.background),
        (c.text, c.inactive_title),
        (c.secondary_text, c.surface),
        (c.secondary_text, c.panel),
        (c.secondary_text, c.background),
        (c.secondary_text, c.inactive_title),
        (c.active_text, c.active_title),
        (c.accent, c.panel),
        (c.accent, c.surface),
    ] {
        if !contrast_at_least(foreground, background, 45) {
            return Err(theme_error::insufficient_contrast);
        }
    }
    if !contrast_at_least(c.cursor_fill, c.cursor_outline, 30)
        || !contrast_at_least(c.focus_outer, c.focus_inner, 30)
    {
        return Err(theme_error::insufficient_contrast);
    }
    // Either side of the two-tone focus outline must remain visible against
    // each adjacent surface; a custom surface cannot erase both outline inks.
    for background in [c.background, c.surface, c.inactive_title, c.active_title] {
        if !contrast_at_least(c.focus_outer, background, 30)
            && !contrast_at_least(c.focus_inner, background, 30)
        {
            return Err(theme_error::insufficient_contrast);
        }
    }
    Ok(())
}

/// WCAG relative-luminance contrast using an integer sRGB lookup table. The
/// threshold is tenths of a ratio (45 = 4.5:1). No host floating point is needed.
pub fn contrast_at_least(a: rgb, b: rgb, threshold_tenths: u32) -> bool {
    let a = luminance(a);
    let b = luminance(b);
    let bright = a.max(b) + 50_000;
    let dark = a.min(b) + 50_000;
    u64::from(bright) * 10 >= u64::from(dark) * u64::from(threshold_tenths)
}
fn luminance(color: rgb) -> u32 {
    let linear = color.map(|value| u64::from(srgb_linear[value as usize]));
    ((linear[0] * 2126 + linear[1] * 7152 + linear[2] * 722) / 10_000) as u32
}

const srgb_linear: [u32; 256] = [
    0, 304, 607, 911, 1214, 1518, 1821, 2125, 2428, 2732, 3035, 3347, 3677, 4025, 4391, 4777, 5182,
    5605, 6049, 6512, 6995, 7499, 8023, 8568, 9134, 9721, 10330, 10960, 11612, 12286, 12983, 13702,
    14444, 15209, 15996, 16807, 17642, 18500, 19382, 20289, 21219, 22174, 23153, 24158, 25187,
    26241, 27321, 28426, 29557, 30713, 31896, 33105, 34340, 35601, 36889, 38204, 39546, 40915,
    42311, 43735, 45186, 46665, 48172, 49707, 51269, 52861, 54480, 56128, 57805, 59511, 61246,
    63010, 64803, 66626, 68478, 70360, 72272, 74214, 76185, 78187, 80220, 82283, 84376, 86500,
    88656, 90842, 93059, 95307, 97587, 99899, 102242, 104616, 107023, 109462, 111932, 114435,
    116971, 119538, 122139, 124772, 127438, 130136, 132868, 135633, 138432, 141263, 144128, 147027,
    149960, 152926, 155926, 158961, 162029, 165132, 168269, 171441, 174647, 177888, 181164, 184475,
    187821, 191202, 194618, 198069, 201556, 205079, 208637, 212231, 215861, 219526, 223228, 226966,
    230740, 234551, 238398, 242281, 246201, 250158, 254152, 258183, 262251, 266356, 270498, 274677,
    278894, 283149, 287441, 291771, 296138, 300544, 304987, 309469, 313989, 318547, 323143, 327778,
    332452, 337164, 341914, 346704, 351533, 356400, 361307, 366253, 371238, 376262, 381326, 386429,
    391572, 396755, 401978, 407240, 412543, 417885, 423268, 428690, 434154, 439657, 445201, 450786,
    456411, 462077, 467784, 473531, 479320, 485150, 491021, 496933, 502886, 508881, 514918, 520996,
    527115, 533276, 539479, 545724, 552011, 558340, 564712, 571125, 577580, 584078, 590619, 597202,
    603827, 610496, 617207, 623960, 630757, 637597, 644480, 651406, 658375, 665387, 672443, 679542,
    686685, 693872, 701102, 708376, 715694, 723055, 730461, 737910, 745404, 752942, 760525, 768151,
    775822, 783538, 791298, 799103, 806952, 814847, 822786, 830770, 838799, 846873, 854993, 863157,
    871367, 879622, 887923, 896269, 904661, 913099, 921582, 930111, 938686, 947307, 955973, 964686,
    973445, 982251, 991102, 1000000,
];

#[cfg(test)]
mod tests;
