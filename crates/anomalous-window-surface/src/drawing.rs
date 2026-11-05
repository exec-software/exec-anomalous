use crate::{geometry, rgb, surface_error};

/// CPU drawing into borrowed RGB8 storage. Operations clip to the active pixel
/// rectangle and preserve all row padding. Coordinate work is bounded by the
/// admitted canvas dimensions, even for extreme caller-provided coordinates.
pub struct canvas<'a> {
    geometry: geometry,
    pixels: &'a mut [u8],
}
impl<'a> canvas<'a> {
    pub fn try_new(
        width: u32,
        height: u32,
        stride: u32,
        pixels: &'a mut [u8],
    ) -> Result<Self, surface_error> {
        let geometry = geometry::checked(width, height, stride, pixels.len())?;
        Ok(Self { geometry, pixels })
    }

    pub const fn width(&self) -> u32 {
        self.geometry.width
    }
    pub const fn height(&self) -> u32 {
        self.geometry.height
    }
    pub const fn stride(&self) -> u32 {
        self.geometry.stride
    }

    pub fn clear(&mut self, color: rgb) {
        self.fill_rect(0, 0, self.width(), self.height(), color);
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: u32, height: u32, color: rgb) {
        // Signed 64-bit sums admit every i32 origin plus every u32 size.
        let left = i64::from(x).max(0);
        let top = i64::from(y).max(0);
        let right = (i64::from(x) + i64::from(width)).min(i64::from(self.width()));
        let bottom = (i64::from(y) + i64::from(height)).min(i64::from(self.height()));
        if left >= right || top >= bottom {
            return;
        }
        for row in top as usize..bottom as usize {
            let start = row * self.stride() as usize + left as usize * 3;
            let end = row * self.stride() as usize + right as usize * 3;
            for pixel in self.pixels[start..end].chunks_exact_mut(3) {
                pixel.copy_from_slice(&color);
            }
        }
    }

    /// Draw a one-pixel line, including both endpoints. Integer clipping is
    /// performed before rasterization, so distant endpoints cannot trigger an
    /// unbounded walk. Intersection rounding is toward zero.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: rgb) {
        let Some((mut x0, mut y0, x1, y1)) = clip_line(
            i64::from(x0),
            i64::from(y0),
            i64::from(x1),
            i64::from(y1),
            i64::from(self.width() - 1),
            i64::from(self.height() - 1),
        ) else {
            return;
        };
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;
        loop {
            let index = y0 as usize * self.stride() as usize + x0 as usize * 3;
            self.pixels[index..index + 3].copy_from_slice(&color);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice_error = 2 * error;
            if twice_error >= dy {
                error += dy;
                x0 += sx;
            }
            if twice_error <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }
}

fn outcode(x: i64, y: i64, right: i64, bottom: i64) -> u8 {
    u8::from(x < 0)
        | (u8::from(x > right) << 1)
        | (u8::from(y < 0) << 2)
        | (u8::from(y > bottom) << 3)
}

fn intersection(origin: i64, delta: i64, distance: i64, divisor: i64) -> i64 {
    // Original endpoints span all of i32; their product can exceed i64. The
    // i128 intermediate retains that range without float or wrapping math.
    origin + ((i128::from(delta) * i128::from(distance)) / i128::from(divisor)) as i64
}

fn clip_line(
    mut x0: i64,
    mut y0: i64,
    mut x1: i64,
    mut y1: i64,
    right: i64,
    bottom: i64,
) -> Option<(i64, i64, i64, i64)> {
    // Cohen-Sutherland clipping eliminates an outside edge each iteration.
    // A fixed defensive bound also covers integer-rounded degenerate cases.
    for _ in 0..16 {
        let first = outcode(x0, y0, right, bottom);
        let second = outcode(x1, y1, right, bottom);
        if first | second == 0 {
            return Some((x0, y0, x1, y1));
        }
        if first & second != 0 {
            return None;
        }
        let outside = if first != 0 { first } else { second };
        let (x, y) = if outside & 8 != 0 {
            (intersection(x0, x1 - x0, bottom - y0, y1 - y0), bottom)
        } else if outside & 4 != 0 {
            (intersection(x0, x1 - x0, -y0, y1 - y0), 0)
        } else if outside & 2 != 0 {
            (right, intersection(y0, y1 - y0, right - x0, x1 - x0))
        } else {
            (0, intersection(y0, y1 - y0, -x0, x1 - x0))
        };
        // Common outcodes rejected above ensure the chosen denominator is
        // nonzero: a parallel outside segment lies wholly beyond that edge.
        if outside == first {
            x0 = x;
            y0 = y;
        } else {
            x1 = x;
            y1 = y;
        }
    }
    None
}
