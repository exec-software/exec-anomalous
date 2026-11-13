//! Immutable decoded concatenation storage. Handles refer only to committed
//! bytes of this execution; locals/frames copy handles without copying owners.

use super::{error, maximum_text_bytes, value};
use crate::decode_text;

#[derive(Clone, Copy)]
pub(super) struct text_span {
    start: usize,
    length: usize,
}

pub(super) struct text_arena {
    pub(super) storage: [u8; maximum_text_bytes],
    pub(super) used: usize,
}

impl text_arena {
    pub(super) const fn new() -> Self {
        Self {
            storage: [0; maximum_text_bytes],
            used: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.storage.fill(0);
        self.used = 0;
    }

    pub(super) fn bytes(&self, span: text_span) -> Result<&[u8], error> {
        let end = span
            .start
            .checked_add(span.length)
            .ok_or(error::invalid_state)?;
        self.storage
            .get(..self.used)
            .and_then(|bytes| bytes.get(span.start..end))
            .ok_or(error::invalid_state)
    }

    fn length(&self, item: value<'_>) -> Result<usize, error> {
        match item {
            value::integer(_) => Err(error::invalid_state),
            value::computed_text(span) => Ok(self.bytes(span)?.len()),
            value::text(escaped) => {
                let mut length = 0usize;
                decode_text(escaped, |bytes| {
                    length = length.checked_add(bytes.len()).ok_or(error::text_limit)?;
                    Ok(())
                })?;
                Ok(length)
            }
        }
    }

    fn copy(&mut self, item: value<'_>, cursor: &mut usize) -> Result<(), error> {
        match item {
            value::integer(_) => Err(error::invalid_state),
            value::computed_text(span) => {
                let length = self.bytes(span)?.len();
                let end = cursor.checked_add(length).ok_or(error::invalid_state)?;
                if end > self.storage.len() || *cursor < self.used {
                    return Err(error::invalid_state);
                }
                // The source is wholly inside committed storage; the destination
                // is wholly beyond it. A span cannot alias a pending allocation.
                self.storage
                    .copy_within(span.start..span.start + length, *cursor);
                *cursor = end;
                Ok(())
            }
            value::text(escaped) => decode_text(escaped, |bytes| {
                let end = cursor
                    .checked_add(bytes.len())
                    .ok_or(error::invalid_state)?;
                self.storage
                    .get_mut(*cursor..end)
                    .ok_or(error::invalid_state)?
                    .copy_from_slice(bytes);
                *cursor = end;
                Ok(())
            }),
        }
    }

    pub(super) fn concatenate(
        &mut self,
        left: value<'_>,
        right: value<'_>,
    ) -> Result<text_span, error> {
        // Measure both complete decoded operands and preflight capacity before
        // touching storage. Integer coercion is outside this language profile.
        let length = self
            .length(left)?
            .checked_add(self.length(right)?)
            .ok_or(error::text_limit)?;
        let start = self.used;
        let end = start.checked_add(length).ok_or(error::text_limit)?;
        if end > self.storage.len() {
            return Err(error::text_limit);
        }
        let mut cursor = start;
        let result = self
            .copy(left, &mut cursor)
            .and_then(|()| self.copy(right, &mut cursor));
        if let Err(failure) = result {
            self.storage[start..end].fill(0);
            return Err(failure);
        }
        if cursor != end {
            self.storage[start..end].fill(0);
            return Err(error::invalid_state);
        }
        self.used = end;
        Ok(text_span { start, length })
    }
}
