use crate::{caret_motion, editor_error};
use anomalous_editor_contract::{edit_effect, local_editor_state, text_capacity};

/// Public scratch actions; neither these values nor window IDs grant authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum editor_action {
    insert(u8),
    backspace,
    delete_forward,
    move_caret { motion: caret_motion, extend: bool },
    select_all,
    collapse_selection,
    undo,
    save_unavailable,
    unsupported_key,
    input_reset,
    display_unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum editor_feedback {
    ready,
    edited,
    history_replaced,
    selection_changed,
    unchanged,
    undone,
    rejected(editor_error),
    save_unavailable,
    unsupported_key,
    input_reset,
    display_unavailable,
}

impl editor_feedback {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ready => "ready",
            Self::edited => "edited",
            Self::history_replaced => "history_replaced",
            Self::selection_changed => "selection_changed",
            Self::unchanged => "unchanged",
            Self::undone => "undone",
            Self::rejected(editor_error::invalid_selection) => "invalid_selection",
            Self::rejected(editor_error::unsupported_text) => "unsupported_text",
            Self::rejected(editor_error::text_capacity) => "text_capacity",
            Self::rejected(editor_error::undo_capacity) => "undo_capacity",
            Self::rejected(editor_error::no_undo) => "no_undo",
            Self::rejected(editor_error::revision_exhausted) => "revision_exhausted",
            Self::save_unavailable => "save_unavailable",
            Self::unsupported_key => "unsupported_key",
            Self::input_reset => "input_reset",
            Self::display_unavailable => "display_unavailable",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ready => "Memory only; Save unavailable",
            Self::edited => "Edited; one undo step",
            Self::history_replaced => "Edited; previous undo replaced",
            Self::selection_changed => "Selection changed",
            Self::unchanged => "Unchanged",
            Self::undone => "Undone; no further undo",
            Self::rejected(editor_error::invalid_selection) => "Selection outside text",
            Self::rejected(editor_error::unsupported_text) => "Printable ASCII only",
            Self::rejected(editor_error::text_capacity) => "31-byte scratch limit",
            Self::rejected(editor_error::undo_capacity) => "Undo capacity reached",
            Self::rejected(editor_error::no_undo) => "Nothing to undo",
            Self::rejected(editor_error::revision_exhausted) => "Edit revision exhausted",
            Self::save_unavailable => "Save unavailable; memory only",
            Self::unsupported_key => "Unsupported key or shortcut",
            Self::input_reset => "Input reset; modifiers cleared",
            Self::display_unavailable => "Enlarge editor to type",
        }
    }
}

/// Compact observation of public text and metadata. Undo/baseline storage stays
/// in the candidate snapshot instead of being copied through every window view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct editor_view {
    text: [u8; text_capacity],
    length: u8,
    anchor: u8,
    head: u8,
    revision: u64,
    undo_depth: u8,
    dirty: bool,
    feedback: editor_feedback,
}

impl editor_view {
    pub fn text(&self) -> &[u8] {
        &self.text[..usize::from(self.length)]
    }
    pub const fn selection(&self) -> (usize, usize) {
        (self.anchor as usize, self.head as usize)
    }
    pub const fn caret(&self) -> usize {
        self.head as usize
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn undo_depth(&self) -> u8 {
        self.undo_depth
    }
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub const fn feedback(&self) -> editor_feedback {
        self.feedback
    }
}

#[derive(Clone, Copy)]
pub(super) struct editor_content {
    state: local_editor_state,
    feedback: editor_feedback,
}

impl editor_content {
    pub(super) fn new() -> Self {
        Self {
            state: local_editor_state::from_text(b"").expect("valid empty scratch"),
            feedback: editor_feedback::ready,
        }
    }

    pub(super) fn view(&self) -> editor_view {
        let state = self.state.state();
        let mut text = [0; text_capacity];
        text[..state.text().len()].copy_from_slice(state.text());
        let (anchor, head) = state.selection();
        editor_view {
            text,
            length: state.text().len() as u8,
            anchor: anchor as u8,
            head: head as u8,
            revision: state.revision(),
            undo_depth: state.undo_depth(),
            dirty: state.is_dirty(),
            feedback: self.feedback,
        }
    }

    pub(super) fn apply(&mut self, action: editor_action) {
        use editor_action::*;
        let previous_selection = self.state.state().selection();
        let result = match action {
            insert(byte) => self.state.insert(&[byte]).map(edit_feedback),
            backspace => self.state.backspace().map(edit_feedback),
            delete_forward => self.state.delete_forward().map(edit_feedback),
            move_caret { motion, extend } => self
                .state
                .move_caret(motion, extend)
                .map(|()| editor_feedback::selection_changed),
            select_all => self
                .state
                .set_selection(0, self.state.state().text().len())
                .map(|()| editor_feedback::selection_changed),
            collapse_selection => {
                let head = self.state.state().caret();
                self.state
                    .set_selection(head, head)
                    .map(|()| editor_feedback::selection_changed)
            }
            undo => self.state.undo().map(|()| editor_feedback::undone),
            save_unavailable => Ok(editor_feedback::save_unavailable),
            unsupported_key => Ok(editor_feedback::unsupported_key),
            input_reset => Ok(editor_feedback::input_reset),
            display_unavailable => Ok(editor_feedback::display_unavailable),
        };
        self.feedback = match result {
            Ok(editor_feedback::selection_changed)
                if previous_selection == self.state.state().selection() =>
            {
                editor_feedback::unchanged
            }
            Ok(feedback) => feedback,
            Err(error) => editor_feedback::rejected(error),
        };
    }
}

fn edit_feedback(effect: edit_effect) -> editor_feedback {
    match effect {
        edit_effect::unchanged => editor_feedback::unchanged,
        edit_effect::changed {
            discarded_undo: false,
        } => editor_feedback::edited,
        edit_effect::changed {
            discarded_undo: true,
        } => editor_feedback::history_replaced,
    }
}
