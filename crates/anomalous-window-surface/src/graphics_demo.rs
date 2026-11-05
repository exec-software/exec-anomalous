use crate::canvas;

pub const demo_width: u32 = 96;
pub const demo_height: u32 = 64;
pub const demo_stride: u32 = demo_width * 3;
pub const demo_byte_count: usize = (demo_stride * demo_height) as usize;

pub const fn graphics_demo_label(scene: u8) -> &'static str {
    if scene & 1 == 0 {
        "Public RGB gradient with orange and teal geometry"
    } else {
        "Public chart with five bars and a rising line"
    }
}

/// Draw either of two original public raster scenes. The nominal content size
/// is 96 by 64; other canvas sizes are safe and clipped. This has no timer,
/// input, allocation, foreign ABI, or publication side effects.
pub fn draw_graphics_demo(target: &mut canvas<'_>, scene: u8) {
    if scene & 1 == 0 {
        let width = target.width();
        let height = target.height();
        for y in 0..height {
            for x in 0..width {
                let color = [
                    (24 + x * 128 / width) as u8,
                    (32 + y * 128 / height) as u8,
                    (72 + (width - x) * 64 / width) as u8,
                ];
                target.fill_rect(x as i32, y as i32, 1, 1, color);
            }
        }
        target.fill_rect(10, 12, 31, 35, [245, 141, 54]);
        target.fill_rect(18, 20, 15, 19, [42, 41, 48]);
        target.fill_rect(54, 25, 29, 24, [69, 209, 188]);
        target.line(46, 12, 87, 52, [252, 238, 212]);
        target.line(46, 52, 87, 12, [252, 238, 212]);
        target.line(8, 56, 87, 56, [252, 238, 212]);
    } else {
        target.clear([22, 29, 37]);
        for y in [12, 24, 36, 48] {
            target.line(9, y, 88, y, [48, 60, 72]);
        }
        target.line(9, 7, 9, 55, [191, 202, 212]);
        target.line(9, 55, 88, 55, [191, 202, 212]);
        for (x, height) in [(17, 14), (31, 23), (45, 19), (59, 33), (73, 42)] {
            target.fill_rect(x, 54 - height, 9, height as u32, [64, 192, 170]);
        }
        let points = [(17, 40), (35, 32), (49, 35), (63, 22), (82, 11)];
        for pair in points.windows(2) {
            target.line(pair[0].0, pair[0].1, pair[1].0, pair[1].1, [250, 151, 61]);
        }
        for (x, y) in points {
            target.fill_rect(x - 1, y - 1, 3, 3, [253, 227, 190]);
        }
    }
}
