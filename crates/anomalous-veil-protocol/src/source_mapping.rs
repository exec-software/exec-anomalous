//! Inert native request geometry for preparing and cancelling a source mapping.
//! Decoding authenticates nothing and publishes nothing. The controller must
//! independently bind the current veil, suspended caller, retained immutable
//! source and complete destination extent before allocating. A
//! successful proof reply reports cleanup, never a mapped address or authority
//! to resume a foreign caller. Source offset zero is implicit in this proof.

use crate::{native_status, source_read::source_binding};

pub const source_mapping_operation: u64 = 25;
pub const page_size: u64 = 4096;
pub const page_limit: u64 = 2048;
pub const user_address_limit: u64 = 1 << 47;
/// User-readable, non-writable and non-executable; no other rights are admitted.
pub const user_read_only: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_mapping_request {
    sequence: u64,
    grant: u64,
    base: u64,
    byte_length: u64,
    populated_byte_length: u64,
    permissions: u64,
    rounded_byte_length: u64,
}

impl source_mapping_request {
    pub fn decode(
        current_sequence: u64,
        binding: source_binding,
        arguments: [u64; 6],
    ) -> Result<Self, native_status> {
        let [
            sequence,
            grant,
            base,
            byte_length,
            populated_byte_length,
            permissions,
        ] = arguments;
        let rounded_byte_length = byte_length
            .checked_add(page_size - 1)
            .map(|length| length & !(page_size - 1))
            .ok_or(native_status::invalid)?;
        if current_sequence == 0
            || sequence != current_sequence
            || base == 0
            || base & (page_size - 1) != 0
            || byte_length == 0
            || rounded_byte_length > page_limit * page_size
            || base
                .checked_add(rounded_byte_length)
                .is_none_or(|end| end > user_address_limit)
            || permissions != user_read_only
            || populated_byte_length != binding.length().min(rounded_byte_length)
        {
            return Err(native_status::invalid);
        }
        if grant != binding.grant() {
            return Err(native_status::denied);
        }
        Ok(Self {
            sequence,
            grant,
            base,
            byte_length,
            populated_byte_length,
            permissions,
            rounded_byte_length,
        })
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn grant(self) -> u64 {
        self.grant
    }
    pub const fn base(self) -> u64 {
        self.base
    }
    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }
    pub const fn populated_byte_length(self) -> u64 {
        self.populated_byte_length
    }
    pub const fn permissions(self) -> u64 {
        self.permissions
    }
    pub const fn rounded_byte_length(self) -> u64 {
        self.rounded_byte_length
    }
    pub const fn virtual_pages(self) -> u64 {
        self.rounded_byte_length / page_size
    }
    pub const fn data_pages(self) -> u64 {
        self.populated_byte_length.div_ceil(page_size)
    }
    pub const fn arguments(self) -> [u64; 6] {
        [
            self.sequence,
            self.grant,
            self.base,
            self.byte_length,
            self.populated_byte_length,
            self.permissions,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_read::{source_byte_limit, source_grant};

    fn binding(length: u64) -> source_binding {
        source_binding::new(source_grant, length, 0x1234).unwrap()
    }

    fn arguments() -> [u64; 6] {
        [
            11,
            source_grant,
            0x1_0000_0000,
            2_231_696,
            2_190_608,
            user_read_only,
        ]
    }

    #[test]
    fn source_prefix_and_virtual_reservation_have_separate_page_counts() {
        let arguments = arguments();
        let request = source_mapping_request::decode(11, binding(2_190_608), arguments).unwrap();
        assert_eq!(request.arguments(), arguments);
        assert_eq!(request.sequence(), 11);
        assert_eq!(request.grant(), source_grant);
        assert_eq!(request.base(), 0x1_0000_0000);
        assert_eq!(request.byte_length(), 2_231_696);
        assert_eq!(request.populated_byte_length(), 2_190_608);
        assert_eq!(request.permissions(), user_read_only);
        assert_eq!(request.rounded_byte_length(), 545 * page_size);
        assert_eq!((request.data_pages(), request.virtual_pages()), (535, 545));
    }

    #[test]
    fn rounding_includes_source_bytes_beyond_the_raw_last_byte() {
        for (length, source_length, populated, pages) in [
            (1, 1, 1, 1),
            (1, 4097, 4096, 1),
            (4096, 4097, 4096, 1),
            (4097, 8193, 8192, 2),
            (4097, 5000, 5000, 2),
        ] {
            let request = source_mapping_request::decode(
                3,
                binding(source_length),
                [
                    3,
                    source_grant,
                    page_size,
                    length,
                    populated,
                    user_read_only,
                ],
            )
            .unwrap();
            assert_eq!(request.virtual_pages(), pages);
            assert_eq!(request.data_pages(), pages);
        }
        assert_eq!(
            source_mapping_request::decode(
                3,
                binding(5000),
                [3, source_grant, page_size, 4097, 4097, user_read_only],
            ),
            Err(native_status::invalid)
        );
    }

    #[test]
    fn malformed_shape_and_sequence_cannot_decode() {
        let valid = arguments();
        for (index, value) in [
            (0, 0),
            (0, 10),
            (0, 12),
            (2, 0),
            (2, 0x1_0000_0001),
            (2, user_address_limit),
            (2, user_address_limit - page_size),
            (2, u64::MAX - (page_size - 1)),
            (3, 0),
            (3, u64::MAX),
            (3, page_limit * page_size + 1),
            (4, 0),
            (4, valid[4] - 1),
            (4, valid[4] + 1),
            (5, 0),
            (5, 2),
            (5, 3),
            (5, u64::MAX),
        ] {
            let mut invalid = valid;
            invalid[index] = value;
            assert_eq!(
                source_mapping_request::decode(11, binding(2_190_608), invalid),
                Err(native_status::invalid),
                "field {index} value {value}"
            );
        }
        assert_eq!(
            source_mapping_request::decode(0, binding(2_190_608), valid),
            Err(native_status::invalid)
        );
    }

    #[test]
    fn wrong_grant_is_denied_only_after_shape_validation() {
        let mut arguments = arguments();
        arguments[1] ^= 1;
        assert_eq!(
            source_mapping_request::decode(11, binding(2_190_608), arguments),
            Err(native_status::denied)
        );
        arguments[0] = 10;
        assert_eq!(
            source_mapping_request::decode(11, binding(2_190_608), arguments),
            Err(native_status::invalid)
        );
    }

    #[test]
    fn inclusive_budget_and_exclusive_canonical_end_are_checked() {
        assert_eq!(source_byte_limit, page_limit * page_size);
        let length = page_limit * page_size;
        let request = source_mapping_request::decode(
            1,
            binding(length),
            [
                1,
                source_grant,
                user_address_limit - length,
                length,
                length,
                user_read_only,
            ],
        )
        .unwrap();
        assert_eq!(request.data_pages(), page_limit);
        assert_eq!(request.virtual_pages(), page_limit);
        assert_eq!(
            request.base() + request.rounded_byte_length(),
            user_address_limit
        );
    }
}
