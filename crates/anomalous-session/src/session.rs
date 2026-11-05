use core::fmt;

use crate::action::{semantic_action, semantic_action_kind, submission_origin};
use crate::identity::{
    consumer_identity, language_version, route_epoch, session_identity, session_revision,
    submission_sequence, wait_identity,
};

pub const session_queue_capacity: usize = 4;
const first_sequence: u64 = 1;
const first_revision: u64 = 1;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum session_error {
    invalid_action,
    paste_restricted,
    session_closed,
    session_still_open,
    terminal_wake_pending,
    session_generation_exhausted,
    consumer_generation_exhausted,
    route_epoch_exhausted,
    wrong_session,
    wrong_consumer,
    stale_route,
    queue_full,
    submission_sequence_exhausted,
    wait_sequence_exhausted,
    revision_exhausted,
    queue_empty,
    wait_already_present,
    stale_wait,
    stale_revision,
    wake_not_pending,
    wake_already_pending,
    wake_acknowledgement_required,
    invariant_violation,
}

impl session_error {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::invalid_action => "invalid_action",
            Self::paste_restricted => "paste_restricted",
            Self::session_closed => "session_closed",
            Self::session_still_open => "session_still_open",
            Self::terminal_wake_pending => "terminal_wake_pending",
            Self::session_generation_exhausted => "session_generation_exhausted",
            Self::consumer_generation_exhausted => "consumer_generation_exhausted",
            Self::route_epoch_exhausted => "route_epoch_exhausted",
            Self::wrong_session => "wrong_session",
            Self::wrong_consumer => "wrong_consumer",
            Self::stale_route => "stale_route",
            Self::queue_full => "queue_full",
            Self::submission_sequence_exhausted => "submission_sequence_exhausted",
            Self::wait_sequence_exhausted => "wait_sequence_exhausted",
            Self::revision_exhausted => "revision_exhausted",
            Self::queue_empty => "queue_empty",
            Self::wait_already_present => "wait_already_present",
            Self::stale_wait => "stale_wait",
            Self::stale_revision => "stale_revision",
            Self::wake_not_pending => "wake_not_pending",
            Self::wake_already_pending => "wake_already_pending",
            Self::wake_acknowledgement_required => "wake_acknowledgement_required",
            Self::invariant_violation => "invariant_violation",
        }
    }
}

impl fmt::Debug for session_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for session_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum close_reason {
    requested,
    route_replaced,
    consumer_replaced,
    source_failed,
}

pub struct prepared_submission {
    session: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
    origin: submission_origin,
    action: semantic_action,
}

impl prepared_submission {
    #[must_use]
    pub fn origin(&self) -> submission_origin {
        self.origin
    }

    /// Exposes the prepared action's content-derived discriminator locally.
    #[must_use]
    pub fn action_kind(&self) -> Option<semantic_action_kind> {
        self.action.kind()
    }

    /// Returns the private action to its owner without exposing its contents.
    #[must_use]
    pub fn into_action(self) -> semantic_action {
        self.action
    }
}

pub struct prepare_rejection {
    error: session_error,
    action: semantic_action,
}

impl prepare_rejection {
    #[must_use]
    pub fn error(&self) -> session_error {
        self.error
    }

    #[must_use]
    pub fn action(&self) -> &semantic_action {
        &self.action
    }

    #[must_use]
    pub fn into_action(self) -> semantic_action {
        self.action
    }
}

pub struct publish_rejection {
    error: session_error,
    submission: prepared_submission,
}

impl publish_rejection {
    #[must_use]
    pub fn error(&self) -> session_error {
        self.error
    }

    #[must_use]
    pub fn submission(&self) -> &prepared_submission {
        &self.submission
    }

    #[must_use]
    pub fn into_submission(self) -> prepared_submission {
        self.submission
    }
}

pub struct publish_receipt {
    sequence: submission_sequence,
    wake: Option<wait_identity>,
}

impl publish_receipt {
    #[must_use]
    pub fn sequence(&self) -> submission_sequence {
        self.sequence
    }

    #[must_use]
    pub fn wake(&self) -> Option<wait_identity> {
        self.wake
    }
}

pub struct session_submission {
    session: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
    sequence: submission_sequence,
    origin: submission_origin,
    action: semantic_action,
}

impl session_submission {
    #[must_use]
    pub fn session(&self) -> session_identity {
        self.session
    }

    #[must_use]
    pub fn consumer(&self) -> consumer_identity {
        self.consumer
    }

    #[must_use]
    pub fn route(&self) -> route_epoch {
        self.route
    }

    #[must_use]
    pub fn sequence(&self) -> submission_sequence {
        self.sequence
    }

    #[must_use]
    pub fn origin(&self) -> submission_origin {
        self.origin
    }

    #[must_use]
    pub fn action(&self) -> &semantic_action {
        &self.action
    }
}

pub struct prepared_wait {
    session: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
    proposed_sequence: u64,
    revision: u64,
}

pub enum receive_preparation {
    ready,
    wait(prepared_wait),
}

pub enum wait_commit {
    ready,
    blocked(wait_identity),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum wake_outcome {
    input_ready,
    terminal(close_reason),
}

pub struct wake_acknowledgement {
    identity: wait_identity,
    outcome: wake_outcome,
}

impl wake_acknowledgement {
    #[must_use]
    pub fn identity(&self) -> wait_identity {
        self.identity
    }

    #[must_use]
    pub fn outcome(&self) -> wake_outcome {
        self.outcome
    }
}

pub struct wait_cancellation {
    identity: wait_identity,
}

impl wait_cancellation {
    #[must_use]
    pub fn identity(&self) -> wait_identity {
        self.identity
    }
}

pub struct close_receipt {
    first_close: bool,
    discarded: u8,
    terminal_wake: Option<wait_identity>,
}

impl close_receipt {
    #[must_use]
    pub fn was_first_close(&self) -> bool {
        self.first_close
    }

    #[must_use]
    pub fn discarded_count(&self) -> u8 {
        self.discarded
    }

    #[must_use]
    pub fn terminal_wake(&self) -> Option<wait_identity> {
        self.terminal_wake
    }
}

#[derive(Clone, Copy)]
struct sequence_cursor {
    next: u64,
    exhausted: bool,
}

impl sequence_cursor {
    const fn new() -> Self {
        Self {
            next: first_sequence,
            exhausted: false,
        }
    }

    const fn prepare(self) -> Option<sequence_issue> {
        if self.exhausted || self.next == 0 {
            return None;
        }
        Some(sequence_issue {
            value: self.next,
            next: if self.next == u64::MAX {
                u64::MAX
            } else {
                self.next + 1
            },
            exhausted: self.next == u64::MAX,
        })
    }

    fn commit(&mut self, issue: sequence_issue) {
        self.next = issue.next;
        self.exhausted = issue.exhausted;
    }

    #[cfg(test)]
    fn invariants_hold(self) -> bool {
        self.next != 0 && (!self.exhausted || self.next == u64::MAX)
    }

    #[cfg(test)]
    fn has_issued(self, value: u64) -> bool {
        value != 0 && (self.exhausted || value < self.next)
    }

    #[cfg(test)]
    fn is_most_recent(self, value: u64) -> bool {
        if value == 0 {
            return false;
        }
        if self.exhausted {
            return value == u64::MAX;
        }
        self.next.checked_sub(1) == Some(value)
    }
}

#[derive(Clone, Copy)]
struct sequence_issue {
    value: u64,
    next: u64,
    exhausted: bool,
}

struct queue_slot {
    occupied: bool,
    sequence: u64,
    origin: submission_origin,
    action: semantic_action,
}

impl queue_slot {
    fn empty() -> Self {
        Self {
            occupied: false,
            sequence: 0,
            origin: submission_origin::human_text,
            action: semantic_action::empty(),
        }
    }

    fn install(&mut self, sequence: u64, origin: submission_origin, action: semantic_action) {
        debug_assert!(!self.occupied);
        debug_assert!(sequence != 0);
        debug_assert!(action.is_present());
        self.sequence = sequence;
        self.origin = origin;
        self.action = action;
        self.occupied = true;
    }

    fn take(
        &mut self,
        session: session_identity,
        consumer: consumer_identity,
        route: route_epoch,
    ) -> Option<session_submission> {
        if !self.occupied || self.sequence == 0 || !self.action.is_present() {
            return None;
        }
        let action = core::mem::replace(&mut self.action, semantic_action::empty());
        let submission = session_submission {
            session,
            consumer,
            route,
            sequence: submission_sequence(self.sequence),
            origin: self.origin,
            action,
        };
        self.occupied = false;
        self.sequence = 0;
        self.origin = submission_origin::human_text;
        Some(submission)
    }

    fn clear(&mut self) {
        self.action.clear();
        self.occupied = false;
        self.sequence = 0;
        self.origin = submission_origin::human_text;
    }

    #[cfg(test)]
    fn storage_is_clear(&self) -> bool {
        !self.occupied
            && self.sequence == 0
            && self.origin == submission_origin::human_text
            && self.action.storage_is_clear()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum wait_state {
    none,
    blocked(wait_identity),
    wake_pending(wait_identity),
    terminal(wait_identity, close_reason),
}

/// One fixed-capacity shell session with one exact consumer and input route.
///
/// The type is intentionally neither cloneable nor formattable. It stores no
/// ambient process state, authority, persistent history, or physical input.
/// Dropping it clears owned private storage as a fail-safe, but is not a
/// protocol close: callers must invoke [`Self::close`] to preserve discard
/// accounting and deliver any terminal wake before destroying the owner.
#[must_use = "call close before dropping a live session to preserve protocol guarantees"]
pub struct shell_session {
    identity: session_identity,
    consumer: consumer_identity,
    route: route_epoch,
    language: language_version,
    open: bool,
    queue: [queue_slot; session_queue_capacity],
    head: usize,
    tail: usize,
    length: usize,
    submissions: sequence_cursor,
    waits: sequence_cursor,
    revision: u64,
    wait: wait_state,
}

impl shell_session {
    /// Creates an initial session incarnation.
    ///
    /// The trusted host must reserve `identity.slot()` as this owner's exclusive
    /// lineage anchor. That slot, across every generation, must never be assigned
    /// to another independent owner, live or retired. This allocation-free crate
    /// has no global registry; [`Self::replace_closed`] advances safely only
    /// inside that exclusively reserved namespace.
    pub fn new(
        identity: session_identity,
        consumer: consumer_identity,
        route: route_epoch,
        language: language_version,
    ) -> Self {
        Self {
            identity,
            consumer,
            route,
            language,
            open: true,
            queue: core::array::from_fn(|_| queue_slot::empty()),
            head: 0,
            tail: 0,
            length: 0,
            submissions: sequence_cursor::new(),
            waits: sequence_cursor::new(),
            revision: first_revision,
            wait: wait_state::none,
        }
    }

    #[must_use]
    pub fn identity(&self) -> session_identity {
        self.identity
    }

    #[must_use]
    pub fn consumer(&self) -> consumer_identity {
        self.consumer
    }

    #[must_use]
    pub fn route(&self) -> route_epoch {
        self.route
    }

    #[must_use]
    pub fn language(&self) -> language_version {
        self.language
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    #[must_use]
    pub fn queue_depth(&self) -> u8 {
        self.length as u8
    }

    #[must_use]
    pub fn revision(&self) -> session_revision {
        session_revision(self.revision)
    }

    /// Places a pristine session at its final submission sequence.
    ///
    /// This is deliberately available only to the kernel's differential test
    /// harness. It exposes no private content or state and is absent from the
    /// default crate surface.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_final_submission_sequence_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.submissions = sequence_cursor {
            next: u64::MAX,
            exhausted: false,
        };
        Ok(())
    }

    /// Places a pristine session at its final wait sequence.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_final_wait_sequence_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.waits = sequence_cursor {
            next: u64::MAX,
            exhausted: false,
        };
        Ok(())
    }

    /// Leaves exactly three revision advances for wait, close, and terminal
    /// acknowledgement in a pristine session.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_revision_retirement_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.revision = u64::MAX - 3;
        Ok(())
    }

    /// Leaves close headroom but rejects every ordinary mutation in a pristine
    /// session.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_close_only_revision_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.revision = u64::MAX - 2;
        Ok(())
    }

    /// Exhausts revision space in a pristine session so a composite owner can
    /// prove that a rejected close preserves its surrounding authority state.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_exhausted_revision_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.revision = u64::MAX;
        Ok(())
    }

    /// Leaves the final headroom at which wait, publication, input-ready
    /// acknowledgement, and receive can all complete while preserving close
    /// retirement space.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_last_wake_delivery_revision_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.revision = u64::MAX - 6;
        Ok(())
    }

    /// Leaves one revision too few for publication to a blocked wait and its
    /// later acknowledgement-plus-receive completion.
    #[cfg(feature = "kernel_conformance_test_support")]
    #[doc(hidden)]
    pub fn prepare_rejected_wake_delivery_revision_for_kernel_conformance(
        &mut self,
    ) -> Result<(), session_error> {
        if !self.is_pristine_kernel_conformance_fixture() {
            return Err(session_error::invariant_violation);
        }
        self.revision = u64::MAX - 5;
        Ok(())
    }

    /// Stamps a semantic action with this trusted router's current origin label.
    ///
    /// The origin enum is attribution data, not authentication or authority.
    /// An untrusted source must not call this API directly and self-select an
    /// origin; the trusted session host validates the source and supplies it.
    // Returning private ownership on rejection is intentionally allocation-free.
    #[allow(clippy::result_large_err)]
    pub fn prepare_submission(
        &self,
        origin: submission_origin,
        action: semantic_action,
    ) -> Result<prepared_submission, prepare_rejection> {
        if !self.open {
            return Err(prepare_rejection {
                error: session_error::session_closed,
                action,
            });
        }
        let Some(kind) = action.kind() else {
            return Err(prepare_rejection {
                error: session_error::invalid_action,
                action,
            });
        };
        if origin == submission_origin::paste && kind != semantic_action_kind::insert_text {
            return Err(prepare_rejection {
                error: session_error::paste_restricted,
                action,
            });
        }
        Ok(prepared_submission {
            session: self.identity,
            consumer: self.consumer,
            route: self.route,
            origin,
            action,
        })
    }

    // Returning private ownership on rejection is intentionally allocation-free.
    #[allow(clippy::result_large_err)]
    pub fn publish(
        &mut self,
        submission: prepared_submission,
    ) -> Result<publish_receipt, publish_rejection> {
        let error = if submission.session != self.identity {
            Some(session_error::wrong_session)
        } else if submission.consumer != self.consumer {
            Some(session_error::wrong_consumer)
        } else if submission.route != self.route {
            Some(session_error::stale_route)
        } else if !self.open {
            Some(session_error::session_closed)
        } else if self.length == session_queue_capacity {
            Some(session_error::queue_full)
        } else if self.queue[self.tail].occupied {
            Some(session_error::invariant_violation)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(publish_rejection { error, submission });
        }

        let Some(sequence) = self.submissions.prepare() else {
            return Err(publish_rejection {
                error: session_error::submission_sequence_exhausted,
                submission,
            });
        };
        let wake = match self.wait {
            wait_state::blocked(identity) => Some(identity),
            wait_state::none | wait_state::wake_pending(_) => None,
            wait_state::terminal(_, _) => {
                return Err(publish_rejection {
                    error: session_error::invariant_violation,
                    submission,
                });
            }
        };
        // A live wait completes as two host-model transitions: acknowledge,
        // then receive. Preserve headroom for both plus close retirement so an
        // accepted publication cannot strand a wake between those operations.
        let next_revision = match self.wait {
            wait_state::none => self.next_ordinary_revision(),
            wait_state::blocked(_) | wait_state::wake_pending(_) => {
                self.next_revision_with_reserve(4)
            }
            wait_state::terminal(_, _) => Err(session_error::invariant_violation),
        };
        let next_revision = match next_revision {
            Ok(next_revision) => next_revision,
            Err(error) => return Err(publish_rejection { error, submission }),
        };

        self.queue[self.tail].install(sequence.value, submission.origin, submission.action);
        self.tail = (self.tail + 1) % session_queue_capacity;
        self.length += 1;
        self.submissions.commit(sequence);
        if let Some(identity) = wake {
            self.wait = wait_state::wake_pending(identity);
        }
        self.revision = next_revision;
        Ok(publish_receipt {
            sequence: submission_sequence(sequence.value),
            wake,
        })
    }

    pub fn prepare_receive(
        &self,
        consumer: consumer_identity,
    ) -> Result<receive_preparation, session_error> {
        if consumer != self.consumer {
            return Err(session_error::wrong_consumer);
        }
        if !self.open {
            return Err(session_error::session_closed);
        }
        if self.wait != wait_state::none {
            return Err(session_error::wait_already_present);
        }
        if self.length != 0 {
            return Ok(receive_preparation::ready);
        }
        let Some(wait) = self.waits.prepare() else {
            return Err(session_error::wait_sequence_exhausted);
        };
        Ok(receive_preparation::wait(prepared_wait {
            session: self.identity,
            consumer: self.consumer,
            route: self.route,
            proposed_sequence: wait.value,
            revision: self.revision,
        }))
    }

    pub fn commit_wait(&mut self, wait: prepared_wait) -> Result<wait_commit, session_error> {
        self.validate_wait_parts(wait.session, wait.consumer, wait.route)?;
        if !self.open {
            return Err(session_error::session_closed);
        }
        if self.wait != wait_state::none {
            return Err(session_error::wait_already_present);
        }
        let Some(sequence) = self.waits.prepare() else {
            return Err(session_error::wait_sequence_exhausted);
        };
        if sequence.value != wait.proposed_sequence {
            return Err(session_error::stale_wait);
        }
        if self.length != 0 {
            return Ok(wait_commit::ready);
        }
        if self.revision != wait.revision {
            return Err(session_error::stale_revision);
        }
        let next_revision = self.next_ordinary_revision()?;

        let identity = wait_identity {
            session: self.identity,
            consumer: self.consumer,
            route: self.route,
            sequence: sequence.value,
        };
        self.waits.commit(sequence);
        self.wait = wait_state::blocked(identity);
        self.revision = next_revision;
        Ok(wait_commit::blocked(identity))
    }

    pub fn acknowledge_wake(
        &mut self,
        consumer: consumer_identity,
        identity: wait_identity,
    ) -> Result<wake_acknowledgement, session_error> {
        self.validate_wait_parts(identity.session, consumer, identity.route)?;
        if identity.consumer != consumer {
            return Err(session_error::wrong_consumer);
        }
        match self.wait {
            wait_state::wake_pending(expected) => {
                if identity != expected {
                    return Err(session_error::stale_wait);
                }
                if !self.open || self.length == 0 {
                    return Err(session_error::invariant_violation);
                }
                let next_revision = self.next_ordinary_revision()?;
                self.wait = wait_state::none;
                self.revision = next_revision;
                Ok(wake_acknowledgement {
                    identity,
                    outcome: wake_outcome::input_ready,
                })
            }
            wait_state::terminal(expected, reason) => {
                if identity != expected {
                    return Err(session_error::stale_wait);
                }
                if self.open || self.length != 0 {
                    return Err(session_error::invariant_violation);
                }
                let next_revision = self.next_revision_with_reserve(0)?;
                self.wait = wait_state::none;
                self.revision = next_revision;
                Ok(wake_acknowledgement {
                    identity,
                    outcome: wake_outcome::terminal(reason),
                })
            }
            wait_state::blocked(expected) if identity == expected => {
                Err(session_error::wake_not_pending)
            }
            wait_state::none | wait_state::blocked(_) => Err(session_error::stale_wait),
        }
    }

    pub fn cancel_wait(
        &mut self,
        consumer: consumer_identity,
        identity: wait_identity,
    ) -> Result<wait_cancellation, session_error> {
        self.validate_wait_parts(identity.session, consumer, identity.route)?;
        if identity.consumer != consumer {
            return Err(session_error::wrong_consumer);
        }
        if !self.open {
            return Err(session_error::session_closed);
        }
        match self.wait {
            wait_state::blocked(expected) if identity == expected => {}
            wait_state::wake_pending(expected) if identity == expected => {
                return Err(session_error::wake_already_pending);
            }
            _ => return Err(session_error::stale_wait),
        }
        let next_revision = self.next_ordinary_revision()?;
        self.wait = wait_state::none;
        self.revision = next_revision;
        Ok(wait_cancellation { identity })
    }

    pub fn receive(
        &mut self,
        consumer: consumer_identity,
    ) -> Result<session_submission, session_error> {
        if consumer != self.consumer {
            return Err(session_error::wrong_consumer);
        }
        if !self.open {
            return Err(session_error::session_closed);
        }
        if self.wait != wait_state::none {
            return Err(session_error::wake_acknowledgement_required);
        }
        if self.length == 0 {
            return Err(session_error::queue_empty);
        }
        if !self.queue[self.head].occupied {
            return Err(session_error::invariant_violation);
        }
        let next_revision = self.next_ordinary_revision()?;
        let Some(submission) = self.queue[self.head].take(self.identity, self.consumer, self.route)
        else {
            return Err(session_error::invariant_violation);
        };
        self.head = (self.head + 1) % session_queue_capacity;
        self.length -= 1;
        self.revision = next_revision;
        Ok(submission)
    }

    pub fn close(&mut self, reason: close_reason) -> Result<close_receipt, session_error> {
        if !self.open {
            return Ok(close_receipt {
                first_close: false,
                discarded: 0,
                terminal_wake: None,
            });
        }
        let terminal_wake = match self.wait {
            wait_state::none => None,
            wait_state::blocked(identity) | wait_state::wake_pending(identity) => Some(identity),
            wait_state::terminal(_, _) => return Err(session_error::invariant_violation),
        };
        let next_revision = self.next_revision_with_reserve(u64::from(terminal_wake.is_some()))?;
        let discarded = self.length as u8;
        self.clear_queue();
        self.open = false;
        self.wait = match terminal_wake {
            Some(identity) => wait_state::terminal(identity, reason),
            None => wait_state::none,
        };
        self.revision = next_revision;
        Ok(close_receipt {
            first_close: true,
            discarded,
            terminal_wake,
        })
    }

    /// Starts the next incarnation in this session owner's exact lineage.
    ///
    /// Replacement is valid only after explicit close and acknowledgement of
    /// any terminal tombstone. All identity successors are preflighted before
    /// mutation, so exhaustion cannot partially change the closed owner.
    pub fn replace_closed(&mut self) -> Result<(), session_error> {
        if self.open {
            return Err(session_error::session_still_open);
        }
        match self.wait {
            wait_state::none => {}
            wait_state::terminal(_, _) => return Err(session_error::terminal_wake_pending),
            wait_state::blocked(_) | wait_state::wake_pending(_) => {
                return Err(session_error::invariant_violation);
            }
        }
        if self.length != 0 {
            return Err(session_error::invariant_violation);
        }

        let next_identity = self
            .identity
            .next_generation()
            .map_err(|_| session_error::session_generation_exhausted)?;
        let next_consumer = self
            .consumer
            .next_generation()
            .map_err(|_| session_error::consumer_generation_exhausted)?;
        let next_route = self
            .route
            .next()
            .map_err(|_| session_error::route_epoch_exhausted)?;

        self.clear_queue();
        self.identity = next_identity;
        self.consumer = next_consumer;
        self.route = next_route;
        self.submissions = sequence_cursor::new();
        self.waits = sequence_cursor::new();
        self.revision = first_revision;
        self.wait = wait_state::none;
        self.open = true;
        Ok(())
    }

    fn next_ordinary_revision(&self) -> Result<u64, session_error> {
        self.next_revision_with_reserve(2)
    }

    #[cfg(feature = "kernel_conformance_test_support")]
    fn is_pristine_kernel_conformance_fixture(&self) -> bool {
        self.open
            && self.length == 0
            && self.head == 0
            && self.tail == 0
            && self.wait == wait_state::none
            && self.submissions.next == first_sequence
            && !self.submissions.exhausted
            && self.waits.next == first_sequence
            && !self.waits.exhausted
            && self.revision == first_revision
            && self.queue.iter().all(|slot| !slot.occupied)
    }

    fn next_revision_with_reserve(&self, remaining: u64) -> Result<u64, session_error> {
        let Some(next) = self.revision.checked_add(1) else {
            return Err(session_error::revision_exhausted);
        };
        if u64::MAX - next < remaining {
            return Err(session_error::revision_exhausted);
        }
        Ok(next)
    }

    fn validate_wait_parts(
        &self,
        session: session_identity,
        consumer: consumer_identity,
        route: route_epoch,
    ) -> Result<(), session_error> {
        if session != self.identity {
            return Err(session_error::wrong_session);
        }
        if consumer != self.consumer {
            return Err(session_error::wrong_consumer);
        }
        if route != self.route {
            return Err(session_error::stale_route);
        }
        Ok(())
    }

    fn clear_queue(&mut self) {
        for slot in &mut self.queue {
            slot.clear();
        }
        self.head = 0;
        self.tail = 0;
        self.length = 0;
    }

    #[cfg(test)]
    fn invariants_hold(&self) -> bool {
        if self.head >= session_queue_capacity
            || self.tail >= session_queue_capacity
            || self.length > session_queue_capacity
            || self.revision == 0
            || !self.submissions.invariants_hold()
            || !self.waits.invariants_hold()
            || self.identity.slot() == 0
            || self.identity.generation() == 0
            || self.consumer.value() == 0
            || self.consumer.generation() == 0
            || self.route.get() == 0
            || (!self.open && self.length != 0)
        {
            return false;
        }
        if self.queue.iter().filter(|slot| slot.occupied).count() != self.length {
            return false;
        }

        let mut previous_sequence = 0;
        for offset in 0..session_queue_capacity {
            let expected = offset < self.length;
            let index = (self.head + offset) % session_queue_capacity;
            let slot = &self.queue[index];
            if slot.occupied != expected {
                return false;
            }
            if expected {
                if slot.sequence == 0
                    || !self.submissions.has_issued(slot.sequence)
                    || !slot.action.is_present()
                    || !slot.action.invariants_hold()
                    || (previous_sequence != 0 && slot.sequence <= previous_sequence)
                {
                    return false;
                }
                previous_sequence = slot.sequence;
            } else if !slot.storage_is_clear() {
                return false;
            }
        }
        if self.tail != (self.head + self.length) % session_queue_capacity {
            return false;
        }

        let valid_wait = |identity: wait_identity| {
            identity.session == self.identity
                && identity.consumer == self.consumer
                && identity.route == self.route
                && identity.sequence != 0
                && self.waits.is_most_recent(identity.sequence)
        };
        match self.wait {
            wait_state::none => self.open || self.length == 0,
            wait_state::blocked(identity) => self.open && self.length == 0 && valid_wait(identity),
            wait_state::wake_pending(identity) => {
                self.open && self.length != 0 && valid_wait(identity)
            }
            wait_state::terminal(identity, _) => {
                !self.open && self.length == 0 && valid_wait(identity)
            }
        }
    }
}

/// Privacy fail-safe clearing only; this does not perform protocol close.
impl Drop for shell_session {
    fn drop(&mut self) {
        self.clear_queue();
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
