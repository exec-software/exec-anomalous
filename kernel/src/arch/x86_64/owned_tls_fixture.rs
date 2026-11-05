#![cfg_attr(not(test), allow(dead_code))]

//! Pure expectations shared by the native FS proof and hostile host tests.
//! These values describe synthetic fixtures and carry no task authority.

const stack_base: u64 = 0x7fff00000000;
const canary_a: u64 = 0x745153aaf00d0001;
const canary_b: u64 = 0x745153bbface0001;

pub(super) const fn base_offset(index: usize) -> u64 {
    0x100 + index as u64 * 0x1000
}
pub(super) const fn base(index: usize) -> u64 {
    stack_base + base_offset(index)
}

pub(super) fn request_case(index: usize, step: u8) -> Option<(u64, u64, u64)> {
    if index >= 2 {
        return None;
    }
    Some(match step {
        0 | 7 => (13, 0, 0),
        1 => (12, 0x0000800000000000, 1),
        2 => (12, 0xffff800000001000, 1),
        3 | 8 => (12, base(index), 0),
        4 | 9 => (13, 0, base(index)),
        5 => (12, base(index) + 0x400, 0),
        6 => (12, 0, 0),
        _ => return None,
    })
}

pub(super) fn memory_matches(
    index: usize,
    step: u8,
    progress: u64,
    observed: [Option<u64>; 4],
) -> bool {
    let expected = [
        if step >= 4 {
            canary_a + index as u64
        } else {
            0
        },
        if step >= 4 { progress } else { 0 },
        if step >= 6 {
            canary_b + index as u64
        } else {
            0
        },
        u64::from(step >= 6),
    ];
    observed == expected.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_initial_and_live_tls_word_is_checked() {
        for index in 0..2 {
            for (step, expected) in [
                (0, [0, 0, 0, 0]),
                (3, [0, 0, 0, 0]),
                (4, [canary_a + index as u64, 7, 0, 0]),
                (5, [canary_a + index as u64, 7, 0, 0]),
                (6, [canary_a + index as u64, 7, canary_b + index as u64, 1]),
                (10, [canary_a + index as u64, 7, canary_b + index as u64, 1]),
            ] {
                assert!(memory_matches(index, step, 7, expected.map(Some)));
                for word in 0..4 {
                    let mut changed = expected.map(Some);
                    changed[word] = Some(expected[word] ^ 1);
                    assert!(!memory_matches(index, step, 7, changed));
                    changed[word] = None;
                    assert!(!memory_matches(index, step, 7, changed));
                }
            }
        }
    }

    #[test]
    fn exact_sequence_has_distinct_full_width_bases_and_zero_transitions() {
        for index in 0..2 {
            assert!(base(index) > u64::from(u32::MAX));
            assert!(base_offset(index) + 0x410 < 16 * 4096 - 256);
            let cases: [_; 10] =
                core::array::from_fn(|step| request_case(index, step as u8).unwrap());
            assert_eq!(cases.iter().filter(|(op, _, _)| *op == 13).count(), 4);
            assert_eq!(
                cases
                    .iter()
                    .filter(|(op, _, result)| *op == 12 && *result == 0)
                    .count(),
                4
            );
            assert_eq!(
                cases.iter().filter(|(_, _, result)| *result == 1).count(),
                2
            );
            assert_eq!(cases[6], (12, 0, 0));
            assert_eq!(cases[7], (13, 0, 0));
            assert_eq!(cases[8], (12, base(index), 0));
            assert_eq!(cases[9], (13, 0, base(index)));
            assert_eq!(request_case(index, 10), None);
        }
        assert_ne!(base(0), base(1));
        assert_eq!(request_case(2, 0), None);
    }
}
