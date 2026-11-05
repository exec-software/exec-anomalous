//! Independent host oracle for ADR 0026's fixed semantic desktop frame.
//!
//! The ring-3 target paints one row at a time from a compact capsule. This
//! oracle deliberately does neither: it applies the accepted semantic paint
//! operations to a complete host-owned RGB8 frame. Keeping the implementations
//! structurally separate makes a byte-for-byte QEMU capture comparison useful
//! evidence rather than a second call into the target renderer.

pub(super) const frame_width: usize = 1024;
pub(super) const frame_height: usize = 768;
pub(super) const channel_count: usize = 3;
pub(super) const ppm_header: &[u8] = b"P6\n1024 768\n255\n";
#[cfg(test)]
pub(super) const frozen_frame_hash: u64 = 0x6998_951a_a0fa_83e3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct rgb8 {
    red: u8,
    green: u8,
    blue: u8,
}

impl rgb8 {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct rectangle {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl rectangle {
    const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn checked_end(self) -> Result<(usize, usize), String> {
        let right = self
            .x
            .checked_add(self.width)
            .ok_or_else(|| "semantic oracle rectangle x extent overflowed".to_string())?;
        let bottom = self
            .y
            .checked_add(self.height)
            .ok_or_else(|| "semantic oracle rectangle y extent overflowed".to_string())?;
        if self.width == 0 || self.height == 0 || right > frame_width || bottom > frame_height {
            return Err("semantic oracle rectangle is outside the fixed frame".into());
        }
        Ok((right, bottom))
    }
}

const canvas: rgb8 = rgb8::new(0xe7, 0xec, 0xf3);
const surface: rgb8 = rgb8::new(0xf5, 0xf7, 0xfb);
const primary_fill: rgb8 = rgb8::new(0x1e, 0x40, 0xaf);
const disabled: rgb8 = rgb8::new(0xcb, 0xd5, 0xe1);
const primary_text: rgb8 = rgb8::new(0x17, 0x20, 0x33);
const on_accent: rgb8 = rgb8::new(0xff, 0xff, 0xff);
const outline: rgb8 = rgb8::new(0x52, 0x65, 0x7a);
const focus_outer: rgb8 = rgb8::new(0x11, 0x18, 0x27);
const focus_inner: rgb8 = rgb8::new(0xff, 0xff, 0xff);
const protected_status: rgb8 = rgb8::new(0x6b, 0x21, 0xa8);
const shadow: rgb8 = rgb8::new(0xa9, 0xb4, 0xc4);
const highlight: rgb8 = rgb8::new(0xff, 0xff, 0xff);

const root: rectangle = rectangle::new(0, 0, 1024, 768);
const masthead: rectangle = rectangle::new(32, 24, 960, 88);
const navigation: rectangle = rectangle::new(32, 136, 256, 600);
const primary_action: rectangle = rectangle::new(56, 184, 208, 64);
const secondary_action: rectangle = rectangle::new(56, 272, 208, 64);
const content: rectangle = rectangle::new(312, 136, 680, 600);

const glyph_width: usize = 5;
const glyph_height: usize = 7;

pub(super) fn render_frame() -> Result<Vec<u8>, String> {
    let byte_length = frame_width
        .checked_mul(frame_height)
        .and_then(|pixels| pixels.checked_mul(channel_count))
        .ok_or_else(|| "semantic oracle frame extent overflowed".to_string())?;
    let mut frame = vec![0_u8; byte_length];

    // These are the 25 accepted semantic operations in their canonical order.
    fill(&mut frame, root, canvas)?;
    fill(&mut frame, masthead, surface)?;
    paint_outline(&mut frame, masthead, 2, outline)?;
    paint_label(&mut frame, masthead, 56, 57, 3, "Anomalous", primary_text)?;

    paint_offset_fill(&mut frame, navigation, 8, 8, shadow)?;
    fill(&mut frame, navigation, surface)?;
    paint_highlight(&mut frame, navigation, highlight)?;
    paint_outline(&mut frame, navigation, 2, outline)?;
    paint_label(
        &mut frame,
        navigation,
        56,
        160,
        2,
        "Navigation",
        primary_text,
    )?;

    fill(&mut frame, primary_action, primary_fill)?;
    paint_outline(&mut frame, primary_action, 2, outline)?;
    paint_label(
        &mut frame,
        primary_action,
        76,
        209,
        2,
        "Open shell",
        on_accent,
    )?;
    paint_enabled_marker(&mut frame, primary_action, on_accent)?;
    paint_focus(&mut frame, primary_action, focus_outer, focus_inner)?;

    fill(&mut frame, secondary_action, disabled)?;
    paint_outline(&mut frame, secondary_action, 2, outline)?;
    paint_label(
        &mut frame,
        secondary_action,
        76,
        297,
        2,
        "Inspect system",
        primary_text,
    )?;
    paint_disabled_marker(&mut frame, secondary_action, outline)?;
    // The canonical secondary focus operation is inactive and paints nothing.

    paint_offset_fill(&mut frame, content, 8, 8, shadow)?;
    fill(&mut frame, content, surface)?;
    paint_highlight(&mut frame, content, highlight)?;
    paint_outline(&mut frame, content, 2, outline)?;
    paint_label(
        &mut frame,
        rectangle::new(352, 176, 600, 48),
        352,
        189,
        3,
        "System workspace",
        primary_text,
    )?;
    paint_label(
        &mut frame,
        rectangle::new(352, 248, 600, 96),
        352,
        264,
        2,
        "Protected status",
        protected_status,
    )?;

    Ok(frame)
}

pub(super) fn verify_ppm(ppm: &[u8]) -> Result<(), String> {
    let expected_pixels = frame_width
        .checked_mul(frame_height)
        .and_then(|pixels| pixels.checked_mul(channel_count))
        .ok_or_else(|| "semantic oracle PPM extent overflowed".to_string())?;
    let expected_length = ppm_header
        .len()
        .checked_add(expected_pixels)
        .ok_or_else(|| "semantic oracle PPM length overflowed".to_string())?;
    if ppm.len() != expected_length || !ppm.starts_with(ppm_header) {
        return Err("renderer capture is not exact canonical P6 1024x768 RGB8".into());
    }
    let expected = render_frame()?;
    let observed = &ppm[ppm_header.len()..];
    if let Some(index) = observed
        .iter()
        .zip(expected.iter())
        .position(|(observed, expected)| observed != expected)
    {
        let pixel = index / channel_count;
        let x = pixel % frame_width;
        let y = pixel / frame_width;
        let channel = ["red", "green", "blue"][index % channel_count];
        return Err(format!(
            "renderer capture differs from the semantic oracle at ({x}, {y}) {channel}"
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn hash_frame(frame: &[u8]) -> Result<u64, String> {
    let expected_length = frame_width
        .checked_mul(frame_height)
        .and_then(|pixels| pixels.checked_mul(channel_count))
        .ok_or_else(|| "semantic oracle hash extent overflowed".to_string())?;
    if frame.len() != expected_length {
        return Err("semantic oracle hash input has the wrong length".into());
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in frame {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Ok(hash)
}

fn fill(frame: &mut [u8], bounds: rectangle, color: rgb8) -> Result<(), String> {
    let (right, bottom) = bounds.checked_end()?;
    for y in bounds.y..bottom {
        paint_span(frame, y, bounds.x, right, color);
    }
    Ok(())
}

fn paint_offset_fill(
    frame: &mut [u8],
    bounds: rectangle,
    offset_x: usize,
    offset_y: usize,
    color: rgb8,
) -> Result<(), String> {
    let x = bounds
        .x
        .checked_add(offset_x)
        .ok_or_else(|| "semantic oracle shadow x overflowed".to_string())?;
    let y = bounds
        .y
        .checked_add(offset_y)
        .ok_or_else(|| "semantic oracle shadow y overflowed".to_string())?;
    let right = x.saturating_add(bounds.width).min(frame_width);
    let bottom = y.saturating_add(bounds.height).min(frame_height);
    if x >= right || y >= bottom {
        return Err("semantic oracle shadow collapsed".into());
    }
    fill(frame, rectangle::new(x, y, right - x, bottom - y), color)
}

fn paint_outline(
    frame: &mut [u8],
    bounds: rectangle,
    thickness: usize,
    color: rgb8,
) -> Result<(), String> {
    let (right, bottom) = bounds.checked_end()?;
    if thickness == 0 || thickness * 2 > bounds.width || thickness * 2 > bounds.height {
        return Err("semantic oracle outline thickness is invalid".into());
    }
    fill(
        frame,
        rectangle::new(bounds.x, bounds.y, bounds.width, thickness),
        color,
    )?;
    fill(
        frame,
        rectangle::new(bounds.x, bottom - thickness, bounds.width, thickness),
        color,
    )?;
    if bounds.height > thickness * 2 {
        fill(
            frame,
            rectangle::new(
                bounds.x,
                bounds.y + thickness,
                thickness,
                bounds.height - thickness * 2,
            ),
            color,
        )?;
        fill(
            frame,
            rectangle::new(
                right - thickness,
                bounds.y + thickness,
                thickness,
                bounds.height - thickness * 2,
            ),
            color,
        )?;
    }
    Ok(())
}

fn paint_highlight(frame: &mut [u8], bounds: rectangle, color: rgb8) -> Result<(), String> {
    let _ = bounds.checked_end()?;
    if bounds.width <= 4 || bounds.height <= 4 {
        return Err("semantic oracle highlight bounds are too small".into());
    }
    fill(
        frame,
        rectangle::new(bounds.x + 2, bounds.y, bounds.width - 4, 2),
        color,
    )?;
    fill(
        frame,
        rectangle::new(bounds.x + 2, bounds.y + 2, 2, bounds.height - 4),
        color,
    )
}

fn paint_label(
    frame: &mut [u8],
    bounds: rectangle,
    x: usize,
    y: usize,
    scale: usize,
    text: &str,
    color: rgb8,
) -> Result<(), String> {
    let (right, bottom) = bounds.checked_end()?;
    if scale == 0 || x < bounds.x || y < bounds.y {
        return Err("semantic oracle label origin is invalid".into());
    }
    let advance = (glyph_width + 1)
        .checked_mul(scale)
        .ok_or_else(|| "semantic oracle glyph advance overflowed".to_string())?;
    let text_width = text
        .len()
        .checked_mul(advance)
        .ok_or_else(|| "semantic oracle label width overflowed".to_string())?;
    if x.checked_add(text_width).is_none_or(|end| end > right)
        || y.checked_add(glyph_height * scale)
            .is_none_or(|end| end > bottom)
    {
        return Err("semantic oracle label exceeds its semantic bounds".into());
    }

    for (character_index, byte) in text.bytes().enumerate() {
        let rows = glyph_rows(byte)
            .ok_or_else(|| "semantic oracle label uses an unsupported glyph".to_string())?;
        if byte == b' ' {
            continue;
        }
        let glyph_x = x + character_index * advance;
        for (row_index, bits) in rows.iter().copied().enumerate() {
            for column in 0..glyph_width {
                if bits & (1 << (glyph_width - 1 - column)) != 0 {
                    fill(
                        frame,
                        rectangle::new(
                            glyph_x + column * scale,
                            y + row_index * scale,
                            scale,
                            scale,
                        ),
                        color,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn paint_enabled_marker(frame: &mut [u8], bounds: rectangle, color: rgb8) -> Result<(), String> {
    let marker = marker_bounds(bounds)?;
    fill(frame, marker, color)
}

fn paint_disabled_marker(frame: &mut [u8], bounds: rectangle, color: rgb8) -> Result<(), String> {
    let marker = marker_bounds(bounds)?;
    paint_outline(frame, marker, 2, color)?;
    for local_y in 2..marker.height - 2 {
        let y = marker.y + local_y;
        paint_span(frame, y, marker.x + local_y, marker.x + local_y + 1, color);
        let mirrored = marker.x + marker.width - 1 - local_y;
        paint_span(frame, y, mirrored, mirrored + 1, color);
    }
    Ok(())
}

fn marker_bounds(bounds: rectangle) -> Result<rectangle, String> {
    let _ = bounds.checked_end()?;
    if bounds.width < 28 || bounds.height < 12 {
        return Err("semantic oracle marker bounds are too small".into());
    }
    Ok(rectangle::new(
        bounds.x + bounds.width - 28,
        bounds.y + (bounds.height - 12) / 2,
        12,
        12,
    ))
}

fn paint_focus(
    frame: &mut [u8],
    bounds: rectangle,
    outer: rgb8,
    inner: rgb8,
) -> Result<(), String> {
    paint_outline(frame, expand(bounds, 6)?, 2, outer)?;
    paint_outline(frame, expand(bounds, 3)?, 1, inner)
}

fn expand(bounds: rectangle, amount: usize) -> Result<rectangle, String> {
    let (right, bottom) = bounds.checked_end()?;
    let x = bounds.x.saturating_sub(amount);
    let y = bounds.y.saturating_sub(amount);
    let right = right.saturating_add(amount).min(frame_width);
    let bottom = bottom.saturating_add(amount).min(frame_height);
    Ok(rectangle::new(x, y, right - x, bottom - y))
}

fn paint_span(frame: &mut [u8], y: usize, start: usize, end: usize, color: rgb8) {
    debug_assert!(y < frame_height);
    debug_assert!(start <= end && end <= frame_width);
    for x in start..end {
        let offset = (y * frame_width + x) * channel_count;
        frame[offset] = color.red;
        frame[offset + 1] = color.green;
        frame[offset + 2] = color.blue;
    }
}

fn glyph_rows(byte: u8) -> Option<[u8; glyph_height]> {
    let upper = if byte.is_ascii_lowercase() {
        byte - (b'a' - b'A')
    } else {
        byte
    };
    match upper {
        b' ' => Some([0; glyph_height]),
        b'A' => Some([0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
        b'C' => Some([0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e]),
        b'D' => Some([0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e]),
        b'E' => Some([0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f]),
        b'G' => Some([0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f]),
        b'H' => Some([0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
        b'I' => Some([0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1f]),
        b'K' => Some([0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
        b'L' => Some([0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f]),
        b'M' => Some([0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11]),
        b'N' => Some([0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11]),
        b'O' => Some([0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
        b'P' => Some([0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10]),
        b'R' => Some([0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11]),
        b'S' => Some([0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e]),
        b'T' => Some([0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        b'U' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
        b'V' => Some([0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04]),
        b'W' => Some([0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a]),
        b'Y' => Some([0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anomalous_desktop_renderer::{
        render_capsule, render_row, row_byte_count, scene_conformance::build_fixture_capsule,
    };

    fn protected_source(multiplier: u32, offset: u32) -> String {
        (0_u32..37)
            .map(|index| {
                char::from_u32(u32::from(b'a') + (index * multiplier + offset) % 26)
                    .expect("lowercase ASCII")
            })
            .collect()
    }

    #[test]
    fn independent_oracle_matches_the_canonical_scene_and_frozen_frame_hash() {
        let frame = render_frame().expect("bounded semantic oracle");
        assert_eq!(frame.len(), frame_width * frame_height * channel_count);
        let first = build_fixture_capsule(&protected_source(7, 19))
            .expect("canonical scene projects to a capsule");
        let second = build_fixture_capsule(&protected_source(11, 23))
            .expect("alternate protected source projects to a capsule");
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_eq!(first.as_bytes(), render_capsule::fixture().as_bytes());

        let mut target_row = [0_u8; row_byte_count];
        for y in 0..frame_height {
            render_row(&first, y as u32, &mut target_row).expect("target reference row");
            let start = y * row_byte_count;
            if let Some(index) = frame[start..start + row_byte_count]
                .iter()
                .zip(target_row.iter())
                .position(|(oracle, target)| oracle != target)
            {
                panic!(
                    "oracle diverged from target at ({}, {y}) channel {}: oracle={:#04x} target={:#04x}",
                    index / channel_count,
                    index % channel_count,
                    frame[start + index],
                    target_row[index]
                );
            }
        }
        assert_eq!(hash_frame(&frame).expect("exact frame"), frozen_frame_hash);
    }

    #[test]
    fn strict_ppm_verifier_rejects_every_surface_mutation() {
        let frame = render_frame().expect("bounded semantic oracle");
        let mut ppm = ppm_header.to_vec();
        ppm.extend_from_slice(&frame);
        verify_ppm(&ppm).expect("canonical capture");

        let last = ppm.len() - 1;
        ppm[last] ^= 1;
        assert!(verify_ppm(&ppm).is_err());
        ppm[last] ^= 1;
        assert!(verify_ppm(&ppm[..last]).is_err());

        let mut noncanonical = ppm;
        noncanonical[1] = b'3';
        assert!(verify_ppm(&noncanonical).is_err());
    }
}
