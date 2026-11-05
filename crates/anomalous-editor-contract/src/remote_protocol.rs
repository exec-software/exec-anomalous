//! Experimental copied editor messages, not a stable application ABI.
//!
//! Every field is untrusted correlation. Decoding and constructing these values
//! confer no task, route, document or desktop authority. The embedding adapter
//! must authenticate the installed task/channel incarnation before execution or
//! observation admission. ANUI and the document protocol retain their formats.

use crate::{
    caret_motion, interactive_audit, interactive_audit_size, interactive_close_result,
    interactive_editor_action, interactive_editor_error, interactive_editor_session,
    interactive_editor_view, opened_editor_error, save_acknowledgement, save_snapshot,
};

pub const remote_request_size: usize = 64;
pub const remote_message_size: usize = 192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum remote_error {
    length,
    encoding,
    fields,
    action,
    outcome,
    observation,
    connection,
    action_id,
    exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum remote_action {
    edit(interactive_editor_action),
    save,
    request_close,
}

/// Copyable request data; the exact sequence is checked again by the sole owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct remote_request {
    connection: u64,
    action_id: u64,
    expected_sequence: u64,
    action: remote_action,
}

impl remote_request {
    pub fn try_new(
        connection: u64,
        action_id: u64,
        expected_sequence: u64,
        action: remote_action,
    ) -> Result<Self, remote_error> {
        if connection == 0 || action_id == 0 || expected_sequence == 0 {
            return Err(remote_error::fields);
        }
        Ok(Self {
            connection,
            action_id,
            expected_sequence,
            action,
        })
    }
    pub const fn connection(&self) -> u64 {
        self.connection
    }
    pub const fn action_id(&self) -> u64 {
        self.action_id
    }
    pub const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }
    pub const fn action(&self) -> remote_action {
        self.action
    }

    pub fn encode(&self) -> [u8; remote_request_size] {
        let mut bytes = [0; remote_request_size];
        bytes[..8].copy_from_slice(b"ANEA\x01\x00\x40\x00");
        put_word(&mut bytes, 8, self.connection);
        put_word(&mut bytes, 16, self.action_id);
        put_word(&mut bytes, 24, self.expected_sequence);
        let (tag, first, second) = action_words(self.action);
        bytes[32] = tag;
        put_word(&mut bytes, 40, first);
        put_word(&mut bytes, 48, second);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, remote_error> {
        if bytes.len() != remote_request_size {
            return Err(remote_error::length);
        }
        if &bytes[..8] != b"ANEA\x01\x00\x40\x00"
            || bytes[33..40].iter().chain(&bytes[56..64]).any(|b| *b != 0)
        {
            return Err(remote_error::encoding);
        }
        Self::try_new(
            word(bytes, 8),
            word(bytes, 16),
            word(bytes, 24),
            decode_action(bytes[32], word(bytes, 40), word(bytes, 48))?,
        )
    }
}

fn action_words(action: remote_action) -> (u8, u64, u64) {
    use interactive_editor_action as edit;
    match action {
        remote_action::edit(edit::insert(byte)) => (1, u64::from(byte), 0),
        remote_action::edit(edit::backspace) => (2, 0, 0),
        remote_action::edit(edit::delete_forward) => (3, 0, 0),
        remote_action::edit(edit::move_caret { motion, extend }) => (
            4,
            match motion {
                caret_motion::left => 0,
                caret_motion::right => 1,
                caret_motion::start => 2,
                caret_motion::end => 3,
            },
            u64::from(extend),
        ),
        remote_action::edit(edit::set_selection { anchor, head }) => {
            (5, anchor as u64, head as u64)
        }
        remote_action::edit(edit::select_all) => (6, 0, 0),
        remote_action::edit(edit::collapse_selection) => (7, 0, 0),
        remote_action::edit(edit::undo) => (8, 0, 0),
        remote_action::edit(edit::input_reset) => (9, 0, 0),
        remote_action::edit(edit::unsupported_key) => (10, 0, 0),
        remote_action::save => (11, 0, 0),
        remote_action::request_close => (12, 0, 0),
    }
}
fn decode_action(tag: u8, first: u64, second: u64) -> Result<remote_action, remote_error> {
    use interactive_editor_action as edit;
    let action = match (tag, first, second) {
        (1, byte @ 0..=255, 0) => remote_action::edit(edit::insert(byte as u8)),
        (2, 0, 0) => remote_action::edit(edit::backspace),
        (3, 0, 0) => remote_action::edit(edit::delete_forward),
        (4, motion @ 0..=3, extend @ 0..=1) => remote_action::edit(edit::move_caret {
            motion: match motion {
                0 => caret_motion::left,
                1 => caret_motion::right,
                2 => caret_motion::start,
                _ => caret_motion::end,
            },
            extend: extend != 0,
        }),
        (5, anchor, head) => remote_action::edit(edit::set_selection {
            anchor: usize::try_from(anchor).map_err(|_| remote_error::action)?,
            head: usize::try_from(head).map_err(|_| remote_error::action)?,
        }),
        (6, 0, 0) => remote_action::edit(edit::select_all),
        (7, 0, 0) => remote_action::edit(edit::collapse_selection),
        (8, 0, 0) => remote_action::edit(edit::undo),
        (9, 0, 0) => remote_action::edit(edit::input_reset),
        (10, 0, 0) => remote_action::edit(edit::unsupported_key),
        (11, 0, 0) => remote_action::save,
        (12, 0, 0) => remote_action::request_close,
        _ => return Err(remote_error::action),
    };
    Ok(action)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum remote_outcome {
    /// The edit was processed. ANUI feedback may report a legitimate rejected edit.
    edit_applied,
    /// A snapshot was reserved locally, never a durable acknowledgement.
    save_reserved,
    close_confirmation,
    closed,
    stale,
    rejected(interactive_editor_error),
}

/// Authorship must be established separately before accepting any copied ANUI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct remote_message {
    connection: u64,
    action_id: u64,
    expected_sequence: u64,
    outcome: Option<remote_outcome>,
    audit: [u8; interactive_audit_size],
}

impl remote_message {
    pub fn try_observation(connection: u64, audit: &[u8]) -> Result<Self, remote_error> {
        Self::try_new(connection, 0, 0, None, audit)
    }
    pub fn try_completion(
        connection: u64,
        action_id: u64,
        expected_sequence: u64,
        outcome: remote_outcome,
        audit: &[u8],
    ) -> Result<Self, remote_error> {
        Self::try_new(
            connection,
            action_id,
            expected_sequence,
            Some(outcome),
            audit,
        )
    }
    fn try_new(
        connection: u64,
        action_id: u64,
        expected_sequence: u64,
        outcome: Option<remote_outcome>,
        audit: &[u8],
    ) -> Result<Self, remote_error> {
        if connection == 0
            || (outcome.is_none() && (action_id != 0 || expected_sequence != 0))
            || (outcome.is_some() && (action_id == 0 || expected_sequence == 0))
        {
            return Err(remote_error::fields);
        }
        let observation =
            interactive_audit::decode(audit).map_err(|_| remote_error::observation)?;
        if let Some(outcome) = outcome {
            validate_outcome(outcome, expected_sequence, &observation)?;
        }
        Ok(Self {
            connection,
            action_id,
            expected_sequence,
            outcome,
            audit: audit.try_into().map_err(|_| remote_error::length)?,
        })
    }
    pub const fn connection(&self) -> u64 {
        self.connection
    }
    pub const fn action_id(&self) -> u64 {
        self.action_id
    }
    pub const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }
    pub const fn outcome(&self) -> Option<remote_outcome> {
        self.outcome
    }
    pub const fn audit_bytes(&self) -> &[u8; interactive_audit_size] {
        &self.audit
    }
    pub fn observation(&self) -> interactive_audit<'_> {
        // Construction validates the immutable owned bytes.
        interactive_audit::decode(&self.audit).unwrap()
    }
    pub fn encode(&self) -> [u8; remote_message_size] {
        let mut bytes = [0; remote_message_size];
        bytes[..8].copy_from_slice(b"ANER\x01\x00\xc0\x00");
        bytes[8] = if self.outcome.is_some() { 2 } else { 1 };
        let (tag, error) = outcome_words(self.outcome).unwrap();
        bytes[9] = tag;
        bytes[10] = error;
        put_word(&mut bytes, 16, self.connection);
        put_word(&mut bytes, 24, self.action_id);
        put_word(&mut bytes, 32, self.expected_sequence);
        bytes[64..].copy_from_slice(&self.audit);
        bytes
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, remote_error> {
        if bytes.len() != remote_message_size {
            return Err(remote_error::length);
        }
        if &bytes[..8] != b"ANER\x01\x00\xc0\x00"
            || bytes[11..16].iter().chain(&bytes[40..64]).any(|b| *b != 0)
        {
            return Err(remote_error::encoding);
        }
        let outcome = decode_outcome(bytes[8], bytes[9], bytes[10])?;
        Self::try_new(
            word(bytes, 16),
            word(bytes, 24),
            word(bytes, 32),
            outcome,
            &bytes[64..],
        )
    }
}

fn outcome_words(outcome: Option<remote_outcome>) -> Result<(u8, u8), remote_error> {
    Ok(match outcome {
        None => (0, 0),
        Some(remote_outcome::edit_applied) => (1, 0),
        Some(remote_outcome::save_reserved) => (2, 0),
        Some(remote_outcome::close_confirmation) => (3, 0),
        Some(remote_outcome::closed) => (4, 0),
        Some(remote_outcome::stale) => (5, 0),
        Some(remote_outcome::rejected(error)) => (
            6,
            match error {
                interactive_editor_error::closed => 1,
                interactive_editor_error::sequence_exhausted => 2,
                interactive_editor_error::opened(opened_editor_error::save_pending) => 3,
                interactive_editor_error::opened(opened_editor_error::request_exhausted) => 4,
                interactive_editor_error::opened(opened_editor_error::version_exhausted) => 5,
                _ => return Err(remote_error::outcome),
            },
        ),
    })
}
fn decode_outcome(kind: u8, tag: u8, error: u8) -> Result<Option<remote_outcome>, remote_error> {
    let value = match (kind, tag, error) {
        (1, 0, 0) => return Ok(None),
        (2, 1, 0) => remote_outcome::edit_applied,
        (2, 2, 0) => remote_outcome::save_reserved,
        (2, 3, 0) => remote_outcome::close_confirmation,
        (2, 4, 0) => remote_outcome::closed,
        (2, 5, 0) => remote_outcome::stale,
        (2, 6, error @ 1..=5) => remote_outcome::rejected(match error {
            1 => interactive_editor_error::closed,
            2 => interactive_editor_error::sequence_exhausted,
            3 => interactive_editor_error::opened(opened_editor_error::save_pending),
            4 => interactive_editor_error::opened(opened_editor_error::request_exhausted),
            _ => interactive_editor_error::opened(opened_editor_error::version_exhausted),
        }),
        _ => return Err(remote_error::outcome),
    };
    Ok(Some(value))
}
fn validate_outcome(
    outcome: remote_outcome,
    expected: u64,
    view: &interactive_audit<'_>,
) -> Result<(), remote_error> {
    outcome_words(Some(outcome))?;
    let advanced = expected.checked_add(1) == Some(view.sequence);
    let live = !view.closed && !view.close_armed;
    let valid = match outcome {
        remote_outcome::edit_applied => {
            advanced && live && matches!(view.feedback, 1..=7 | 16..=21)
        }
        remote_outcome::save_reserved => {
            advanced
                && live
                && view.feedback == 8
                && view.outcome == 1
                && view.pending_revision == view.revision
        }
        remote_outcome::close_confirmation => {
            advanced
                && !view.closed
                && view.close_armed
                && view.feedback == 12
                && (view.dirty || view.outcome != 0)
        }
        remote_outcome::closed => {
            advanced && view.closed && !view.close_armed && view.feedback == 13
        }
        remote_outcome::stale => expected != view.sequence,
        remote_outcome::rejected(error) => {
            expected == view.sequence
                && match error {
                    interactive_editor_error::closed => view.closed && view.feedback == 13,
                    interactive_editor_error::sequence_exhausted => {
                        !view.closed && expected == u64::MAX
                    }
                    interactive_editor_error::opened(opened_editor_error::save_pending) => {
                        !view.closed && expected != u64::MAX && view.pending_request != 0
                    }
                    interactive_editor_error::opened(opened_editor_error::request_exhausted) => {
                        !view.closed
                            && expected != u64::MAX
                            && view.outcome == 0
                            && view.version != u64::MAX
                    }
                    interactive_editor_error::opened(opened_editor_error::version_exhausted) => {
                        !view.closed
                            && expected != u64::MAX
                            && view.outcome == 0
                            && view.version == u64::MAX
                    }
                    _ => false,
                }
        }
    };
    if valid {
        Ok(())
    } else {
        Err(remote_error::outcome)
    }
}

/// The snapshot remains on the editor side; it is never encoded in a response.
#[derive(Debug, Eq, PartialEq)]
pub enum remote_effect {
    none,
    save(save_snapshot),
}
#[derive(Debug, Eq, PartialEq)]
pub struct remote_execution {
    pub message: remote_message,
    pub effect: remote_effect,
}

/// One non-cloneable owner. `execute` is a trusted embedding boundary: its
/// caller authenticates the request route before invoking it. Connection IDs
/// alone do not authenticate native tasks. No mutable session escape is exposed.
pub struct remote_editor_session {
    connection: u64,
    next_action: u64,
    session: interactive_editor_session,
}
impl remote_editor_session {
    pub fn new(connection: u64, session: interactive_editor_session) -> Result<Self, remote_error> {
        if connection == 0 {
            return Err(remote_error::connection);
        }
        Ok(Self {
            connection,
            next_action: 1,
            session,
        })
    }
    pub fn view(&self) -> interactive_editor_view {
        self.session.view()
    }
    pub fn observation(&self) -> remote_message {
        let audit = interactive_audit::from_view(&self.view()).encode().unwrap();
        remote_message::try_observation(self.connection, &audit).unwrap()
    }
    pub fn execute_bytes(&mut self, bytes: &[u8]) -> Result<remote_execution, remote_error> {
        self.execute(remote_request::decode(bytes)?)
    }
    pub fn execute(&mut self, request: remote_request) -> Result<remote_execution, remote_error> {
        if request.connection != self.connection {
            return Err(remote_error::connection);
        }
        if request.action_id != self.next_action {
            return Err(remote_error::action_id);
        }
        // No effect may happen if we cannot retain the next replay boundary.
        let next = self
            .next_action
            .checked_add(1)
            .ok_or(remote_error::exhausted)?;
        let mut effect = remote_effect::none;
        let outcome = if request.expected_sequence != self.session.view().sequence() {
            remote_outcome::stale
        } else {
            let result = match request.action {
                remote_action::edit(action) => self
                    .session
                    .apply(action)
                    .map(|()| remote_outcome::edit_applied),
                remote_action::save => self.session.begin_save().map(|snapshot| {
                    effect = remote_effect::save(snapshot);
                    remote_outcome::save_reserved
                }),
                remote_action::request_close => {
                    self.session.request_close().map(|outcome| match outcome {
                        interactive_close_result::confirmation_required => {
                            remote_outcome::close_confirmation
                        }
                        interactive_close_result::closed => remote_outcome::closed,
                    })
                }
            };
            result.unwrap_or_else(remote_outcome::rejected)
        };
        // These fields come exclusively from the validated session transitions;
        // construction is infallible for those transitions, including rejection.
        let audit = interactive_audit::from_view(&self.view()).encode().unwrap();
        let message = remote_message::try_completion(
            self.connection,
            request.action_id,
            request.expected_sequence,
            outcome,
            &audit,
        )
        .unwrap();
        self.next_action = next;
        Ok(remote_execution { message, effect })
    }
    /// The embedding adapter has authenticated the live document reply route.
    /// The session independently matches every field against its retained save.
    pub fn acknowledge_save(
        &mut self,
        reply: save_acknowledgement<'_>,
    ) -> Result<(), interactive_editor_error> {
        self.session.acknowledge_save(reply)
    }
    /// Admit only an authenticated definitive rejection. Packet ACKs and a
    /// stale-request response cannot prove that an earlier save did not commit.
    pub fn reject_save(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        self.session
            .reject_save(document, generation, request, base_version)
    }
    /// Preserve the pending snapshot after uncertain delivery or service loss;
    /// this does not cancel it or manufacture a negative durable acknowledgement.
    pub fn mark_save_unknown(
        &mut self,
        document: u64,
        generation: u64,
        request: u64,
        base_version: u64,
    ) -> Result<(), interactive_editor_error> {
        self.session
            .mark_save_unknown(document, generation, request, base_version)
    }
}

fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn put_word(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
#[path = "remote_protocol_tests.rs"]
mod tests;
