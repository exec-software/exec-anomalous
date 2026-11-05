//! Qualification-only release of a held first save reply on the authenticated
//! editor-to-service route. This is correlation, not authority, a save request,
//! a durable acknowledgement or part of the unchanged ANDC document protocol.

use crate::{codec_error, operation, record};

pub const release_size: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct held_save_release {
    words: [u64; 6],
}

impl held_save_release {
    pub fn try_new(save: &record, later_revision: u64) -> Result<Self, codec_error> {
        if save.kind() != operation::save || later_revision <= save.revision() {
            return Err(codec_error::fields);
        }
        Ok(Self {
            words: [
                save.document(),
                save.generation(),
                save.request(),
                save.base_version(),
                save.revision(),
                later_revision,
            ],
        })
    }
    pub fn matches_saved(&self, saved: &record) -> bool {
        saved.kind() == operation::saved
            && self.words[..5]
                == [
                    saved.document(),
                    saved.generation(),
                    saved.request(),
                    saved.base_version(),
                    saved.revision(),
                ]
    }
    pub fn matches_request(&self, request: &record) -> bool {
        request.kind() == operation::save
            && self.words[..5]
                == [
                    request.document(),
                    request.generation(),
                    request.request(),
                    request.base_version(),
                    request.revision(),
                ]
    }
    pub fn encode(&self) -> [u8; release_size] {
        let mut bytes = [0; release_size];
        bytes[..8].copy_from_slice(&[b'A', b'N', b'D', b'H', 1, 0, 128, 0]);
        for (index, word) in self.words.iter().enumerate() {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, codec_error> {
        if bytes.len() != release_size {
            return Err(codec_error::length);
        }
        if &bytes[..4] != b"ANDH" {
            return Err(codec_error::magic);
        }
        if bytes[4..8] != [1, 0, 128, 0] {
            return Err(codec_error::version);
        }
        if bytes[56..].iter().any(|byte| *byte != 0) {
            return Err(codec_error::reserved);
        }
        let mut words = [0; 6];
        for (index, word) in words.iter_mut().enumerate() {
            *word = u64::from_le_bytes(bytes[8 + index * 8..16 + index * 8].try_into().unwrap());
        }
        if words[..5].contains(&0) || words[5] <= words[4] {
            return Err(codec_error::fields);
        }
        Ok(Self { words })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_release_correlates_only_with_exact_saved_tuple() {
        let save = record::save(2, 3, 5, 7, 11, b"B").unwrap();
        let release = held_save_release::try_new(&save, 13).unwrap();
        let mut expected = [0; 128];
        expected[..8].copy_from_slice(&[65, 78, 68, 72, 1, 0, 128, 0]);
        for (index, value) in [2u64, 3, 5, 7, 11, 13].iter().enumerate() {
            expected[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(release.encode(), expected);
        assert_eq!(held_save_release::decode(&expected), Ok(release));
        let mut saved_bytes = save.encode();
        saved_bytes[8] = 6;
        saved_bytes[48..56].copy_from_slice(&8u64.to_le_bytes());
        let saved = record::decode(&saved_bytes).unwrap();
        assert!(release.matches_saved(&saved));
        assert!(!release.matches_saved(&save));
        assert!(release.matches_request(&save));
        assert!(!release.matches_request(&saved));
        for offset in [16, 24, 32, 40, 56] {
            let mut bytes = saved.encode();
            bytes[offset] ^= 0x10;
            if let Ok(changed) = record::decode(&bytes) {
                assert!(!release.matches_saved(&changed));
            }
        }
        for later in [0, 10, 11] {
            assert!(held_save_release::try_new(&save, later).is_err());
        }
        assert!(held_save_release::try_new(&saved, 13).is_err());
    }

    #[test]
    fn malformed_release_never_repairs_or_supplies_missing_correlation() {
        let bytes = held_save_release::try_new(&record::save(2, 3, 5, 7, 11, b"B").unwrap(), 13)
            .unwrap()
            .encode();
        for length in 0..128 {
            assert!(held_save_release::decode(&bytes[..length]).is_err());
        }
        assert_eq!(
            held_save_release::decode(&[0; 129]),
            Err(codec_error::length)
        );
        for offset in 0..8 {
            let mut changed = bytes;
            changed[offset] ^= 1;
            assert!(held_save_release::decode(&changed).is_err());
        }
        for offset in 56..128 {
            let mut changed = bytes;
            changed[offset] = 1;
            assert_eq!(
                held_save_release::decode(&changed),
                Err(codec_error::reserved)
            );
        }
        for offset in [8, 16, 24, 32, 40, 48] {
            let mut changed = bytes;
            changed[offset..offset + 8].fill(0);
            assert_eq!(
                held_save_release::decode(&changed),
                Err(codec_error::fields)
            );
        }
    }
}
