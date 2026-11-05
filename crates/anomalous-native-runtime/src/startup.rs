pub const startup_size: usize = 128;
pub const arena_byte_count: u64 = 32768;
const text_capacity: usize = 31;
pub(crate) const user_end: u64 = 0x0000_8000_0000_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum startup_error {
    encoding,
    role,
    scenario,
    arena,
    handles,
    document,
}

/// Copied launch data, never a task identity or a capability by itself. The
/// kernel's exact live binding remains authoritative for each opaque handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    role: u64,
    scenario: u64,
    arena_base: u64,
    arena_length: u64,
    outbound_handle: u64,
    inbound_handle: u64,
    document: [u8; text_capacity],
    document_length: usize,
}

impl startup_descriptor {
    pub fn try_new(
        role: u64,
        scenario: u64,
        arena_base: u64,
        arena_length: u64,
        outbound_handle: u64,
        inbound_handle: u64,
        document: &[u8],
    ) -> Result<Self, startup_error> {
        if role > 1 {
            return Err(startup_error::role);
        }
        if scenario > 3 {
            return Err(startup_error::scenario);
        }
        if arena_base == 0
            || !arena_base.is_multiple_of(4096)
            || arena_length != arena_byte_count
            || arena_base
                .checked_add(arena_length)
                .is_none_or(|end| end > user_end)
        {
            return Err(startup_error::arena);
        }
        if outbound_handle == 0 || inbound_handle == 0 || outbound_handle == inbound_handle {
            return Err(startup_error::handles);
        }
        if document.len() > text_capacity
            || document.iter().any(|byte| !(b' '..=b'~').contains(byte))
        {
            return Err(startup_error::document);
        }
        let mut text = [0; text_capacity];
        text[..document.len()].copy_from_slice(document);
        Ok(Self {
            role,
            scenario,
            arena_base,
            arena_length,
            outbound_handle,
            inbound_handle,
            document: text,
            document_length: document.len(),
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, startup_error> {
        if bytes.len() != startup_size
            || &bytes[..4] != b"ANRT"
            || bytes[4..8] != [1, 0, 128, 0]
            || bytes[56] as usize > text_capacity
        {
            return Err(startup_error::encoding);
        }
        let length = usize::from(bytes[56]);
        if bytes[57 + length..].iter().any(|byte| *byte != 0) {
            return Err(startup_error::encoding);
        }
        let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        Self::try_new(
            word(8),
            word(16),
            word(24),
            word(32),
            word(40),
            word(48),
            &bytes[57..57 + length],
        )
    }

    #[must_use]
    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = [0; startup_size];
        bytes[..4].copy_from_slice(b"ANRT");
        bytes[4..8].copy_from_slice(&[1, 0, 128, 0]);
        for (offset, value) in [
            (8, self.role),
            (16, self.scenario),
            (24, self.arena_base),
            (32, self.arena_length),
            (40, self.outbound_handle),
            (48, self.inbound_handle),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[56] = self.document_length as u8;
        bytes[57..57 + self.document_length].copy_from_slice(self.document());
        bytes
    }

    #[must_use]
    pub const fn role(&self) -> u64 {
        self.role
    }
    #[must_use]
    pub const fn scenario(&self) -> u64 {
        self.scenario
    }
    #[must_use]
    pub const fn arena_base(&self) -> u64 {
        self.arena_base
    }
    #[must_use]
    pub const fn arena_length(&self) -> u64 {
        self.arena_length
    }
    #[must_use]
    pub const fn outbound_handle(&self) -> u64 {
        self.outbound_handle
    }
    #[must_use]
    pub const fn inbound_handle(&self) -> u64 {
        self.inbound_handle
    }
    #[must_use]
    pub fn document(&self) -> &[u8] {
        &self.document[..self.document_length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> startup_descriptor {
        startup_descriptor::try_new(
            0,
            0,
            0xa0_0000,
            arena_byte_count,
            0x31,
            0x52,
            b"const before = 1;",
        )
        .unwrap()
    }

    #[test]
    fn canonical_roundtrip_preserves_variable_documents_and_handles() {
        for document in [b"const before = 1;".as_slice(), b"const before = 2;", b""] {
            let value = startup_descriptor::try_new(
                1,
                1,
                0xc0_0000,
                arena_byte_count,
                0x91,
                0xb2,
                document,
            )
            .unwrap();
            assert_eq!(startup_descriptor::decode(&value.encode()), Ok(value));
            assert_eq!(value.document(), document);
            assert_eq!(value.outbound_handle(), 0x91);
            assert_eq!(value.inbound_handle(), 0xb2);
        }
    }

    #[test]
    fn every_truncation_and_reserved_encoding_is_rejected() {
        let bytes = valid().encode();
        for length in 0..startup_size {
            assert!(startup_descriptor::decode(&bytes[..length]).is_err());
        }
        for index in [0, 4, 5, 6, 7, 56, 87, 88, 127] {
            let mut changed = bytes;
            changed[index] ^= 0x80;
            assert!(
                startup_descriptor::decode(&changed).is_err(),
                "index {index}"
            );
        }
        let mut extra = [0; startup_size + 1];
        extra[..startup_size].copy_from_slice(&bytes);
        assert_eq!(
            startup_descriptor::decode(&extra),
            Err(startup_error::encoding)
        );
    }

    #[test]
    fn invalid_geometry_roles_handles_and_text_are_not_admitted() {
        let bytes = valid().encode();
        for (offset, value, error) in [
            (8, 2, startup_error::role),
            (16, 4, startup_error::scenario),
            (24, 0, startup_error::arena),
            (24, 0xa0_0001, startup_error::arena),
            (24, u64::MAX - 4095, startup_error::arena),
            (24, user_end, startup_error::arena),
            (32, arena_byte_count - 1, startup_error::arena),
            (40, 0, startup_error::handles),
            (48, 0x31, startup_error::handles),
        ] {
            let mut changed = bytes;
            changed[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
            assert_eq!(startup_descriptor::decode(&changed), Err(error));
        }
        for text in [b"\0".as_slice(), b"\n", &[0x80], &[b'a'; 32]] {
            assert_eq!(
                startup_descriptor::try_new(0, 0, 0xa0_0000, arena_byte_count, 1, 2, text),
                Err(startup_error::document)
            );
        }
    }
}
