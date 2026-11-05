use crate::{edit_effect, editor_error, loaded_editor_state};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum caret_motion {
    left,
    right,
    start,
    end,
}

/// Public scratch content for a local transactional UI. Copies carry only
/// bounded editing state and the initial comparison baseline, never document,
/// task or service authority. There is no save operation or pending request.
/// Like the opened repeated model, each changed edit retains one predecessor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct local_editor_state {
    inner: loaded_editor_state,
}

impl local_editor_state {
    pub fn from_text(text: &[u8]) -> Result<Self, editor_error> {
        Ok(Self {
            inner: loaded_editor_state::from_text(text)?,
        })
    }

    pub const fn state(&self) -> &loaded_editor_state {
        &self.inner
    }

    pub fn insert(&mut self, text: &[u8]) -> Result<edit_effect, editor_error> {
        self.inner.insert_repeated(text)
    }

    pub fn backspace(&mut self) -> Result<edit_effect, editor_error> {
        self.inner.delete_repeated(false)
    }

    pub fn delete_forward(&mut self) -> Result<edit_effect, editor_error> {
        self.inner.delete_repeated(true)
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), editor_error> {
        self.inner.set_selection(anchor, head)
    }

    pub fn move_caret(&mut self, motion: caret_motion, extend: bool) -> Result<(), editor_error> {
        let (anchor, head) = self.inner.selection();
        let next = match motion {
            caret_motion::left if !extend && anchor != head => anchor.min(head),
            caret_motion::right if !extend && anchor != head => anchor.max(head),
            caret_motion::left => head.saturating_sub(1),
            caret_motion::right => head.saturating_add(1).min(self.inner.text().len()),
            caret_motion::start => 0,
            caret_motion::end => self.inner.text().len(),
        };
        self.inner
            .set_selection(if extend { anchor } else { next }, next)
    }

    pub fn undo(&mut self) -> Result<(), editor_error> {
        self.inner.undo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_scratch_edits_and_undo_retain_only_the_immediate_predecessor() {
        let mut state = local_editor_state::from_text(b"").unwrap();
        for (index, byte) in b"abc".iter().enumerate() {
            assert_eq!(
                state.insert(&[*byte]),
                Ok(edit_effect::changed {
                    discarded_undo: index != 0
                })
            );
        }
        assert_eq!(state.state().text(), b"abc");
        assert_eq!(state.state().revision(), 4);
        assert!(state.state().is_dirty());
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"ab");
        assert_eq!(state.state().selection(), (2, 2));
        let before = state;
        assert_eq!(state.undo(), Err(editor_error::no_undo));
        assert_eq!(state, before);
    }

    #[test]
    fn every_selection_and_deletion_matches_independent_bytes_and_restores_original_caret() {
        for anchor in 0..=4 {
            for head in 0..=4 {
                for forward in [false, true] {
                    let mut state = local_editor_state::from_text(b"abcd").unwrap();
                    state.set_selection(anchor, head).unwrap();
                    let before = state;
                    let (from, to) = if anchor != head {
                        (anchor.min(head), anchor.max(head))
                    } else if forward {
                        (head, (head + 1).min(4))
                    } else {
                        (head.saturating_sub(1), head)
                    };
                    let effect = if forward {
                        state.delete_forward()
                    } else {
                        state.backspace()
                    }
                    .unwrap();
                    if from == to {
                        assert_eq!(effect, edit_effect::unchanged);
                        assert_eq!(state, before);
                    } else {
                        assert_eq!(
                            effect,
                            edit_effect::changed {
                                discarded_undo: false
                            }
                        );
                        let mut expected = [0; 4];
                        expected[..from].copy_from_slice(&b"abcd"[..from]);
                        expected[from..4 - (to - from)].copy_from_slice(&b"abcd"[to..]);
                        assert_eq!(state.state().text(), &expected[..4 - (to - from)]);
                        assert_eq!(state.state().selection(), (from, from));
                        state.undo().unwrap();
                        assert_eq!(state.state().text(), b"abcd");
                        assert_eq!(state.state().selection(), (anchor, head));
                        assert!(!state.state().is_dirty());
                    }
                }
            }
        }
    }

    #[test]
    fn caret_navigation_collapses_directionally_and_extends_from_the_original_anchor() {
        let mut state = local_editor_state::from_text(b"abcd").unwrap();
        for (anchor, head) in [(1, 3), (3, 1)] {
            for (motion, next) in [(caret_motion::left, 1), (caret_motion::right, 3)] {
                state.set_selection(anchor, head).unwrap();
                state.move_caret(motion, false).unwrap();
                assert_eq!(state.state().selection(), (next, next));
            }
        }
        state.set_selection(3, 1).unwrap();
        state.move_caret(caret_motion::left, true).unwrap();
        assert_eq!(state.state().selection(), (3, 0));
        state.move_caret(caret_motion::left, true).unwrap();
        assert_eq!(state.state().selection(), (3, 0));
        state.move_caret(caret_motion::end, true).unwrap();
        assert_eq!(state.state().selection(), (3, 4));
        state.move_caret(caret_motion::right, true).unwrap();
        assert_eq!(state.state().selection(), (3, 4));
        state.move_caret(caret_motion::start, false).unwrap();
        assert_eq!(state.state().selection(), (0, 0));
        assert_eq!(state.state().revision(), 1);
        assert_eq!(state.state().undo_depth(), 0);
    }

    #[test]
    fn failed_edits_and_boundary_deletion_preserve_history_and_noop_never_evicts_it() {
        let mut state = local_editor_state::from_text(&[b'a'; crate::text_capacity]).unwrap();
        state.set_selection(0, 1).unwrap();
        state.insert(b"b").unwrap();
        for bytes in [b"xx".as_slice(), b"\n", b"\0", &[0xff]] {
            let before = state;
            assert!(state.insert(bytes).is_err());
            assert_eq!(state, before);
        }
        let before = state;
        assert_eq!(
            state.set_selection(0, 32),
            Err(editor_error::invalid_selection)
        );
        assert_eq!(state, before);
        state.move_caret(caret_motion::start, false).unwrap();
        let before = state;
        assert_eq!(state.backspace(), Ok(edit_effect::unchanged));
        assert_eq!(state, before);
        state.move_caret(caret_motion::end, false).unwrap();
        let before = state;
        assert_eq!(state.delete_forward(), Ok(edit_effect::unchanged));
        assert_eq!(state, before);
        state.set_selection(1, 0).unwrap();
        assert_eq!(state.insert(b"b"), Ok(edit_effect::unchanged));
        state.undo().unwrap();
        assert_eq!(state.state().text(), &[b'a'; crate::text_capacity]);
        assert_eq!(state.state().selection(), (0, 1));
    }
}
