//! Optimization checks against the original scalar pixel equations. These
//! exercise clipping and extreme legal geometry, not only authored screenshots.
use crate::paint::{rect, rgb, rounded_coverage, row_canvas};
use crate::{frame_width, row_byte_count};

fn blend(output: &mut [u8], x: i64, color: rgb, alpha: u8) {
    if !(0..i64::from(frame_width)).contains(&x) {
        return;
    }
    for (channel, foreground) in color.into_iter().enumerate() {
        let index = x as usize * 3 + channel;
        output[index] = ((u32::from(foreground) * u32::from(alpha)
            + u32::from(output[index]) * (255 - u32::from(alpha))
            + 127)
            / 255) as u8;
    }
}

fn background() -> [u8; row_byte_count] {
    core::array::from_fn(|index| (index.wrapping_mul(73).wrapping_add(11) % 256) as u8)
}

#[test]
fn opaque_blend_fast_path_preserves_the_scalar_source_over_equation() {
    for alpha in 0..=255 {
        let mut actual = background();
        let mut expected = actual;
        for x in [-1, 0, 1, 317, 1023, 1024] {
            let color = [17, 139, 251];
            row_canvas::new(0, &mut actual).blend_pixel(x, color, alpha);
            blend(&mut expected, x, color, alpha);
        }
        assert_eq!(actual, expected, "alpha {alpha}");
    }
}

#[test]
fn opaque_frame_spans_match_scalar_coverage_for_clipped_and_degenerate_geometry() {
    for bounds in [
        rect::new(-8, 7, 24, 20),
        rect::new(12, 7, 1, 1),
        rect::new(12, 7, 3, 9),
        rect::new(12, 7, 24, 20),
        rect::new(1015, 7, 24, 20),
        rect::new(-50, 7, 10, 20),
        rect::new(1050, 7, 10, 20),
        rect::new(12, 7, 0, 20),
        rect::new(12, 7, 24, -1),
    ] {
        for radius in [-1, 0, 1, 2, 6, 12, 40] {
            for width in [-1, 0, 1, 2, 4, 12, 40] {
                for row in 5..=28 {
                    let mut actual = background();
                    let mut expected = actual;
                    let fill = [241, 13, 97];
                    let border = [7, 191, 43];
                    row_canvas::new(row, &mut actual)
                        .rounded_frame(bounds, radius, width, fill, border);
                    if bounds.width > 0 && bounds.height > 0 {
                        let width = width.max(0).min(bounds.width.min(bounds.height) / 2);
                        let inner = bounds.inset(width);
                        for x in
                            bounds.x.max(0)..(bounds.x + bounds.width).min(i64::from(frame_width))
                        {
                            let outer =
                                u32::from(rounded_coverage(bounds, radius, x, i64::from(row)));
                            if outer == 0 {
                                continue;
                            }
                            let inside = u32::from(rounded_coverage(
                                inner,
                                (radius - width).max(0),
                                x,
                                i64::from(row),
                            ))
                            .min(outer);
                            let color = core::array::from_fn(|channel| {
                                ((u32::from(fill[channel]) * inside
                                    + u32::from(border[channel]) * (outer - inside)
                                    + outer / 2)
                                    / outer) as u8
                            });
                            blend(&mut expected, x, color, outer as u8);
                        }
                    }
                    assert_eq!(
                        actual, expected,
                        "x={} size={}x{} radius={radius} width={width} row={row}",
                        bounds.x, bounds.width, bounds.height
                    );
                }
            }
        }
    }
}

#[test]
fn off_row_outline_and_shadow_guards_preserve_visible_pixels() {
    let bounds = rect::new(10, 30, 20, 16);
    for radius in [0, 1, 6, 12] {
        for row in 0..70 {
            let color = [17, 139, 251];
            let mut actual = background();
            let mut expected = actual;
            row_canvas::new(row, &mut actual).rounded_outline(bounds, radius, 2, color);
            for x in 0..i64::from(frame_width) {
                let outer = rounded_coverage(bounds, radius, x, i64::from(row));
                let inner =
                    rounded_coverage(bounds.inset(2), (radius - 2).max(0), x, i64::from(row));
                blend(&mut expected, x, color, outer.saturating_sub(inner));
            }
            assert_eq!(actual, expected);
            for spread in [0, 1, 6, 16] {
                for offset in [0, 5, 12] {
                    let mut actual = background();
                    let mut expected = actual;
                    row_canvas::new(row, &mut actual)
                        .soft_shadow(bounds, radius, spread, 103, color, offset);
                    let shifted = bounds.translated(0, offset);
                    for x in 0..i64::from(frame_width) {
                        if spread == 0 {
                            continue;
                        }
                        // The shifted base shadow only needs painting outside
                        // the original shape's certainly opaque core.
                        let inset = if i64::from(row) < bounds.y + radius.min(8)
                            || i64::from(row) >= bounds.y + bounds.height - radius.min(8)
                        {
                            radius.min(8)
                        } else {
                            0
                        };
                        let inside_core = i64::from(row) >= bounds.y
                            && i64::from(row) < bounds.y + bounds.height
                            && x >= bounds.x + inset
                            && x < bounds.x + bounds.width - inset;
                        if !inside_core && x >= shifted.x && x < shifted.x + shifted.width {
                            let coverage = rounded_coverage(shifted, radius, x, i64::from(row));
                            blend(
                                &mut expected,
                                x,
                                color,
                                (103 * u16::from(coverage) / 255) as u8,
                            );
                        }
                        for distance in (1..=i64::from(spread)).rev() {
                            let outer = rounded_coverage(
                                shifted.inset(-distance),
                                radius + distance,
                                x,
                                i64::from(row),
                            );
                            let inner = rounded_coverage(
                                shifted.inset(1 - distance),
                                radius + distance - 1,
                                x,
                                i64::from(row),
                            );
                            let weight = i64::from(spread) + 1 - distance;
                            let alpha = 103 * weight * weight / i64::from(spread).pow(2);
                            blend(
                                &mut expected,
                                x,
                                color,
                                (alpha * i64::from(outer.saturating_sub(inner)) / 255) as u8,
                            );
                        }
                    }
                    assert_eq!(
                        actual, expected,
                        "radius={radius} row={row} spread={spread} offset={offset}"
                    );
                }
            }
        }
    }
}
