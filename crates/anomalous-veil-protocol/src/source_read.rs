//! Inert configuration and request shapes for a bounded native source-read proof.
//! Decoding supplies no caller, source or destination authority. The controller
//! separately authenticates the current veil, suspended caller and retained
//! source before fully preflighting a writable destination. These copied fields
//! neither open a Linux file nor authorize returning bytes to the Linux caller.

use crate::{native_status, wire_error};

pub const source_read_operation: u64 = 23;
pub const source_grant: u64 = 0x4c45_0002;
pub const source_byte_limit: u64 = 8 * 1024 * 1024;
pub const source_chunk_limit: usize = 4096;
pub const stream_first_chunk: usize = 17;
pub const binding_offset: usize = 136;
pub const binding_size: usize = 32;
pub const binding_magic: [u8; 8] = *b"ANFSB001";
pub const report_offset: usize = 176;
pub const report_size: usize = 32;
pub const report_magic: [u8; 8] = *b"ANFSR001";
pub const fnv_offset: u64 = 0xcbf2_9ce4_8422_2325;
pub const fnv_prime: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_binding {
    grant: u64,
    length: u64,
    fnv64: u64,
}

impl source_binding {
    pub fn new(grant: u64, length: u64, fnv64: u64) -> Result<Self, wire_error> {
        if grant != source_grant || !(1..=source_byte_limit).contains(&length) {
            return Err(wire_error::value);
        }
        Ok(Self {
            grant,
            length,
            fnv64,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, wire_error> {
        if bytes.len() != binding_size {
            return Err(wire_error::size);
        }
        if bytes[..8] != binding_magic {
            return Err(wire_error::header);
        }
        Self::new(
            word(&bytes[8..16]),
            word(&bytes[16..24]),
            word(&bytes[24..32]),
        )
    }

    pub fn encode(self) -> [u8; binding_size] {
        let mut bytes = [0; binding_size];
        bytes[..8].copy_from_slice(&binding_magic);
        bytes[8..16].copy_from_slice(&self.grant.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.length.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.fnv64.to_le_bytes());
        bytes
    }

    pub const fn grant(self) -> u64 {
        self.grant
    }
    pub const fn length(self) -> u64 {
        self.length
    }
    pub const fn fnv64(self) -> u64 {
        self.fnv64
    }
}

/// Native op23 arguments. The destination remains an opaque untrusted integer;
/// even a canonical address requires full current-owner mapping preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_request {
    pub sequence: u64,
    pub grant: u64,
    pub offset: u64,
    pub destination: u64,
    pub length: usize,
}

impl source_request {
    pub fn decode(
        current_sequence: u64,
        binding: source_binding,
        arguments: [u64; 6],
    ) -> Result<Self, native_status> {
        let [sequence, grant, offset, destination, length, reserved] = arguments;
        if current_sequence == 0
            || sequence != current_sequence
            || reserved != 0
            || !(1..=source_chunk_limit as u64).contains(&length)
            || offset
                .checked_add(length)
                .is_none_or(|end| end > binding.length)
        {
            return Err(native_status::invalid);
        }
        if grant != binding.grant {
            return Err(native_status::denied);
        }
        Ok(Self {
            sequence,
            grant,
            offset,
            destination,
            length: length as usize,
        })
    }
}

/// Diagnostic observation of bytes actually consumed by the veil. This report
/// is not an authority receipt and must be compared with independent kernel
/// source-copy evidence. FNV is evidence bookkeeping, not authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct source_report {
    pub length: u64,
    pub fnv64: u64,
    pub chunks: u64,
}

impl source_report {
    pub fn new(length: u64, fnv64: u64, chunks: u64) -> Result<Self, wire_error> {
        if stream_chunk_count(length) != Some(chunks) {
            return Err(wire_error::value);
        }
        Ok(Self {
            length,
            fnv64,
            chunks,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, wire_error> {
        if bytes.len() != report_size {
            return Err(wire_error::size);
        }
        if bytes[..8] != report_magic {
            return Err(wire_error::header);
        }
        Self::new(
            word(&bytes[8..16]),
            word(&bytes[16..24]),
            word(&bytes[24..32]),
        )
    }

    pub fn encode(self) -> [u8; report_size] {
        let mut bytes = [0; report_size];
        bytes[..8].copy_from_slice(&report_magic);
        bytes[8..16].copy_from_slice(&self.length.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.fnv64.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.chunks.to_le_bytes());
        bytes
    }
}

pub fn stream_chunk_count(length: u64) -> Option<u64> {
    if !(1..=source_byte_limit).contains(&length) {
        return None;
    }
    Some(
        1 + length
            .saturating_sub(stream_first_chunk as u64)
            .div_ceil(source_chunk_limit as u64),
    )
}

pub const fn hash_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(fnv_prime)
}

fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("exact source record word"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> source_binding {
        source_binding::new(source_grant, 8193, 0x1234_5678_90ab_cdef).unwrap()
    }

    #[test]
    fn binding_has_exact_versioned_little_endian_geometry() {
        let binding = binding();
        let encoded = binding.encode();
        assert_eq!(&encoded[..8], b"ANFSB001");
        assert_eq!(&encoded[8..16], &[2, 0, 0x45, 0x4c, 0, 0, 0, 0]);
        assert_eq!(&encoded[16..24], &8193u64.to_le_bytes());
        assert_eq!(&encoded[24..], &0x1234_5678_90ab_cdefu64.to_le_bytes());
        assert_eq!(source_binding::decode(&encoded), Ok(binding));
        assert_eq!(binding.grant(), source_grant);
        assert_eq!(binding.length(), 8193);
        assert_eq!(binding.fnv64(), 0x1234_5678_90ab_cdef);
        assert_eq!(
            (binding_offset, binding_size, report_offset, report_size),
            (136, 32, 176, 32)
        );
    }

    #[test]
    fn malformed_binding_never_invents_source_authority() {
        let bytes = binding().encode();
        for length in 0..binding_size {
            assert_eq!(
                source_binding::decode(&bytes[..length]),
                Err(wire_error::size)
            );
        }
        assert_eq!(
            source_binding::decode(&[0; binding_size + 1]),
            Err(wire_error::size)
        );
        for index in 0..8 {
            let mut changed = bytes;
            changed[index] ^= 1;
            assert_eq!(source_binding::decode(&changed), Err(wire_error::header));
        }
        for grant in [0, 1, source_grant - 1, source_grant + 1, u64::MAX] {
            assert_eq!(source_binding::new(grant, 1, 0), Err(wire_error::value));
            let mut changed = bytes;
            changed[8..16].copy_from_slice(&grant.to_le_bytes());
            assert_eq!(source_binding::decode(&changed), Err(wire_error::value));
        }
        for length in [0, source_byte_limit + 1, u64::MAX] {
            assert_eq!(
                source_binding::new(source_grant, length, 0),
                Err(wire_error::value)
            );
        }
        assert!(source_binding::new(source_grant, 1, 0).is_ok());
        assert!(source_binding::new(source_grant, source_byte_limit, u64::MAX).is_ok());
    }

    #[test]
    fn read_shape_checks_exact_sequence_extent_and_local_work_budget() {
        let binding = binding();
        for (offset, length) in [(0, 1), (17, 4096), (8192, 1), (4097, 4096)] {
            let request =
                source_request::decode(4, binding, [4, source_grant, offset, 0x8000d0, length, 0])
                    .unwrap();
            assert_eq!(
                request,
                source_request {
                    sequence: 4,
                    grant: source_grant,
                    offset,
                    destination: 0x8000d0,
                    length: length as usize
                }
            );
        }
        for sequence in [0, 3, 5, u64::MAX] {
            assert_eq!(
                source_request::decode(4, binding, [sequence, source_grant, 0, 1, 1, 0]),
                Err(native_status::invalid)
            );
        }
        assert_eq!(
            source_request::decode(0, binding, [0, source_grant, 0, 1, 1, 0]),
            Err(native_status::invalid)
        );
        for (offset, length) in [
            (0, 0),
            (0, 4097),
            (0, u64::MAX),
            (u64::MAX, 2),
            (8193, 1),
            (8192, 2),
        ] {
            assert_eq!(
                source_request::decode(4, binding, [4, source_grant, offset, 1, length, 0]),
                Err(native_status::invalid)
            );
        }
        for reserved in [1, u64::MAX] {
            assert_eq!(
                source_request::decode(4, binding, [4, source_grant, 0, 1, 1, reserved]),
                Err(native_status::invalid)
            );
        }
    }

    #[test]
    fn numeric_destination_is_not_mistaken_for_mapping_authority() {
        for destination in [0, 0x400000, 0x801ff8, u64::MAX] {
            assert!(
                source_request::decode(4, binding(), [4, source_grant, 0, destination, 16, 0])
                    .is_ok()
            );
        }
        assert_eq!(
            source_request::decode(4, binding(), [4, source_grant + 1, 0, 1, 1, 0]),
            Err(native_status::denied)
        );
        // Malformed shape is invalid even when its grant is also wrong.
        assert_eq!(
            source_request::decode(4, binding(), [4, source_grant + 1, 0, 1, 0, 0]),
            Err(native_status::invalid)
        );
    }

    #[test]
    fn report_geometry_matches_first_short_chunk_then_exact_stream() {
        for (length, chunks) in [
            (1, 1),
            (17, 1),
            (18, 2),
            (4113, 2),
            (4114, 3),
            (2_190_608, 536),
            (source_byte_limit, 2049),
        ] {
            assert_eq!(stream_chunk_count(length), Some(chunks));
            let report = source_report::new(length, 7, chunks).unwrap();
            assert_eq!(source_report::decode(&report.encode()), Ok(report));
            assert_eq!(
                source_report::new(length, 7, chunks + 1),
                Err(wire_error::value)
            );
        }
        assert_eq!(stream_chunk_count(0), None);
        assert_eq!(stream_chunk_count(source_byte_limit + 1), None);
        assert_eq!(stream_chunk_count(u64::MAX), None);
        assert_eq!(
            source_report::decode(&[0; report_size]),
            Err(wire_error::header)
        );
        assert_eq!(
            source_report::decode(&[0; report_size - 1]),
            Err(wire_error::size)
        );
        assert_eq!(
            source_report::decode(&[0; report_size + 1]),
            Err(wire_error::size)
        );
        assert_eq!(source_report::new(0, fnv_offset, 0), Err(wire_error::value));
    }

    #[test]
    fn consumed_hash_preserves_stream_order_and_chunk_independence() {
        let expected = 0xa430_d846_80aa_bd0b;
        let mut hash = fnv_offset;
        for byte in b"hello" {
            hash = hash_byte(hash, *byte);
        }
        assert_eq!(hash, expected);
        let mut split = fnv_offset;
        for bytes in [b"he".as_slice(), b"llo"] {
            for byte in bytes {
                split = hash_byte(split, *byte);
            }
        }
        assert_eq!(split, expected);
        let mut reversed = fnv_offset;
        for byte in b"olleh" {
            reversed = hash_byte(reversed, *byte);
        }
        assert_ne!(reversed, expected);
    }
}
