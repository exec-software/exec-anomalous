#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental copied invocation records and native veil mechanisms.
//! Sequence values correlate a kernel-retained invocation; encoded bytes are
//! not authority to select a task, root, saved frame or endpoint binding.
//! Linux interpretation is compiled only for the standalone userspace veil.

#[cfg(feature = "linux-policy")]
pub mod linux_policy;

#[cfg(feature = "linux-loader-policy")]
pub mod loader_policy;

#[cfg(feature = "read-only-files")]
pub mod file_service;

#[cfg(feature = "source-read")]
pub mod source_read;

#[cfg(feature = "source-mapping")]
pub mod source_mapping;

#[cfg(feature = "source-mapping")]
pub mod source_mapping_lifetime;

#[cfg(all(
    feature = "linux-loader-policy",
    feature = "read-only-files",
    feature = "source-mapping"
))]
pub mod loader_mapping_policy;

pub const request_size: usize = 80;
pub const packet_capacity: usize = 4096;
pub const request_operation: u64 = 16;
pub const query_operation: u64 = 17;
pub const caller_read_operation: u64 = 18;
pub const stage_operation: u64 = 19;
pub const complete_operation: u64 = 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum wire_error {
    size,
    header,
    sequence,
    reserved,
    value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct invocation {
    sequence: u64,
    number: u64,
    arguments: [u64; 6],
}

impl invocation {
    pub const fn new(sequence: u64, number: u64, arguments: [u64; 6]) -> Result<Self, wire_error> {
        if sequence == 0 {
            return Err(wire_error::sequence);
        }
        Ok(Self {
            sequence,
            number,
            arguments,
        })
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn number(self) -> u64 {
        self.number
    }
    pub const fn arguments(self) -> [u64; 6] {
        self.arguments
    }

    pub fn encode(self) -> [u8; request_size] {
        let mut bytes = [0; request_size];
        bytes[..4].copy_from_slice(b"ANVR");
        bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
        bytes[6..8].copy_from_slice(&(request_size as u16).to_le_bytes());
        bytes[8..16].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.number.to_le_bytes());
        for (index, value) in self.arguments.iter().enumerate() {
            bytes[24 + index * 8..32 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, wire_error> {
        if bytes.len() != request_size {
            return Err(wire_error::size);
        }
        if &bytes[..4] != b"ANVR"
            || bytes[4..6] != 1u16.to_le_bytes()
            || bytes[6..8] != (request_size as u16).to_le_bytes()
        {
            return Err(wire_error::header);
        }
        if bytes[72..].iter().any(|byte| *byte != 0) {
            return Err(wire_error::reserved);
        }
        let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        let mut arguments = [0; 6];
        for (index, value) in arguments.iter_mut().enumerate() {
            *value = word(24 + index * 8);
        }
        Self::new(word(8), word(16), arguments)
    }
}

/// Native mechanism outcomes. They are not errno values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum native_status {
    ok = 0,
    not_ready = 1,
    denied = 2,
    bad_copy = 3,
    invalid = 4,
    closed = 5,
    too_small = 6,
    exhausted = 7,
}

impl native_status {
    pub const fn from_raw(value: u64) -> Option<Self> {
        match value {
            0 => Some(Self::ok),
            1 => Some(Self::not_ready),
            2 => Some(Self::denied),
            3 => Some(Self::bad_copy),
            4 => Some(Self::invalid),
            5 => Some(Self::closed),
            6 => Some(Self::too_small),
            7 => Some(Self::exhausted),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum channel_phase {
    empty = 0,
    queued = 1,
    delivered = 2,
    closed = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct channel_snapshot {
    phase: channel_phase,
    exhausted: bool,
    length: u16,
}

impl channel_snapshot {
    pub const fn new(
        phase: channel_phase,
        exhausted: bool,
        length: usize,
    ) -> Result<Self, wire_error> {
        if length > packet_capacity
            || (matches!(phase, channel_phase::empty | channel_phase::closed) && length != 0)
            || (matches!(phase, channel_phase::queued | channel_phase::delivered) && length == 0)
        {
            return Err(wire_error::value);
        }
        Ok(Self {
            phase,
            exhausted,
            length: length as u16,
        })
    }
    pub const fn phase(self) -> channel_phase {
        self.phase
    }
    pub const fn exhausted(self) -> bool {
        self.exhausted
    }
    pub const fn length(self) -> usize {
        self.length as usize
    }
    pub const fn encode(self) -> u64 {
        self.phase as u64 | ((self.exhausted as u64) << 8) | ((self.length as u64) << 16)
    }
    pub const fn decode(value: u64) -> Result<Self, wire_error> {
        if value & !0x0000_0000_ffff_0103 != 0 {
            return Err(wire_error::reserved);
        }
        let phase = match value & 3 {
            0 => channel_phase::empty,
            1 => channel_phase::queued,
            2 => channel_phase::delivered,
            _ => channel_phase::closed,
        };
        Self::new(phase, value & 0x100 != 0, ((value >> 16) & 0xffff) as usize)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum stage_kind {
    submit = 1,
    consume = 2,
    copyout = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum completion_action {
    return_to_caller = 0,
    yield_caller = 1,
    retire_caller = 2,
}

/// A proposal, never a caller-supplied saved frame or return address. A zero
/// token means no staged memory/channel effect; only the kernel can mint a
/// successful nonzero token for this invocation's retained stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct completion {
    pub result_bits: u64,
    pub action: completion_action,
    pub stage_token: u64,
}

impl completion {
    pub const fn returned(result: i64) -> Self {
        Self {
            result_bits: result as u64,
            action: completion_action::return_to_caller,
            stage_token: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct stage_receipt {
    pub token: u64,
    pub length: u64,
}

/// Native effects on the authenticated suspended caller, never on the veil.
/// Query takes [sequence, 0, 0, 0, 0, 0]; stage takes [sequence, base, 0, 0, 0, 0].
/// The stage returns the sequence token and zero length. Only exact completion
/// may commit it; rejection or veil retirement discards the proposal.
pub const caller_fs_query_operation: u64 = 21;
pub const caller_fs_stage_operation: u64 = 22;

#[cfg(feature = "linux-tls-policy")]
pub mod linux_tls_policy;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_exact_little_endian_and_rejects_every_reserved_byte() {
        let request = invocation::new(0x0102030405060708, u64::MAX, [1, 2, 3, 4, 5, 6]).unwrap();
        let bytes = request.encode();
        assert_eq!(&bytes[..8], b"ANVR\x01\0\x50\0");
        assert_eq!(&bytes[8..16], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(invocation::decode(&bytes), Ok(request));
        assert_eq!(invocation::decode(&bytes[..79]), Err(wire_error::size));
        let mut long = [0; 81];
        long[..80].copy_from_slice(&bytes);
        assert_eq!(invocation::decode(&long), Err(wire_error::size));
        for offset in 0..8 {
            let mut changed = bytes;
            changed[offset] ^= 0x80;
            assert_eq!(invocation::decode(&changed), Err(wire_error::header));
        }
        for offset in 72..80 {
            let mut changed = bytes;
            changed[offset] = 1;
            assert_eq!(invocation::decode(&changed), Err(wire_error::reserved));
        }
        let mut zero = bytes;
        zero[8..16].fill(0);
        assert_eq!(invocation::decode(&zero), Err(wire_error::sequence));
        assert_eq!(invocation::new(0, 0, [0; 6]), Err(wire_error::sequence));
    }

    #[test]
    fn snapshots_and_native_statuses_are_closed_bounded_values() {
        for phase in [
            channel_phase::empty,
            channel_phase::queued,
            channel_phase::delivered,
            channel_phase::closed,
        ] {
            for exhausted in [false, true] {
                let length = if matches!(phase, channel_phase::queued | channel_phase::delivered) {
                    4096
                } else {
                    0
                };
                let snapshot = channel_snapshot::new(phase, exhausted, length).unwrap();
                assert_eq!(channel_snapshot::decode(snapshot.encode()), Ok(snapshot));
                for bit in [2, 7, 9, 15, 32, 63] {
                    assert_eq!(
                        channel_snapshot::decode(snapshot.encode() | (1 << bit)),
                        Err(wire_error::reserved)
                    );
                }
            }
        }
        assert!(channel_snapshot::new(channel_phase::empty, false, 1).is_err());
        assert!(channel_snapshot::new(channel_phase::queued, false, 0).is_err());
        assert!(channel_snapshot::new(channel_phase::queued, false, 4097).is_err());
        for value in 0..8 {
            assert_eq!(native_status::from_raw(value).unwrap() as u64, value);
        }
        assert_eq!(native_status::from_raw(8), None);
        assert_eq!(native_status::from_raw(u64::MAX), None);
    }
}

#[cfg(feature = "source-mapping")]
pub mod source_mapping_execute;

#[cfg(all(
    feature = "linux-loader-policy",
    feature = "read-only-files",
    feature = "source-mapping"
))]
pub mod loader_execute_policy;
