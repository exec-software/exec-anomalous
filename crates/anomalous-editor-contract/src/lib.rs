#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! A deliberately bounded native port of real Antimatter editor behavior.
//!
//! The reference is `renderer/src/environment/editorBridge.ts::insertAtCursor`
//! and the CodeMirror state-preservation test in
//! `test/editor-terminal-theme.test.ts`, at Antimatter commit `0679c44`.
//! Replacing the selected range moves the caret immediately after inserted
//! text. Appearance and selection changes preserve text and undo history.
//!
//! This prototype admits one printable-ASCII line, one undo entry, and a fixed
//! software surface. ASCII indices also equal the reference's UTF-16 indices;
//! that equivalence does not extend to arbitrary Unicode. It is not CodeMirror,
//! an Electron port, a filesystem, or a stable editor/application ABI.

mod font;
mod interactive;
mod interactive_render;
mod loaded;
mod local;
mod opened;
mod render;
mod repeated;
pub use interactive::{
    interactive_close_result, interactive_editor_action, interactive_editor_error,
    interactive_editor_feedback, interactive_editor_session, interactive_editor_view,
};
pub use interactive_render::{
    interactive_audit, interactive_audit_error, interactive_audit_size,
    interactive_frame_byte_count, interactive_frame_height, interactive_frame_width,
    render_interactive, render_interactive_audit,
};
pub use loaded::{loaded_editor_state, render_loaded};
pub use local::{caret_motion, local_editor_state};
pub use opened::{
    opened_editor_error, opened_editor_state, render_opened, save_acknowledgement, save_outcome,
    save_snapshot,
};
pub use render::{frame_byte_count, frame_height, frame_metadata, frame_width, render};
pub use repeated::{edit_effect, repeated_opened_editor_state};

pub const text_capacity: usize = 31;
pub const initial_text: &[u8] = b"const before = 1;";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum theme {
    light = 0,
    dark = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum editor_error {
    invalid_selection,
    unsupported_text,
    text_capacity,
    undo_capacity,
    no_undo,
    revision_exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct document {
    text: [u8; text_capacity],
    length: usize,
    anchor: usize,
    head: usize,
}

/// State stays private so every observable document satisfies the same bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct editor_state {
    document: document,
    previous: Option<document>,
    revision: u64,
    appearance: theme,
}

impl Default for editor_state {
    fn default() -> Self {
        Self::new()
    }
}

impl editor_state {
    #[must_use]
    pub fn new() -> Self {
        let mut text = [0; text_capacity];
        text[..initial_text.len()].copy_from_slice(initial_text);
        Self {
            document: document {
                text,
                length: initial_text.len(),
                anchor: 0,
                head: 0,
            },
            previous: None,
            revision: 1,
            appearance: theme::dark,
        }
    }

    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.document.text[..self.document.length]
    }

    #[must_use]
    pub fn selection(&self) -> (usize, usize) {
        (self.document.anchor, self.document.head)
    }

    #[must_use]
    pub fn caret(&self) -> usize {
        self.document.head
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn appearance(&self) -> theme {
        self.appearance
    }

    /// This fixed document's saved reference is its initial fixture text.
    /// No operation here claims to save a file or acknowledge durable storage.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.text() != initial_text
    }

    #[must_use]
    pub fn undo_depth(&self) -> u8 {
        u8::from(self.previous.is_some())
    }

    /// Selection changes are not text edits and do not advance the revision.
    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), editor_error> {
        if anchor > self.document.length || head > self.document.length {
            return Err(editor_error::invalid_selection);
        }
        self.document.anchor = anchor;
        self.document.head = head;
        Ok(())
    }

    pub fn set_theme(&mut self, appearance: theme) {
        self.appearance = appearance;
    }

    /// Replace the normalized current selection and collapse it after the new
    /// text, matching Antimatter's production `insertAtCursor` behavior.
    /// All rejection checks precede mutation. A second text edit requires the
    /// one retained undo entry to be consumed first; history is never discarded.
    pub fn insert(&mut self, text: &[u8]) -> Result<(), editor_error> {
        let from = self.document.anchor.min(self.document.head);
        let to = self.document.anchor.max(self.document.head);
        let length = (self.document.length - (to - from))
            .checked_add(text.len())
            .filter(|length| *length <= text_capacity)
            .ok_or(editor_error::text_capacity)?;
        if text.iter().any(|byte| !(b' '..=b'~').contains(byte)) {
            return Err(editor_error::unsupported_text);
        }
        let caret = from + text.len();
        let mut candidate = document {
            text: [0; text_capacity],
            length,
            anchor: caret,
            head: caret,
        };
        candidate.text[..from].copy_from_slice(&self.document.text[..from]);
        candidate.text[from..caret].copy_from_slice(text);
        candidate.text[caret..length]
            .copy_from_slice(&self.document.text[to..self.document.length]);
        if candidate.text[..length] == *self.text() {
            self.document = candidate;
            return Ok(());
        }
        if self.previous.is_some() {
            return Err(editor_error::undo_capacity);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(editor_error::revision_exhausted)?;
        self.previous = Some(self.document);
        self.document = candidate;
        self.revision = revision;
        Ok(())
    }

    /// The opt-in repeated editor stages a complete candidate before replacing
    /// its single undo entry. Legacy `insert` keeps its rejecting policy.
    fn insert_repeated(&mut self, text: &[u8]) -> Result<edit_effect, editor_error> {
        let mut candidate = *self;
        let previous = candidate.previous.take();
        candidate.insert(text)?;
        let effect = if candidate.revision == self.revision {
            // A content no-op may move the selection, but must not discard the
            // earlier text edit's undo entry.
            candidate.previous = previous;
            edit_effect::unchanged
        } else {
            edit_effect::changed {
                discarded_undo: previous.is_some(),
            }
        };
        *self = candidate;
        Ok(effect)
    }

    /// A synthetic deletion range must not replace the original selection in
    /// undo history. Stage both the range and edit before publishing either.
    fn delete_repeated(&mut self, forward: bool) -> Result<edit_effect, editor_error> {
        let original = self.document;
        let (mut from, mut to) = self.selection();
        if from == to {
            if forward {
                to = to.saturating_add(1).min(self.text().len());
            } else {
                from = from.saturating_sub(1);
            }
            if from == to {
                return Ok(edit_effect::unchanged);
            }
        }
        let mut candidate = *self;
        candidate.set_selection(from, to)?;
        let effect = candidate.insert_repeated(b"")?;
        if matches!(effect, edit_effect::changed { .. }) {
            candidate.previous = Some(original);
        }
        *self = candidate;
        Ok(effect)
    }

    /// Restore the previous document/selection while retaining current theme.
    pub fn undo(&mut self) -> Result<(), editor_error> {
        let previous = self.previous.ok_or(editor_error::no_undo)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(editor_error::revision_exhausted)?;
        self.document = previous;
        self.previous = None;
        self.revision = revision;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(feature = "remote_editor")]
pub use remote_protocol::{
    remote_action, remote_editor_session, remote_effect, remote_error, remote_execution,
    remote_message, remote_message_size, remote_outcome, remote_request, remote_request_size,
};
#[cfg(feature = "remote_editor")]
pub mod remote_protocol;
