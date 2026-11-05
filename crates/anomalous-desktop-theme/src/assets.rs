//! Compiled window-control vectors shared by the native painter and SVG export.
//!
//! Recipes are authored once in a 24-unit coordinate space. The native sampler
//! uses sixteen integer samples per output pixel and never parses external data,
//! allocates, or evaluates geometry outside these four bounded recipes.

/// Source coordinate extent, also used by the exported SVG view box.
pub const source_extent: u32 = 24;
pub const min_raster_size: u32 = 8;
pub const max_raster_size: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum control_asset {
    minimize,
    maximize,
    restore,
    close,
}

pub const control_assets: [control_asset; 4] = [
    control_asset::minimize,
    control_asset::maximize,
    control_asset::restore,
    control_asset::close,
];

impl control_asset {
    pub const fn id(self) -> &'static str {
        match self {
            Self::minimize => "minimize",
            Self::maximize => "maximize",
            Self::restore => "restore",
            Self::close => "close",
        }
    }
}

/// An opaque stroke with round end caps, in source coordinates.
///
/// The rounded strokes form a union: crossings do not darken when rasterized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct stroke_segment {
    pub x1: u8,
    pub y1: u8,
    pub x2: u8,
    pub y2: u8,
    pub stroke_width: u8,
}

const fn segment(x1: u8, y1: u8, x2: u8, y2: u8) -> stroke_segment {
    stroke_segment {
        x1,
        y1,
        x2,
        y2,
        stroke_width: 2,
    }
}

const minimize: [stroke_segment; 1] = [segment(7, 16, 17, 16)];
const maximize: [stroke_segment; 4] = [
    segment(7, 7, 17, 7),
    segment(17, 7, 17, 17),
    segment(17, 17, 7, 17),
    segment(7, 17, 7, 7),
];
const restore: [stroke_segment; 7] = [
    segment(9, 6, 18, 6),
    segment(18, 6, 18, 15),
    segment(18, 15, 17, 15),
    segment(6, 9, 15, 9),
    segment(15, 9, 15, 18),
    segment(15, 18, 6, 18),
    segment(6, 18, 6, 9),
];
const close: [stroke_segment; 2] = [segment(8, 8, 16, 16), segment(16, 8, 8, 16)];

pub const fn recipe(asset: control_asset) -> &'static [stroke_segment] {
    match asset {
        control_asset::minimize => &minimize,
        control_asset::maximize => &maximize,
        control_asset::restore => &restore,
        control_asset::close => &close,
    }
}

/// Coverage of one pixel in a square output glyph, in the range 0..=255.
///
/// Sizes outside 8..=32 and coordinates outside the requested output return zero.
/// Four by four evenly centered samples avoid floating point in the native path.
/// Each pixel is bounded by 16 samples times at most 7 constant stroke segments.
pub fn coverage(asset: control_asset, x: u32, y: u32, size: u32) -> u8 {
    if !(min_raster_size..=max_raster_size).contains(&size) || x >= size || y >= size {
        return 0;
    }
    let denominator = i64::from(8 * size);
    let mut covered = 0_u16;
    for sample_y in 0..4 {
        for sample_x in 0..4 {
            let px = i64::from((8 * x + 2 * sample_x + 1) * source_extent);
            let py = i64::from((8 * y + 2 * sample_y + 1) * source_extent);
            if recipe(asset)
                .iter()
                .any(|stroke| contains_sample(*stroke, px, py, denominator))
            {
                covered += 1;
            }
        }
    }
    ((covered * 255 + 8) / 16) as u8
}

fn contains_sample(stroke: stroke_segment, px: i64, py: i64, denominator: i64) -> bool {
    let x1 = i64::from(stroke.x1) * denominator;
    let y1 = i64::from(stroke.y1) * denominator;
    let x2 = i64::from(stroke.x2) * denominator;
    let y2 = i64::from(stroke.y2) * denominator;
    let vx = x2 - x1;
    let vy = y2 - y1;
    let dx = px - x1;
    let dy = py - y1;
    let radius = i64::from(stroke.stroke_width) * denominator / 2;
    let radius_squared = radius * radius;
    let length_squared = vx * vx + vy * vy;
    let projection = dx * vx + dy * vy;
    if projection <= 0 {
        return dx * dx + dy * dy <= radius_squared;
    }
    if projection >= length_squared {
        let dx = px - x2;
        let dy = py - y2;
        return dx * dx + dy * dy <= radius_squared;
    }
    // The largest coordinates are below 24 * 8 * 32 = 6144. Even the squared
    // cross product remains below 2^54, leaving ample headroom in signed i64.
    let cross = dx * vy - dy * vx;
    cross * cross <= radius_squared * length_squared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_rasters_are_empty_without_coordinate_overflow() {
        for asset in control_assets {
            for size in [0, 1, 7, 33, u32::MAX] {
                assert_eq!(coverage(asset, 0, 0, size), 0);
                assert_eq!(coverage(asset, u32::MAX, u32::MAX, size), 0);
            }
            for size in min_raster_size..=max_raster_size {
                assert_eq!(coverage(asset, size, 0, size), 0);
                assert_eq!(coverage(asset, 0, size, size), 0);
                assert_eq!(coverage(asset, u32::MAX, 0, size), 0);
            }
        }
    }

    #[test]
    fn all_supported_sizes_have_ink_and_clear_clip_edges() {
        for asset in control_assets {
            for size in min_raster_size..=max_raster_size {
                let mut ink = 0;
                for y in 0..size {
                    for x in 0..size {
                        ink += u32::from(coverage(asset, x, y, size));
                    }
                    assert_eq!(coverage(asset, 0, y, size), 0);
                    assert_eq!(coverage(asset, size - 1, y, size), 0);
                    assert_eq!(coverage(asset, y, 0, size), 0);
                    assert_eq!(coverage(asset, y, size - 1, size), 0);
                }
                assert!(ink > 255, "empty asset {:?} at {}", asset, size);
            }
        }
    }

    #[test]
    fn coverage_is_antialiased_and_geometric_interiors_remain_clear() {
        let mut partial = false;
        for y in 0..24 {
            for x in 0..24 {
                let alpha = coverage(control_asset::close, x, y, 24);
                partial |= alpha > 0 && alpha < 255;
            }
        }
        assert!(partial);
        assert_eq!(coverage(control_asset::close, 12, 12, 24), 255);
        assert_eq!(coverage(control_asset::maximize, 12, 12, 24), 0);
        assert_eq!(coverage(control_asset::minimize, 12, 15, 24), 255);
        assert_eq!(coverage(control_asset::restore, 10, 12, 24), 0);
    }

    #[test]
    fn symmetrical_assets_keep_symmetry_at_every_supported_scale() {
        for asset in [control_asset::maximize, control_asset::close] {
            for size in min_raster_size..=max_raster_size {
                for y in 0..size {
                    for x in 0..size {
                        let alpha = coverage(asset, x, y, size);
                        assert_eq!(alpha, coverage(asset, size - 1 - x, y, size));
                        assert_eq!(alpha, coverage(asset, x, size - 1 - y, size));
                    }
                }
            }
        }
    }

    #[test]
    fn recipes_are_bounded_source_vectors_with_unique_ids() {
        for (index, asset) in control_assets.iter().copied().enumerate() {
            let strokes = recipe(asset);
            assert!(!strokes.is_empty() && strokes.len() <= 7);
            for stroke in strokes {
                for coordinate in [stroke.x1, stroke.y1, stroke.x2, stroke.y2] {
                    assert!((1..source_extent as u8).contains(&coordinate));
                }
                assert_eq!(stroke.stroke_width, 2);
                assert_ne!((stroke.x1, stroke.y1), (stroke.x2, stroke.y2));
            }
            for other in &control_assets[..index] {
                assert_ne!(asset.id(), other.id());
            }
        }
    }
}
