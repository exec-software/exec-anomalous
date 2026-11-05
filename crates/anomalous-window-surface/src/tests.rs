use super::*;
use std::vec;

#[test]
fn frame_exposes_visible_rgb_rows_and_owned_label_metadata() {
    let pixels = [1, 2, 3, 4, 5, 6, 199, 199, 7, 8, 9, 10, 11, 12, 199, 199];
    let frame = frame::try_new(2, 2, 8, "Public image", &pixels).unwrap();
    let descriptor = frame.descriptor();
    assert_ne!(descriptor.id(), 0);
    assert_eq!(
        (descriptor.width(), descriptor.height(), descriptor.stride()),
        (2, 2, 8)
    );
    assert_eq!(descriptor.label(), "Public image");
    assert_eq!(frame.row(0), Some(&pixels[..6]));
    assert_eq!(frame.row(1), Some(&pixels[8..14]));
    assert_eq!(frame.row(2), None);
    assert_eq!(frame.row(u32::MAX), None);
    assert_eq!(frame.descriptor(), descriptor);
}

#[test]
fn owned_frame_retains_pixels_and_identity_across_borrows_and_moves() {
    let mut pixels = [1, 2, 3, 4, 5, 6, 199, 199, 7, 8, 9, 10, 11, 12, 199, 199];
    let original = pixels;
    let owner = owned_frame::try_new(2, 2, 8, "Owned public image", pixels).unwrap();
    pixels.fill(77);
    let descriptor = owner.descriptor();
    let first = owner.as_frame();
    let second = owner.as_frame();
    assert_eq!(first.descriptor(), descriptor);
    assert_eq!(second.descriptor(), descriptor);
    assert_eq!(first.row(0), Some(&original[..6]));
    assert_eq!(second.row(1), Some(&original[8..14]));
    assert_eq!(second.row(2), None);
    assert_eq!(second.row(u32::MAX), None);
    let moved = owner;
    assert_eq!(moved.descriptor(), descriptor);
    assert_eq!(moved.as_frame().descriptor(), descriptor);
    assert_eq!(moved.into_pixels(), original);
}

#[test]
fn reclaiming_and_reusing_owned_pixels_requires_a_new_content_identity() {
    let owner = owned_frame::try_new(1, 1, 4, "Public image", [1, 2, 3, 199]).unwrap();
    let descriptor = owner.descriptor();
    let pixels = owner.into_pixels();
    let replacement = owned_frame::try_new(1, 1, 4, "Public image", pixels).unwrap();
    assert_ne!(replacement.descriptor().id(), descriptor.id());
    assert_eq!(replacement.as_frame().row(0), Some(&pixels[..3]));
    assert_eq!(replacement.into_pixels(), pixels);
}

#[test]
fn owned_frame_uses_the_same_dimension_stride_length_and_label_boundaries() {
    for (width, height) in [(0, 1), (1, 0), (1025, 1), (1, 769), (u32::MAX, u32::MAX)] {
        assert_eq!(
            owned_frame::try_new(width, height, 3, "Sample", []).err(),
            Some(surface_error::invalid_dimensions)
        );
    }
    for stride in [0, 1, 2, 4097, u32::MAX] {
        assert_eq!(
            owned_frame::try_new(1, 1, stride, "Sample", []).err(),
            Some(surface_error::invalid_stride)
        );
    }
    for error in [
        owned_frame::try_new(1, 1, 3, "Sample", []).err(),
        owned_frame::try_new(1, 1, 3, "Sample", [0; 2]).err(),
        owned_frame::try_new(1, 1, 3, "Sample", [0; 4]).err(),
        owned_frame::try_new(1, 2, 4, "Sample", [0; 7]).err(),
    ] {
        assert_eq!(error, Some(surface_error::invalid_length));
    }
    for label in ["", "line\nbreak", "nul\0", "delete\u{7f}", "café"] {
        assert_eq!(
            owned_frame::try_new(1, 1, 3, label, [0; 3]).err(),
            Some(surface_error::invalid_label)
        );
    }
    let too_long = "a".repeat(maximum_label_bytes + 1);
    assert_eq!(
        owned_frame::try_new(1, 1, 3, &too_long, [0; 3]).err(),
        Some(surface_error::invalid_label)
    );
}

#[test]
fn owned_frame_admits_maximum_axes_and_retains_its_label() {
    let label = "a".repeat(maximum_label_bytes);
    let wide = owned_frame::try_new(
        maximum_width,
        1,
        maximum_stride,
        &label,
        [99; maximum_stride as usize],
    )
    .unwrap();
    drop(label);
    assert_eq!(wide.descriptor().label().len(), maximum_label_bytes);
    assert!(wide.descriptor().label().bytes().all(|byte| byte == b'a'));
    assert_eq!(
        wide.as_frame().row(0).unwrap().len(),
        maximum_width as usize * 3
    );
    assert_eq!(wide.as_frame().row(1), None);
    let tall = owned_frame::try_new(
        1,
        maximum_height,
        3,
        "Tall",
        [88; maximum_height as usize * 3],
    )
    .unwrap();
    assert_eq!(tall.as_frame().row(maximum_height - 1), Some(&[88; 3][..]));
    assert_eq!(tall.as_frame().row(maximum_height), None);
}

#[test]
fn malformed_dimensions_stride_and_lengths_are_rejected() {
    for (width, height) in [(0, 1), (1, 0), (1025, 1), (1, 769), (u32::MAX, u32::MAX)] {
        assert_eq!(
            frame::try_new(width, height, 3, "Sample", &[]).err(),
            Some(surface_error::invalid_dimensions)
        );
    }
    for stride in [0, 1, 2, 4097, u32::MAX] {
        assert_eq!(
            frame::try_new(1, 1, stride, "Sample", &[]).err(),
            Some(surface_error::invalid_stride)
        );
    }
    for length in [0, 1, 2, 4, 100] {
        let bytes = vec![0; length];
        assert_eq!(
            frame::try_new(1, 1, 3, "Sample", &bytes).err(),
            Some(surface_error::invalid_length)
        );
    }
    // A final row's padding is part of the admitted storage contract too.
    assert_eq!(
        frame::try_new(1, 2, 4, "Sample", &[0; 7]).err(),
        Some(surface_error::invalid_length)
    );
}

#[test]
fn maximum_geometry_is_admitted_without_overflow() {
    let pixels = vec![0; maximum_stride as usize * maximum_height as usize];
    let frame = frame::try_new(
        maximum_width,
        maximum_height,
        maximum_stride,
        "Maximum",
        &pixels,
    )
    .unwrap();
    assert_eq!(
        frame.row(maximum_height - 1).unwrap().len(),
        maximum_width as usize * 3
    );
}

#[test]
fn only_bounded_printable_ascii_labels_are_admitted() {
    for label in [
        "",
        "line\nbreak",
        "tab\there",
        "nul\0",
        "delete\u{7f}",
        "café",
        "图形",
    ] {
        assert_eq!(
            frame::try_new(1, 1, 3, label, &[0; 3]).err(),
            Some(surface_error::invalid_label)
        );
    }
    let limit = "a".repeat(maximum_label_bytes);
    let frame = frame::try_new(1, 1, 3, &limit, &[0; 3]).unwrap();
    assert_eq!(frame.descriptor().label(), limit);
    let too_long = "a".repeat(maximum_label_bytes + 1);
    assert_eq!(
        super::frame::try_new(1, 1, 3, &too_long, &[0; 3]).err(),
        Some(surface_error::invalid_label)
    );
}

#[test]
fn frame_identities_distinguish_equal_pixels_and_do_not_wrap() {
    let pixels = [0; 3];
    let first = frame::try_new(1, 1, 3, "Same", &pixels).unwrap();
    let second = frame::try_new(1, 1, 3, "Same", &pixels).unwrap();
    assert_ne!(first.descriptor(), second.descriptor());
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(mint_identity(&counter), Ok(u64::MAX - 1));
    assert_eq!(mint_identity(&counter), Ok(u64::MAX));
    assert_eq!(
        mint_identity(&counter),
        Err(surface_error::identity_exhausted)
    );
    assert_eq!(
        mint_identity(&counter),
        Err(surface_error::identity_exhausted)
    );
    assert_eq!(counter.load(Ordering::Relaxed), 0);
}

#[test]
fn identity_minting_is_unique_under_concurrent_borrowers() {
    let counter = AtomicU64::new(1);
    std::thread::scope(|scope| {
        let threads: std::vec::Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    (0..64)
                        .map(|_| mint_identity(&counter).unwrap())
                        .collect::<std::vec::Vec<_>>()
                })
            })
            .collect();
        let mut ids = threads
            .into_iter()
            .flat_map(|thread| thread.join().unwrap())
            .collect::<std::vec::Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, (1..=512).collect::<std::vec::Vec<_>>());
    });
}

#[test]
fn rejected_canvas_preserves_caller_bytes() {
    let mut bytes = [99; 10];
    assert!(canvas::try_new(2, 2, 6, &mut bytes).is_err());
    assert_eq!(bytes, [99; 10]);
}

#[test]
fn clear_and_rectangles_preserve_padding_and_outer_canaries() {
    let mut bytes = [199; 38];
    {
        let mut target = canvas::try_new(3, 3, 12, &mut bytes[1..37]).unwrap();
        target.clear([1, 2, 3]);
        target.fill_rect(-1, 1, 3, 4, [4, 5, 6]);
        target.fill_rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX, [99; 3]);
        target.fill_rect(-10, -10, 0, 0, [99; 3]);
    }
    assert_eq!(bytes[0], 199);
    assert_eq!(bytes[37], 199);
    for row in 0..3 {
        for column in 0..3 {
            let start = 1 + row * 12 + column * 3;
            assert_eq!(
                &bytes[start..start + 3],
                if row > 0 && column < 2 {
                    &[4, 5, 6]
                } else {
                    &[1, 2, 3]
                }
            );
        }
        assert_eq!(&bytes[1 + row * 12 + 9..1 + row * 12 + 12], &[199; 3]);
    }
}

#[test]
fn extreme_rectangle_can_cover_the_canvas_without_wrapping() {
    let mut bytes = [0; 12];
    let mut target = canvas::try_new(2, 2, 6, &mut bytes).unwrap();
    target.fill_rect(i32::MIN, i32::MIN, u32::MAX, u32::MAX, [1, 2, 3]);
    assert_eq!(bytes, [1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3]);
}

#[test]
fn clipped_lines_include_visible_endpoints_and_preserve_padding() {
    let mut bytes = [199; 3 * 12];
    {
        let mut target = canvas::try_new(3, 3, 12, &mut bytes).unwrap();
        target.clear([0; 3]);
        target.line(i32::MIN, 1, i32::MAX, 1, [1, 2, 3]);
        target.line(1, i32::MIN, 1, i32::MAX, [4, 5, 6]);
        target.line(i32::MIN, i32::MIN, i32::MAX, i32::MAX, [7, 8, 9]);
        target.line(2, 0, 2, 0, [10, 11, 12]);
        target.line(i32::MIN, -1, i32::MAX, -1, [99; 3]);
    }
    let expected = [
        [[7, 8, 9], [4, 5, 6], [10, 11, 12]],
        [[1, 2, 3], [7, 8, 9], [1, 2, 3]],
        [[0, 0, 0], [4, 5, 6], [7, 8, 9]],
    ];
    for row in 0..3 {
        for column in 0..3 {
            assert_eq!(
                &bytes[row * 12 + column * 3..row * 12 + column * 3 + 3],
                &expected[row][column]
            );
        }
        assert_eq!(&bytes[row * 12 + 9..row * 12 + 12], &[199; 3]);
    }
}

#[test]
fn extreme_line_permutations_remain_bounded_and_in_the_active_area() {
    let coordinates = [i32::MIN, -9, -1, 0, 1, 2, 5, 9, i32::MAX];
    let mut bytes = [199; 35];
    for x0 in coordinates {
        for y0 in coordinates {
            for x1 in coordinates {
                for y1 in coordinates {
                    {
                        let mut target = canvas::try_new(3, 3, 11, &mut bytes[1..34]).unwrap();
                        target.line(x0, y0, x1, y1, [1, 2, 3]);
                    }
                    assert_eq!(bytes[0], 199);
                    assert_eq!(bytes[34], 199);
                    for row in 0..3 {
                        assert_eq!(&bytes[1 + row * 11 + 9..1 + row * 11 + 11], &[199; 2]);
                    }
                }
            }
        }
    }
}

#[test]
fn graphics_scenes_are_deterministic_distinct_and_publicly_described() {
    let mut first = [0; demo_byte_count];
    let mut repeat = [99; demo_byte_count];
    let mut second = [0; demo_byte_count];
    for (bytes, scene) in [(&mut first, 0), (&mut repeat, 2), (&mut second, 1)] {
        let mut target = canvas::try_new(demo_width, demo_height, demo_stride, bytes).unwrap();
        draw_graphics_demo(&mut target, scene);
    }
    assert_eq!(first, repeat);
    assert_ne!(first, second);
    assert!(graphics_demo_label(0).starts_with("Public "));
    assert!(graphics_demo_label(1).starts_with("Public "));
    for scene in [0, 1] {
        let mut tiny = [199; 5];
        let mut target = canvas::try_new(1, 1, 5, &mut tiny).unwrap();
        draw_graphics_demo(&mut target, scene);
        assert_eq!(&tiny[3..], &[199; 2]);
    }
}
