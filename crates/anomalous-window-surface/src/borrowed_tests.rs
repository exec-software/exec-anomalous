use super::*;

#[test]
fn borrowed_storage_keeps_pointer_identity_and_immutable_descriptor_until_reclaimed() {
    let mut pixels = [1, 2, 3, 99, 4, 5, 6, 98];
    let pointer = pixels.as_ptr();
    let owner = borrowed_frame::try_new(1, 2, 4, "Retained pixels", &mut pixels)
        .ok()
        .unwrap();
    let descriptor = owner.descriptor();
    assert_eq!(owner.as_frame().row(0).unwrap(), &[1, 2, 3]);
    assert_eq!(owner.as_frame().row(1).unwrap(), &[4, 5, 6]);
    assert_eq!(owner.as_frame().row(0).unwrap().as_ptr(), pointer);
    assert_eq!(owner.as_frame().descriptor(), descriptor);
    assert!(owner.as_frame().row(2).is_none());
    let moved = owner;
    assert_eq!(moved.as_frame().descriptor(), descriptor);
    let pixels = moved.into_pixels();
    assert_eq!(pixels.as_ptr(), pointer);
    assert_eq!(*pixels, [1, 2, 3, 99, 4, 5, 6, 98]);
    pixels[0] = 7;
    let reused = borrowed_frame::try_new(1, 2, 4, "Retained pixels", pixels)
        .ok()
        .unwrap();
    assert_ne!(reused.descriptor().id(), descriptor.id());
    assert_eq!(reused.as_frame().row(0).unwrap(), &[7, 2, 3]);
}

#[test]
fn malformed_storage_is_returned_unchanged_before_content_identity_is_used() {
    for (width, height, stride, label, expected) in [
        (0, 1, 3, "Public", surface_error::invalid_dimensions),
        (2, 1, 3, "Public", surface_error::invalid_stride),
        (1, 2, 3, "Public", surface_error::invalid_length),
        (1, 1, 3, "", surface_error::invalid_label),
        (1, 1, 3, "bad\nlabel", surface_error::invalid_label),
    ] {
        let mut pixels = [11, 22, 33];
        let pointer = pixels.as_ptr();
        let rejected = borrowed_frame::try_new(width, height, stride, label, &mut pixels)
            .err()
            .unwrap();
        assert_eq!(rejected.error(), expected);
        let pixels = rejected.into_pixels();
        assert_eq!(pixels.as_ptr(), pointer);
        assert_eq!(*pixels, [11, 22, 33]);
    }
}

#[test]
fn retained_handle_size_is_independent_of_pixel_allocation_size() {
    use core::mem::size_of;
    assert_eq!(
        size_of::<borrowed_frame<'_, 3>>(),
        size_of::<borrowed_frame<'_, 18432>>()
    );
    assert!(size_of::<borrowed_frame<'_, 18432>>() <= 128);
    assert!(size_of::<rejected_borrowed_frame<'_, 18432>>() <= 32);
}
