//! Inert requests for one retained source-mapping lifetime.
//!
//! Retain uses the six geometry words decoded by `source_mapping_request`.
//! Its successful native reply contains a nonzero generation and service
//! incarnation. Later requests repeat both words and the pending invocation
//! sequence. These observations cannot construct authority: every operation
//! must authenticate the genuine owner, caller, source and current phase.
//! Publication and release report populated/virtual page counts; inspection
//! reports source byte count and digest. None commits a foreign caller reply.
//! Operation 25 keeps its separate synchronous prepare/cancel contract.

use crate::native_status;

pub const source_mapping_retain_operation: u64 = 26;
pub const source_mapping_publish_operation: u64 = 27;
pub const source_mapping_inspect_operation: u64 = 28;
pub const source_mapping_release_operation: u64 = 29;

/// Correlation metadata for a retained kernel-selected slot, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_mapping_handle {
    generation: u64,
    incarnation: u64,
}

impl source_mapping_handle {
    pub fn new(generation: u64, incarnation: u64) -> Result<Self, native_status> {
        if generation == 0 || incarnation == 0 {
            return Err(native_status::invalid);
        }
        Ok(Self {
            generation,
            incarnation,
        })
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn incarnation(self) -> u64 {
        self.incarnation
    }

    pub const fn arguments(self, sequence: u64) -> [u64; 6] {
        [sequence, self.generation, self.incarnation, 0, 0, 0]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_mapping_control_request {
    sequence: u64,
    handle: source_mapping_handle,
}

impl source_mapping_control_request {
    /// Validate shape and correlation against an already selected live slot.
    /// This does not validate ownership or admit a lifecycle transition.
    pub fn decode(
        current_sequence: u64,
        expected_handle: source_mapping_handle,
        arguments: [u64; 6],
    ) -> Result<Self, native_status> {
        if current_sequence == 0 || arguments != expected_handle.arguments(current_sequence) {
            return Err(native_status::invalid);
        }
        Ok(Self {
            sequence: current_sequence,
            handle: expected_handle,
        })
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn handle(self) -> source_mapping_handle {
        self.handle
    }

    pub const fn arguments(self) -> [u64; 6] {
        self.handle.arguments(self.sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_handles_require_nonzero_generation_and_incarnation() {
        for (generation, incarnation) in [(0, 0), (0, 1), (1, 0)] {
            assert_eq!(
                source_mapping_handle::new(generation, incarnation),
                Err(native_status::invalid)
            );
        }
        let handle = source_mapping_handle::new(u64::MAX, u64::MAX).unwrap();
        assert_eq!(handle.generation(), u64::MAX);
        assert_eq!(handle.incarnation(), u64::MAX);
    }

    #[test]
    fn control_shape_preserves_full_sequence_generation_and_incarnation() {
        let handle = source_mapping_handle::new((1 << 32) | 7, (1 << 32) | 9).unwrap();
        let sequence = (1 << 32) | 11;
        let request =
            source_mapping_control_request::decode(sequence, handle, handle.arguments(sequence))
                .unwrap();
        assert_eq!(request.sequence(), sequence);
        assert_eq!(request.handle(), handle);
        assert_eq!(
            request.arguments(),
            [sequence, (1 << 32) | 7, (1 << 32) | 9, 0, 0, 0]
        );
        for (index, value) in [(0, 11), (1, 7), (2, 9)] {
            let mut narrowed = request.arguments();
            narrowed[index] = value;
            assert_eq!(
                source_mapping_control_request::decode(sequence, handle, narrowed),
                Err(native_status::invalid)
            );
        }
    }

    #[test]
    fn control_rejects_stale_zero_foreign_and_reserved_words() {
        let handle = source_mapping_handle::new(7, 9).unwrap();
        let valid = handle.arguments(11);
        for (index, values) in [
            (0, [0, 10, 12]),
            (1, [0, 6, 8]),
            (2, [0, 8, 10]),
            (3, [1, 7, u64::MAX]),
            (4, [1, 7, u64::MAX]),
            (5, [1, 7, u64::MAX]),
        ] {
            for value in values {
                let mut invalid = valid;
                invalid[index] = value;
                assert_eq!(
                    source_mapping_control_request::decode(11, handle, invalid),
                    Err(native_status::invalid),
                    "field {index} value {value}"
                );
            }
        }
        assert_eq!(
            source_mapping_control_request::decode(0, handle, handle.arguments(0)),
            Err(native_status::invalid)
        );
    }
}
