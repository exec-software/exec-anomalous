use crate::{
    caret_motion, edit_effect, editor_error, opened_editor_error, repeated_opened_editor_state,
    save_acknowledgement, save_outcome, save_snapshot, text_capacity,
};

/// Local input semantics, independent of a keyboard encoding or authority route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum interactive_editor_action {
    insert(u8),
    backspace,
    delete_forward,
    move_caret { motion: caret_motion, extend: bool },
    set_selection { anchor: usize, head: usize },
    select_all,
    collapse_selection,
    undo,
    input_reset,
    unsupported_key,
}

/// Feedback is independent of dirty state and the retained save outcome. An
/// edit while a save is pending must not hide that pending operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum interactive_editor_feedback {
    ready,
    edited,
    history_replaced,
    selection_changed,
    unchanged,
    undone,
    rejected(editor_error),
    input_reset,
    unsupported_key,
    save_started,
    save_acknowledged,
    save_rejected,
    save_unknown,
    close_confirmation,
    closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum interactive_editor_error {
    closed,
    sequence_exhausted,
    opened(opened_editor_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum interactive_close_result {
    confirmation_required,
    closed,
}

/// A copied observation, not a save owner or authority. Disclosure of these
/// bytes still belongs to the embedding application's authenticated route.
/// Session sequence changes for selection, feedback and save outcomes as well
/// as text edits; `revision` is only the existing local text revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct interactive_editor_view {
    text: [u8; text_capacity],
    length: u8,
    anchor: u8,
    head: u8,
    sequence: u64,
    revision: u64,
    version: u64,
    pending: Option<(u64, u64)>,
    undo_depth: u8,
    dirty: bool,
    outcome: save_outcome,
    feedback: interactive_editor_feedback,
    close_armed: bool,
    closed: bool,
}

impl interactive_editor_view {
    pub fn text(&self) -> &[u8] {
        &self.text[..usize::from(self.length)]
    }
    pub const fn selection(&self) -> (usize, usize) {
        (self.anchor as usize, self.head as usize)
    }
    pub const fn caret(&self) -> usize {
        self.head as usize
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn version(&self) -> u64 {
        self.version
    }
    pub const fn pending_request(&self) -> Option<u64> {
        match self.pending {
            Some((request, _)) => Some(request),
            None => None,
        }
    }
    pub const fn pending_revision(&self) -> Option<u64> {
        match self.pending {
            Some((_, revision)) => Some(revision),
            None => None,
        }
    }
    pub const fn undo_depth(&self) -> u8 {
        self.undo_depth
    }
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub const fn save_outcome(&self) -> save_outcome {
        self.outcome
    }
    pub const fn feedback(&self) -> interactive_editor_feedback {
        self.feedback
    }
    pub const fn close_armed(&self) -> bool {
        self.close_armed
    }
    pub const fn is_closed(&self) -> bool {
        self.closed
    }
}

/// The single serialized owner of an interactive opened document. It is not
/// Copy or Clone and never exposes mutable access to its model. Local state is
/// authoritative before publishing its immutable observation; failed display
/// presentation cannot undo an acknowledged save or fork pending request IDs.
///
/// The caller authenticates input and service replies. Numeric correlation
/// fields and copied views grant no rights. This type performs no transport,
/// persistence, or window policy, and does not authorize reply disclosure.
#[derive(Debug, Eq, PartialEq)]
pub struct interactive_editor_session {
    model: repeated_opened_editor_state,
    sequence: u64,
    feedback: interactive_editor_feedback,
    close_armed: bool,
    closed: bool,
}

impl interactive_editor_session {
    pub fn from_open(
        document: u64,
        generation: u64,
        version: u64,
        text: &[u8],
    ) -> Result<Self, opened_editor_error> {
        Ok(Self {
            model: repeated_opened_editor_state::from_open(document, generation, version, text)?,
            sequence: 1,
            feedback: interactive_editor_feedback::ready,
            close_armed: false,
            closed: false,
        })
    }

    pub const fn model(&self) -> &repeated_opened_editor_state {
        &self.model
    }

    pub fn view(&self) -> interactive_editor_view {
        let state = self.model.state();
        let mut text = [0; text_capacity];
        text[..state.text().len()].copy_from_slice(state.text());
        let (anchor, head) = state.selection();
        interactive_editor_view {
            text,
            length: state.text().len() as u8,
            anchor: anchor as u8,
            head: head as u8,
            sequence: self.sequence,
            revision: state.revision(),
            version: state.version(),
            pending: state
                .pending_save()
                .map(|save| (save.request(), save.revision())),
            undo_depth: state.undo_depth(),
            dirty: state.is_dirty(),
            outcome: state.save_outcome(),
            feedback: self.feedback,
            close_armed: self.close_armed,
            closed: self.closed,
        }
    }

    fn next_sequence(&self) -> Result<u64, interactive_editor_error> {
        if self.closed {
            return Err(interactive_editor_error::closed);
        }
        self.sequence
            .checked_add(1)
            .ok_or(interactive_editor_error::sequence_exhausted)
    }

    fn publish(&mut self, sequence: u64, feedback: interactive_editor_feedback) {
        self.sequence = sequence;
        self.feedback = feedback;
        self.close_armed = false;
    }

    /// A legitimate local edit rejection is an observable feedback event. The
    /// underlying edit is atomic; text, history, saved and pending bytes stay
    /// intact. Even an unchanged action advances the session sequence.
    pub fn apply(
        &mut self,
        action: interactive_editor_action,
    ) -> Result<(), interactive_editor_error> {
        use interactive_editor_action::*;
        use interactive_editor_feedback as feedback;
        let sequence = self.next_sequence()?;
        let previous_selection = self.model.state().selection();
        let result = match action {
            insert(byte) => self.model.insert(&[byte]).map(edit_feedback),
            backspace => self.model.backspace().map(edit_feedback),
            delete_forward => self.model.delete_forward().map(edit_feedback),
            move_caret { motion, extend } => self
                .model
                .move_caret(motion, extend)
                .map(|()| feedback::selection_changed),
            set_selection { anchor, head } => self
                .model
                .set_selection(anchor, head)
                .map(|()| feedback::selection_changed),
            select_all => self
                .model
                .set_selection(0, self.model.state().text().len())
                .map(|()| feedback::selection_changed),
            collapse_selection => {
                let head = self.model.state().caret();
                self.model
                    .set_selection(head, head)
                    .map(|()| feedback::selection_changed)
            }
            undo => self.model.undo().map(|()| feedback::undone),
            input_reset => Ok(feedback::input_reset),
            unsupported_key => Ok(feedback::unsupported_key),
        };
        let feedback = match result {
            Ok(feedback::selection_changed)
                if previous_selection == self.model.state().selection() =>
            {
                feedback::unchanged
            }
            Ok(feedback) => feedback,
            Err(error) => feedback::rejected(error),
        };
        self.publish(sequence, feedback);
        Ok(())
    }

    /// Reserve one exact snapshot before sending it. Transport non-admission
    /// may retry that snapshot; uncertain delivery must retain it as unknown.
    /// Errors never consume a session sequence or arm/disarm close confirmation.
    pub fn begin_save(&mut self) -> Result<save_snapshot, interactive_editor_error> {
        let sequence = self.next_sequence()?;
        let snapshot = self
            .model
            .begin_save()
            .map_err(interactive_editor_error::opened)?;
        self.publish(sequence, interactive_editor_feedback::save_started);
        Ok(snapshot)
    }

    pub fn acknowledge_save(
        &mut self,
        reply: save_acknowledgement<'_>,
    ) -> Result<(), interactive_editor_error> {
        let sequence = self.next_sequence()?;
        self.model
            .acknowledge_save(reply)
            .map_err(interactive_editor_error::opened)?;
        self.publish(sequence, interactive_editor_feedback::save_acknowledged);
        Ok(())
    }

    /// The caller must have an authenticated definitive rejection of this
    /// attempt. A stale-request rejection cannot disprove an earlier commit.
    pub fn reject_save(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        let sequence = self.next_sequence()?;
        self.model
            .reject_save(document, generation, request, base_version)
            .map_err(interactive_editor_error::opened)?;
        self.publish(sequence, interactive_editor_feedback::save_rejected);
        Ok(())
    }

    pub fn mark_save_unknown(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        let sequence = self.next_sequence()?;
        self.model
            .mark_save_unknown(document, generation, request, base_version)
            .map_err(interactive_editor_error::opened)?;
        self.publish(sequence, interactive_editor_feedback::save_unknown);
        Ok(())
    }

    /// Closing dirty, pending or uncertain content requires a second close
    /// with no successful intervening action or save transition. This is local
    /// abandonment, not a service close ACK, save cancellation, or revocation.
    /// The caller still tears down routes and authority. Retained bytes remain
    /// readable until the owner is dropped; closed sessions cannot mutate.
    pub fn request_close(&mut self) -> Result<interactive_close_result, interactive_editor_error> {
        let sequence = self.next_sequence()?;
        let state = self.model.state();
        let needs_confirmation = state.is_dirty() || state.save_outcome() != save_outcome::idle;
        if needs_confirmation && !self.close_armed {
            self.publish(sequence, interactive_editor_feedback::close_confirmation);
            self.close_armed = true;
            return Ok(interactive_close_result::confirmation_required);
        }
        self.publish(sequence, interactive_editor_feedback::closed);
        self.closed = true;
        Ok(interactive_close_result::closed)
    }
}

fn edit_feedback(effect: edit_effect) -> interactive_editor_feedback {
    match effect {
        edit_effect::unchanged => interactive_editor_feedback::unchanged,
        edit_effect::changed {
            discarded_undo: false,
        } => interactive_editor_feedback::edited,
        edit_effect::changed {
            discarded_undo: true,
        } => interactive_editor_feedback::history_replaced,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use self::std::{format, string::String};
    use super::*;
    use interactive_editor_action as action;
    use interactive_editor_feedback as feedback;

    fn session(text: &[u8]) -> interactive_editor_session {
        interactive_editor_session::from_open(11, 7, 1, text).unwrap()
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

    // A test-only complete observation includes private history and request
    // counters without making the live session or save owner cloneable.
    fn observed(state: &interactive_editor_session) -> String {
        format!("{state:?}")
    }

    #[test]
    fn pending_snapshot_survives_navigation_deletion_late_ack_and_repeated_saves() {
        let mut state = session(b"");
        let initial = state.view();
        assert_eq!(initial.sequence(), 1);
        assert_eq!(initial.revision(), 1);
        assert_eq!(initial.version(), 1);
        assert_eq!(initial.pending_request(), None);
        assert_eq!(initial.pending_revision(), None);
        assert_eq!(initial.feedback(), feedback::ready);
        for byte in b"abc" {
            state.apply(action::insert(*byte)).unwrap();
        }
        assert_eq!(state.view().sequence(), 4);
        assert_eq!(state.view().revision(), 4);
        assert_eq!(state.view().feedback(), feedback::history_replaced);
        let first = state.begin_save().unwrap();
        assert_eq!(
            (first.request(), first.revision(), first.text()),
            (1, 4, b"abc".as_slice())
        );
        assert_eq!(state.view().pending_request(), Some(1));
        assert_eq!(state.view().pending_revision(), Some(4));
        state
            .apply(action::move_caret {
                motion: caret_motion::start,
                extend: false,
            })
            .unwrap();
        state.apply(action::delete_forward).unwrap();
        state.apply(action::backspace).unwrap();
        assert_eq!(state.view().feedback(), feedback::unchanged);
        state.apply(action::input_reset).unwrap();
        assert_eq!(state.view().feedback(), feedback::input_reset);
        assert_eq!(state.model().state().pending_save(), Some(&first));
        assert_eq!(state.view().save_outcome(), save_outcome::awaiting);
        state.acknowledge_save(reply(&first)).unwrap();
        assert_eq!(state.view().sequence(), 10);
        assert_eq!(state.view().revision(), 5);
        assert_eq!(state.view().version(), 2);
        assert_eq!(state.view().text(), b"bc");
        assert_eq!(state.view().selection(), (0, 0));
        assert_eq!(state.view().undo_depth(), 1);
        assert!(state.view().is_dirty());
        assert_eq!(state.model().state().saved_text(), b"abc");
        state.apply(action::undo).unwrap();
        assert_eq!(state.view().text(), b"abc");
        assert_eq!(state.view().selection(), (0, 0));
        assert_eq!(state.view().revision(), 6);
        assert!(!state.view().is_dirty());
        let second = state.begin_save().unwrap();
        assert_eq!(
            (second.request(), second.base_version(), second.revision()),
            (2, 2, 6)
        );
        state.apply(action::insert(b'd')).unwrap();
        state.reject_save(11, 7, 2, 2).unwrap();
        assert_eq!(state.view().feedback(), feedback::save_rejected);
        assert_eq!(state.view().text(), b"dabc");
        assert_eq!(state.view().undo_depth(), 1);
        assert_eq!(state.view().version(), 2);
        assert!(state.view().is_dirty());
        let third = state.begin_save().unwrap();
        assert_eq!(
            (third.request(), third.base_version(), third.revision()),
            (3, 2, 7)
        );
        state.acknowledge_save(reply(&third)).unwrap();
        assert_eq!(state.view().sequence(), 16);
        assert_eq!(state.view().version(), 3);
        assert!(!state.view().is_dirty());
        assert_eq!(state.view().pending_request(), None);
        // Copies are observations: the old initial view never becomes live.
        assert_eq!(initial.text(), b"");
        assert_eq!(initial.sequence(), 1);
    }

    #[test]
    fn malformed_and_duplicate_acknowledgements_do_not_change_any_session_state() {
        let mut state = session(b"a");
        state.apply(action::insert(b'b')).unwrap();
        let snapshot = state.begin_save().unwrap();
        state.request_close().unwrap();
        let exact = reply(&snapshot);
        for invalid in [
            save_acknowledgement {
                document: 12,
                ..exact
            },
            save_acknowledgement {
                generation: 8,
                ..exact
            },
            save_acknowledgement {
                request: 2,
                ..exact
            },
            save_acknowledgement {
                base_version: 2,
                ..exact
            },
            save_acknowledgement {
                revision: 1,
                ..exact
            },
            save_acknowledgement {
                version: 3,
                ..exact
            },
            save_acknowledgement {
                text: b"other",
                ..exact
            },
        ] {
            let before = observed(&state);
            assert!(state.acknowledge_save(invalid).is_err());
            assert_eq!(observed(&state), before);
        }
        for operation in [0, 1] {
            let before = observed(&state);
            let result = if operation == 0 {
                state.reject_save(11, 8, 1, 1)
            } else {
                state.mark_save_unknown(11, 8, 1, 1)
            };
            assert_eq!(
                result,
                Err(interactive_editor_error::opened(
                    opened_editor_error::wrong_snapshot
                ))
            );
            assert_eq!(observed(&state), before);
        }
        state.acknowledge_save(exact).unwrap();
        assert!(!state.view().close_armed());
        assert!(!state.view().is_dirty());
        let before = observed(&state);
        assert_eq!(
            state.acknowledge_save(exact),
            Err(interactive_editor_error::opened(
                opened_editor_error::no_pending_save
            ))
        );
        assert_eq!(observed(&state), before);
    }

    #[test]
    fn unknown_outcome_survives_undo_more_editing_and_input_reset_until_exact_success() {
        let mut state = session(b"a");
        state.apply(action::select_all).unwrap();
        state.apply(action::insert(b'b')).unwrap();
        let snapshot = state.begin_save().unwrap();
        state.mark_save_unknown(11, 7, 1, 1).unwrap();
        state.apply(action::undo).unwrap();
        assert_eq!(state.view().text(), b"a");
        assert_eq!(state.model().state().saved_text(), b"a");
        assert!(state.view().is_dirty());
        state.apply(action::input_reset).unwrap();
        state.apply(action::insert(b'c')).unwrap();
        assert_eq!(state.view().text(), b"c");
        assert_eq!(state.model().state().pending_save(), Some(&snapshot));
        assert_eq!(state.view().save_outcome(), save_outcome::unknown);
        let before = observed(&state);
        assert_eq!(
            state.begin_save(),
            Err(interactive_editor_error::opened(
                opened_editor_error::save_pending
            ))
        );
        assert_eq!(observed(&state), before);
        assert_eq!(
            state.reject_save(11, 7, 1, 1),
            Err(interactive_editor_error::opened(
                opened_editor_error::outcome_unknown
            ))
        );
        assert_eq!(observed(&state), before);
        state.acknowledge_save(reply(&snapshot)).unwrap();
        assert_eq!(state.view().text(), b"c");
        assert_eq!(state.model().state().saved_text(), b"b");
        assert_eq!(state.view().save_outcome(), save_outcome::idle);
        assert!(state.view().is_dirty());
    }

    #[test]
    fn deletion_and_navigation_preserve_pending_owner_and_original_selection_for_undo() {
        for anchor in 0..=4 {
            for head in 0..=4 {
                for forward in [false, true] {
                    let mut state = session(b"abcd");
                    let snapshot = state.begin_save().unwrap();
                    state.apply(action::set_selection { anchor, head }).unwrap();
                    let (from, to) = if anchor != head {
                        (anchor.min(head), anchor.max(head))
                    } else if forward {
                        (head, (head + 1).min(4))
                    } else {
                        (head.saturating_sub(1), head)
                    };
                    state
                        .apply(if forward {
                            action::delete_forward
                        } else {
                            action::backspace
                        })
                        .unwrap();
                    let mut expected = [0; 4];
                    expected[..from].copy_from_slice(&b"abcd"[..from]);
                    expected[from..4 - (to - from)].copy_from_slice(&b"abcd"[to..]);
                    assert_eq!(state.view().text(), &expected[..4 - (to - from)]);
                    assert_eq!(state.model().state().pending_save(), Some(&snapshot));
                    assert_eq!(state.model().state().saved_text(), b"abcd");
                    assert_eq!(state.view().revision(), if from == to { 1 } else { 2 });
                    if from != to {
                        state.apply(action::undo).unwrap();
                        assert_eq!(state.view().text(), b"abcd");
                        assert_eq!(state.view().selection(), (anchor, head));
                    }
                    for (motion, next) in [
                        (caret_motion::left, anchor.min(head)),
                        (caret_motion::right, anchor.max(head)),
                    ] {
                        if anchor == head {
                            continue;
                        }
                        state.apply(action::set_selection { anchor, head }).unwrap();
                        state
                            .apply(action::move_caret {
                                motion,
                                extend: false,
                            })
                            .unwrap();
                        assert_eq!(state.view().selection(), (next, next));
                        assert_eq!(state.model().state().pending_save(), Some(&snapshot));
                    }
                }
            }
        }
        let mut state = session(b"abcd");
        state
            .apply(action::set_selection { anchor: 3, head: 1 })
            .unwrap();
        state
            .apply(action::move_caret {
                motion: caret_motion::left,
                extend: true,
            })
            .unwrap();
        assert_eq!(state.view().selection(), (3, 0));
        state
            .apply(action::move_caret {
                motion: caret_motion::end,
                extend: true,
            })
            .unwrap();
        assert_eq!(state.view().selection(), (3, 4));
        assert_eq!(state.view().revision(), 1);
    }

    #[test]
    fn rejected_edits_publish_feedback_without_evicting_history_or_pending_bytes() {
        let mut state = session(&[b'a'; text_capacity]);
        state
            .apply(action::set_selection { anchor: 0, head: 1 })
            .unwrap();
        state.apply(action::insert(b'b')).unwrap();
        let snapshot = state.begin_save().unwrap();
        for (action, error) in [
            (action::insert(b'x'), editor_error::text_capacity),
            (
                action::set_selection {
                    anchor: 32,
                    head: 0,
                },
                editor_error::invalid_selection,
            ),
        ] {
            let sequence = state.view().sequence();
            state.apply(action).unwrap();
            assert_eq!(state.view().feedback(), feedback::rejected(error));
            assert_eq!(state.view().sequence(), sequence + 1);
            assert_eq!(state.view().selection(), (1, 1));
            assert_eq!(state.view().revision(), 2);
            assert_eq!(state.view().undo_depth(), 1);
            assert_eq!(state.model().state().pending_save(), Some(&snapshot));
        }
        state
            .apply(action::set_selection { anchor: 1, head: 0 })
            .unwrap();
        state.apply(action::insert(0)).unwrap();
        assert_eq!(
            state.view().feedback(),
            feedback::rejected(editor_error::unsupported_text)
        );
        assert_eq!(state.view().selection(), (1, 0));
        state.apply(action::insert(b'b')).unwrap();
        assert_eq!(state.view().feedback(), feedback::unchanged);
        assert_eq!(state.view().selection(), (1, 1));
        assert_eq!(state.view().revision(), 2);
        assert_eq!(state.view().undo_depth(), 1);
        state.apply(action::undo).unwrap();
        assert_eq!(state.view().text(), &[b'a'; text_capacity]);
        assert_eq!(state.view().selection(), (0, 1));
        assert_eq!(state.model().state().pending_save(), Some(&snapshot));
    }

    #[test]
    fn close_requires_consecutive_confirmation_for_dirty_pending_or_unknown_state() {
        let mut clean = session(b"a");
        assert_eq!(clean.request_close(), Ok(interactive_close_result::closed));
        assert!(clean.view().is_closed());
        for outcome in [
            save_outcome::idle,
            save_outcome::awaiting,
            save_outcome::unknown,
        ] {
            let mut state = session(b"a");
            state.apply(action::select_all).unwrap();
            state.apply(action::insert(b'b')).unwrap();
            let snapshot = if outcome == save_outcome::idle {
                None
            } else {
                Some(state.begin_save().unwrap())
            };
            if outcome == save_outcome::unknown {
                state.mark_save_unknown(11, 7, 1, 1).unwrap();
            }
            state.apply(action::undo).unwrap();
            if outcome == save_outcome::idle {
                state.apply(action::insert(b'c')).unwrap();
            }
            // Awaiting+undo may be clean by bytes, but still needs confirmation.
            assert_eq!(
                state.request_close(),
                Ok(interactive_close_result::confirmation_required)
            );
            assert!(state.view().close_armed());
            state.apply(action::input_reset).unwrap();
            assert!(!state.view().close_armed());
            assert_eq!(
                state.request_close(),
                Ok(interactive_close_result::confirmation_required)
            );
            state.apply(action::backspace).unwrap();
            assert!(!state.view().close_armed());
            assert_eq!(
                state.request_close(),
                Ok(interactive_close_result::confirmation_required)
            );
            assert_eq!(state.request_close(), Ok(interactive_close_result::closed));
            assert_eq!(state.model().state().pending_save(), snapshot.as_ref());
            let before = observed(&state);
            assert_eq!(
                state.apply(action::insert(b'z')),
                Err(interactive_editor_error::closed)
            );
            assert_eq!(state.begin_save(), Err(interactive_editor_error::closed));
            assert_eq!(state.request_close(), Err(interactive_editor_error::closed));
            if let Some(snapshot) = snapshot {
                assert_eq!(
                    state.acknowledge_save(reply(&snapshot)),
                    Err(interactive_editor_error::closed)
                );
            }
            assert_eq!(observed(&state), before);
        }
    }

    #[test]
    fn session_sequence_exhaustion_rejects_before_any_model_or_close_mutation() {
        let mut state = session(b"a");
        state.apply(action::insert(b'b')).unwrap();
        let snapshot = state.begin_save().unwrap();
        state.sequence = u64::MAX;
        let before = observed(&state);
        let error = Err(interactive_editor_error::sequence_exhausted);
        assert_eq!(state.apply(action::insert(b'c')), error);
        assert_eq!(state.apply(action::input_reset), error);
        assert_eq!(state.acknowledge_save(reply(&snapshot)), error);
        assert_eq!(state.reject_save(11, 7, 1, 1), error);
        assert_eq!(state.mark_save_unknown(11, 7, 1, 1), error);
        assert_eq!(
            state.begin_save(),
            Err(interactive_editor_error::sequence_exhausted)
        );
        assert_eq!(
            state.request_close(),
            Err(interactive_editor_error::sequence_exhausted)
        );
        assert_eq!(observed(&state), before);
        let mut exhausted_version =
            interactive_editor_session::from_open(11, 7, u64::MAX, b"a").unwrap();
        let before = observed(&exhausted_version);
        assert_eq!(
            exhausted_version.begin_save(),
            Err(interactive_editor_error::opened(
                opened_editor_error::version_exhausted
            ))
        );
        assert_eq!(observed(&exhausted_version), before);
    }
}
