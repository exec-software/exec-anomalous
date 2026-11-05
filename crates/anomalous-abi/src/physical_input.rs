//! Copied physical events for the experimental native desktop owner.
//! These bytes carry observations, never task, focus, or action authority.

pub const rite_physical_input_v1_size: usize = 48;

pub struct rite_physical_input_v1 {
    bytes: [u8; rite_physical_input_v1_size],
}

impl rite_physical_input_v1 {
    pub fn key(
        generation: u32,
        sequence: u64,
        code: u8,
        extended: bool,
        pressed: bool,
    ) -> Option<Self> {
        if generation == 0 || code == 0 || code >= 128 {
            return None;
        }
        let mut value = Self::header(generation);
        value.bytes[4] = 1;
        value.bytes[5] = if pressed { 1 } else { 2 };
        value.bytes[6] = code;
        value.bytes[7] = u8::from(extended);
        value.bytes[16..24].copy_from_slice(&sequence.to_le_bytes());
        value.bytes[24..32].copy_from_slice(&sequence.to_le_bytes());
        Some(value)
    }

    pub fn loss(
        generation: u32,
        new_generation: u32,
        first: u64,
        last: u64,
        count: u64,
    ) -> Option<Self> {
        if generation == 0 || new_generation <= generation || first > last || count == 0 {
            return None;
        }
        let mut value = Self::header(generation);
        value.bytes[4] = 2;
        value.bytes[12..16].copy_from_slice(&new_generation.to_le_bytes());
        value.bytes[16..24].copy_from_slice(&first.to_le_bytes());
        value.bytes[24..32].copy_from_slice(&last.to_le_bytes());
        value.bytes[32..40].copy_from_slice(&count.to_le_bytes());
        Some(value)
    }

    /// Relative physical motion; positive Y points down in desktop coordinates.
    pub fn pointer(generation: u32, sequence: u64, dx: i16, dy: i16, buttons: u8) -> Option<Self> {
        if generation == 0 || buttons & !7 != 0 {
            return None;
        }
        let mut value = Self::header(generation);
        value.bytes[4] = 3;
        value.bytes[5] = buttons;
        value.bytes[16..24].copy_from_slice(&sequence.to_le_bytes());
        value.bytes[24..32].copy_from_slice(&sequence.to_le_bytes());
        value.bytes[32..34].copy_from_slice(&dx.to_le_bytes());
        value.bytes[34..36].copy_from_slice(&dy.to_le_bytes());
        Some(value)
    }

    pub fn pointer_loss(
        generation: u32,
        new_generation: u32,
        first: u64,
        last: u64,
        count: u64,
    ) -> Option<Self> {
        let mut value = Self::loss(generation, new_generation, first, last, count)?;
        value.bytes[4] = 4;
        Some(value)
    }

    fn header(generation: u32) -> Self {
        let mut bytes = [0; rite_physical_input_v1_size];
        bytes[0..2].copy_from_slice(&(rite_physical_input_v1_size as u16).to_le_bytes());
        bytes[2..4].copy_from_slice(&1_u16.to_le_bytes());
        bytes[8..12].copy_from_slice(&generation.to_le_bytes());
        Self { bytes }
    }

    pub fn decode(bytes: [u8; rite_physical_input_v1_size]) -> Option<Self> {
        if bytes[0..2] != (rite_physical_input_v1_size as u16).to_le_bytes()
            || bytes[2..4] != 1_u16.to_le_bytes()
            || bytes[40..48] != [0; 8]
        {
            return None;
        }
        let value = Self { bytes };
        let expected = match value.bytes[4] {
            1 if matches!(value.bytes[5], 1 | 2) && value.bytes[7] <= 1 => Self::key(
                value.source_generation(),
                value.sequence(),
                value.make_code(),
                value.extended(),
                value.pressed(),
            )?,
            2 => Self::loss(
                value.source_generation(),
                value.new_generation(),
                value.sequence(),
                value.last_sequence(),
                value.lost_count(),
            )?,
            3 => Self::pointer(
                value.source_generation(),
                value.sequence(),
                value.delta_x(),
                value.delta_y(),
                value.buttons(),
            )?,
            4 => Self::pointer_loss(
                value.source_generation(),
                value.new_generation(),
                value.sequence(),
                value.last_sequence(),
                value.lost_count(),
            )?,
            _ => return None,
        };
        (expected.bytes == value.bytes).then_some(value)
    }

    pub const fn encode(&self) -> &[u8; rite_physical_input_v1_size] {
        &self.bytes
    }
    pub const fn is_loss(&self) -> bool {
        self.bytes[4] == 2
    }
    pub const fn is_pointer(&self) -> bool {
        self.bytes[4] == 3
    }
    pub const fn is_pointer_loss(&self) -> bool {
        self.bytes[4] == 4
    }
    pub fn delta_x(&self) -> i16 {
        i16::from_le_bytes(self.bytes[32..34].try_into().unwrap())
    }
    pub fn delta_y(&self) -> i16 {
        i16::from_le_bytes(self.bytes[34..36].try_into().unwrap())
    }
    pub const fn buttons(&self) -> u8 {
        self.bytes[5]
    }
    pub const fn pressed(&self) -> bool {
        self.bytes[5] == 1
    }
    pub const fn make_code(&self) -> u8 {
        self.bytes[6]
    }
    pub const fn extended(&self) -> bool {
        self.bytes[7] == 1
    }
    pub fn source_generation(&self) -> u32 {
        u32::from_le_bytes(self.bytes[8..12].try_into().unwrap())
    }
    pub fn new_generation(&self) -> u32 {
        u32::from_le_bytes(self.bytes[12..16].try_into().unwrap())
    }
    pub fn sequence(&self) -> u64 {
        u64::from_le_bytes(self.bytes[16..24].try_into().unwrap())
    }
    pub fn last_sequence(&self) -> u64 {
        u64::from_le_bytes(self.bytes[24..32].try_into().unwrap())
    }
    pub fn lost_count(&self) -> u64 {
        u64::from_le_bytes(self.bytes[32..40].try_into().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_and_loss_have_exact_validated_roundtrips() {
        for input in [
            rite_physical_input_v1::key(1, 3, 0x1c, false, true).unwrap(),
            rite_physical_input_v1::key(7, u64::MAX, 0x38, true, false).unwrap(),
            rite_physical_input_v1::loss(1, 2, 100, 107, 8).unwrap(),
            rite_physical_input_v1::pointer(4, 12, -256, 255, 7).unwrap(),
            rite_physical_input_v1::pointer_loss(4, 5, 12, 13, 2).unwrap(),
        ] {
            let decoded = rite_physical_input_v1::decode(*input.encode()).unwrap();
            assert_eq!(input.encode(), decoded.encode());
            for offset in [0, 2, 40, 47] {
                let mut invalid = *input.encode();
                invalid[offset] ^= 1;
                assert!(rite_physical_input_v1::decode(invalid).is_none());
            }
        }
        assert!(rite_physical_input_v1::key(0, 0, 1, false, true).is_none());
        assert!(rite_physical_input_v1::key(1, 0, 128, false, true).is_none());
        assert!(rite_physical_input_v1::loss(2, 2, 0, 1, 1).is_none());
        assert!(rite_physical_input_v1::loss(1, 2, 2, 1, 1).is_none());
    }

    #[test]
    fn key_rejects_loss_metadata_bad_discriminants_and_reserved_bytes() {
        let key = rite_physical_input_v1::key(1, 7, 0x2a, false, true).unwrap();
        for (offset, value) in [
            (4, 3),
            (5, 0),
            (5, 3),
            (6, 0),
            (6, 128),
            (7, 2),
            (12, 1),
            (24, 8),
            (32, 1),
            (40, 1),
            (47, 1),
        ] {
            let mut bytes = *key.encode();
            bytes[offset] = value;
            assert!(rite_physical_input_v1::decode(bytes).is_none());
        }
    }

    #[test]
    fn loss_rejects_key_metadata_nonadvancing_generation_and_empty_or_reversed_gap() {
        let loss = rite_physical_input_v1::loss(1, 2, 8, 9, 2).unwrap();
        for (offset, value) in [
            (5, 1),
            (6, 0x2a),
            (7, 1),
            (8, 0),
            (12, 1),
            (16, 10),
            (32, 0),
            (40, 1),
        ] {
            let mut bytes = *loss.encode();
            bytes[offset] = value;
            assert!(rite_physical_input_v1::decode(bytes).is_none());
        }
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;
    #[test]
    fn pointer_is_distinct_and_reserved_bytes_are_rejected() {
        let pointer = rite_physical_input_v1::pointer(7, 42, -255, 256, 5).unwrap();
        assert!(pointer.is_pointer());
        assert!(!pointer.is_loss());
        assert!(!pointer.is_pointer_loss());
        assert_eq!(
            (pointer.delta_x(), pointer.delta_y(), pointer.buttons()),
            (-255, 256, 5)
        );
        for offset in [6, 7, 12, 15, 36, 39, 40, 47] {
            let mut bytes = *pointer.encode();
            bytes[offset] = 1;
            assert!(rite_physical_input_v1::decode(bytes).is_none());
        }
        assert!(rite_physical_input_v1::pointer(1, 0, 0, 0, 8).is_none());
        assert!(rite_physical_input_v1::pointer(0, 0, 0, 0, 0).is_none());
        let loss = rite_physical_input_v1::pointer_loss(7, 8, 42, 42, 1).unwrap();
        assert!(loss.is_pointer_loss());
        assert!(!loss.is_pointer());
        assert!(!loss.is_loss());
        let mut bytes = *loss.encode();
        bytes[5] = 1;
        assert!(rite_physical_input_v1::decode(bytes).is_none());
    }
}
