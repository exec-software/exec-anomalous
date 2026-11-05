use core::fmt;

use crate::private_text;

pub const erase_scalar_limit: u16 = 256;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum semantic_action_kind {
    insert_text = 1,
    erase_previous = 2,
    submit = 3,
    cancel_submission = 4,
    source_discontinuity = 5,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum submission_origin {
    human_text = 1,
    machine_text = 2,
    paste = 3,
    accessibility = 4,
    remote_text = 5,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum action_error {
    empty_text,
    zero_erase_count,
    erase_limit_exceeded,
}

impl action_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::empty_text => "empty_text",
            Self::zero_erase_count => "zero_erase_count",
            Self::erase_limit_exceeded => "erase_limit_exceeded",
        }
    }
}

impl fmt::Debug for action_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for action_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

/// One move-only semantic editing action.
///
/// Text is data only. A line feed in `insert_text` never changes this action to
/// `submit`, and no action is parsed or evaluated by this crate.
/// Every content-derived getter is an explicit local disclosure boundary:
/// [`Self::kind`] reveals the action discriminator, [`Self::expose_text`]
/// reveals insertion content, and [`Self::expose_erase_count`] reveals the
/// exact edit count. The trusted session consumer owns each disclosure decision.
pub struct semantic_action {
    present: bool,
    kind: semantic_action_kind,
    erase_count: u16,
    text: private_text,
}

impl semantic_action {
    pub fn insert_text(text: private_text) -> Result<Self, action_error> {
        if text.is_empty() {
            return Err(action_error::empty_text);
        }
        Ok(Self {
            present: true,
            kind: semantic_action_kind::insert_text,
            erase_count: 0,
            text,
        })
    }

    pub fn erase_previous(count: u16) -> Result<Self, action_error> {
        if count == 0 {
            return Err(action_error::zero_erase_count);
        }
        if count > erase_scalar_limit {
            return Err(action_error::erase_limit_exceeded);
        }
        Ok(Self {
            present: true,
            kind: semantic_action_kind::erase_previous,
            erase_count: count,
            text: private_text::empty(),
        })
    }

    #[must_use]
    pub fn submit() -> Self {
        Self {
            present: true,
            kind: semantic_action_kind::submit,
            erase_count: 0,
            text: private_text::empty(),
        }
    }

    #[must_use]
    pub fn cancel_submission() -> Self {
        Self {
            present: true,
            kind: semantic_action_kind::cancel_submission,
            erase_count: 0,
            text: private_text::empty(),
        }
    }

    #[must_use]
    pub fn source_discontinuity() -> Self {
        Self {
            present: true,
            kind: semantic_action_kind::source_discontinuity,
            erase_count: 0,
            text: private_text::empty(),
        }
    }

    /// Exposes the content-derived action discriminator locally.
    #[must_use]
    pub fn kind(&self) -> Option<semantic_action_kind> {
        self.present.then_some(self.kind)
    }

    /// Exposes private insertion text locally when this is an insertion.
    #[must_use]
    pub fn expose_text(&self) -> Option<&str> {
        if self.present && self.kind == semantic_action_kind::insert_text {
            return Some(self.text.expose_str());
        }
        None
    }

    #[must_use]
    pub fn expose_erase_count(&self) -> Option<u16> {
        if self.present && self.kind == semantic_action_kind::erase_previous {
            return Some(self.erase_count);
        }
        None
    }

    pub(crate) fn empty() -> Self {
        Self {
            present: false,
            kind: semantic_action_kind::insert_text,
            erase_count: 0,
            text: private_text::empty(),
        }
    }

    pub(crate) fn is_present(&self) -> bool {
        self.present
    }

    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.present = false;
        self.kind = semantic_action_kind::insert_text;
        self.erase_count = 0;
    }

    #[cfg(test)]
    pub(crate) fn invariants_hold(&self) -> bool {
        if !self.present {
            return self.kind == semantic_action_kind::insert_text
                && self.erase_count == 0
                && self.text.storage_is_clear();
        }
        match self.kind {
            semantic_action_kind::insert_text => {
                self.erase_count == 0 && !self.text.is_empty() && self.text.unused_tail_is_clear()
            }
            semantic_action_kind::erase_previous => {
                self.erase_count != 0
                    && self.erase_count <= erase_scalar_limit
                    && self.text.storage_is_clear()
            }
            semantic_action_kind::submit
            | semantic_action_kind::cancel_submission
            | semantic_action_kind::source_discontinuity => {
                self.erase_count == 0 && self.text.storage_is_clear()
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn storage_is_clear(&self) -> bool {
        !self.present && self.invariants_hold()
    }
}

impl Drop for semantic_action {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{action_error, erase_scalar_limit, semantic_action, semantic_action_kind};
    use crate::private_text;

    #[test]
    fn every_semantic_action_has_an_exact_shape() {
        let insertion = semantic_action::insert_text(
            private_text::try_from_str("private\ntext").expect("valid text"),
        )
        .expect("valid insertion");
        assert!(insertion.kind() == Some(semantic_action_kind::insert_text));
        assert!(insertion.expose_text() == Some("private\ntext"));
        assert!(insertion.expose_erase_count().is_none());

        let erase = semantic_action::erase_previous(3).expect("valid erase");
        assert!(erase.kind() == Some(semantic_action_kind::erase_previous));
        assert!(erase.expose_text().is_none());
        assert!(erase.expose_erase_count() == Some(3));

        for (action, kind) in [
            (semantic_action::submit(), semantic_action_kind::submit),
            (
                semantic_action::cancel_submission(),
                semantic_action_kind::cancel_submission,
            ),
            (
                semantic_action::source_discontinuity(),
                semantic_action_kind::source_discontinuity,
            ),
        ] {
            assert!(action.kind() == Some(kind));
            assert!(action.expose_text().is_none());
            assert!(action.expose_erase_count().is_none());
        }
    }

    #[test]
    fn erase_count_is_nonzero_and_bounded() {
        assert!(matches!(
            semantic_action::erase_previous(0),
            Err(action_error::zero_erase_count)
        ));
        assert!(semantic_action::erase_previous(erase_scalar_limit).is_ok());
        assert!(matches!(
            semantic_action::erase_previous(erase_scalar_limit + 1),
            Err(action_error::erase_limit_exceeded)
        ));
    }

    #[test]
    fn action_clear_zeroes_private_and_scalar_storage() {
        let mut insertion = semantic_action::insert_text(
            private_text::try_from_str("private sentinel").expect("valid text"),
        )
        .expect("valid insertion");
        insertion.clear();
        assert!(insertion.storage_is_clear());

        let mut erase = semantic_action::erase_previous(19).expect("valid erase");
        erase.clear();
        assert!(erase.storage_is_clear());
    }

    #[test]
    fn literal_line_feed_remains_insertion_data() {
        let action = semantic_action::insert_text(
            private_text::try_from_str("first\nsecond").expect("valid text"),
        )
        .expect("valid insertion");
        assert!(action.kind() == Some(semantic_action_kind::insert_text));
        assert!(action.expose_text() == Some("first\nsecond"));
    }

    #[test]
    fn explicitly_cleared_text_cannot_become_an_insertion() {
        let mut text = private_text::try_from_str("discard me").expect("valid text");
        text.clear();
        assert!(matches!(
            semantic_action::insert_text(text),
            Err(action_error::empty_text)
        ));
    }
}
