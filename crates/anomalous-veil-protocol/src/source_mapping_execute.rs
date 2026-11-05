//! Inert geometry for replacing read-only permissions within a retained source.
//!
//! This experimental twelve-word shape is not a stabilized ABI and has no
//! native operation number or transport. All inputs and constructors are public
//! metadata: decoding authenticates neither a live owner nor an immutable source.
//! Repeated decoding is permitted and does not consume a pending invocation.
//! A future controller must separately authenticate the current veil, suspended
//! caller, source and mapping owners, and the complete current permission state.
//! The current sealed call, saved frame and mutation epoch are not represented
//! by these words and must be bound separately before any future effect.
//! It must retain those owners through mutation, uncertainty and withdrawal.
//! This module neither changes permissions nor authorizes caller resumption.

use crate::native_status;
use crate::source_mapping::{page_size, source_mapping_request, user_read_only};
use crate::source_mapping_lifetime::source_mapping_handle;
use crate::source_read::source_binding;

/// User-readable and executable, never writable. No other rights are admitted.
pub const user_read_execute: u64 = 5;
pub const execute_word_count: usize = 12;

/// Correlated creation observations, not a retained owner or capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_mapping_retained_context {
    creation: source_mapping_request,
    binding: source_binding,
    handle: source_mapping_handle,
}

impl source_mapping_retained_context {
    pub fn new(
        creation: source_mapping_request,
        binding: source_binding,
        handle: source_mapping_handle,
    ) -> Result<Self, native_status> {
        if creation.grant() != binding.grant()
            || creation.populated_byte_length()
                != binding.length().min(creation.rounded_byte_length())
            || creation.permissions() != user_read_only
        {
            return Err(native_status::invalid);
        }
        Ok(Self {
            creation,
            binding,
            handle,
        })
    }

    pub const fn creation(self) -> source_mapping_request {
        self.creation
    }
    pub const fn binding(self) -> source_binding {
        self.binding
    }
    pub const fn handle(self) -> source_mapping_handle {
        self.handle
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_mapping_execute_request {
    sequence: u64,
    handle: source_mapping_handle,
    binding: source_binding,
    base: u64,
    byte_length: u64,
    source_offset: u64,
    first_page: u64,
}

impl source_mapping_execute_request {
    /// Check the complete copied shape against already selected metadata.
    /// The two reserved words must be zero. Source offsets remain explicit and
    /// must preserve the creation mapping's source-prefix correspondence.
    /// Full pages must contain source bytes; neither zero padding in the last
    /// data page nor the inaccessible virtual tail may become executable.
    pub fn decode(
        current_sequence: u64,
        current_binding: source_binding,
        retained: source_mapping_retained_context,
        words: [u64; execute_word_count],
    ) -> Result<Self, native_status> {
        let [
            sequence,
            generation,
            incarnation,
            grant,
            source_length,
            source_fnv64,
            base,
            byte_length,
            source_offset,
            permissions,
            reserved_a,
            reserved_b,
        ] = words;
        let relative_base = base.checked_sub(retained.creation.base());
        let relative_end = relative_base.and_then(|offset| offset.checked_add(byte_length));
        let source_end = source_offset.checked_add(byte_length);
        if current_sequence == 0
            || sequence != current_sequence
            || sequence <= retained.creation.sequence()
            || reserved_a != 0
            || reserved_b != 0
            || byte_length == 0
            || base & (page_size - 1) != 0
            || byte_length & (page_size - 1) != 0
            || source_offset & (page_size - 1) != 0
            || base.checked_add(byte_length).is_none()
            || relative_base != Some(source_offset)
            || relative_end.is_none_or(|end| end > retained.creation.rounded_byte_length())
            || source_end.is_none_or(|end| end > retained.creation.populated_byte_length())
            || permissions != user_read_execute
        {
            return Err(native_status::invalid);
        }
        // The digest is correlation metadata, not authentication of bytes.
        // A genuinely retained source must still be independently established.
        if generation != retained.handle.generation()
            || incarnation != retained.handle.incarnation()
            || current_binding != retained.binding
            || grant != current_binding.grant()
            || source_length != current_binding.length()
            || source_fnv64 != current_binding.fnv64()
        {
            return Err(native_status::denied);
        }
        Ok(Self {
            sequence,
            handle: retained.handle,
            binding: current_binding,
            base,
            byte_length,
            source_offset,
            first_page: source_offset / page_size,
        })
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn handle(self) -> source_mapping_handle {
        self.handle
    }
    pub const fn binding(self) -> source_binding {
        self.binding
    }
    pub const fn base(self) -> u64 {
        self.base
    }
    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }
    pub const fn source_offset(self) -> u64 {
        self.source_offset
    }
    pub const fn permissions(self) -> u64 {
        user_read_execute
    }
    pub const fn first_page(self) -> u64 {
        self.first_page
    }
    pub const fn page_count(self) -> u64 {
        self.byte_length / page_size
    }
    pub const fn words(self) -> [u64; execute_word_count] {
        [
            self.sequence,
            self.handle.generation(),
            self.handle.incarnation(),
            self.binding.grant(),
            self.binding.length(),
            self.binding.fnv64(),
            self.base,
            self.byte_length,
            self.source_offset,
            user_read_execute,
            0,
            0,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_mapping::user_address_limit;
    use crate::source_read::source_grant;

    fn binding() -> source_binding {
        source_binding::new(source_grant, 2_190_608, 0x1234).unwrap()
    }

    fn retained() -> source_mapping_retained_context {
        let creation = source_mapping_request::decode(
            11,
            binding(),
            [
                11,
                source_grant,
                0x1_0000_0000,
                2_231_696,
                2_190_608,
                user_read_only,
            ],
        )
        .unwrap();
        source_mapping_retained_context::new(
            creation,
            binding(),
            source_mapping_handle::new(7, 9).unwrap(),
        )
        .unwrap()
    }

    fn words() -> [u64; execute_word_count] {
        [
            12,
            7,
            9,
            source_grant,
            2_190_608,
            0x1234,
            0x1_0002_8000,
            0x198000,
            0x28000,
            user_read_execute,
            0,
            0,
        ]
    }

    #[test]
    fn observed_range_preserves_source_correspondence_without_allocating_or_authorizing() {
        let request =
            source_mapping_execute_request::decode(12, binding(), retained(), words()).unwrap();
        assert_eq!(request.words(), words());
        assert_eq!(request.sequence(), 12);
        assert_eq!(request.handle(), retained().handle());
        assert_eq!(request.binding(), binding());
        assert_eq!(request.base(), 0x1_0002_8000);
        assert_eq!(request.byte_length(), 0x198000);
        assert_eq!(request.source_offset(), 0x28000);
        assert_eq!(request.permissions(), user_read_execute);
        assert_eq!((request.first_page(), request.page_count()), (40, 408));
        assert_eq!(retained().creation().data_pages(), 535);
        assert_eq!(retained().creation().virtual_pages(), 545);
        // An inert decoder permits replay: neither this copied handle nor the
        // successful shape establishes ownership, mutation state or a ticket.
        assert_eq!(
            source_mapping_execute_request::decode(12, binding(), retained(), words()),
            Ok(request)
        );
    }

    #[test]
    fn full_width_fresh_sequence_and_handle_are_correlation_not_linux_policy() {
        let handle = source_mapping_handle::new((1 << 32) | 7, (1 << 32) | 9).unwrap();
        let retained =
            source_mapping_retained_context::new(retained().creation(), binding(), handle).unwrap();
        let sequence = (1 << 32) | 12;
        let mut words = words();
        words[0] = sequence;
        words[1] = handle.generation();
        words[2] = handle.incarnation();
        assert_eq!(
            source_mapping_execute_request::decode(sequence, binding(), retained, words)
                .unwrap()
                .words(),
            words
        );
        for index in 0..3 {
            let mut narrowed = words;
            narrowed[index] &= u32::MAX as u64;
            assert!(
                source_mapping_execute_request::decode(sequence, binding(), retained, narrowed)
                    .is_err()
            );
        }
    }

    #[test]
    fn shape_checks_freshness_alignment_offsets_permissions_and_reserved_words() {
        for (index, value) in [
            (0, 0),
            (0, 11),
            (0, 13),
            (6, 0),
            (6, 0x1_0002_8001),
            (6, user_address_limit),
            (6, !(page_size - 1)),
            (7, 0),
            (7, 1),
            (7, 0x198001),
            (7, u64::MAX),
            (7, !(page_size - 1)),
            (8, 0),
            (8, 0x28001),
            (8, 0x29000),
            (8, !(page_size - 1)),
            (9, 0),
            (9, 1),
            (9, 3),
            (9, 4),
            (9, 7),
            (9, u64::MAX),
            (10, 1),
            (10, u64::MAX),
            (11, 1),
            (11, u64::MAX),
        ] {
            let mut changed = words();
            changed[index] = value;
            assert_eq!(
                source_mapping_execute_request::decode(12, binding(), retained(), changed),
                Err(native_status::invalid),
                "field {index} value {value}"
            );
        }
        for sequence in [0, 10, 11] {
            let mut changed = words();
            changed[0] = sequence;
            assert_eq!(
                source_mapping_execute_request::decode(sequence, binding(), retained(), changed),
                Err(native_status::invalid)
            );
        }
    }

    #[test]
    fn current_and_retained_bindings_and_handles_must_agree_completely() {
        for index in 1..=5 {
            let mut changed = words();
            changed[index] ^= 1;
            assert_eq!(
                source_mapping_execute_request::decode(12, binding(), retained(), changed),
                Err(native_status::denied),
                "field {index}"
            );
        }
        for current in [
            source_binding::new(source_grant, binding().length() + 1, binding().fnv64()).unwrap(),
            source_binding::new(source_grant, binding().length(), binding().fnv64() ^ 1).unwrap(),
        ] {
            let mut changed = words();
            changed[4] = current.length();
            changed[5] = current.fnv64();
            assert_eq!(
                source_mapping_execute_request::decode(12, current, retained(), changed),
                Err(native_status::denied)
            );
        }
        let wrong_length =
            source_binding::new(source_grant, binding().length() - 1, 0x1234).unwrap();
        assert_eq!(
            source_mapping_retained_context::new(
                retained().creation(),
                wrong_length,
                retained().handle()
            ),
            Err(native_status::invalid)
        );
        let mut malformed_and_foreign = words();
        malformed_and_foreign[7] = 0;
        malformed_and_foreign[3] = 0;
        assert_eq!(
            source_mapping_execute_request::decode(
                12,
                binding(),
                retained(),
                malformed_and_foreign
            ),
            Err(native_status::invalid)
        );
    }

    #[test]
    fn executable_pages_cannot_include_source_padding_or_virtual_reservation() {
        for (page, length, accepted) in [
            (0, page_size, true),
            (533, page_size, true),
            (0, 534 * page_size, true),
            (533, 2 * page_size, false),
            (534, page_size, false),
            (535, page_size, false),
            (544, page_size, false),
            (545, page_size, false),
        ] {
            let mut changed = words();
            changed[6] = retained().creation().base() + page * page_size;
            changed[7] = length;
            changed[8] = page * page_size;
            assert_eq!(
                source_mapping_execute_request::decode(12, binding(), retained(), changed).is_ok(),
                accepted,
                "page {page} length {length}"
            );
        }
    }

    #[test]
    fn large_source_does_not_expand_the_retained_virtual_extent() {
        let source = source_binding::new(source_grant, 4 * page_size, 0).unwrap();
        let creation = source_mapping_request::decode(
            1,
            source,
            [
                1,
                source_grant,
                user_address_limit - 2 * page_size,
                page_size + 1,
                2 * page_size,
                user_read_only,
            ],
        )
        .unwrap();
        let retained = source_mapping_retained_context::new(
            creation,
            source,
            source_mapping_handle::new(1, 1).unwrap(),
        )
        .unwrap();
        let words = [
            2,
            1,
            1,
            source_grant,
            source.length(),
            0,
            creation.base(),
            2 * page_size,
            0,
            user_read_execute,
            0,
            0,
        ];
        assert!(source_mapping_execute_request::decode(2, source, retained, words).is_ok());
        let mut beyond = words;
        beyond[7] = 3 * page_size;
        assert_eq!(
            source_mapping_execute_request::decode(2, source, retained, beyond),
            Err(native_status::invalid)
        );
    }
}
