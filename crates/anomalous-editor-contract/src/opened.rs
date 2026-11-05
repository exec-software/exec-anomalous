use crate::{
    edit_effect, editor_error, frame_byte_count, frame_metadata, loaded_editor_state,
    render_loaded, text_capacity, theme,
};

/// Correlation data for one copied save request, never service authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct save_snapshot {
    document: u64,
    generation: u64,
    request: u64,
    base_version: u64,
    revision: u64,
    text: [u8; text_capacity],
    length: usize,
}

impl save_snapshot {
    pub const fn document(&self) -> u64 {
        self.document
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn request(&self) -> u64 {
        self.request
    }
    pub const fn base_version(&self) -> u64 {
        self.base_version
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub fn text(&self) -> &[u8] {
        &self.text[..self.length]
    }
}

/// Untrusted decoded reply fields. `acknowledge_save` validates all of them
/// against the retained request before changing any saved state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct save_acknowledgement<'a> {
    pub document: u64,
    pub generation: u64,
    pub request: u64,
    pub base_version: u64,
    pub revision: u64,
    pub version: u64,
    pub text: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum save_outcome {
    idle,
    awaiting,
    unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum opened_editor_error {
    invalid_binding,
    invalid_version,
    save_pending,
    no_pending_save,
    outcome_unknown,
    request_exhausted,
    version_exhausted,
    wrong_snapshot,
    wrong_version,
    wrong_content,
    editor(editor_error),
}

/// One opened bounded document with independently retained current and saved
/// content. A save reply names the submitted snapshot, never whatever text is
/// current when the reply arrives. This client model is deliberately not Copy
/// or Clone: pending request sequences must not be accidentally forked.
#[derive(Debug, Eq, PartialEq)]
pub struct opened_editor_state {
    inner: loaded_editor_state,
    document: u64,
    generation: u64,
    version: u64,
    saved: [u8; text_capacity],
    saved_length: usize,
    next_request: u64,
    pending: Option<save_snapshot>,
    outcome: save_outcome,
}

impl opened_editor_state {
    /// The caller has already authenticated the open reply's service route.
    /// These IDs correlate content; accepting integers alone grants no rights.
    pub fn from_open(
        document: u64,
        generation: u64,
        version: u64,
        text: &[u8],
    ) -> Result<Self, opened_editor_error> {
        if document == 0 || generation == 0 {
            return Err(opened_editor_error::invalid_binding);
        }
        if version == 0 {
            return Err(opened_editor_error::invalid_version);
        }
        let inner = loaded_editor_state::from_text(text).map_err(opened_editor_error::editor)?;
        let mut saved = [0; text_capacity];
        saved[..text.len()].copy_from_slice(text);
        Ok(Self {
            inner,
            document,
            generation,
            version,
            saved,
            saved_length: text.len(),
            next_request: 1,
            pending: None,
            outcome: save_outcome::idle,
        })
    }

    pub fn text(&self) -> &[u8] {
        self.inner.text()
    }
    pub fn saved_text(&self) -> &[u8] {
        &self.saved[..self.saved_length]
    }
    pub const fn document(&self) -> u64 {
        self.document
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn version(&self) -> u64 {
        self.version
    }
    pub fn selection(&self) -> (usize, usize) {
        self.inner.selection()
    }
    pub fn caret(&self) -> usize {
        self.inner.caret()
    }
    pub fn revision(&self) -> u64 {
        self.inner.revision()
    }
    pub fn appearance(&self) -> theme {
        self.inner.appearance()
    }
    pub fn undo_depth(&self) -> u8 {
        self.inner.undo_depth()
    }
    pub fn is_dirty(&self) -> bool {
        // A lost reply may hide a committed snapshot different from both the
        // current buffer and its last acknowledged baseline.
        self.outcome == save_outcome::unknown || self.text() != self.saved_text()
    }
    pub const fn save_outcome(&self) -> save_outcome {
        self.outcome
    }
    pub fn pending_save(&self) -> Option<&save_snapshot> {
        self.pending.as_ref()
    }
    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), editor_error> {
        self.inner.set_selection(anchor, head)
    }
    pub fn set_theme(&mut self, appearance: theme) {
        self.inner.set_theme(appearance);
    }
    pub fn insert(&mut self, text: &[u8]) -> Result<(), editor_error> {
        self.inner.insert(text)
    }
    pub(crate) fn insert_repeated(&mut self, text: &[u8]) -> Result<edit_effect, editor_error> {
        self.inner.insert_repeated(text)
    }
    pub(crate) fn delete_repeated(&mut self, forward: bool) -> Result<edit_effect, editor_error> {
        self.inner.delete_repeated(forward)
    }
    pub fn undo(&mut self) -> Result<(), editor_error> {
        self.inner.undo()
    }

    /// Reserve one request and copy its exact bytes before submitting it. A
    /// failed transport submission may retry this same pending snapshot. A
    /// second save cannot overtake it; local editing remains available.
    pub fn begin_save(&mut self) -> Result<save_snapshot, opened_editor_error> {
        if self.pending.is_some() {
            return Err(opened_editor_error::save_pending);
        }
        self.version
            .checked_add(1)
            .ok_or(opened_editor_error::version_exhausted)?;
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(opened_editor_error::request_exhausted)?;
        let mut text = [0; text_capacity];
        text[..self.text().len()].copy_from_slice(self.text());
        let snapshot = save_snapshot {
            document: self.document,
            generation: self.generation,
            request: self.next_request,
            base_version: self.version,
            revision: self.revision(),
            text,
            length: self.text().len(),
        };
        self.pending = Some(snapshot.clone());
        self.next_request = next;
        self.outcome = save_outcome::awaiting;
        Ok(snapshot)
    }

    fn matching_snapshot(
        &self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<&save_snapshot, opened_editor_error> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(opened_editor_error::no_pending_save)?;
        if (document, generation, request, base_version)
            != (
                pending.document,
                pending.generation,
                pending.request,
                pending.base_version,
            )
        {
            return Err(opened_editor_error::wrong_snapshot);
        }
        Ok(pending)
    }

    /// A success response must echo the exact copied bytes and advance exactly
    /// the submitted store version. All checks precede the baseline change.
    /// In particular, newer edits/selection/theme/undo/revision are preserved.
    pub fn acknowledge_save(
        &mut self,
        reply: save_acknowledgement<'_>,
    ) -> Result<(), opened_editor_error> {
        let pending = self.matching_snapshot(
            reply.document,
            reply.generation,
            reply.request,
            reply.base_version,
        )?;
        if pending.revision != reply.revision {
            return Err(opened_editor_error::wrong_snapshot);
        }
        if pending.base_version.checked_add(1) != Some(reply.version) {
            return Err(opened_editor_error::wrong_version);
        }
        if pending.text() != reply.text {
            return Err(opened_editor_error::wrong_content);
        }
        let (saved, length) = (pending.text, pending.length);
        self.saved = saved;
        self.saved_length = length;
        self.version = reply.version;
        self.pending = None;
        self.outcome = save_outcome::idle;
        Ok(())
    }

    /// A correlated definitive negative response leaves the saved baseline
    /// unchanged. The consumed request ID is never reused by another snapshot.
    /// The caller must know this attempt did not commit. A stale-request reply
    /// is not such proof: the original may already have committed, even while
    /// this model still says awaiting; use mark_save_unknown in that case.
    pub fn reject_save(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), opened_editor_error> {
        self.matching_snapshot(document, generation, request, base_version)?;
        if self.outcome == save_outcome::unknown {
            // A retry can be rejected as stale precisely because the original
            // committed. A negative reply cannot resolve that uncertainty.
            return Err(opened_editor_error::outcome_unknown);
        }
        self.pending = None;
        self.outcome = save_outcome::idle;
        Ok(())
    }

    /// Lost reply/service failure is not proof of either commit or rejection.
    /// Retain its exact snapshot and block another save until the client closes
    /// this model and performs an authenticated reopen/reconciliation.
    pub fn mark_save_unknown(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), opened_editor_error> {
        self.matching_snapshot(document, generation, request, base_version)?;
        self.outcome = save_outcome::unknown;
        Ok(())
    }
}

pub fn render_opened(
    state: &opened_editor_state,
    pixels: &mut [u8; frame_byte_count],
) -> frame_metadata {
    let mut metadata = render_loaded(&state.inner, pixels);
    metadata.dirty = state.is_dirty();
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened() -> opened_editor_state {
        opened_editor_state::from_open(11, 7, 3, b"const before = 1;").unwrap()
    }
    fn edit(state: &mut opened_editor_state) {
        state.set_selection(6, 12).unwrap();
        state.insert(b"after").unwrap();
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
    // A deliberate test-only value snapshot, not a public cloneable client.
    type observation = (
        loaded_editor_state,
        u64,
        u64,
        u64,
        [u8; text_capacity],
        usize,
        u64,
        Option<save_snapshot>,
        save_outcome,
    );
    fn observed(state: &opened_editor_state) -> observation {
        (
            state.inner,
            state.document,
            state.generation,
            state.version,
            state.saved,
            state.saved_length,
            state.next_request,
            state.pending.clone(),
            state.outcome,
        )
    }

    #[test]
    fn open_establishes_exact_clean_content_and_validates_binding() {
        let state = opened();
        assert_eq!(state.text(), state.saved_text());
        assert!(!state.is_dirty());
        assert_eq!(
            (state.document(), state.generation(), state.version()),
            (11, 7, 3)
        );
        assert_eq!(state.save_outcome(), save_outcome::idle);
        for (document, generation) in [(0, 7), (11, 0)] {
            assert_eq!(
                opened_editor_state::from_open(document, generation, 3, b""),
                Err(opened_editor_error::invalid_binding)
            );
        }
        assert_eq!(
            opened_editor_state::from_open(11, 7, 0, b""),
            Err(opened_editor_error::invalid_version)
        );
        for text in [b"\n".as_slice(), &[b'a'; 32]] {
            assert!(opened_editor_state::from_open(11, 7, 3, text).is_err());
        }
        assert!(
            !opened_editor_state::from_open(11, 7, 3, b"")
                .unwrap()
                .is_dirty()
        );
    }

    #[test]
    fn late_ack_saves_submitted_snapshot_and_preserves_newer_edits() {
        let mut state = opened();
        edit(&mut state);
        let snapshot = state.begin_save().unwrap();
        assert_eq!(snapshot.text(), b"const after = 1;");
        assert_eq!(snapshot.revision(), 2);
        state.undo().unwrap();
        state.set_theme(theme::light);
        state.set_selection(8, 8).unwrap();
        let before = state.inner;
        state.acknowledge_save(reply(&snapshot)).unwrap();
        assert_eq!(state.inner, before);
        assert_eq!(state.saved_text(), b"const after = 1;");
        assert!(state.is_dirty());
        let next = state.begin_save().unwrap();
        assert_eq!((next.request(), next.base_version()), (2, 4));
        state.acknowledge_save(reply(&next)).unwrap();
        assert!(!state.is_dirty());
        let reopened =
            opened_editor_state::from_open(11, 8, state.version(), state.saved_text()).unwrap();
        assert_eq!(reopened.text(), b"const before = 1;");
        assert!(!reopened.is_dirty());
    }

    #[test]
    fn malformed_cross_document_and_duplicate_acknowledgements_are_atomic() {
        let mut state = opened();
        edit(&mut state);
        let snapshot = state.begin_save().unwrap();
        let stable = observed(&state);
        assert_eq!(state.begin_save(), Err(opened_editor_error::save_pending));
        let valid = reply(&snapshot);
        for invalid in [
            save_acknowledgement {
                document: 12,
                ..valid
            },
            save_acknowledgement {
                generation: 8,
                ..valid
            },
            save_acknowledgement {
                request: 2,
                ..valid
            },
            save_acknowledgement {
                base_version: 2,
                ..valid
            },
            save_acknowledgement {
                revision: 3,
                ..valid
            },
            save_acknowledgement {
                version: 5,
                ..valid
            },
            save_acknowledgement {
                text: b"forged",
                ..valid
            },
        ] {
            assert!(state.acknowledge_save(invalid).is_err());
            assert_eq!(observed(&state), stable);
        }
        state.acknowledge_save(valid).unwrap();
        let stable = observed(&state);
        assert_eq!(
            state.acknowledge_save(valid),
            Err(opened_editor_error::no_pending_save)
        );
        assert_eq!(observed(&state), stable);
    }

    #[test]
    fn lost_reply_retains_unknown_snapshot_and_failed_save_never_cleans() {
        let mut state = opened();
        edit(&mut state);
        let snapshot = state.begin_save().unwrap();
        assert_eq!(
            state.mark_save_unknown(11, 8, 1, 3),
            Err(opened_editor_error::wrong_snapshot)
        );
        assert_eq!(state.save_outcome(), save_outcome::awaiting);
        state.mark_save_unknown(11, 7, 1, 3).unwrap();
        assert_eq!(state.save_outcome(), save_outcome::unknown);
        assert_eq!(state.pending_save(), Some(&snapshot));
        assert!(state.is_dirty());
        assert_eq!(state.saved_text(), b"const before = 1;");
        assert_eq!(state.begin_save(), Err(opened_editor_error::save_pending));
        let unknown = observed(&state);
        assert_eq!(
            state.reject_save(11, 7, 1, 3),
            Err(opened_editor_error::outcome_unknown)
        );
        assert_eq!(observed(&state), unknown);
        // Only a matching positive result can resolve this model in place.
        state.acknowledge_save(reply(&snapshot)).unwrap();
        assert!(!state.is_dirty());
        let mut state = opened();
        edit(&mut state);
        state.begin_save().unwrap();
        state.reject_save(11, 7, 1, 3).unwrap();
        assert!(state.is_dirty());
        let next = state.begin_save().unwrap();
        assert_eq!(next.request(), 2);
        assert_eq!(
            state.reject_save(11, 7, 1, 3),
            Err(opened_editor_error::wrong_snapshot)
        );
        assert_eq!(state.pending_save(), Some(&next));
    }

    #[test]
    fn editing_away_and_back_uses_content_not_revision_for_dirty_state() {
        let mut state = opened();
        let snapshot = state.begin_save().unwrap();
        edit(&mut state);
        state.undo().unwrap();
        assert_ne!(state.revision(), snapshot.revision());
        state.acknowledge_save(reply(&snapshot)).unwrap();
        assert!(!state.is_dirty());
        let mut pixels = [0; frame_byte_count];
        let metadata = render_opened(&state, &mut pixels);
        assert!(!metadata.dirty);
        assert_eq!(metadata.revision, 3);
    }

    #[test]
    fn undo_before_unknown_stays_unsynchronized_until_positive_resolution() {
        let mut state = opened();
        edit(&mut state);
        let snapshot = state.begin_save().unwrap();
        state.undo().unwrap();
        assert!(!state.is_dirty());
        state.mark_save_unknown(11, 7, 1, 3).unwrap();
        assert!(state.is_dirty());
        assert_eq!(state.pending_save(), Some(&snapshot));
        let mut pixels = [0; frame_byte_count];
        assert!(render_opened(&state, &mut pixels).dirty);
        let stable = observed(&state);
        assert_eq!(
            state.reject_save(11, 7, 1, 3),
            Err(opened_editor_error::outcome_unknown)
        );
        assert_eq!(observed(&state), stable);
        state.acknowledge_save(reply(&snapshot)).unwrap();
        assert_eq!(state.save_outcome(), save_outcome::idle);
        assert_eq!(state.text(), b"const before = 1;");
        assert_eq!(state.saved_text(), b"const after = 1;");
        assert!(state.is_dirty());
    }

    #[test]
    fn exhausted_ids_and_versions_do_not_create_pending_requests() {
        let mut state = opened();
        state.next_request = u64::MAX;
        let stable = observed(&state);
        assert_eq!(
            state.begin_save(),
            Err(opened_editor_error::request_exhausted)
        );
        assert_eq!(observed(&state), stable);
        state.next_request = 1;
        state.version = u64::MAX;
        let stable = observed(&state);
        assert_eq!(
            state.begin_save(),
            Err(opened_editor_error::version_exhausted)
        );
        assert_eq!(observed(&state), stable);
    }

    #[test]
    fn acknowledged_edit_then_undo_and_reedit_preserves_next_save_binding() {
        let mut state = opened_editor_state::from_open(11, 1, 1, b"const before = 1;").unwrap();
        edit(&mut state);
        let first = state.begin_save().unwrap();
        state.acknowledge_save(reply(&first)).unwrap();
        assert_eq!(state.text(), b"const after = 1;");
        assert_eq!(state.version(), 2);
        assert!(!state.is_dirty());
        assert_eq!(state.undo_depth(), 1);

        state.undo().unwrap();
        assert_eq!(state.text(), b"const before = 1;");
        assert_eq!(state.revision(), 3);
        assert_eq!(state.undo_depth(), 0);
        assert!(state.is_dirty());
        state.set_selection(6, 12).unwrap();
        state.insert(b"later").unwrap();
        let next = state.begin_save().unwrap();
        assert_eq!(next.text(), b"const later = 1;");
        assert_eq!(next.document(), 11);
        assert_eq!(next.generation(), 1);
        assert_eq!(next.base_version(), 2);
        assert_eq!(next.request(), 2);
        assert_eq!(next.revision(), 4);
        assert_eq!(state.saved_text(), b"const after = 1;");
        assert_eq!(state.selection(), (11, 11));
        assert_eq!(state.undo_depth(), 1);
        assert!(state.is_dirty());
    }
}
