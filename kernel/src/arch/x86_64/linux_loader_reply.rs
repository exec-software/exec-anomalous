#![forbid(unsafe_code)]
#![cfg_attr(not(test), allow(dead_code))]

//! Pure caller-copy shape checks and saved-frame helpers.
//! The runtime separately authenticates the task, saved frames and sealed call
//! authority. Result bits carry no Linux interpretation or mapping authority.
use super::scheduler_test::scheduler_frame;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum reply_error {
    sequence,
    reserved,
    length,
}

/// Native caller-read operation budget, not a pathname or image-size limit.
pub(super) const caller_read_capacity: usize = 4096;

/// Inert op18 arguments. Exact current veil and sealed pending-call authority
/// must be authenticated separately before either checked copy. These addresses
/// are untrusted integers, including when this shape validation succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct caller_read {
    pub(super) sequence: u64,
    pub(super) source: u64,
    pub(super) destination: u64,
    pub(super) length: usize,
}

impl caller_read {
    pub(super) fn decode(
        current_sequence: u64,
        value: scheduler_frame,
    ) -> Result<Self, reply_error> {
        if current_sequence == 0 || value.destination_index != current_sequence {
            return Err(reply_error::sequence);
        }
        // Unlike completion, op18 uses R10 as its byte count, not a stage token.
        if value.register_8 != 0 || value.register_9 != 0 {
            return Err(reply_error::reserved);
        }
        if !(1..=caller_read_capacity as u64).contains(&value.register_10) {
            return Err(reply_error::length);
        }
        Ok(Self {
            sequence: current_sequence,
            source: value.source_index,
            destination: value.data,
            length: value.register_10 as usize,
        })
    }
}

/// Explicit normalized assembly-frame order; no pointer cast or padding read.
pub(super) fn frame_words(value: scheduler_frame) -> [u64; 22] {
    [
        value.accumulator,
        value.base,
        value.counter,
        value.data,
        value.frame_pointer,
        value.source_index,
        value.destination_index,
        value.register_8,
        value.register_9,
        value.register_10,
        value.register_11,
        value.register_12,
        value.register_13,
        value.register_14,
        value.register_15,
        value.vector,
        value.error_code,
        value.instruction_pointer,
        value.code_segment,
        value.flags,
        value.stack_pointer,
        value.stack_segment,
    ]
}

/// Replace only the result register after separate retained-frame admission.
pub(super) fn returned_frame(mut before: scheduler_frame, result: u64) -> scheduler_frame {
    before.accumulator = result;
    before
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distinct_frame() -> scheduler_frame {
        scheduler_frame {
            accumulator: 0x8000,
            base: 0x8001,
            counter: 0x8002,
            data: 0x8003,
            frame_pointer: 0x8004,
            source_index: 0x8005,
            destination_index: 0x8006,
            register_8: 0x8007,
            register_9: 0x8008,
            register_10: 0x8009,
            register_11: 0x800a,
            register_12: 0x800b,
            register_13: 0x800c,
            register_14: 0x800d,
            register_15: 0x800e,
            vector: 0x800f,
            error_code: 0x8010,
            instruction_pointer: 0x8011,
            code_segment: 0x8012,
            flags: 0x8013,
            stack_pointer: 0x8014,
            stack_segment: 0x8015,
        }
    }

    fn reply(sequence: u64, action: u64, result: u64) -> scheduler_frame {
        let mut value = distinct_frame();
        value.destination_index = sequence;
        value.source_index = result;
        value.data = action;
        value.register_8 = 0;
        value.register_9 = 0;
        value.register_10 = 0;
        value
    }

    #[test]
    fn every_saved_word_is_ordered_and_only_the_result_changes() {
        let before = distinct_frame();
        let expected = core::array::from_fn(|index| 0x8000 + index as u64);
        assert_eq!(frame_words(before), expected);
        for result in [0, 1, 0x8000, 0x8000_0000_0000_0000, u64::MAX] {
            let after = frame_words(returned_frame(before, result));
            assert_eq!(after[0], result);
            assert_eq!(after[1..], expected[1..]);
            assert_eq!(frame_words(before), expected);
        }
    }

    fn read_request(sequence: u64, source: u64, destination: u64, length: u64) -> scheduler_frame {
        let mut value = reply(sequence, destination, source);
        value.accumulator = 18;
        value.register_10 = length;
        value
    }

    #[test]
    fn caller_read_accepts_exact_budget_without_conferring_pointer_authority() {
        for length in [1, 255, 256, 257, 4095, 4096] {
            for (source, destination) in [(0, 0), (1, 2), (u64::MAX, u64::MAX)] {
                let value = read_request(u64::MAX, source, destination, length);
                let before = frame_words(value);
                assert_eq!(
                    caller_read::decode(u64::MAX, value),
                    Ok(caller_read {
                        sequence: u64::MAX,
                        source,
                        destination,
                        length: length as usize,
                    })
                );
                assert_eq!(frame_words(value), before);
            }
        }
    }

    #[test]
    fn caller_read_rejects_stale_zero_and_wrong_sequence_without_touching_frame() {
        for (current, supplied) in [
            (0, 0),
            (0, 1),
            (2, 0),
            (2, 1),
            (3, 2),
            (2, 3),
            (4, 3),
            (3, 4),
        ] {
            let value = read_request(supplied, 0x1000, 0x2000, 1);
            let before = frame_words(value);
            assert_eq!(
                caller_read::decode(current, value),
                Err(reply_error::sequence)
            );
            assert_eq!(frame_words(value), before);
        }
    }

    #[test]
    fn caller_read_rejects_zero_oversized_and_each_reserved_argument() {
        for length in [0, 4097, 1 << 32, u64::MAX] {
            let value = read_request(2, 0x1000, 0x2000, length);
            let before = frame_words(value);
            assert_eq!(caller_read::decode(2, value), Err(reply_error::length));
            assert_eq!(frame_words(value), before);
        }
        for register in [8, 9] {
            for bad in [1, u64::MAX] {
                let mut value = read_request(2, 0x1000, 0x2000, 4096);
                if register == 8 {
                    value.register_8 = bad;
                } else {
                    value.register_9 = bad;
                }
                let before = frame_words(value);
                assert_eq!(caller_read::decode(2, value), Err(reply_error::reserved));
                assert_eq!(frame_words(value), before);
            }
        }
    }
}
