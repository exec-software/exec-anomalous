//! Copied launch metadata for the bounded durable failure proof. This version
//! adds a case selector to ANDD v1 without changing its meaning or granting
//! authority. A recovery descriptor contains neither expected recovered bytes
//! nor a mount epoch. This is a fixture format, not a stable application ABI.

use crate::{client_identity, codec_error, durable::disk_identity, durable_startup};

pub const startup_size: usize = durable_startup::startup_size;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct startup_descriptor {
    base: durable_startup::startup_descriptor,
    case: u64,
}

impl startup_descriptor {
    pub fn try_new(
        base: durable_startup::startup_descriptor,
        case: u64,
    ) -> Result<Self, codec_error> {
        if !matches!(base.phase(), 0 | 2) || !(1..=7).contains(&case) {
            return Err(codec_error::fields);
        }
        Ok(Self { base, case })
    }

    pub const fn base(&self) -> &durable_startup::startup_descriptor {
        &self.base
    }
    pub const fn case(&self) -> u64 {
        self.case
    }
    pub const fn role(&self) -> u64 {
        self.base.role()
    }
    pub const fn phase(&self) -> u64 {
        self.base.phase()
    }
    pub const fn arena_base(&self) -> u64 {
        self.base.arena_base()
    }
    pub const fn arena_length(&self) -> u64 {
        self.base.arena_length()
    }
    pub const fn handles(&self) -> [u64; 4] {
        self.base.handles()
    }
    pub const fn authority(&self) -> client_identity {
        self.base.authority()
    }
    pub const fn identity(&self) -> disk_identity {
        self.base.identity()
    }
    pub const fn device_handle(&self) -> u64 {
        self.base.device_handle()
    }
    pub fn challenge(&self) -> &[u8] {
        self.base.challenge()
    }

    pub fn encode(&self) -> [u8; startup_size] {
        let mut bytes = self.base.encode();
        bytes[4] = 2;
        bytes[144..152].copy_from_slice(&self.case.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != startup_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDD" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [2, 0, 192, 0] {
            return Err(codec_error::version);
        }
        if bytes[152..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        let mut case = [0; 8];
        case.copy_from_slice(&bytes[144..152]);
        let mut base = [0; startup_size];
        base.copy_from_slice(bytes);
        base[4] = 1;
        base[144..152].fill(0);
        Self::try_new(
            durable_startup::startup_descriptor::decode(&base)?,
            u64::from_le_bytes(case),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(role: u64, phase: u64) -> durable_startup::startup_descriptor {
        durable_startup::startup_descriptor::try_new(
            role,
            phase,
            [0x804000, durable_startup::arena_byte_count],
            [0x31, 0x32, 0x33, 0x34],
            client_identity::try_new(1, 3).unwrap(),
            disk_identity::try_new(7, 11).unwrap(),
            if role == 1 { 0x7001 } else { 0 },
            if role == 0 && phase == 0 {
                b"challenge"
            } else {
                b""
            },
        )
        .unwrap()
    }

    #[test]
    fn every_case_preserves_exact_base_fields_and_round_trips() {
        for role in 0..=1 {
            for phase in [0, 2] {
                for case in 1..=7 {
                    let base = base(role, phase);
                    let descriptor = startup_descriptor::try_new(base, case).unwrap();
                    let bytes = descriptor.encode();
                    assert_eq!(startup_descriptor::decode(&bytes), Ok(descriptor));
                    assert_eq!(descriptor.base(), &base);
                    assert_eq!(descriptor.case(), case);
                    assert_eq!(&bytes[8..144], &base.encode()[8..144]);
                    assert_eq!(&bytes[144..152], &case.to_le_bytes());
                    assert!(bytes[152..].iter().all(|byte| *byte == 0));
                }
            }
        }
    }

    #[test]
    fn versions_are_not_interchangeable() {
        let base = base(0, 0);
        assert_eq!(
            startup_descriptor::decode(&base.encode()),
            Err(codec_error::version)
        );
        let bytes = startup_descriptor::try_new(base, 1).unwrap().encode();
        assert_eq!(
            durable_startup::startup_descriptor::decode(&bytes),
            Err(codec_error::version)
        );
    }

    #[test]
    fn phases_and_case_selectors_are_bounded() {
        assert_eq!(
            startup_descriptor::try_new(base(0, 1), 1),
            Err(codec_error::fields)
        );
        for case in [0, 8, u64::MAX] {
            assert_eq!(
                startup_descriptor::try_new(base(0, 0), case),
                Err(codec_error::fields)
            );
            let mut bytes = startup_descriptor::try_new(base(0, 0), 1).unwrap().encode();
            bytes[144..152].copy_from_slice(&case.to_le_bytes());
            assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
        }
    }

    #[test]
    fn every_reserved_byte_and_header_field_is_validated() {
        let descriptor = startup_descriptor::try_new(base(0, 0), 7).unwrap();
        for offset in 122..144 {
            let mut bytes = descriptor.encode();
            bytes[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&bytes),
                Err(codec_error::reserved)
            );
        }
        for offset in 152..192 {
            let mut bytes = descriptor.encode();
            bytes[offset] = 1;
            assert_eq!(
                startup_descriptor::decode(&bytes),
                Err(codec_error::reserved)
            );
        }
        for offset in 0..8 {
            let mut bytes = descriptor.encode();
            bytes[offset] ^= 1;
            assert!(startup_descriptor::decode(&bytes).is_err());
        }
        assert_eq!(
            startup_descriptor::decode(&descriptor.encode()[..191]),
            Err(codec_error::length)
        );
        assert_eq!(
            startup_descriptor::decode(&[0; 193]),
            Err(codec_error::length)
        );
    }

    #[test]
    fn wrapper_retains_base_authority_and_challenge_validation() {
        let descriptor = startup_descriptor::try_new(base(0, 0), 1).unwrap();
        for offset in [24, 32, 40, 72, 80, 88, 96] {
            let mut bytes = descriptor.encode();
            bytes[offset..offset + 8].fill(0);
            assert!(startup_descriptor::decode(&bytes).is_err());
        }
        let mut bytes = descriptor.encode();
        bytes[104] = 1;
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
        for byte in [0, 0x1f, 0x7f, 0xff] {
            let mut bytes = descriptor.encode();
            bytes[113] = byte;
            assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::text));
        }
        let mut bytes = descriptor.encode();
        bytes[16] = 2;
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
        let mut bytes = startup_descriptor::try_new(base(1, 0), 1).unwrap().encode();
        bytes[112] = 1;
        bytes[113] = b'x';
        assert_eq!(startup_descriptor::decode(&bytes), Err(codec_error::fields));
    }
}
