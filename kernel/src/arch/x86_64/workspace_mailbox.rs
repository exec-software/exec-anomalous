//! The persistent workspace copies only its admitted mailbox body into owned
//! scratch. Word accesses reduce transfer cost without widening that range.

use core::ptr;

/// # Safety
/// `mailbox` must name a live, initialized 4096-byte normal-memory mailbox,
/// disjoint from `scratch`. Its sole user owner must remain stopped throughout
/// the copy. Wider volatile accesses are valid for this mapping, not MMIO.
pub(super) unsafe fn copy_body(
    mailbox: *const u8,
    size: usize,
    scratch: &mut [u8; 4096],
) -> Result<(), &'static str> {
    if size > 4056 {
        return Err("workspace_body_bounds");
    }
    scratch.fill(0);
    // SAFETY: The caller provides the complete mailbox; its 40-byte header is
    // skipped and the checked body size leaves every access within the page.
    let source = unsafe { mailbox.add(40) };
    let mut index = 0;
    while index < size && (source as usize + index) & 7 != 0 {
        // SAFETY: The prefix touches only admitted bytes before word alignment.
        scratch[index] = unsafe { ptr::read_volatile(source.add(index)) };
        index += 1;
    }
    while size - index >= 8 {
        // SAFETY: The prefix aligns source + index, and all eight bytes are
        // admitted. The destination slice needs no u64 alignment guarantee.
        let value = unsafe { ptr::read_volatile(source.add(index).cast::<u64>()) };
        scratch[index..index + 8].copy_from_slice(&value.to_ne_bytes());
        index += 8;
    }
    while index < size {
        // SAFETY: The byte tail never reads beyond the admitted body length.
        scratch[index] = unsafe { ptr::read_volatile(source.add(index)) };
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::copy_body;

    #[repr(align(8))]
    struct aligned_mailbox([u8; 4104]);

    #[test]
    fn copies_only_the_body_for_all_alignments_and_word_tails() {
        let mut mailbox = aligned_mailbox([0; 4104]);
        for (index, byte) in mailbox.0.iter_mut().enumerate() {
            *byte = index.wrapping_mul(71).wrapping_add(index / 256) as u8;
        }
        for offset in 0..8 {
            for size in 0..=4056 {
                let mut scratch = [0xa5; 4096];
                // SAFETY: The array contains an initialized complete mailbox
                // at every tested offset, separate from writable scratch.
                assert_eq!(
                    unsafe { copy_body(mailbox.0.as_ptr().add(offset), size, &mut scratch) },
                    Ok(())
                );
                assert_eq!(scratch[..size], mailbox.0[offset + 40..offset + 40 + size]);
                assert!(scratch[size..].iter().all(|byte| *byte == 0));
            }
        }
    }

    #[test]
    fn invalid_size_preserves_scratch_and_zero_body_clears_it() {
        let mailbox = aligned_mailbox([0x71; 4104]);
        for size in [4057, 4096, usize::MAX] {
            let mut scratch = [0xa5; 4096];
            // SAFETY: The complete source mailbox is live and disjoint.
            assert_eq!(
                unsafe { copy_body(mailbox.0.as_ptr(), size, &mut scratch) },
                Err("workspace_body_bounds")
            );
            assert_eq!(scratch, [0xa5; 4096]);
        }
        let mut scratch = [0xa5; 4096];
        // SAFETY: As above; an admitted zero-byte body still clears scratch.
        assert_eq!(
            unsafe { copy_body(mailbox.0.as_ptr(), 0, &mut scratch) },
            Ok(())
        );
        assert_eq!(scratch, [0; 4096]);
    }
}
