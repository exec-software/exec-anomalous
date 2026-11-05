//! Pure expectations for the opt-in supervised FS/eager-CPU fixture.
pub(super) const base_a: u64 = 0x800020;
pub(super) const base_b: u64 = 0x800040;
pub(super) const full_width_base: u64 = 0x7fff12345000;
pub(super) const canary_a: u64 = 0x745153aaf00d0001;
pub(super) const canary_b: u64 = 0x745153bbface0001;

pub(super) fn mapped_canary(base: u64) -> Option<(u64, u64)> {
    match base {
        base_a => Some((32, canary_a)),
        base_b => Some((64, canary_b)),
        _ => None,
    }
}

pub(super) fn fp_matches(bytes: &[u8; 512]) -> bool {
    bytes[0..2] == 0x077fu16.to_le_bytes()
        && bytes[2..4] == 0x3800u16.to_le_bytes()
        && bytes[4] == 0x80
        && bytes[24..28] == 0x3f80u32.to_le_bytes()
        && bytes[32..42] == [0, 0, 0, 0, 0, 0, 0, 0x80, 0xff, 0x3f]
        && bytes[400..408] == 0x31415926aabbccddu64.to_le_bytes()
        && bytes[408..416] == 0x27182818eeff0011u64.to_le_bytes()
}

pub(super) fn operation_arguments_valid(operation: u64, args: [u64; 6]) -> bool {
    matches!(operation, 21 | 22)
        && args[0] != 0
        && (operation != 21 || args[1] == 0)
        && args[2..].iter().all(|value| *value == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_fp_witness_covers_every_meaningful_selected_byte() {
        let mut bytes = [0; 512];
        bytes[0..2].copy_from_slice(&0x077fu16.to_le_bytes());
        bytes[2..4].copy_from_slice(&0x3800u16.to_le_bytes());
        bytes[4] = 0x80;
        bytes[24..28].copy_from_slice(&0x3f80u32.to_le_bytes());
        bytes[32..42].copy_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0x80, 0xff, 0x3f]);
        bytes[400..408].copy_from_slice(&0x31415926aabbccddu64.to_le_bytes());
        bytes[408..416].copy_from_slice(&0x27182818eeff0011u64.to_le_bytes());
        assert!(fp_matches(&bytes));
        for index in (0..5).chain(24..28).chain(32..42).chain(400..416) {
            for bit in 0..8 {
                let mut changed = bytes;
                changed[index] ^= 1 << bit;
                assert!(!fp_matches(&changed));
            }
        }
        assert_eq!(mapped_canary(base_a), Some((32, canary_a)));
        assert_eq!(mapped_canary(base_b), Some((64, canary_b)));
        assert_eq!(mapped_canary(full_width_base), None);
        assert_eq!(mapped_canary(base_a + 1), None);
    }

    #[test]
    fn query_and_stage_require_exact_reserved_arguments() {
        assert!(operation_arguments_valid(21, [1, 0, 0, 0, 0, 0]));
        assert!(operation_arguments_valid(
            22,
            [1, full_width_base, 0, 0, 0, 0]
        ));
        for operation in [21, 22] {
            assert!(!operation_arguments_valid(operation, [0; 6]));
            for index in (if operation == 21 { 1 } else { 2 })..6 {
                let mut args = [1, 0, 0, 0, 0, 0];
                args[index] = 1;
                assert!(!operation_arguments_valid(operation, args));
            }
        }
        assert!(!operation_arguments_valid(20, [1, 0, 0, 0, 0, 0]));
        assert!(!operation_arguments_valid(23, [1, 0, 0, 0, 0, 0]));
    }
}
