//! Experimental three-owner copied startup metadata, never a route/device grant.
//! The kernel independently retains task, channel and device ownership. No text
//! or expected recovery answer is present in this descriptor.

use crate::{client_identity, codec_error, durable::disk_identity};

pub const startup_size: usize = 192;
pub const application_arena_byte_count: u64 = 32_768;
pub const workspace_arena_byte_count: u64 = 36_864;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    words: [u64; 19],
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
        connection: u64,
        workspace: [u64; 2],
        route_generations: [u64; 2],
    ) -> Result<Self, codec_error> {
        let expected_arena = if role == 0 {
            workspace_arena_byte_count
        } else {
            application_arena_byte_count
        };
        if role > 2
            || !(1..=2).contains(&phase)
            || proof_mode > 2
            || (role == 2) != (device_handle != 0)
            || connection == 0
            || workspace.contains(&0)
            || workspace[0] == authority.task()
            || route_generations.contains(&0)
            || arena[0] == 0
            || !arena[0].is_multiple_of(4096)
            || arena[1] != expected_arena
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
            words: [
                role,
                phase,
                arena[0],
                arena[1],
                handles[0],
                handles[1],
                handles[2],
                handles[3],
                authority.task(),
                authority.generation(),
                identity.volume(),
                identity.object(),
                device_handle,
                proof_mode,
                connection,
                workspace[0],
                workspace[1],
                route_generations[0],
                route_generations[1],
            ],
        })
    }
    pub const fn role(&self) -> u64 {
        self.words[0]
    }
    pub const fn phase(&self) -> u64 {
        self.words[1]
    }
    pub const fn arena_base(&self) -> u64 {
        self.words[2]
    }
    pub const fn arena_length(&self) -> u64 {
        self.words[3]
    }
    pub const fn handles(&self) -> [u64; 4] {
        [self.words[4], self.words[5], self.words[6], self.words[7]]
    }
    pub fn authority(&self) -> client_identity {
        client_identity::try_new(self.words[8], self.words[9]).unwrap()
    }
    pub fn identity(&self) -> disk_identity {
        disk_identity::try_new(self.words[10], self.words[11]).unwrap()
    }
    pub const fn device_handle(&self) -> u64 {
        self.words[12]
    }
    pub const fn proof_mode(&self) -> u64 {
        self.words[13]
    }
    pub const fn connection(&self) -> u64 {
        self.words[14]
    }
    pub const fn workspace(&self) -> [u64; 2] {
        [self.words[15], self.words[16]]
    }
    pub const fn route_generations(&self) -> [u64; 2] {
        [self.words[17], self.words[18]]
    }

    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = [0; startup_size];
        bytes[..8].copy_from_slice(&[b'A', b'N', b'E', b'W', 1, 0, 192, 0]);
        for (index, word) in self.words.iter().enumerate() {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != startup_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANEW" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, 192, 0] {
            return Err(codec_error::version);
        }
        if bytes[160..].iter().any(|byte| *byte != 0) {
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
            word(120),
            [word(128), word(136)],
            [word(144), word(152)],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(role: u64, phase: u64, mode: u64) -> startup_descriptor {
        startup_descriptor::try_new(
            role,
            phase,
            [
                0x810000,
                if role == 0 {
                    workspace_arena_byte_count
                } else {
                    application_arena_byte_count
                },
            ],
            [1, 2, 3, 4],
            client_identity::try_new(2, 7).unwrap(),
            disk_identity::try_new(13, 17).unwrap(),
            if role == 2 { 1 } else { 0 },
            mode,
            19,
            [1, 5],
            [23, 29],
        )
        .unwrap()
    }

    #[test]
    fn three_roles_modes_and_phases_have_literal_canonical_words() {
        for role in 0..=2 {
            for phase in 1..=2 {
                for mode in 0..=2 {
                    let value = descriptor(role, phase, mode);
                    let mut expected = [0; 192];
                    expected[..8].copy_from_slice(&[65, 78, 69, 87, 1, 0, 192, 0]);
                    let arena = if role == 0 { 36_864 } else { 32_768 };
                    let device = if role == 2 { 1 } else { 0 };
                    for (index, word) in [
                        role, phase, 0x810000, arena, 1, 2, 3, 4, 2, 7, 13, 17, device, mode, 19,
                        1, 5, 23, 29,
                    ]
                    .iter()
                    .enumerate()
                    {
                        expected[8 + index * 8..16 + index * 8]
                            .copy_from_slice(&word.to_le_bytes());
                    }
                    assert_eq!(value.encode(), expected);
                    assert_eq!(startup_descriptor::decode(&expected), Ok(value));
                }
            }
        }
    }

    #[test]
    fn malformed_envelopes_roles_routes_and_extents_are_rejected() {
        let bytes = descriptor(0, 1, 0).encode();
        for length in 0..192 {
            assert!(startup_descriptor::decode(&bytes[..length]).is_err());
        }
        assert_eq!(
            startup_descriptor::decode(&[0; 193]),
            Err(codec_error::length)
        );
        for offset in 0..8 {
            let mut changed = bytes;
            changed[offset] ^= 1;
            assert!(startup_descriptor::decode(&changed).is_err());
        }
        for offset in 160..192 {
            let mut changed = bytes;
            changed[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&changed),
                Err(codec_error::reserved)
            );
        }
        for (offset, word) in [
            (8, 3u64),
            (16, 0),
            (16, 3),
            (24, 0),
            (24, 1),
            (24, (1 << 47) - 4096),
            (32, 32768),
            (40, 0),
            (40, 2),
            (72, 0),
            (80, 0),
            (88, 0),
            (96, 0),
            (104, 1),
            (112, 3),
            (120, 0),
            (128, 0),
            (128, 2),
            (136, 0),
            (144, 0),
            (152, 0),
        ] {
            let mut changed = bytes;
            changed[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
            assert_eq!(
                startup_descriptor::decode(&changed),
                Err(codec_error::fields)
            );
        }
        let mut changed = descriptor(2, 2, 2).encode();
        changed[104..112].fill(0);
        assert_eq!(
            startup_descriptor::decode(&changed),
            Err(codec_error::fields)
        );
    }
}
