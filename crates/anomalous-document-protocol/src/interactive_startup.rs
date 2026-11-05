//! Information for the isolated interactive editor, never a device or route grant.
//! No text, expected recovery answer or keyboard policy enters this descriptor.

use crate::{client_identity, codec_error, durable::disk_identity};

pub const startup_size: usize = 192;
pub const arena_byte_count: u64 = 32_768;
pub const ui_width: usize = 128;
pub const ui_height: usize = 48;
pub const ui_scale: usize = 3;
pub const display_width: usize = ui_width * ui_scale;
pub const display_height: usize = ui_height * ui_scale;
pub const row_bytes: usize = display_width * 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    role: u64,
    phase: u64,
    arena: [u64; 2],
    handles: [u64; 4],
    authority: client_identity,
    identity: disk_identity,
    device_handle: u64,
    proof_mode: u64,
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
        proof_mode: u64,
    ) -> Result<Self, codec_error> {
        if role > 1
            || phase > 1
            || proof_mode > 2
            || (role == 1) != (device_handle != 0)
            || arena[0] == 0
            || !arena[0].is_multiple_of(4096)
            || arena[1] != arena_byte_count
            || arena[0]
                .checked_add(arena[1])
                .is_none_or(|end| end > 1 << 47)
        {
            return Err(codec_error::fields);
        }
        for (index, handle) in handles.iter().enumerate() {
            if *handle == 0 || handles[..index].contains(handle) {
                return Err(codec_error::fields);
            }
        }
        Ok(Self {
            role,
            phase,
            arena,
            handles,
            authority,
            identity,
            device_handle,
            proof_mode,
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
    pub const fn proof_mode(&self) -> u64 {
        self.proof_mode
    }
    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = [0; startup_size];
        bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'I', 1, 0, 192, 0]);
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
            self.proof_mode,
        ]
        .iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != startup_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDI" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, 192, 0] {
            return Err(codec_error::version);
        }
        if bytes[120..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        Self::try_new(
            word(8),
            word(16),
            [word(24), word(32)],
            [word(40), word(48), word(56), word(64)],
            client_identity::try_new(word(72), word(80))?,
            disk_identity::try_new(word(88), word(96)).map_err(|_| codec_error::fields)?,
            word(104),
            word(112),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn descriptor(role: u64, phase: u64, proof: u64) -> startup_descriptor {
        startup_descriptor::try_new(
            role,
            phase,
            [0x804000, arena_byte_count],
            [1, 2, 3, 4],
            client_identity::try_new(5, 6).unwrap(),
            disk_identity::try_new(7, 8).unwrap(),
            if role == 1 { 9 } else { 0 },
            proof,
        )
        .unwrap()
    }
    #[test]
    fn all_modes_round_trip_without_text_or_authority_invention() {
        for role in 0..=1 {
            for phase in 0..=1 {
                for proof in 0..=2 {
                    let value = descriptor(role, phase, proof);
                    let bytes = value.encode();
                    assert_eq!(startup_descriptor::decode(&bytes), Ok(value));
                    assert!(bytes[120..].iter().all(|byte| *byte == 0));
                    assert_eq!(bytes[112..120], proof.to_le_bytes());
                }
            }
        }
    }
    #[test]
    fn malformed_lengths_modes_identity_and_reserved_bytes_reject() {
        let bytes = descriptor(0, 0, 0).encode();
        for length in 0..startup_size {
            assert!(startup_descriptor::decode(&bytes[..length]).is_err());
        }
        for offset in 120..startup_size {
            let mut changed = bytes;
            changed[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&changed),
                Err(codec_error::reserved)
            );
        }
        for (offset, value) in [
            (8, 2u64),
            (16, 2),
            (24, 0),
            (32, 8192),
            (40, 2),
            (72, 0),
            (80, 0),
            (88, 0),
            (96, 0),
            (104, 1),
            (112, 3),
        ] {
            let mut changed = bytes;
            changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert_eq!(
                startup_descriptor::decode(&changed),
                Err(codec_error::fields)
            );
        }
    }
}
