//! Fixed native manifest-loader diagnostic report, not an admission witness.
//! Any task can write bytes with this shape. Decoding checks their canonical
//! representation only; it creates no capability, signature or kernel grant.
//! Semantic admission remains in the userspace manifest crate.

pub const report_bytes: usize = 768;
pub const package_offset: usize = 4096;
pub const console_scratch_offset: usize = 1024;
pub const magic: &[u8; 8] = b"ANMRPT01";
pub const version: u16 = 1;
pub const console_marker: &[u8] = b"Native manifest: console admitted\n";

pub mod offset {
    pub const version: usize = 8;
    pub const length: usize = 10;
    pub const status: usize = 12;
    pub const reason: usize = 16;
    /// All flags are zero: in particular, no edition has executed.
    pub const flags: usize = 20;
    pub const ceiling: usize = 24;
    pub const required_services: usize = 32;
    pub const granted_services: usize = 40;
    pub const edition_length: usize = 48;
    pub const image_length: usize = 56;
    pub const read_calls: usize = 64;
    pub const read_bytes: usize = 72;
    pub const console_calls: usize = 80;
    pub const identity: usize = 88;
    pub const intent_length: usize = 120;
    pub const intent: usize = 128;
}

pub mod status {
    pub const admitted: u32 = 1;
    pub const refused: u32 = 2;
}

pub mod reason {
    pub const none: u32 = 0;
    pub const edition: u32 = 1;
    pub const syntax: u32 = 2;
    pub const unsupported: u32 = 3;
    pub const intent: u32 = 4;
    pub const limit: u32 = 5;
    pub const image: u32 = 6;
    pub const encoding: u32 = 7;
    pub const version: u32 = 8;
    pub const profile: u32 = 9;
    pub const layout: u32 = 10;
    pub const identity: u32 = 11;
    pub const mismatch: u32 = 12;
    pub const ceiling: u32 = 13;
    pub const output: u32 = 14;

    pub const fn as_str(value: u32) -> Option<&'static str> {
        Some(match value {
            none => "none",
            edition => "manifest_edition",
            syntax => "manifest_syntax",
            unsupported => "manifest_unsupported",
            intent => "manifest_intent",
            limit => "manifest_limit",
            image => "manifest_image",
            encoding => "manifest_encoding",
            version => "manifest_version",
            profile => "manifest_profile",
            layout => "manifest_layout",
            identity => "manifest_identity",
            mismatch => "manifest_mismatch",
            ceiling => "manifest_ceiling",
            output => "manifest_output",
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct report_error;

/// A borrowed view of a canonical diagnostic, never proof of its truth.
#[derive(Debug)]
pub struct report<'a> {
    bytes: &'a [u8],
}

impl<'a> report<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, report_error> {
        if bytes.len() != report_bytes
            || &bytes[..8] != magic
            || u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != version
            || u16::from_le_bytes(bytes[10..12].try_into().unwrap()) != report_bytes as u16
        {
            return Err(report_error);
        }
        let value = Self { bytes };
        let intent_length = value.word32(offset::intent_length) as usize;
        if value.word32(offset::flags) != 0
            || value.ceiling() > 1
            || value.image_length() == 0
            || value.image_length() > 66_208
            || value.read_bytes() != value.image_length()
            || value.read_calls() != value.image_length().div_ceil(4096)
            || intent_length > 512
            || bytes[124..128].iter().any(|byte| *byte != 0)
            || bytes[offset::intent + intent_length..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(report_error);
        }
        match value.status() {
            status::admitted => {
                if value.reason() != reason::none
                    || value.required_services() > 1
                    || value.required_services() & !value.ceiling() != 0
                    || value.granted_services() != value.required_services()
                    || value.console_calls() != value.granted_services()
                    || value.edition_length() == 0
                    || value.edition_length() > 65_536
                    || value.edition_length() >= value.image_length()
                    || intent_length == 0
                {
                    return Err(report_error);
                }
                let intent =
                    core::str::from_utf8(&bytes[offset::intent..offset::intent + intent_length])
                        .map_err(|_| report_error)?;
                if intent
                    .chars()
                    .any(|ch| ch.is_control() || ch == '"' || ch == '\\')
                {
                    return Err(report_error);
                }
            }
            status::refused => {
                if value.reason() == reason::none
                    || reason::as_str(value.reason()).is_none()
                    || value.required_services() != 0
                    || value.granted_services() != 0
                    || value.edition_length() != 0
                    || value.console_calls() != 0
                    || intent_length != 0
                    || value.identity().iter().any(|byte| *byte != 0)
                {
                    return Err(report_error);
                }
            }
            _ => return Err(report_error),
        }
        Ok(value)
    }

    fn word32(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap())
    }
    fn word64(&self, at: usize) -> u64 {
        u64::from_le_bytes(self.bytes[at..at + 8].try_into().unwrap())
    }
    pub fn status(&self) -> u32 {
        self.word32(offset::status)
    }
    pub fn reason(&self) -> u32 {
        self.word32(offset::reason)
    }
    pub fn ceiling(&self) -> u64 {
        self.word64(offset::ceiling)
    }
    pub fn required_services(&self) -> u64 {
        self.word64(offset::required_services)
    }
    pub fn granted_services(&self) -> u64 {
        self.word64(offset::granted_services)
    }
    pub fn edition_length(&self) -> u64 {
        self.word64(offset::edition_length)
    }
    pub fn image_length(&self) -> u64 {
        self.word64(offset::image_length)
    }
    pub fn read_calls(&self) -> u64 {
        self.word64(offset::read_calls)
    }
    pub fn read_bytes(&self) -> u64 {
        self.word64(offset::read_bytes)
    }
    pub fn console_calls(&self) -> u64 {
        self.word64(offset::console_calls)
    }
    pub fn identity(&self) -> &[u8; 32] {
        self.bytes[offset::identity..offset::identity + 32]
            .try_into()
            .unwrap()
    }
    pub fn intent(&self) -> &str {
        let length = self.word32(offset::intent_length) as usize;
        core::str::from_utf8(&self.bytes[offset::intent..offset::intent + length]).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> [u8; report_bytes] {
        let mut bytes = [0; report_bytes];
        bytes[..8].copy_from_slice(magic);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&768u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
        for (at, value) in [(24, 1u64), (48, 300), (56, 5000), (64, 2), (72, 5000)] {
            bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[88..120].fill(0x42);
        bytes[120..124].copy_from_slice(&4u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"Pure");
        bytes
    }

    #[test]
    fn accepted_report_is_a_strict_diagnostic_with_no_execution_flag() {
        let bytes = fixture();
        let value = report::decode(&bytes).unwrap();
        assert_eq!(value.intent(), "Pure");
        assert_eq!(value.read_calls(), 2);
        assert_eq!(value.granted_services(), 0);
        assert_eq!(value.console_calls(), 0);
        for at in [
            0, 8, 10, 12, 16, 20, 28, 32, 40, 55, 64, 72, 80, 123, 124, 132, 767,
        ] {
            let mut changed = bytes;
            changed[at] ^= 1;
            assert!(report::decode(&changed).is_err(), "byte {at}");
        }
        assert!(report::decode(&bytes[..767]).is_err());
        let mut console = bytes;
        for at in [32, 40, 80] {
            console[at..at + 8].copy_from_slice(&1u64.to_le_bytes());
        }
        assert_eq!(report::decode(&console).unwrap().console_calls(), 1);
        console[24] = 0;
        assert!(report::decode(&console).is_err());
    }

    #[test]
    fn refusal_carries_no_unverified_claims_or_console_effects() {
        let mut bytes = fixture();
        bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&12u32.to_le_bytes());
        bytes[48..56].fill(0);
        bytes[88..].fill(0);
        let value = report::decode(&bytes).unwrap();
        assert_eq!(reason::as_str(value.reason()), Some("manifest_mismatch"));
        assert_eq!(value.intent(), "");
        for at in [16, 32, 40, 48, 80, 88, 120, 128] {
            let mut changed = bytes;
            changed[at] = if at == 16 { 0 } else { 1 };
            assert!(report::decode(&changed).is_err(), "byte {at}");
        }
    }
}
