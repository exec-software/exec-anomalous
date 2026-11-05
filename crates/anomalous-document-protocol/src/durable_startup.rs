//! Copied metadata for the bounded native disk proof, never device authority.
//! The kernel retains the actual task/extent binding separately. Recovery
//! descriptors contain no expected content and mount epochs come from the
//! userspace store, not the launch controller.

use crate::{client_identity, codec_error, durable::disk_identity, text_capacity, validate_text};

pub const startup_size: usize = 192;
pub const mount_ready_size: usize = 128;
pub const arena_byte_count: u64 = 32_768;
const user_end: u64 = 1 << 47;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    role: u64,
    phase: u64,
    arena: [u64; 2],
    handles: [u64; 4],
    authority: client_identity,
    identity: disk_identity,
    device_handle: u64,
    challenge: [u8; text_capacity],
    challenge_length: usize,
}

impl startup_descriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        role: u64,
        phase: u64,
        arena: [u64; 2],
        handles: [u64; 4],
        authority: client_identity,
        identity: disk_identity,
        device_handle: u64,
        challenge: &[u8],
    ) -> Result<Self, codec_error> {
        if role > 1 || phase > 2 || (role == 1) != (device_handle != 0) {
            return Err(codec_error::fields);
        }
        if arena[0] == 0
            || !arena[0].is_multiple_of(4096)
            || arena[1] != arena_byte_count
            || arena[0]
                .checked_add(arena[1])
                .is_none_or(|end| end > user_end)
        {
            return Err(codec_error::fields);
        }
        for (index, handle) in handles.iter().enumerate() {
            if *handle == 0 || handles[..index].contains(handle) {
                return Err(codec_error::fields);
            }
        }
        validate_text(challenge)?;
        if (role == 0 && phase == 0) == challenge.is_empty() {
            return Err(codec_error::fields);
        }
        let mut copied = [0; text_capacity];
        copied[..challenge.len()].copy_from_slice(challenge);
        Ok(Self {
            role,
            phase,
            arena,
            handles,
            authority,
            identity,
            device_handle,
            challenge: copied,
            challenge_length: challenge.len(),
        })
    }

    pub const fn role(&self) -> u64 {
        self.role
    }
    pub const fn phase(&self) -> u64 {
        self.phase
    }
    pub const fn arena_base(&self) -> u64 {
        self.arena[0]
    }
    pub const fn arena_length(&self) -> u64 {
        self.arena[1]
    }
    pub const fn handles(&self) -> [u64; 4] {
        self.handles
    }
    pub const fn authority(&self) -> client_identity {
        self.authority
    }
    pub const fn identity(&self) -> disk_identity {
        self.identity
    }
    pub const fn device_handle(&self) -> u64 {
        self.device_handle
    }
    pub fn challenge(&self) -> &[u8] {
        &self.challenge[..self.challenge_length]
    }

    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = [0; startup_size];
        bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'D', 1, 0, 192, 0]);
        for (index, value) in [
            self.role,
            self.phase,
            self.arena[0],
            self.arena[1],
            self.handles[0],
            self.handles[1],
            self.handles[2],
            self.handles[3],
            self.authority.task(),
            self.authority.generation(),
            self.identity.volume(),
            self.identity.object(),
            self.device_handle,
        ]
        .iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[112] = self.challenge_length as u8;
        bytes[113..113 + self.challenge_length].copy_from_slice(self.challenge());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != startup_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDD" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, 192, 0] {
            return Err(codec_error::version);
        }
        let length = usize::from(bytes[112]);
        if length > text_capacity {
            return Err(codec_error::text);
        }
        if bytes[113 + length..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        Self::try_new(
            read(bytes, 8),
            read(bytes, 16),
            [read(bytes, 24), read(bytes, 32)],
            [
                read(bytes, 40),
                read(bytes, 48),
                read(bytes, 56),
                read(bytes, 64),
            ],
            client_identity::try_new(read(bytes, 72), read(bytes, 80))?,
            disk_identity::try_new(read(bytes, 88), read(bytes, 96))
                .map_err(|_| codec_error::fields)?,
            read(bytes, 104),
            &bytes[113..113 + length],
        )
    }
}

/// Sent over the service's copied reply route only after mount's final flush.
/// It conveys no recovered document bytes and cannot mint route authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct mount_ready {
    document: u64,
    version: u64,
    sequence: u64,
}

impl mount_ready {
    pub fn try_new(document: u64, version: u64, sequence: u64) -> Result<Self, codec_error> {
        if document == 0 || version == 0 || document.checked_add(version) != Some(sequence) {
            return Err(codec_error::fields);
        }
        Ok(Self {
            document,
            version,
            sequence,
        })
    }

    pub const fn document(&self) -> u64 {
        self.document
    }
    pub const fn version(&self) -> u64 {
        self.version
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn encode(&self) -> [u8; mount_ready_size] {
        let mut bytes = [0; mount_ready_size];
        bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'M', 1, 0, 128, 0]);
        for (index, value) in [self.document, self.version, self.sequence]
            .iter()
            .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != mount_ready_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDM" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, 128, 0] {
            return Err(codec_error::version);
        }
        if bytes[32..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        Self::try_new(read(bytes, 8), read(bytes, 16), read(bytes, 24))
    }
}

fn read(bytes: &[u8], offset: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(
        role: u64,
        phase: u64,
        challenge: &[u8],
    ) -> Result<startup_descriptor, codec_error> {
        startup_descriptor::try_new(
            role,
            phase,
            [0x804000, arena_byte_count],
            [0x31, 0x32, 0x33, 0x34],
            client_identity::try_new(1, 7).unwrap(),
            disk_identity::try_new(0x101, 0x202).unwrap(),
            if role == 1 { 0x51 + phase * 0x100 } else { 0 },
            challenge,
        )
    }

    #[test]
    fn role_phase_contract_has_no_recovery_text_or_editor_device() {
        for role in 0..2 {
            for phase in 0..3 {
                let challenge = if role == 0 && phase == 0 {
                    b"host challenge".as_slice()
                } else {
                    b""
                };
                let value = descriptor(role, phase, challenge).unwrap();
                let encoded = value.encode();
                assert_eq!(startup_descriptor::decode(&encoded), Ok(value));
                assert_eq!(read(&encoded, 72), 1);
                assert_eq!(read(&encoded, 80), 7);
                assert_eq!(read(&encoded, 88), 0x101);
                assert_eq!(read(&encoded, 96), 0x202);
                assert_eq!(encoded[112] as usize, challenge.len());
                assert!(encoded[144..].iter().all(|byte| *byte == 0));
            }
        }
        for (role, phase, challenge) in [
            (0, 0, b"".as_slice()),
            (0, 1, b"seed"),
            (0, 2, b"seed"),
            (1, 0, b"seed"),
        ] {
            assert_eq!(descriptor(role, phase, challenge), Err(codec_error::fields));
        }
    }

    #[test]
    fn startup_rejects_wrong_version_extents_handles_and_noncanonical_tail() {
        let original = descriptor(1, 1, b"").unwrap().encode();
        for (offset, value) in [
            (8, 2),
            (16, 3),
            (24, 0),
            (24, 0x804001),
            (24, u64::MAX - 4095),
            (32, 4096),
            (40, 0),
            (48, 0x31),
            (72, 0),
            (80, 0),
            (88, 0),
            (96, 0),
            (104, 0),
        ] {
            let mut bytes = original;
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
        }
        for offset in 113..startup_size {
            let mut bytes = original;
            bytes[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&bytes),
                Err(codec_error::reserved)
            );
        }
        for offset in 4..8 {
            let mut bytes = original;
            bytes[offset] ^= 1;
            assert_eq!(
                startup_descriptor::decode(&bytes),
                Err(codec_error::version)
            );
        }
        let mut editor = descriptor(0, 0, b"x").unwrap().encode();
        editor[104] = 1;
        assert_eq!(
            startup_descriptor::decode(&editor),
            Err(codec_error::fields)
        );
        assert_eq!(
            startup_descriptor::decode(&original[..191]),
            Err(codec_error::length)
        );
        assert_eq!(
            startup_descriptor::decode(&[0; 193]),
            Err(codec_error::length)
        );
    }

    #[test]
    fn challenge_is_bounded_printable_ascii_without_truncation() {
        assert!(descriptor(0, 0, &[b'x'; 31]).is_ok());
        for bad in [b"a\0b".as_slice(), b"a\nb", "é".as_bytes(), &[b'x'; 32]] {
            assert_eq!(descriptor(0, 0, bad), Err(codec_error::text));
        }
        let mut bytes = descriptor(0, 0, b"x").unwrap().encode();
        bytes[112] = 32;
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::text));
    }

    #[test]
    fn mount_ready_requires_exact_mounted_state_and_zero_reserved_bytes() {
        for (document, version) in [(1, 1), (1, 2), (2, 2), (3, 2)] {
            let value = mount_ready::try_new(document, version, document + version).unwrap();
            assert_eq!(mount_ready::decode(&value.encode()), Ok(value));
        }
        for tuple in [(0, 1, 1), (1, 0, 1), (1, 2, 4), (u64::MAX, 1, 0)] {
            assert_eq!(
                mount_ready::try_new(tuple.0, tuple.1, tuple.2),
                Err(codec_error::fields)
            );
        }
        let original = mount_ready::try_new(2, 2, 4).unwrap().encode();
        for offset in 32..mount_ready_size {
            let mut bytes = original;
            bytes[offset] = 1;
            assert_eq!(mount_ready::decode(&bytes), Err(codec_error::reserved));
        }
        for offset in 4..8 {
            let mut bytes = original;
            bytes[offset] ^= 1;
            assert_eq!(mount_ready::decode(&bytes), Err(codec_error::version));
        }
        assert_eq!(
            mount_ready::decode(&original[..127]),
            Err(codec_error::length)
        );
        assert_eq!(mount_ready::decode(&[0; 129]), Err(codec_error::length));
    }
}
