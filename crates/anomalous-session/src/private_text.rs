use core::fmt;

pub const private_text_capacity: usize = 256;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum private_text_error {
    empty,
    invalid_utf8,
    limit_exceeded,
    forbidden_scalar,
}

impl private_text_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::empty => "empty_text",
            Self::invalid_utf8 => "invalid_utf8",
            Self::limit_exceeded => "text_limit_exceeded",
            Self::forbidden_scalar => "forbidden_scalar",
        }
    }
}

impl fmt::Debug for private_text_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for private_text_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// One move-only, bounded UTF-8 fragment owned by a session operation.
///
/// The type deliberately has no `Clone`, `Copy`, `Debug`, `Display`, or `Hash`
/// implementation. [`Self::clear`] is a logical owned-storage clear, not a
/// secure-erasure claim for caller copies, compiler temporaries, or hardware.
/// [`Self::expose_str`] and [`Self::expose_byte_len`] are explicit local
/// disclosure boundaries: callers decide where private content and its exact
/// byte length may flow and how long either may remain.
pub struct private_text {
    bytes: [u8; private_text_capacity],
    length: u16,
}

impl private_text {
    pub fn try_from_str(text: &str) -> Result<Self, private_text_error> {
        Self::try_from_bytes(text.as_bytes())
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, private_text_error> {
        if bytes.is_empty() {
            return Err(private_text_error::empty);
        }
        if bytes.len() > private_text_capacity {
            return Err(private_text_error::limit_exceeded);
        }
        let text = core::str::from_utf8(bytes).map_err(|_| private_text_error::invalid_utf8)?;
        if text.chars().any(forbidden_scalar) {
            return Err(private_text_error::forbidden_scalar);
        }

        let mut owned = Self::empty();
        owned.bytes[..bytes.len()].copy_from_slice(bytes);
        owned.length = bytes.len() as u16;
        Ok(owned)
    }

    #[must_use]
    pub fn expose_str(&self) -> &str {
        match core::str::from_utf8(&self.bytes[..self.expose_byte_len()]) {
            Ok(text) => text,
            Err(_) => panic!("private text invariant violated"),
        }
    }

    #[must_use]
    pub const fn expose_byte_len(&self) -> usize {
        self.length as usize
    }

    #[must_use]
    pub(crate) const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn clear(&mut self) {
        self.bytes.fill(0);
        self.length = 0;
    }

    pub(crate) const fn empty() -> Self {
        Self {
            bytes: [0; private_text_capacity],
            length: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn storage_is_clear(&self) -> bool {
        self.length == 0 && self.bytes.iter().all(|byte| *byte == 0)
    }

    #[cfg(test)]
    pub(crate) fn unused_tail_is_clear(&self) -> bool {
        self.bytes[self.expose_byte_len()..]
            .iter()
            .all(|byte| *byte == 0)
    }
}

impl Drop for private_text {
    fn drop(&mut self) {
        self.clear();
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{0008}'
            | '\u{000b}'..='\u{001f}'
            | '\u{007f}'..='\u{009f}'
            | '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{feff}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
    )
}

#[cfg(test)]
mod tests {
    use super::{forbidden_scalar, private_text, private_text_capacity, private_text_error};

    fn frozen_reference(value: u32) -> bool {
        matches!(
            value,
            0x0000..=0x0008
                | 0x000b..=0x001f
                | 0x007f..=0x009f
                | 0x00ad
                | 0x034f
                | 0x061c
                | 0x180e
                | 0xfeff
                | 0x200b..=0x200f
                | 0x202a..=0x202e
                | 0x2060..=0x206f
        )
    }

    #[test]
    fn valid_text_is_exact_and_has_a_zero_tail() {
        let text = private_text::try_from_str("human λ\tmachine\n").expect("valid text");
        assert!(text.expose_str() == "human λ\tmachine\n");
        assert!(text.expose_byte_len() == 17);
        assert!(text.unused_tail_is_clear());
    }

    #[test]
    fn exact_capacity_is_accepted_without_truncation() {
        let bytes = [b'a'; private_text_capacity];
        let text = private_text::try_from_bytes(&bytes).expect("exact capacity");
        assert!(text.expose_byte_len() == private_text_capacity);
        assert!(text.expose_str().as_bytes() == bytes);
        assert!(text.unused_tail_is_clear());
    }

    #[test]
    fn empty_malformed_and_oversized_inputs_are_distinct() {
        assert!(matches!(
            private_text::try_from_bytes(&[]),
            Err(private_text_error::empty)
        ));
        assert!(matches!(
            private_text::try_from_bytes(&[0xf0, 0x28, 0x8c, 0x28]),
            Err(private_text_error::invalid_utf8)
        ));
        let oversized = [b'a'; private_text_capacity + 1];
        assert!(matches!(
            private_text::try_from_bytes(&oversized),
            Err(private_text_error::limit_exceeded)
        ));
    }

    #[test]
    fn every_frozen_forbidden_scalar_is_rejected_by_public_constructor() {
        let mut rejected = 0;
        for value in 0..=0x10ffff {
            if !frozen_reference(value) {
                continue;
            }
            let scalar = char::from_u32(value).expect("forbidden value is a scalar");
            let mut encoded = [0; 4];
            let text = scalar.encode_utf8(&mut encoded);
            assert!(matches!(
                private_text::try_from_str(text),
                Err(private_text_error::forbidden_scalar)
            ));
            rejected += 1;
        }
        assert!(rejected == 94);
        assert!(matches!(
            private_text::try_from_str("allowed λ before \u{2066} after"),
            Err(private_text_error::forbidden_scalar)
        ));
    }

    #[test]
    fn frozen_policy_matches_every_valid_unicode_scalar() {
        for value in 0..=0x10ffff {
            let Some(scalar) = char::from_u32(value) else {
                continue;
            };
            assert!(forbidden_scalar(scalar) == frozen_reference(value));
        }
    }

    #[test]
    fn adjacent_visible_scalars_and_literal_tab_and_line_feed_are_accepted() {
        for value in [
            '\u{0020}', '\u{00ac}', '\u{00ae}', '\u{034e}', '\u{0350}', '\u{061b}', '\u{061d}',
            '\u{180d}', '\u{180f}', '\u{200a}', '\u{2010}', '\u{2029}', '\u{202f}', '\u{205f}',
            '\u{2070}', '\u{fefe}', '\u{ff00}', '\t', '\n',
        ] {
            let mut encoded = [0; 4];
            let text = value.encode_utf8(&mut encoded);
            assert!(private_text::try_from_str(text).is_ok());
        }
    }

    #[test]
    fn explicit_clear_zeroes_all_owned_storage() {
        let mut text = private_text::try_from_str("private sentinel").expect("valid text");
        text.clear();
        assert!(text.is_empty());
        assert!(text.storage_is_clear());
    }

    #[test]
    fn errors_format_as_static_content_free_codes() {
        assert!(std::format!("{}", private_text_error::forbidden_scalar) == "forbidden_scalar");
        assert!(std::format!("{:?}", private_text_error::invalid_utf8) == "invalid_utf8");
    }
}
