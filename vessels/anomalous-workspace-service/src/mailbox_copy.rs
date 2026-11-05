//! Exact copies for the native workspace's normal-memory mailbox. Only the
//! mapped side is volatile; ordinary slices need no alignment guarantee.

#![deny(unsafe_op_in_unsafe_fn)]

use core::ptr;

/// # Safety
/// `destination` must admit writes of exactly `source.len()` bytes in a live
/// normal-memory mapping, disjoint from `source`. No other code may access that
/// range during this copy. This is not an MMIO or concurrent-memory primitive.
pub(crate) unsafe fn write(destination: *mut u8, source: &[u8]) {
    let mut index = 0;
    while index < source.len() && (destination as usize + index) & 7 != 0 {
        // SAFETY: The caller supplies the whole range; index is inside it.
        unsafe { ptr::write_volatile(destination.add(index), source[index]) };
        index += 1;
    }
    while source.len() - index >= 8 {
        let value = u64::from_ne_bytes(source[index..index + 8].try_into().unwrap());
        // SAFETY: The prefix aligns destination + index, and all eight bytes
        // lie inside the caller's writable range. The source read is bounded
        // by its slice and does not require u64 alignment.
        unsafe { ptr::write_volatile(destination.add(index).cast::<u64>(), value) };
        index += 8;
    }
    while index < source.len() {
        // SAFETY: The tail touches only remaining bytes, never a wider word.
        unsafe { ptr::write_volatile(destination.add(index), source[index]) };
        index += 1;
    }
}

/// # Safety
/// `source` must admit reads of exactly `destination.len()` initialized bytes in
/// a live normal-memory mapping, disjoint from `destination`. No other code may
/// modify that range during this copy. This is not an MMIO primitive.
pub(crate) unsafe fn read(source: *const u8, destination: &mut [u8]) {
    let mut index = 0;
    while index < destination.len() && (source as usize + index) & 7 != 0 {
        // SAFETY: The caller supplies the whole range; index is inside it.
        destination[index] = unsafe { ptr::read_volatile(source.add(index)) };
        index += 1;
    }
    while destination.len() - index >= 8 {
        // SAFETY: The prefix aligns source + index, and all eight bytes lie
        // inside the caller's initialized range. Slice writes remain bounded
        // and do not require destination to have u64 alignment.
        let value = unsafe { ptr::read_volatile(source.add(index).cast::<u64>()) };
        destination[index..index + 8].copy_from_slice(&value.to_ne_bytes());
        index += 8;
    }
    while index < destination.len() {
        // SAFETY: The tail touches only remaining bytes, never a wider word.
        destination[index] = unsafe { ptr::read_volatile(source.add(index)) };
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    #[allow(non_camel_case_types)]
    #[repr(align(8))]
    struct aligned_bytes([u8; 4112]);

    #[test]
    fn copies_exact_ranges_for_all_alignments_and_word_tails() {
        let mut source = aligned_bytes([0; 4112]);
        for (index, byte) in source.0.iter_mut().enumerate() {
            *byte = index.wrapping_mul(71).wrapping_add(index / 256) as u8;
        }
        let counts = [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 31, 32, 33, 39, 40, 41,
            3071, 3072, 3073, 4055, 4056, 4095, 4096,
        ];
        for source_offset in 0..8 {
            for destination_offset in 0..8 {
                for count in counts {
                    let mut expected = aligned_bytes([0xa5; 4112]);
                    expected.0[destination_offset..destination_offset + count]
                        .copy_from_slice(&source.0[source_offset..source_offset + count]);
                    let mut written = aligned_bytes([0xa5; 4112]);
                    let mut read = aligned_bytes([0xa5; 4112]);
                    // SAFETY: Each distinct stack array contains the exact range,
                    // including unaligned starts and zero-length end positions.
                    unsafe {
                        super::write(
                            written.0.as_mut_ptr().add(destination_offset),
                            &source.0[source_offset..source_offset + count],
                        );
                        super::read(
                            source.0.as_ptr().add(source_offset),
                            &mut read.0[destination_offset..destination_offset + count],
                        );
                    }
                    assert_eq!(
                        written.0, expected.0,
                        "write {source_offset}/{destination_offset}/{count}"
                    );
                    assert_eq!(
                        read.0, expected.0,
                        "read {source_offset}/{destination_offset}/{count}"
                    );
                }
            }
        }
    }

    #[test]
    fn copies_every_mailbox_length_without_touching_adjacent_bytes() {
        let source = aligned_bytes([0x71; 4112]);
        for count in 0..=4096 {
            let offset = count % 8;
            let mut written = aligned_bytes([0xa5; 4112]);
            let mut read = aligned_bytes([0xa5; 4112]);
            // SAFETY: Both arrays provide the complete disjoint bounded range.
            unsafe {
                super::write(written.0.as_mut_ptr().add(offset), &source.0[..count]);
                super::read(
                    source.0.as_ptr().add(offset),
                    &mut read.0[offset..offset + count],
                );
            }
            for result in [&written.0, &read.0] {
                assert!(result[..offset].iter().all(|byte| *byte == 0xa5));
                assert!(
                    result[offset..offset + count]
                        .iter()
                        .all(|byte| *byte == 0x71)
                );
                assert!(result[offset + count..].iter().all(|byte| *byte == 0xa5));
            }
        }
    }
}
