use crate::{
    caret_motion, editor_error, opened_editor_error, opened_editor_state, save_acknowledgement,
    save_snapshot, theme,
};

/// Result of an opt-in repeated edit. `unchanged` means unchanged text: the
/// selection may still collapse after the replacement. Evicting an older undo
/// entry is explicit and never changes the saved baseline or a pending save.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum edit_effect {
    unchanged,
    changed { discarded_undo: bool },
}

/// An opened bounded document with a rolling, one-step undo policy. Every
/// successful text change retains the immediately preceding text and selection;
/// an older undo entry is replaced only after the edit passes all validation.
/// Undo consumes that entry; there is no redo or multi-entry history.
///
/// The original editor types retain their rejecting history policy. This opt-in
/// wrapper retains their exact save correlation and unknown-outcome rules, is
/// neither Clone nor Copy, and exposes no mutable access to the wrapped state.
#[derive(Debug, Eq, PartialEq)]
pub struct repeated_opened_editor_state {
    inner: opened_editor_state,
}

impl repeated_opened_editor_state {
    pub fn from_open(
        document: u64,
        generation: u64,
        version: u64,
        text: &[u8],
    ) -> Result<Self, opened_editor_error> {
        Ok(Self {
            inner: opened_editor_state::from_open(document, generation, version, text)?,
        })
    }

    /// Read-only state for existing getters and `render_opened`.
    pub const fn state(&self) -> &opened_editor_state {
        &self.inner
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), editor_error> {
        self.inner.set_selection(anchor, head)
    }

    pub fn set_theme(&mut self, appearance: theme) {
        self.inner.set_theme(appearance);
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

    pub fn begin_save(&mut self) -> Result<save_snapshot, opened_editor_error> {
        self.inner.begin_save()
    }

    pub fn acknowledge_save(
        &mut self,
        reply: save_acknowledgement<'_>,
    ) -> Result<(), opened_editor_error> {
        self.inner.acknowledge_save(reply)
    }

    pub fn reject_save(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), opened_editor_error> {
        self.inner
            .reject_save(document, generation, request, base_version)
    }

    pub fn mark_save_unknown(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), opened_editor_error> {
        self.inner
            .mark_save_unknown(document, generation, request, base_version)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use self::std::vec::Vec;
    use super::*;
    use crate::{frame_byte_count, render_opened, save_outcome};

    fn opened(text: &[u8]) -> repeated_opened_editor_state {
        repeated_opened_editor_state::from_open(11, 7, 1, text).unwrap()
    }

    fn reply(snapshot: &save_snapshot) -> save_acknowledgement<'_> {
        save_acknowledgement {
            document: snapshot.document(),
            generation: snapshot.generation(),
            request: snapshot.request(),
            base_version: snapshot.base_version(),
            revision: snapshot.revision(),
            version: snapshot.base_version() + 1,
            text: snapshot.text(),
        }
    }

    fn replace_all(state: &mut repeated_opened_editor_state, text: &[u8]) -> edit_effect {
        state.set_selection(0, state.state().text().len()).unwrap();
        state.insert(text).unwrap()
    }

    #[test]
    fn consecutive_edits_keep_only_the_immediately_previous_document_and_selection() {
        let mut state = opened(b"");
        assert_eq!(
            state.insert(b"a"),
            Ok(edit_effect::changed {
                discarded_undo: false
            })
        );
        for text in [b"b", b"c"] {
            assert_eq!(
                state.insert(text),
                Ok(edit_effect::changed {
                    discarded_undo: true
                })
            );
        }
        assert_eq!(state.state().text(), b"abc");
        assert_eq!(state.state().selection(), (3, 3));
        assert_eq!(state.state().revision(), 4);
        assert_eq!(state.state().undo_depth(), 1);
        state.set_theme(theme::light);
        state.set_selection(0, 3).unwrap();
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"ab");
        assert_eq!(state.state().selection(), (2, 2));
        assert_eq!(state.state().appearance(), theme::light);
        assert_eq!(state.state().revision(), 5);
        assert_eq!(state.state().undo_depth(), 0);
        assert_eq!(state.undo(), Err(editor_error::no_undo));
        assert_eq!(
            state.insert(b"d"),
            Ok(edit_effect::changed {
                discarded_undo: false
            })
        );
        assert_eq!(state.state().text(), b"abd");
    }

    #[test]
    fn no_op_collapses_selection_without_evicting_history_or_advancing_revision() {
        let mut state = opened(b"before");
        state.set_selection(6, 0).unwrap();
        state.insert(b"after").unwrap();
        state.set_selection(5, 0).unwrap();
        assert_eq!(state.insert(b"after"), Ok(edit_effect::unchanged));
        assert_eq!(state.state().selection(), (5, 5));
        assert_eq!(state.state().revision(), 2);
        assert_eq!(state.state().undo_depth(), 1);
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"before");
        assert_eq!(state.state().selection(), (6, 0));
        assert!(!state.state().is_dirty());
    }

    #[test]
    fn selection_replacement_and_deletion_match_an_independent_vector_model() {
        for anchor in 0..=4 {
            for head in 0..=4 {
                for replacement in [b"".as_slice(), b"X", b"yz", b"abcd"] {
                    let mut state = opened(b"abcd");
                    state.set_selection(anchor, head).unwrap();
                    let mut expected = Vec::from(b"abcd".as_slice());
                    drop(expected.splice(
                        anchor.min(head)..anchor.max(head),
                        replacement.iter().copied(),
                    ));
                    let changed = expected.as_slice() != b"abcd";
                    assert_eq!(
                        state.insert(replacement),
                        Ok(if changed {
                            edit_effect::changed {
                                discarded_undo: false,
                            }
                        } else {
                            edit_effect::unchanged
                        })
                    );
                    let caret = anchor.min(head) + replacement.len();
                    assert_eq!(state.state().text(), expected.as_slice());
                    assert_eq!(state.state().selection(), (caret, caret));
                    // A second actual insertion always succeeds, including when
                    // the first replacement already filled the undo slot.
                    assert_eq!(
                        state.insert(b"!"),
                        Ok(edit_effect::changed {
                            discarded_undo: changed
                        })
                    );
                    state.undo().unwrap();
                    assert_eq!(state.state().text(), expected.as_slice());
                    assert_eq!(state.state().selection(), (caret, caret));
                    assert_eq!(state.state().revision(), if changed { 4 } else { 3 });
                }
            }
        }
    }

    #[test]
    fn invalid_edits_and_selection_preserve_current_history_and_pending_snapshot() {
        let mut state = opened(b"before");
        replace_all(&mut state, b"after");
        let pending = state.begin_save().unwrap();
        for (text, error) in [
            (b"\n".as_slice(), editor_error::unsupported_text),
            (b"\0".as_slice(), editor_error::unsupported_text),
            (b"\xc3\xa9".as_slice(), editor_error::unsupported_text),
            (&[b'x'; 32], editor_error::text_capacity),
        ] {
            assert_eq!(state.insert(text), Err(error));
            assert_eq!(state.state().text(), b"after");
            assert_eq!(state.state().selection(), (5, 5));
            assert_eq!(state.state().revision(), 2);
            assert_eq!(state.state().undo_depth(), 1);
            assert_eq!(state.state().pending_save(), Some(&pending));
            assert_eq!(state.state().saved_text(), b"before");
            assert_eq!(state.state().save_outcome(), save_outcome::awaiting);
        }
        assert_eq!(
            state.set_selection(usize::MAX, 0),
            Err(editor_error::invalid_selection)
        );
        assert_eq!(state.state().selection(), (5, 5));
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"before");
        assert_eq!(state.state().selection(), (0, 6));
        assert_eq!(state.state().pending_save(), Some(&pending));
    }

    #[test]
    fn exact_capacity_edit_and_no_op_do_not_require_an_empty_undo_slot() {
        let mut state = opened(b"");
        replace_all(&mut state, &[b'x'; 31]);
        state.set_selection(31, 0).unwrap();
        assert_eq!(state.insert(&[b'x'; 31]), Ok(edit_effect::unchanged));
        state.set_selection(31, 30).unwrap();
        assert_eq!(
            state.insert(b"y"),
            Ok(edit_effect::changed {
                discarded_undo: true
            })
        );
        assert_eq!(state.state().text().len(), 31);
        state.undo().unwrap();
        assert_eq!(state.state().text(), &[b'x'; 31]);
        assert_eq!(state.state().selection(), (31, 30));
    }

    #[test]
    fn late_ack_preserves_direct_newer_edit_and_second_and_third_save_bindings() {
        let mut state = opened(b"A");
        replace_all(&mut state, b"B");
        let first = state.begin_save().unwrap();
        assert_eq!(
            replace_all(&mut state, b"C"),
            edit_effect::changed {
                discarded_undo: true
            }
        );
        state.set_theme(theme::light);
        state.acknowledge_save(reply(&first)).unwrap();
        assert_eq!(state.state().text(), b"C");
        assert_eq!(state.state().saved_text(), b"B");
        assert_eq!(state.state().revision(), 3);
        assert_eq!(state.state().undo_depth(), 1);
        assert!(state.state().is_dirty());
        assert_eq!(state.state().appearance(), theme::light);
        let second = state.begin_save().unwrap();
        assert_eq!(
            (second.request(), second.base_version(), second.revision()),
            (2, 2, 3)
        );
        state.acknowledge_save(reply(&second)).unwrap();
        assert!(!state.state().is_dirty());
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"B");
        assert_eq!(state.state().saved_text(), b"C");
        assert!(state.state().is_dirty());
        let third = state.begin_save().unwrap();
        assert_eq!(
            (third.request(), third.base_version(), third.revision()),
            (3, 3, 4)
        );
        assert_eq!(
            state.acknowledge_save(reply(&first)),
            Err(opened_editor_error::wrong_snapshot)
        );
        assert_eq!(state.state().pending_save(), Some(&third));
        state.acknowledge_save(reply(&third)).unwrap();
        assert_eq!(state.state().version(), 4);
        assert!(!state.state().is_dirty());
        assert_eq!(
            state.acknowledge_save(reply(&third)),
            Err(opened_editor_error::no_pending_save)
        );
        let mut pixels = [0; frame_byte_count];
        let metadata = render_opened(state.state(), &mut pixels);
        assert!(!metadata.dirty);
        assert_eq!(metadata.revision, 4);
        assert_eq!(metadata.undo_depth, 0);
    }

    #[test]
    fn unknown_save_allows_local_edits_but_never_a_second_save_or_false_clean() {
        let mut state = opened(b"A");
        replace_all(&mut state, b"B");
        let pending = state.begin_save().unwrap();
        replace_all(&mut state, b"C");
        state.mark_save_unknown(11, 7, 1, 1).unwrap();
        replace_all(&mut state, b"D");
        replace_all(&mut state, b"A");
        assert_eq!(state.state().text(), state.state().saved_text());
        assert!(state.state().is_dirty());
        assert_eq!(state.state().pending_save(), Some(&pending));
        assert_eq!(state.begin_save(), Err(opened_editor_error::save_pending));
        assert_eq!(
            state.reject_save(11, 7, 1, 1),
            Err(opened_editor_error::outcome_unknown)
        );
        assert_eq!(state.state().save_outcome(), save_outcome::unknown);
        let mut forged = reply(&pending);
        forged.text = b"A";
        assert_eq!(
            state.acknowledge_save(forged),
            Err(opened_editor_error::wrong_content)
        );
        let mut pixels = [0; frame_byte_count];
        assert!(render_opened(state.state(), &mut pixels).dirty);
        state.acknowledge_save(reply(&pending)).unwrap();
        assert_eq!(state.state().saved_text(), b"B");
        assert_eq!(state.state().text(), b"A");
        assert_eq!(state.state().save_outcome(), save_outcome::idle);
        assert!(state.state().is_dirty());
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"D");
    }

    #[test]
    fn definitive_rejection_preserves_history_and_consumes_request_identity() {
        let mut state = opened(b"A");
        replace_all(&mut state, b"B");
        let first = state.begin_save().unwrap();
        replace_all(&mut state, b"C");
        state
            .reject_save(11, 7, first.request(), first.base_version())
            .unwrap();
        assert_eq!(state.state().text(), b"C");
        assert_eq!(state.state().saved_text(), b"A");
        assert_eq!(state.state().version(), 1);
        let next = state.begin_save().unwrap();
        assert_eq!((next.request(), next.base_version()), (2, 1));
        state.undo().unwrap();
        assert_eq!(state.state().text(), b"B");
        assert_eq!(state.state().pending_save(), Some(&next));
    }

    #[test]
    fn original_opened_editor_still_rejects_a_second_changed_insert() {
        let mut legacy = opened_editor_state::from_open(11, 7, 1, b"").unwrap();
        legacy.insert(b"a").unwrap();
        assert_eq!(legacy.insert(b"b"), Err(editor_error::undo_capacity));
        assert_eq!(legacy.text(), b"a");
        assert_eq!(legacy.revision(), 2);
        legacy.undo().unwrap();
        assert_eq!(legacy.text(), b"");
    }
}
