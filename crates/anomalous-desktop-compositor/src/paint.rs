use crate::font::glyph;
use crate::{frame_width, row_byte_count};

pub(crate) type rgb = [u8; 3];

#[derive(Clone, Copy)]
pub(crate) struct rect {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) width: i64,
    pub(crate) height: i64,
}

impl rect {
    pub(crate) const fn new(x: i64, y: i64, width: i64, height: i64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) const fn inset(self, amount: i64) -> Self {
        Self::new(
            self.x + amount,
            self.y + amount,
            self.width - 2 * amount,
            self.height - 2 * amount,
        )
    }

    pub(crate) const fn translated(self, x: i64, y: i64) -> Self {
        Self::new(self.x + x, self.y + y, self.width, self.height)
    }
}

/// Four-by-four fixed subpixel coverage for a bounded rounded rectangle.
/// Composition and window picking share this silhouette; transparent corners
/// never become invisible rectangular input blockers.
pub(crate) fn rounded_coverage(bounds: rect, radius: i64, x: i64, y: i64) -> u8 {
    if bounds.width <= 0
        || bounds.height <= 0
        || x < bounds.x
        || y < bounds.y
        || x >= bounds.x + bounds.width
        || y >= bounds.y + bounds.height
    {
        return 0;
    }
    let radius = radius.clamp(0, bounds.width.min(bounds.height) / 2);
    if radius == 0
        || (x >= bounds.x + radius && x < bounds.x + bounds.width - radius)
        || (y >= bounds.y + radius && y < bounds.y + bounds.height - radius)
    {
        return 255;
    }
    let cx = if x < bounds.x + radius {
        bounds.x + radius
    } else {
        bounds.x + bounds.width - radius
    } * 8;
    let cy = if y < bounds.y + radius {
        bounds.y + radius
    } else {
        bounds.y + bounds.height - radius
    } * 8;
    let squared = (radius * 8) * (radius * 8);
    let mut covered = 0;
    for sy in [1, 3, 5, 7] {
        for sx in [1, 3, 5, 7] {
            let dx = x * 8 + sx - cx;
            let dy = y * 8 + sy - cy;
            covered += u16::from(dx * dx + dy * dy <= squared);
        }
    }
    ((covered * 255 + 8) / 16) as u8
}

// Two bounded spans outside the inner shape's certainly opaque core. Skipping
// that core makes shadow work proportional to its perimeter, not window area.
fn outside_spans(outer: rect, inner: rect, radius: i64, row: i64) -> [(i64, i64); 2] {
    let left = outer.x.clamp(0, i64::from(frame_width));
    let right = (outer.x + outer.width).clamp(0, i64::from(frame_width));
    if inner.width <= 0 || inner.height <= 0 || row < inner.y || row >= inner.y + inner.height {
        return [(left, right), (right, right)];
    }
    let radius = radius.clamp(0, inner.width.min(inner.height) / 2);
    let inset = if row < inner.y + radius || row >= inner.y + inner.height - radius {
        radius
    } else {
        0
    };
    [
        (left, (inner.x + inset).clamp(left, right)),
        ((inner.x + inner.width - inset).clamp(left, right), right),
    ]
}

pub(crate) struct row_canvas<'a> {
    pub(crate) row: i64,
    output: &'a mut [u8],
}

impl<'a> row_canvas<'a> {
    pub(crate) fn new(row: u32, output: &'a mut [u8]) -> Self {
        debug_assert_eq!(output.len(), row_byte_count);
        Self {
            row: i64::from(row),
            output,
        }
    }

    pub(crate) fn fill(&mut self, color: rgb) {
        for pixel in self.output.chunks_exact_mut(3) {
            pixel.copy_from_slice(&color);
        }
    }

    /// Opaque RGB8 content fills its client rectangle using deterministic
    /// nearest-neighbor stretch. Source padding is never sampled. All products
    /// use u64: a validated source dimension times one u32 target coordinate
    /// cannot wrap, even before the output-width clip is applied.
    pub(crate) fn surface(&mut self, bounds: rect, surface: &anomalous_window_surface::frame<'_>) {
        if bounds.width <= 0
            || bounds.height <= 0
            || self.row < bounds.y
            || self.row >= bounds.y + bounds.height
        {
            return;
        }
        let descriptor = surface.descriptor();
        let source_y = ((self.row - bounds.y) as u64 * u64::from(descriptor.height())
            / bounds.height as u64) as u32;
        let source = surface.row(source_y).expect("validated bounded source row");
        let left = bounds.x.max(0);
        let right = (bounds.x + bounds.width).min(i64::from(frame_width));
        for x in left..right {
            let source_x = ((x - bounds.x) as u64 * u64::from(descriptor.width())
                / bounds.width as u64) as usize;
            let start = x as usize * 3;
            let source_start = source_x * 3;
            self.output[start..start + 3].copy_from_slice(&source[source_start..source_start + 3]);
        }
    }

    pub(crate) fn blend_pixel(&mut self, x: i64, color: rgb, alpha: u8) {
        if alpha == 0 || !(0..i64::from(frame_width)).contains(&x) {
            return;
        }
        let start = x as usize * 3;
        if alpha == 255 {
            self.output[start..start + 3].copy_from_slice(&color);
            return;
        }
        let a = u32::from(alpha);
        for (old, new) in self.output[start..start + 3].iter_mut().zip(color) {
            *old = ((u32::from(new) * a + u32::from(*old) * (255 - a) + 127) / 255) as u8;
        }
    }

    pub(crate) fn rounded_rectangle(&mut self, bounds: rect, radius: i64, color: rgb) {
        self.rounded_frame(bounds, radius, 0, color, color);
    }

    /// Blend the border and fill together once at the outer coverage edge.
    /// This avoids the dark seam produced by independently layering two masks.
    pub(crate) fn rounded_frame(
        &mut self,
        bounds: rect,
        radius: i64,
        width: i64,
        fill: rgb,
        border: rgb,
    ) {
        if self.row < bounds.y
            || self.row >= bounds.y + bounds.height
            || bounds.width <= 0
            || bounds.height <= 0
        {
            return;
        }
        let width = width.max(0).min(bounds.width.min(bounds.height) / 2);
        let inner = bounds.inset(width);
        // The inner shape's certainly opaque core needs neither subpixel
        // coverage nor source-over arithmetic. Keep the existing scalar
        // calculation at every border and antialiased corner pixel.
        let spans = outside_spans(bounds, inner, (radius - width).max(0), self.row);
        let core_start = spans[0].1;
        let core_end = spans[1].0;
        self.rectangle(
            rect::new(core_start, self.row, core_end - core_start, 1),
            fill,
        );
        for (start, end) in spans {
            for x in start..end {
                let outer = u32::from(rounded_coverage(bounds, radius, x, self.row));
                if outer == 0 {
                    continue;
                }
                let inside = u32::from(rounded_coverage(
                    inner,
                    (radius - width).max(0),
                    x,
                    self.row,
                ))
                .min(outer);
                let mut color = [0; 3];
                for channel in 0..3 {
                    color[channel] = ((u32::from(fill[channel]) * inside
                        + u32::from(border[channel]) * (outer - inside)
                        + outer / 2)
                        / outer) as u8;
                }
                self.blend_pixel(x, color, outer as u8);
            }
        }
    }

    pub(crate) fn rounded_outline(&mut self, bounds: rect, radius: i64, width: i64, color: rgb) {
        if width <= 0
            || bounds.width <= 0
            || bounds.height <= 0
            || self.row < bounds.y
            || self.row >= bounds.y + bounds.height
        {
            return;
        }
        let inner = bounds.inset(width);
        for (start, end) in outside_spans(bounds, inner, (radius - width).max(0), self.row) {
            for x in start..end {
                let a = rounded_coverage(bounds, radius, x, self.row);
                let b = rounded_coverage(inner, (radius - width).max(0), x, self.row);
                self.blend_pixel(x, color, a.saturating_sub(b));
            }
        }
    }

    /// A bounded static shadow made of disjoint antialiased one-pixel rings.
    /// Each destination is shaded once per ring, never with an opaque shadow
    /// color that would replace the lower window's content.
    pub(crate) fn soft_shadow(
        &mut self,
        bounds: rect,
        radius: i64,
        spread: u8,
        opacity: u8,
        color: rgb,
        offset: i64,
    ) {
        if spread == 0 || opacity == 0 || bounds.width <= 0 || bounds.height <= 0 {
            return;
        }
        let spread = i64::from(spread.min(16));
        let shifted = bounds.translated(0, offset.clamp(0, 12));
        if self.row < shifted.y - spread || self.row >= shifted.y + shifted.height + spread {
            return;
        }
        for (start, end) in outside_spans(shifted, bounds, radius, self.row) {
            for x in start..end {
                let coverage = rounded_coverage(shifted, radius, x, self.row);
                self.blend_pixel(
                    x,
                    color,
                    (u16::from(opacity) * u16::from(coverage) / 255) as u8,
                );
            }
        }
        for distance in (1..=spread).rev() {
            let outer = shifted.inset(-distance);
            let inner = shifted.inset(1 - distance);
            if self.row < outer.y || self.row >= outer.y + outer.height {
                continue;
            }
            let weight = spread + 1 - distance;
            let alpha = i64::from(opacity) * weight * weight / (spread * spread);
            for (start, end) in outside_spans(outer, inner, radius + distance - 1, self.row) {
                for x in start..end {
                    let a = rounded_coverage(outer, radius + distance, x, self.row);
                    let b = rounded_coverage(inner, radius + distance - 1, x, self.row);
                    let coverage = a.saturating_sub(b);
                    self.blend_pixel(x, color, (alpha * i64::from(coverage) / 255) as u8);
                }
            }
        }
    }

    pub(crate) fn rectangle(&mut self, bounds: rect, color: rgb) {
        self.clipped_rectangle(
            bounds,
            rect::new(0, 0, i64::from(frame_width), i64::from(crate::frame_height)),
            color,
        );
    }

    pub(crate) fn clipped_rectangle(&mut self, bounds: rect, clip: rect, color: rgb) {
        if bounds.width <= 0
            || bounds.height <= 0
            || clip.width <= 0
            || clip.height <= 0
            || self.row < bounds.y
            || self.row >= bounds.y + bounds.height
            || self.row < clip.y
            || self.row >= clip.y + clip.height
        {
            return;
        }
        let start = bounds.x.max(clip.x).max(0).min(i64::from(frame_width)) as usize;
        let end = (bounds.x + bounds.width)
            .min(clip.x + clip.width)
            .clamp(0, i64::from(frame_width)) as usize;
        if start >= end {
            return;
        }
        for pixel in self.output[start * 3..end * 3].chunks_exact_mut(3) {
            pixel.copy_from_slice(&color);
        }
    }

    pub(crate) fn text(&mut self, x: i64, y: i64, scale: i64, text: &str, color: rgb, clip: rect) {
        if scale <= 0
            || self.row < y
            || self.row >= y + 7 * scale
            || self.row < clip.y
            || self.row >= clip.y + clip.height
        {
            return;
        }
        let glyph_row = ((self.row - y) / scale) as usize;
        let advance = 6 * scale;
        for (index, byte) in text.bytes().enumerate() {
            let origin = x + index as i64 * advance;
            if origin >= clip.x + clip.width || origin >= i64::from(frame_width) {
                break;
            }
            let character = if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '?'
            };
            let bits = glyph(character)[glyph_row];
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    self.clipped_rectangle(
                        rect::new(origin + column * scale, self.row, scale, 1),
                        clip,
                        color,
                    );
                }
            }
        }
    }

    pub(crate) fn number(
        &mut self,
        x: i64,
        y: i64,
        scale: i64,
        value: u64,
        color: rgb,
        clip: rect,
    ) {
        let mut digits = [b'0'; 20];
        let mut cursor = digits.len();
        let mut remainder = value;
        loop {
            cursor -= 1;
            digits[cursor] = b'0' + (remainder % 10) as u8;
            remainder /= 10;
            if remainder == 0 {
                break;
            }
        }
        // Digits are constructed only from ASCII decimal values.
        if let Ok(text) = core::str::from_utf8(&digits[cursor..]) {
            self.text(x, y, scale, text, color, clip);
        }
    }
}
