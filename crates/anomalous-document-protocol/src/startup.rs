use crate::{client_identity, codec_error, text_capacity, validate_text};

pub const startup_size: usize = 128;
pub const arena_byte_count: u64 = 32_768;
const user_end: u64 = 1 << 47;

/// Copied native document launch data, not capability authority. Actual route
/// task generations and mapping extents remain independently kernel-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    role: u64,
    scenario: u64,
    arena: [u64; 2],
    handles: [u64; 4],
    document: u64,
    authority: client_identity,
    seed: [u8; text_capacity],
    seed_length: usize,
}

impl startup_descriptor {
    pub fn try_new(
        role: u64,
        scenario: u64,
        arena: [u64; 2],
        handles: [u64; 4],
        document: u64,
        authority: client_identity,
        seed: &[u8],
    ) -> Result<Self, codec_error> {
        if role > 1 || scenario > 3 || document == 0 {
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
        validate_text(seed)?;
        if role == 0 && !seed.is_empty() {
            return Err(codec_error::fields);
        }
        let mut copied_seed = [0; text_capacity];
        copied_seed[..seed.len()].copy_from_slice(seed);
        Ok(Self {
            role,
            scenario,
            arena,
            handles,
            document,
            authority,
            seed: copied_seed,
            seed_length: seed.len(),
        })
    }

    #[must_use]
    pub fn role(&self) -> u64 {
        self.role
    }
    #[must_use]
    pub fn scenario(&self) -> u64 {
        self.scenario
    }
    #[must_use]
    pub fn arena_base(&self) -> u64 {
        self.arena[0]
    }
    #[must_use]
    pub fn arena_length(&self) -> u64 {
        self.arena[1]
    }
    #[must_use]
    pub fn handles(&self) -> [u64; 4] {
        self.handles
    }
    #[must_use]
    pub fn document(&self) -> u64 {
        self.document
    }
    #[must_use]
    pub fn authority(&self) -> client_identity {
        self.authority
    }
    #[must_use]
    pub fn seed(&self) -> &[u8] {
        &self.seed[..self.seed_length]
    }

    #[must_use]
    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = [0; startup_size];
        bytes[..4].copy_from_slice(b"ANDS");
        bytes[4..8].copy_from_slice(&[1, 0, startup_size as u8, 0]);
        let fields = [
            self.role,
            self.scenario,
            self.arena[0],
            self.arena[1],
            self.handles[0],
            self.handles[1],
            self.handles[2],
            self.handles[3],
            self.document,
            self.authority.task(),
            self.authority.generation(),
        ];
        for (index, value) in fields.iter().enumerate() {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[96] = self.seed_length as u8;
        bytes[97..97 + self.seed_length].copy_from_slice(self.seed());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != startup_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDS" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, startup_size as u8, 0] {
            return Err(codec_error::version);
        }
        let length = usize::from(bytes[96]);
        if length > text_capacity {
            return Err(codec_error::text);
        }
        if bytes[97 + length..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        let read = |offset| {
            let mut value = [0; 8];
            value.copy_from_slice(&bytes[offset..offset + 8]);
            u64::from_le_bytes(value)
        };
        Self::try_new(
            read(8),
            read(16),
            [read(24), read(32)],
            [read(40), read(48), read(56), read(64)],
            read(72),
            client_identity::try_new(read(80), read(88))?,
            &bytes[97..97 + length],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(role: u64, seed: &[u8]) -> Result<startup_descriptor, codec_error> {
        startup_descriptor::try_new(
            role,
            0,
            [0x804000, arena_byte_count],
            [0x31, 0x52, 0x73, 0x94],
            0xd001,
            client_identity::try_new(1, 7).unwrap(),
            seed,
        )
    }

    #[test]
    fn round_trip_roles_and_exact_layout_keep_editor_seed_private() {
        for descriptor in [
            descriptor(0, b"").unwrap(),
            descriptor(1, b"const before = 1;").unwrap(),
        ] {
            let bytes = descriptor.encode();
            assert_eq!(startup_descriptor::decode(&bytes), Ok(descriptor));
            assert_eq!(&bytes[72..80], &0xd001u64.to_le_bytes());
            assert_eq!(&bytes[80..88], &1u64.to_le_bytes());
            assert_eq!(&bytes[88..96], &7u64.to_le_bytes());
        }
        assert_eq!(descriptor(0, b"seed"), Err(codec_error::fields));
    }

    #[test]
    fn malformed_fields_extents_handles_and_tail_are_rejected() {
        let valid = descriptor(1, b"hello").unwrap().encode();
        for (offset, value) in [
            (8, 2),
            (16, 4),
            (24, 0),
            (24, 0x804001),
            (24, u64::MAX - 4095),
            (32, 4096),
            (40, 0),
            (48, 0x31),
            (72, 0),
            (80, 0),
            (88, 0),
        ] {
            let mut bytes = valid;
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
        }
        for offset in 102..startup_size {
            let mut bytes = valid;
            bytes[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&bytes),
                Err(codec_error::reserved)
            );
        }
        let mut bytes = valid;
        bytes[96] = 32;
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::text));
        bytes = valid;
        bytes[97] = 0;
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::text));
        assert_eq!(
            startup_descriptor::decode(&valid[..127]),
            Err(codec_error::length)
        );
        assert_eq!(
            startup_descriptor::decode(&[0; 129]),
            Err(codec_error::length)
        );
    }
}
