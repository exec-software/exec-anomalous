use core::fmt;

pub const scene_text_limit: usize = 96;

pub(crate) const protected_redaction: &str = "Protected status";

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum scene_text_error {
    empty,
    invalid_utf8,
    limit_exceeded,
    forbidden_scalar,
}

impl scene_text_error {
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

impl fmt::Debug for scene_text_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for scene_text_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

struct bounded_scene_text {
    bytes: [u8; scene_text_limit],
    length: u8,
}

impl bounded_scene_text {
    fn try_from_str(text: &str) -> Result<Self, scene_text_error> {
        Self::try_from_bytes(text.as_bytes())
    }

    fn try_from_bytes(bytes: &[u8]) -> Result<Self, scene_text_error> {
        if bytes.is_empty() {
            return Err(scene_text_error::empty);
        }
        if bytes.len() > scene_text_limit {
            return Err(scene_text_error::limit_exceeded);
        }
        let text = core::str::from_utf8(bytes).map_err(|_| scene_text_error::invalid_utf8)?;
        if text.chars().any(forbidden_scalar) {
            return Err(scene_text_error::forbidden_scalar);
        }

        let mut owned = Self {
            bytes: [0; scene_text_limit],
            length: 0,
        };
        owned.bytes[..bytes.len()].copy_from_slice(bytes);
        owned.length = bytes.len() as u8;
        Ok(owned)
    }

    fn expose_str(&self) -> &str {
        match core::str::from_utf8(&self.bytes[..self.length as usize]) {
            Ok(text) => text,
            Err(_) => panic!("scene text invariant violated"),
        }
    }

    const fn expose_byte_len(&self) -> usize {
        self.length as usize
    }

    const fn is_empty(&self) -> bool {
        self.length == 0
    }

    fn clear(&mut self) {
        self.bytes.fill(0);
        self.length = 0;
    }

    #[cfg(test)]
    fn storage_is_clear(&self) -> bool {
        self.length == 0 && self.bytes.iter().all(|byte| *byte == 0)
    }

    #[cfg(test)]
    fn unused_tail_is_clear(&self) -> bool {
        self.bytes[self.length as usize..]
            .iter()
            .all(|byte| *byte == 0)
    }
}

impl Drop for bounded_scene_text {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Move-only one-line text the caller explicitly declares public.
///
/// This assertion does not authorize logging, persistence, transport, or
/// capture. The getters are explicit local disclosure boundaries.
pub struct declared_public_text(bounded_scene_text);

impl declared_public_text {
    pub fn try_from_str(text: &str) -> Result<Self, scene_text_error> {
        bounded_scene_text::try_from_str(text).map(Self)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, scene_text_error> {
        bounded_scene_text::try_from_bytes(bytes).map(Self)
    }

    #[must_use]
    pub fn expose_str(&self) -> &str {
        self.0.expose_str()
    }

    #[must_use]
    pub const fn expose_byte_len(&self) -> usize {
        self.0.expose_byte_len()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[cfg(test)]
    pub(crate) fn storage_is_clear(&self) -> bool {
        self.0.storage_is_clear()
    }

    #[cfg(test)]
    pub(crate) fn unused_tail_is_clear(&self) -> bool {
        self.0.unused_tail_is_clear()
    }
}

/// Move-only one-line text that must not enter either scene projection.
pub struct protected_text(bounded_scene_text);

impl protected_text {
    pub fn try_from_str(text: &str) -> Result<Self, scene_text_error> {
        bounded_scene_text::try_from_str(text).map(Self)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, scene_text_error> {
        bounded_scene_text::try_from_bytes(bytes).map(Self)
    }

    /// Exposes protected contents only to the current local owner.
    #[must_use]
    pub fn expose_str(&self) -> &str {
        self.0.expose_str()
    }

    /// Exposes the protected byte length only to the current local owner.
    #[must_use]
    pub const fn expose_byte_len(&self) -> usize {
        self.0.expose_byte_len()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[cfg(test)]
    pub(crate) fn storage_is_clear(&self) -> bool {
        self.0.storage_is_clear()
    }

    #[cfg(test)]
    pub(crate) fn unused_tail_is_clear(&self) -> bool {
        self.0.unused_tail_is_clear()
    }
}

/// Move-only label owner carried by one scene node.
pub enum scene_label {
    declared_public(declared_public_text),
    protected(protected_text),
}

impl scene_label {
    pub(crate) const fn is_empty(&self) -> bool {
        match self {
            Self::declared_public(text) => text.0.is_empty(),
            Self::protected(text) => text.0.is_empty(),
        }
    }

    #[must_use]
    pub const fn is_declared_public(&self) -> bool {
        matches!(self, Self::declared_public(_))
    }

    #[must_use]
    pub const fn is_protected(&self) -> bool {
        matches!(self, Self::protected(_))
    }

    /// Exposes declared-public text and never exposes protected text.
    #[must_use]
    pub fn expose_declared_public(&self) -> Option<&str> {
        match self {
            Self::declared_public(text) => Some(text.expose_str()),
            Self::protected(_) => None,
        }
    }

    /// Exposes protected text only to the current local owner.
    #[must_use]
    pub fn expose_protected(&self) -> Option<&str> {
        match self {
            Self::declared_public(_) => None,
            Self::protected(text) => Some(text.expose_str()),
        }
    }

    pub(crate) fn projected_str(&self) -> &str {
        match self {
            Self::declared_public(text) => text.expose_str(),
            Self::protected(_) => protected_redaction,
        }
    }

    #[cfg(test)]
    pub(crate) fn storage_is_clear(&self) -> bool {
        match self {
            Self::declared_public(text) => text.storage_is_clear(),
            Self::protected(text) => text.storage_is_clear(),
        }
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{001f}'
            | '\u{007f}'..='\u{009f}'
            | '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        declared_public_text, forbidden_scalar, protected_text, scene_text_error, scene_text_limit,
    };

    fn frozen_reference(value: u32) -> bool {
        matches!(
            value,
            0x0000..=0x001f
                | 0x007f..=0x009f
                | 0x00ad
                | 0x034f
                | 0x061c
                | 0x180e
                | 0xfeff
                | 0x200b..=0x200f
                | 0x2028..=0x202e
                | 0x2060..=0x206f
                | 0xfff9..=0xfffb
        )
    }

    #[test]
    fn exact_limit_and_adjacent_visible_scalars_are_accepted() {
        let bytes = [b'a'; scene_text_limit];
        let text = declared_public_text::try_from_bytes(&bytes).expect("exact limit");
        assert!(text.expose_str().as_bytes() == bytes);
        assert!(text.unused_tail_is_clear());

        for value in [
            ' ', '\u{00ac}', '\u{034e}', '\u{0350}', '\u{2010}', '\u{2027}', '\u{202f}',
            '\u{2070}', '\u{fff8}', '\u{fffc}',
        ] {
            let mut encoded = [0; 4];
            assert!(declared_public_text::try_from_str(value.encode_utf8(&mut encoded)).is_ok());
        }
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
    fn malformed_empty_oversized_and_forbidden_text_are_distinct() {
        assert!(matches!(
            declared_public_text::try_from_bytes(&[]),
            Err(scene_text_error::empty)
        ));
        assert!(matches!(
            declared_public_text::try_from_bytes(&[0xf0, 0x28, 0x8c, 0x28]),
            Err(scene_text_error::invalid_utf8)
        ));
        let oversized = [b'a'; scene_text_limit + 1];
        assert!(matches!(
            declared_public_text::try_from_bytes(&oversized),
            Err(scene_text_error::limit_exceeded)
        ));
        for text in ["tab\t", "line\n", "bidi\u{202e}", "hidden\u{2060}"] {
            assert!(matches!(
                declared_public_text::try_from_str(text),
                Err(scene_text_error::forbidden_scalar)
            ));
        }
    }

    #[test]
    fn explicit_clear_zeroes_owned_storage() {
        let mut public = declared_public_text::try_from_str("public sentinel").expect("text");
        public.clear();
        assert!(public.storage_is_clear());

        let mut protected = protected_text::try_from_str("protected sentinel").expect("text");
        protected.clear();
        assert!(protected.storage_is_clear());
    }
}
