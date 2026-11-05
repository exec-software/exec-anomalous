#![cfg_attr(not(test), allow(dead_code))]

//! Feature-only, architecture-neutral semantic-session input endpoint.
//!
//! This module is the bounded state-machine side of ADR 0022. It owns one
//! exact session route, one exact consumer task, four private FIFO slots, and
//! at most one pending read. It performs no authentication, focus arbitration,
//! scheduler transition, Rite admission, user copy, parsing, or evaluation.
//!
//! # Scheduler and Rite integration seam
//!
//! The intended integration is deliberately narrow:
//!
//! 1. [`session_input_endpoint::prepare_receive`] and
//!    [`session_input_endpoint::commit_wait`] install one exact wait. The
//!    returned [`session_input_blocked_wait`] supplies correlation values for
//!    an owned pending Rite channel; no borrow or user pointer is retained.
//! 2. Publication and route closure are prepare/commit transactions. A commit
//!    may return one move-only [`session_input_prepared_wake`]. The scheduler
//!    borrows its exact accessors during preflight and consumes the whole token
//!    in its endpoint-gated blocked-to-runnable commit. There is intentionally
//!    no task-only wake accessor.
//! 3. After the scheduler selects that exact task, the pending Rite channel
//!    passes its wait/readiness correlation through
//!    [`session_input_endpoint::prepare_delivery`]. After the complete copy,
//!    [`session_input_endpoint::commit_prepared_delivery`] atomically
//!    acknowledges the input-ready wait and consumes the source. A terminal
//!    outcome instead uses [`session_input_endpoint::prepare_terminal`] and
//!    [`session_input_endpoint::commit_prepared_terminal`]; it can never enable
//!    a receive.
//!
//! All numeric identities are freshness values, not authority. The eventual
//! route host, Rite gate, and scheduler remain responsible for authorization,
//! task-state validation, serialized exclusion, and consuming every returned
//! prepared token before releasing that exclusion.

use core::fmt;
use core::num::NonZeroU64;

use crate::identity::{endpoint_id, task_identity};

pub(crate) const session_input_queue_capacity: usize = 4;
pub(crate) const session_input_text_capacity: usize = 256;
pub(crate) const session_input_erase_limit: u16 = 256;

const first_sequence: u64 = 1;
const first_revision: u64 = 1;

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u16)]
pub(crate) enum session_input_action_kind {
    insert_text = 1,
    erase_previous = 2,
    submit = 3,
    cancel_submission = 4,
    source_discontinuity = 5,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u16)]
pub(crate) enum session_input_origin {
    human_text = 1,
    machine_text = 2,
    paste = 3,
    accessibility = 4,
    remote_text = 5,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum session_input_close_reason {
    focus_lost,
    route_replaced,
    consumer_replaced,
    source_failed,
    requested,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum session_input_wake_outcome {
    input_ready,
    terminal(session_input_close_reason),
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum session_input_error {
    zero_endpoint_generation,
    zero_session_slot,
    zero_session_generation,
    zero_consumer,
    zero_consumer_generation,
    zero_route_epoch,
    empty_text,
    invalid_utf8,
    text_limit_exceeded,
    forbidden_scalar,
    zero_erase_count,
    erase_limit_exceeded,
    paste_restricted,
    wrong_endpoint,
    wrong_session,
    wrong_consumer,
    stale_route,
    wrong_task,
    session_closed,
    queue_full,
    queue_empty,
    wait_already_present,
    wait_not_ready,
    wake_acknowledgement_required,
    terminal_acknowledgement_required,
    stale_wait,
    stale_readiness,
    wrong_wake_outcome,
    stale_preparation,
    delivery_sequence_exhausted,
    wait_sequence_exhausted,
    readiness_generation_exhausted,
    revision_exhausted,
    copy_failed,
    invariant_violation,
}

impl session_input_error {
    #[must_use]
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::zero_endpoint_generation => "zero_endpoint_generation",
            Self::zero_session_slot => "zero_session_slot",
            Self::zero_session_generation => "zero_session_generation",
            Self::zero_consumer => "zero_consumer",
            Self::zero_consumer_generation => "zero_consumer_generation",
            Self::zero_route_epoch => "zero_route_epoch",
            Self::empty_text => "empty_text",
            Self::invalid_utf8 => "invalid_utf8",
            Self::text_limit_exceeded => "text_limit_exceeded",
            Self::forbidden_scalar => "forbidden_scalar",
            Self::zero_erase_count => "zero_erase_count",
            Self::erase_limit_exceeded => "erase_limit_exceeded",
            Self::paste_restricted => "paste_restricted",
            Self::wrong_endpoint => "wrong_endpoint",
            Self::wrong_session => "wrong_session",
            Self::wrong_consumer => "wrong_consumer",
            Self::stale_route => "stale_route",
            Self::wrong_task => "wrong_task",
            Self::session_closed => "session_closed",
            Self::queue_full => "queue_full",
            Self::queue_empty => "queue_empty",
            Self::wait_already_present => "wait_already_present",
            Self::wait_not_ready => "wait_not_ready",
            Self::wake_acknowledgement_required => "wake_acknowledgement_required",
            Self::terminal_acknowledgement_required => "terminal_acknowledgement_required",
            Self::stale_wait => "stale_wait",
            Self::stale_readiness => "stale_readiness",
            Self::wrong_wake_outcome => "wrong_wake_outcome",
            Self::stale_preparation => "stale_preparation",
            Self::delivery_sequence_exhausted => "delivery_sequence_exhausted",
            Self::wait_sequence_exhausted => "wait_sequence_exhausted",
            Self::readiness_generation_exhausted => "readiness_generation_exhausted",
            Self::revision_exhausted => "revision_exhausted",
            Self::copy_failed => "copy_failed",
            Self::invariant_violation => "invariant_violation",
        }
    }
}

impl fmt::Debug for session_input_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for session_input_error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_endpoint_identity(endpoint_id);

impl session_input_endpoint_identity {
    pub(crate) const fn try_new(slot: usize, generation: u64) -> Result<Self, session_input_error> {
        let Some(identity) = endpoint_id::try_from_parts(slot, generation) else {
            return Err(session_input_error::zero_endpoint_generation);
        };
        Ok(Self(identity))
    }

    #[must_use]
    pub(crate) const fn kernel_identity(self) -> endpoint_id {
        self.0
    }

    #[must_use]
    pub(crate) const fn slot(self) -> usize {
        self.0.slot()
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.0.generation_value()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_session_identity {
    slot: NonZeroU64,
    generation: NonZeroU64,
}

impl session_input_session_identity {
    pub(crate) const fn try_new(slot: u64, generation: u64) -> Result<Self, session_input_error> {
        let Some(slot) = NonZeroU64::new(slot) else {
            return Err(session_input_error::zero_session_slot);
        };
        let Some(generation) = NonZeroU64::new(generation) else {
            return Err(session_input_error::zero_session_generation);
        };
        Ok(Self { slot, generation })
    }

    #[must_use]
    pub(crate) const fn slot(self) -> u64 {
        self.slot.get()
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.generation.get()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_consumer_identity {
    value: NonZeroU64,
    generation: NonZeroU64,
}

impl session_input_consumer_identity {
    pub(crate) const fn try_new(value: u64, generation: u64) -> Result<Self, session_input_error> {
        let Some(value) = NonZeroU64::new(value) else {
            return Err(session_input_error::zero_consumer);
        };
        let Some(generation) = NonZeroU64::new(generation) else {
            return Err(session_input_error::zero_consumer_generation);
        };
        Ok(Self { value, generation })
    }

    #[must_use]
    pub(crate) const fn value(self) -> u64 {
        self.value.get()
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.generation.get()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_route_epoch(NonZeroU64);

impl session_input_route_epoch {
    pub(crate) const fn try_new(value: u64) -> Result<Self, session_input_error> {
        let Some(value) = NonZeroU64::new(value) else {
            return Err(session_input_error::zero_route_epoch);
        };
        Ok(Self(value))
    }

    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Complete freshness correlation for one fixed consumer route.
///
/// This value is copyable because it is not authority. Every endpoint method
/// still validates every component before inspecting private state.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_binding {
    endpoint: session_input_endpoint_identity,
    session: session_input_session_identity,
    consumer: session_input_consumer_identity,
    route: session_input_route_epoch,
    task: task_identity,
}

impl session_input_binding {
    #[must_use]
    pub(crate) const fn new(
        endpoint: session_input_endpoint_identity,
        session: session_input_session_identity,
        consumer: session_input_consumer_identity,
        route: session_input_route_epoch,
        task: task_identity,
    ) -> Self {
        Self {
            endpoint,
            session,
            consumer,
            route,
            task,
        }
    }

    #[must_use]
    pub(crate) const fn endpoint(self) -> session_input_endpoint_identity {
        self.endpoint
    }

    #[must_use]
    pub(crate) const fn session(self) -> session_input_session_identity {
        self.session
    }

    #[must_use]
    pub(crate) const fn consumer(self) -> session_input_consumer_identity {
        self.consumer
    }

    #[must_use]
    pub(crate) const fn route(self) -> session_input_route_epoch {
        self.route
    }

    #[must_use]
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_delivery_sequence(NonZeroU64);

impl session_input_delivery_sequence {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_revision(NonZeroU64);

impl session_input_revision {
    #[must_use]
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_wait_identity {
    binding: session_input_binding,
    sequence: NonZeroU64,
}

impl session_input_wait_identity {
    #[must_use]
    pub(crate) const fn binding(self) -> session_input_binding {
        self.binding
    }

    #[must_use]
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence.get()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_readiness_identity {
    binding: session_input_binding,
    generation: NonZeroU64,
}

impl session_input_readiness_identity {
    #[must_use]
    pub(crate) const fn binding(self) -> session_input_binding {
        self.binding
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.generation.get()
    }
}

/// Copyable freshness description used only to preflight scheduler placement.
///
/// This value is not a wake authorization. The endpoint issues the actual
/// move-only [`session_input_prepared_wake`] only after publication or close
/// commits successfully.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct session_input_wake_correlation {
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    outcome: session_input_wake_outcome,
}

impl session_input_wake_correlation {
    #[must_use]
    pub(crate) const fn new(
        wait: session_input_wait_identity,
        readiness: session_input_readiness_identity,
        outcome: session_input_wake_outcome,
    ) -> Self {
        Self {
            wait,
            readiness,
            outcome,
        }
    }

    #[must_use]
    pub(crate) const fn binding(self) -> session_input_binding {
        self.wait.binding
    }

    #[must_use]
    pub(crate) const fn wait(self) -> session_input_wait_identity {
        self.wait
    }

    #[must_use]
    pub(crate) const fn readiness(self) -> session_input_readiness_identity {
        self.readiness
    }

    #[must_use]
    pub(crate) const fn outcome(self) -> session_input_wake_outcome {
        self.outcome
    }
}

/// One move-only semantic action with owned private text storage.
///
/// The type deliberately implements none of `Clone`, `Copy`, `Debug`,
/// `Display`, or `Hash`. Every content-derived getter is an explicit local
/// disclosure boundary and grants no permission to format, persist, or log the
/// returned value.
#[must_use]
pub(crate) struct session_input_action {
    present: bool,
    kind: session_input_action_kind,
    erase_count: u16,
    text_length: u16,
    text: [u8; session_input_text_capacity],
}

impl session_input_action {
    pub(crate) fn try_insert_text(bytes: &[u8]) -> Result<Self, session_input_error> {
        if bytes.is_empty() {
            return Err(session_input_error::empty_text);
        }
        if bytes.len() > session_input_text_capacity {
            return Err(session_input_error::text_limit_exceeded);
        }
        let text = core::str::from_utf8(bytes).map_err(|_| session_input_error::invalid_utf8)?;
        if text.chars().any(forbidden_scalar) {
            return Err(session_input_error::forbidden_scalar);
        }

        let mut action = Self::cleared();
        action.present = true;
        action.text[..bytes.len()].copy_from_slice(bytes);
        action.text_length = bytes.len() as u16;
        Ok(action)
    }

    pub(crate) fn erase_previous(count: u16) -> Result<Self, session_input_error> {
        if count == 0 {
            return Err(session_input_error::zero_erase_count);
        }
        if count > session_input_erase_limit {
            return Err(session_input_error::erase_limit_exceeded);
        }
        Ok(Self {
            present: true,
            kind: session_input_action_kind::erase_previous,
            erase_count: count,
            text_length: 0,
            text: [0; session_input_text_capacity],
        })
    }

    pub(crate) fn submit() -> Self {
        Self::without_content(session_input_action_kind::submit)
    }

    pub(crate) fn cancel_submission() -> Self {
        Self::without_content(session_input_action_kind::cancel_submission)
    }

    pub(crate) fn source_discontinuity() -> Self {
        Self::without_content(session_input_action_kind::source_discontinuity)
    }

    /// Exposes the private action discriminator locally.
    #[must_use]
    pub(crate) fn expose_kind(&self) -> session_input_action_kind {
        assert!(self.present, "session input action invariant violated");
        self.kind
    }

    /// Exposes private insertion text locally.
    #[must_use]
    pub(crate) fn expose_text(&self) -> Option<&str> {
        if !self.present || self.kind != session_input_action_kind::insert_text {
            return None;
        }
        let length = usize::from(self.text_length);
        match core::str::from_utf8(&self.text[..length]) {
            Ok(text) => Some(text),
            Err(_) => panic!("session input action UTF-8 invariant violated"),
        }
    }

    /// Exposes the exact private text length locally.
    #[must_use]
    pub(crate) fn expose_byte_len(&self) -> Option<u16> {
        (self.present && self.kind == session_input_action_kind::insert_text)
            .then_some(self.text_length)
    }

    /// Exposes the exact private erase count locally.
    #[must_use]
    pub(crate) fn expose_erase_count(&self) -> Option<u16> {
        (self.present && self.kind == session_input_action_kind::erase_previous)
            .then_some(self.erase_count)
    }

    fn without_content(kind: session_input_action_kind) -> Self {
        Self {
            present: true,
            kind,
            erase_count: 0,
            text_length: 0,
            text: [0; session_input_text_capacity],
        }
    }

    fn cleared() -> Self {
        Self {
            present: false,
            kind: session_input_action_kind::insert_text,
            erase_count: 0,
            text_length: 0,
            text: [0; session_input_text_capacity],
        }
    }

    fn clear(&mut self) {
        self.text.fill(0);
        self.present = false;
        self.kind = session_input_action_kind::insert_text;
        self.erase_count = 0;
        self.text_length = 0;
    }

    fn invariants_hold(&self) -> bool {
        if !self.present {
            return self.kind == session_input_action_kind::insert_text
                && self.erase_count == 0
                && self.text_length == 0
                && self.text.iter().all(|byte| *byte == 0);
        }
        let length = usize::from(self.text_length);
        match self.kind {
            session_input_action_kind::insert_text => {
                self.erase_count == 0
                    && length != 0
                    && length <= session_input_text_capacity
                    && core::str::from_utf8(&self.text[..length])
                        .is_ok_and(|text| !text.chars().any(forbidden_scalar))
                    && self.text[length..].iter().all(|byte| *byte == 0)
            }
            session_input_action_kind::erase_previous => {
                self.erase_count != 0
                    && self.erase_count <= session_input_erase_limit
                    && self.text_length == 0
                    && self.text.iter().all(|byte| *byte == 0)
            }
            session_input_action_kind::submit
            | session_input_action_kind::cancel_submission
            | session_input_action_kind::source_discontinuity => {
                self.erase_count == 0
                    && self.text_length == 0
                    && self.text.iter().all(|byte| *byte == 0)
            }
        }
    }
}

impl Drop for session_input_action {
    fn drop(&mut self) {
        self.clear();
    }
}

fn forbidden_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{0000}'..='\u{0008}'
            | '\u{000b}'..='\u{001f}'
            | '\u{007f}'..='\u{009f}'
            | '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{180e}'
            | '\u{feff}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
    )
}

#[derive(Clone, Copy, Eq, PartialEq)]
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

    fn invariants_hold(self) -> bool {
        self.next != 0 && (!self.exhausted || self.next == u64::MAX)
    }

    fn has_issued(self, value: u64) -> bool {
        value != 0 && (self.exhausted || value < self.next)
    }

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

#[derive(Clone, Copy, Eq, PartialEq)]
struct sequence_issue {
    value: u64,
    next: u64,
    exhausted: bool,
}

struct session_input_queue_slot {
    occupied: bool,
    sequence: u64,
    origin: u16,
    kind: u16,
    erase_count: u16,
    text_length: u16,
    text: [u8; session_input_text_capacity],
}

impl session_input_queue_slot {
    const fn empty() -> Self {
        Self {
            occupied: false,
            sequence: 0,
            origin: 0,
            kind: 0,
            erase_count: 0,
            text_length: 0,
            text: [0; session_input_text_capacity],
        }
    }

    fn install(
        &mut self,
        sequence: u64,
        origin: session_input_origin,
        action: &mut session_input_action,
    ) {
        debug_assert!(self.storage_is_clear());
        debug_assert!(sequence != 0);
        debug_assert!(action.invariants_hold());
        self.sequence = sequence;
        self.origin = origin as u16;
        self.kind = action.kind as u16;
        self.erase_count = action.erase_count;
        self.text_length = action.text_length;
        self.text.copy_from_slice(&action.text);
        self.occupied = true;
        action.clear();
    }

    fn clear(&mut self) {
        self.text.fill(0);
        self.occupied = false;
        self.sequence = 0;
        self.origin = 0;
        self.kind = 0;
        self.erase_count = 0;
        self.text_length = 0;
    }

    fn origin(&self) -> Option<session_input_origin> {
        match self.origin {
            value if value == session_input_origin::human_text as u16 => {
                Some(session_input_origin::human_text)
            }
            value if value == session_input_origin::machine_text as u16 => {
                Some(session_input_origin::machine_text)
            }
            value if value == session_input_origin::paste as u16 => {
                Some(session_input_origin::paste)
            }
            value if value == session_input_origin::accessibility as u16 => {
                Some(session_input_origin::accessibility)
            }
            value if value == session_input_origin::remote_text as u16 => {
                Some(session_input_origin::remote_text)
            }
            _ => None,
        }
    }

    fn kind(&self) -> Option<session_input_action_kind> {
        match self.kind {
            value if value == session_input_action_kind::insert_text as u16 => {
                Some(session_input_action_kind::insert_text)
            }
            value if value == session_input_action_kind::erase_previous as u16 => {
                Some(session_input_action_kind::erase_previous)
            }
            value if value == session_input_action_kind::submit as u16 => {
                Some(session_input_action_kind::submit)
            }
            value if value == session_input_action_kind::cancel_submission as u16 => {
                Some(session_input_action_kind::cancel_submission)
            }
            value if value == session_input_action_kind::source_discontinuity as u16 => {
                Some(session_input_action_kind::source_discontinuity)
            }
            _ => None,
        }
    }

    fn invariants_hold(&self) -> bool {
        if !self.occupied {
            return self.storage_is_clear();
        }
        if self.sequence == 0 || self.origin().is_none() {
            return false;
        }
        let length = usize::from(self.text_length);
        match self.kind() {
            Some(session_input_action_kind::insert_text) => {
                self.erase_count == 0
                    && length != 0
                    && length <= session_input_text_capacity
                    && core::str::from_utf8(&self.text[..length])
                        .is_ok_and(|text| !text.chars().any(forbidden_scalar))
                    && self.text[length..].iter().all(|byte| *byte == 0)
            }
            Some(session_input_action_kind::erase_previous) => {
                self.erase_count != 0
                    && self.erase_count <= session_input_erase_limit
                    && self.text_length == 0
                    && self.text.iter().all(|byte| *byte == 0)
            }
            Some(
                session_input_action_kind::submit
                | session_input_action_kind::cancel_submission
                | session_input_action_kind::source_discontinuity,
            ) => {
                self.erase_count == 0
                    && self.text_length == 0
                    && self.text.iter().all(|byte| *byte == 0)
            }
            None => false,
        }
    }

    fn storage_is_clear(&self) -> bool {
        !self.occupied
            && self.sequence == 0
            && self.origin == 0
            && self.kind == 0
            && self.erase_count == 0
            && self.text_length == 0
            && self.text.iter().all(|byte| *byte == 0)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum session_input_phase {
    open_idle,
    open_blocked,
    open_input_ready,
    closed_idle,
    closed_terminal,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct session_input_wait_record {
    identity: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    terminal_reason: Option<session_input_close_reason>,
}

impl session_input_wait_record {
    const fn blocked(
        identity: session_input_wait_identity,
        readiness: session_input_readiness_identity,
    ) -> Self {
        Self {
            identity,
            readiness,
            terminal_reason: None,
        }
    }

    const fn terminal(self, reason: session_input_close_reason) -> Self {
        Self {
            terminal_reason: Some(reason),
            ..self
        }
    }
}

/// Move-only preparation for installing one empty-queue wait.
#[must_use = "a prepared wait must be committed or explicitly discarded"]
pub(crate) struct session_input_prepared_wait {
    binding: session_input_binding,
    wait: sequence_issue,
    readiness: sequence_issue,
    revision: u64,
}

impl session_input_prepared_wait {
    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.binding
    }

    /// Exact proposed wait correlation for Rite/scheduler preflight only.
    #[must_use]
    pub(crate) fn proposed_wait(&self) -> session_input_wait_identity {
        let Some(sequence) = NonZeroU64::new(self.wait.value) else {
            panic!("prepared session input wait invariant violated");
        };
        session_input_wait_identity {
            binding: self.binding,
            sequence,
        }
    }

    /// Exact proposed readiness correlation for Rite/scheduler preflight only.
    #[must_use]
    pub(crate) fn proposed_readiness(&self) -> session_input_readiness_identity {
        let Some(generation) = NonZeroU64::new(self.readiness.value) else {
            panic!("prepared session input readiness invariant violated");
        };
        session_input_readiness_identity {
            binding: self.binding,
            generation,
        }
    }
}

pub(crate) enum session_input_receive_preparation {
    ready,
    wait(session_input_prepared_wait),
}

/// Exact correlation to transfer into an owned pending Rite channel.
///
/// This owner is move-only. Its scalar accessors expose freshness values only;
/// they do not authorize a block, wake, or delivery.
#[must_use = "an installed wait must be retained by the exact pending Rite channel"]
pub(crate) struct session_input_blocked_wait {
    identity: session_input_wait_identity,
    readiness: session_input_readiness_identity,
}

impl session_input_blocked_wait {
    #[must_use]
    pub(crate) const fn identity(&self) -> session_input_wait_identity {
        self.identity
    }

    #[must_use]
    pub(crate) const fn readiness(&self) -> session_input_readiness_identity {
        self.readiness
    }

    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.identity.binding
    }
}

pub(crate) enum session_input_wait_commit {
    ready,
    blocked(session_input_blocked_wait),
}

/// Move-only publication preparation retaining private action ownership.
#[must_use = "a prepared publication retains private input until committed or rejected"]
pub(crate) struct session_input_prepared_publication {
    binding: session_input_binding,
    origin: session_input_origin,
    action: session_input_action,
    delivery: sequence_issue,
    revision: u64,
    next_revision: u64,
    phase: session_input_phase,
    wait: Option<session_input_wait_record>,
    tail: usize,
}

impl session_input_prepared_publication {
    /// Exposes the private action discriminator locally.
    #[must_use]
    pub(crate) fn expose_action_kind(&self) -> session_input_action_kind {
        self.action.expose_kind()
    }

    /// Returns the exact prospective wake for scheduler preflight. The copyable
    /// description is freshness data only; commit later returns the move-only
    /// wake authorization with the same values.
    #[must_use]
    pub(crate) const fn wake_correlation(&self) -> Option<session_input_wake_correlation> {
        match (self.phase, self.wait) {
            (session_input_phase::open_blocked, Some(wait)) => {
                Some(session_input_wake_correlation {
                    wait: wait.identity,
                    readiness: wait.readiness,
                    outcome: session_input_wake_outcome::input_ready,
                })
            }
            _ => None,
        }
    }
}

/// A rejected publication with returned private action ownership.
///
/// This type intentionally has no implicit formatting, cloning, or hashing.
pub(crate) struct session_input_publication_rejection {
    error: session_input_error,
    action: session_input_action,
}

impl session_input_publication_rejection {
    #[must_use]
    pub(crate) const fn error(&self) -> session_input_error {
        self.error
    }

    /// Exposes the rejected private action locally.
    pub(crate) const fn action(&self) -> &session_input_action {
        &self.action
    }

    pub(crate) fn into_action(self) -> session_input_action {
        self.action
    }
}

/// Endpoint-issued, move-only proof that one exact scheduler wake is pending.
///
/// The future scheduler composite must accept this owner by value, validate
/// every accessor during preflight, and consume it in the exact FIFO-tail
/// placement commit. Copying the contained identities does not copy authority.
#[must_use = "the exact scheduler commit must consume every prepared wake"]
pub(crate) struct session_input_prepared_wake {
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    outcome: session_input_wake_outcome,
}

impl session_input_prepared_wake {
    #[must_use]
    pub(crate) const fn correlation(&self) -> session_input_wake_correlation {
        session_input_wake_correlation {
            wait: self.wait,
            readiness: self.readiness,
            outcome: self.outcome,
        }
    }

    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.wait.binding
    }

    #[must_use]
    pub(crate) const fn endpoint(&self) -> session_input_endpoint_identity {
        self.wait.binding.endpoint
    }

    #[must_use]
    pub(crate) const fn session(&self) -> session_input_session_identity {
        self.wait.binding.session
    }

    #[must_use]
    pub(crate) const fn consumer(&self) -> session_input_consumer_identity {
        self.wait.binding.consumer
    }

    #[must_use]
    pub(crate) const fn route(&self) -> session_input_route_epoch {
        self.wait.binding.route
    }

    #[must_use]
    pub(crate) const fn task(&self) -> task_identity {
        self.wait.binding.task
    }

    #[must_use]
    pub(crate) const fn wait(&self) -> session_input_wait_identity {
        self.wait
    }

    #[must_use]
    pub(crate) const fn readiness(&self) -> session_input_readiness_identity {
        self.readiness
    }

    #[must_use]
    pub(crate) const fn outcome(&self) -> session_input_wake_outcome {
        self.outcome
    }
}

/// Publication result. Any wake remains move-only inside this owner.
#[must_use = "inspect and consume any exact prepared wake"]
pub(crate) struct session_input_publication_receipt {
    sequence: session_input_delivery_sequence,
    wake: Option<session_input_prepared_wake>,
}

impl session_input_publication_receipt {
    #[must_use]
    pub(crate) const fn sequence(&self) -> session_input_delivery_sequence {
        self.sequence
    }

    #[must_use]
    pub(crate) const fn wake(&self) -> Option<&session_input_prepared_wake> {
        self.wake.as_ref()
    }

    #[must_use]
    pub(crate) fn into_wake(self) -> Option<session_input_prepared_wake> {
        self.wake
    }
}

/// Move-only route-close preparation.
#[must_use = "a prepared close must be committed or explicitly discarded"]
pub(crate) struct session_input_prepared_close {
    binding: session_input_binding,
    reason: session_input_close_reason,
    revision: u64,
    next_revision: u64,
    phase: session_input_phase,
    wait: Option<session_input_wait_record>,
    discarded: u8,
}

impl session_input_prepared_close {
    /// Returns a prospective terminal wake only while the consumer is still
    /// blocked. An input-ready consumer was already placed and must not be
    /// enqueued a second time.
    #[must_use]
    pub(crate) const fn wake_correlation(&self) -> Option<session_input_wake_correlation> {
        match (self.phase, self.wait) {
            (session_input_phase::open_blocked, Some(wait)) => {
                Some(session_input_wake_correlation {
                    wait: wait.identity,
                    readiness: wait.readiness,
                    outcome: session_input_wake_outcome::terminal(self.reason),
                })
            }
            _ => None,
        }
    }

    /// Prospective terminal completion for every live wait.
    ///
    /// Unlike `wake_correlation`, this is also present when an input-ready
    /// consumer is already runnable and therefore must not be enqueued again.
    #[must_use]
    pub(crate) const fn terminal_correlation(&self) -> Option<session_input_wake_correlation> {
        match (self.phase, self.wait) {
            (
                session_input_phase::open_blocked | session_input_phase::open_input_ready,
                Some(wait),
            ) => Some(session_input_wake_correlation {
                wait: wait.identity,
                readiness: wait.readiness,
                outcome: session_input_wake_outcome::terminal(self.reason),
            }),
            _ => None,
        }
    }
}

// Both variants remain inline to keep the kernel mechanism allocation-free.
#[allow(clippy::large_enum_variant)]
pub(crate) enum session_input_close_preparation {
    already_closed,
    prepared(session_input_prepared_close),
}

/// Route-close result. A blocked waiter yields one move-only terminal wake;
/// an input-ready waiter was already placed and yields no duplicate wake.
#[must_use = "inspect and consume any exact terminal wake"]
pub(crate) struct session_input_close_receipt {
    first_close: bool,
    discarded: u8,
    wake: Option<session_input_prepared_wake>,
    terminal: Option<session_input_wake_correlation>,
}

impl session_input_close_receipt {
    #[must_use]
    pub(crate) const fn was_first_close(&self) -> bool {
        self.first_close
    }

    #[must_use]
    pub(crate) const fn discarded_count(&self) -> u8 {
        self.discarded
    }

    #[must_use]
    pub(crate) const fn wake(&self) -> Option<&session_input_prepared_wake> {
        self.wake.as_ref()
    }

    #[must_use]
    pub(crate) fn into_wake(self) -> Option<session_input_prepared_wake> {
        self.wake
    }

    /// Exact terminal completion correlation retained even when the consumer
    /// was already made runnable by an earlier input-ready publication.
    #[must_use]
    pub(crate) const fn terminal_correlation(&self) -> Option<session_input_wake_correlation> {
        self.terminal
    }
}

/// Exact acknowledgement supplied only after the scheduler selected the
/// consumer named by this correlation.
///
/// The constructor performs no authorization. The endpoint revalidates every
/// component, while the scheduler/Rite composite must prove that the bound task
/// is now current before constructing the value.
#[must_use]
pub(crate) struct session_input_wait_acknowledgement {
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
}

impl session_input_wait_acknowledgement {
    pub(crate) const fn new(
        wait: session_input_wait_identity,
        readiness: session_input_readiness_identity,
    ) -> Self {
        Self { wait, readiness }
    }

    #[must_use]
    pub(crate) const fn wait(&self) -> session_input_wait_identity {
        self.wait
    }

    #[must_use]
    pub(crate) const fn readiness(&self) -> session_input_readiness_identity {
        self.readiness
    }
}

/// Borrowed private head presented to transport preflight and commit.
///
/// This view is move-only and cannot outlive the endpoint borrow. Every
/// content-derived getter is a local disclosure boundary.
#[must_use]
pub(crate) struct session_input_delivery<'a> {
    binding: session_input_binding,
    slot: &'a session_input_queue_slot,
}

impl session_input_delivery<'_> {
    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.binding
    }

    /// Exposes the private delivery sequence locally.
    #[must_use]
    pub(crate) fn expose_sequence(&self) -> session_input_delivery_sequence {
        let Some(sequence) = NonZeroU64::new(self.slot.sequence) else {
            panic!("session input delivery sequence invariant violated");
        };
        session_input_delivery_sequence(sequence)
    }

    /// Exposes private source attribution locally.
    #[must_use]
    pub(crate) fn expose_origin(&self) -> session_input_origin {
        self.slot
            .origin()
            .unwrap_or_else(|| panic!("session input origin invariant violated"))
    }

    /// Exposes the private action discriminator locally.
    #[must_use]
    pub(crate) fn expose_action_kind(&self) -> session_input_action_kind {
        self.slot
            .kind()
            .unwrap_or_else(|| panic!("session input action invariant violated"))
    }

    /// Exposes the exact private text length locally.
    #[must_use]
    pub(crate) const fn expose_byte_len(&self) -> u16 {
        self.slot.text_length
    }

    /// Exposes private insertion text locally.
    #[must_use]
    pub(crate) fn expose_text(&self) -> Option<&str> {
        if self.expose_action_kind() != session_input_action_kind::insert_text {
            return None;
        }
        let length = usize::from(self.slot.text_length);
        match core::str::from_utf8(&self.slot.text[..length]) {
            Ok(text) => Some(text),
            Err(_) => panic!("session input delivery UTF-8 invariant violated"),
        }
    }

    /// Exposes the exact private erase count locally.
    #[must_use]
    pub(crate) fn expose_erase_count(&self) -> Option<u16> {
        (self.expose_action_kind() == session_input_action_kind::erase_previous)
            .then_some(self.slot.erase_count)
    }

    /// Exposes the fixed, zero-tailed private text field to the one named wire
    /// encoder. Callers must not use this as a general serialization surface.
    #[must_use]
    pub(crate) const fn expose_fixed_text(&self) -> &[u8; session_input_text_capacity] {
        &self.slot.text
    }
}

/// Move-only snapshot of one source head and its exact wait correlation.
///
/// Preparing this value neither copies nor consumes private input. The stable
/// runtime uses [`Self::delivery`] for transport encoding before selection,
/// then gives the owner back to the endpoint's infallible selected-task
/// commit. All fields are freshness checks, never authority.
#[must_use = "a prepared delivery must be committed or explicitly discarded"]
pub(crate) struct session_input_prepared_delivery {
    binding: session_input_binding,
    correlation: Option<session_input_wake_correlation>,
    revision: u64,
    next_revision: u64,
    phase: session_input_phase,
    wait: Option<session_input_wait_record>,
    head: usize,
    sequence: u64,
}

impl session_input_prepared_delivery {
    /// Reborrow the frozen private head for the one named reply encoder.
    ///
    /// This remains fallible so every fact can be checked before the scheduler
    /// selects the consumer. Under serialized exclusion, no check remains for
    /// the post-selection commit other than fail-stop invariant assertions.
    pub(crate) fn delivery<'a>(
        &self,
        endpoint: &'a session_input_endpoint,
    ) -> Result<session_input_delivery<'a>, session_input_error> {
        endpoint.validate_prepared_delivery(self)?;
        Ok(session_input_delivery {
            binding: self.binding,
            slot: &endpoint.queue[self.head],
        })
    }

    #[must_use]
    pub(crate) const fn correlation(&self) -> Option<session_input_wake_correlation> {
        self.correlation
    }
}

/// Move-only snapshot of one closed-terminal tombstone.
///
/// It is prepared before scheduler selection. Its later commit is therefore
/// bounded and infallible under the same serialized exclusion.
#[must_use = "a prepared terminal acknowledgement must be committed or discarded"]
pub(crate) struct session_input_prepared_terminal {
    binding: session_input_binding,
    correlation: session_input_wake_correlation,
    revision: u64,
    next_revision: u64,
    wait: session_input_wait_record,
    reason: session_input_close_reason,
}

impl session_input_prepared_terminal {
    #[must_use]
    pub(crate) const fn correlation(&self) -> session_input_wake_correlation {
        self.correlation
    }
}

/// Content-free record of one successful copy-before-consume transition.
///
/// The delivery sequence is private correlation and must not enter telemetry.
#[must_use]
pub(crate) struct session_input_delivery_receipt {
    sequence: session_input_delivery_sequence,
    acknowledged_wait: bool,
}

impl session_input_delivery_receipt {
    #[must_use]
    pub(crate) const fn sequence(&self) -> session_input_delivery_sequence {
        self.sequence
    }

    #[must_use]
    pub(crate) const fn acknowledged_wait(&self) -> bool {
        self.acknowledged_wait
    }
}

/// Content-free record of one terminal tombstone acknowledgement.
#[must_use]
pub(crate) struct session_input_terminal_acknowledgement {
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    reason: session_input_close_reason,
}

impl session_input_terminal_acknowledgement {
    #[must_use]
    pub(crate) const fn wait(&self) -> session_input_wait_identity {
        self.wait
    }

    #[must_use]
    pub(crate) const fn readiness(&self) -> session_input_readiness_identity {
        self.readiness
    }

    #[must_use]
    pub(crate) const fn reason(&self) -> session_input_close_reason {
        self.reason
    }
}

/// One fixed private session-input endpoint.
///
/// The trusted proof host must assign the complete binding exclusively for the
/// owner's lifetime. Dropping this owner clears its private storage as a
/// fail-safe, but it is not a protocol close and cannot deliver a terminal
/// wake. Callers must complete the explicit close/terminal-acknowledgement
/// sequence before orderly destruction.
#[must_use = "explicitly close and retire any pending terminal wake before dropping"]
pub(crate) struct session_input_endpoint {
    binding: session_input_binding,
    phase: session_input_phase,
    queue: [session_input_queue_slot; session_input_queue_capacity],
    head: usize,
    tail: usize,
    length: usize,
    deliveries: sequence_cursor,
    waits: sequence_cursor,
    readiness: sequence_cursor,
    revision: u64,
    wait: Option<session_input_wait_record>,
}

/// Exact, non-executable logical state used only by cross-layer atomicity
/// tests. It deliberately implements neither `Clone`, `Copy`, nor `Debug`, and
/// clears its copied private queue bytes on drop.
#[cfg(test)]
#[derive(Eq, PartialEq)]
pub(crate) struct session_input_endpoint_test_snapshot {
    binding: session_input_binding,
    phase: session_input_phase,
    queue: [session_input_queue_slot_test_snapshot; session_input_queue_capacity],
    head: usize,
    tail: usize,
    length: usize,
    deliveries: sequence_cursor,
    waits: sequence_cursor,
    readiness: sequence_cursor,
    revision: u64,
    wait: Option<session_input_wait_record>,
}

#[cfg(test)]
#[derive(Eq, PartialEq)]
struct session_input_queue_slot_test_snapshot {
    occupied: bool,
    sequence: u64,
    origin: u16,
    kind: u16,
    erase_count: u16,
    text_length: u16,
    text: [u8; session_input_text_capacity],
}

#[cfg(test)]
impl Drop for session_input_endpoint_test_snapshot {
    fn drop(&mut self) {
        for slot in &mut self.queue {
            slot.text.fill(0);
            slot.occupied = false;
            slot.sequence = 0;
            slot.origin = 0;
            slot.kind = 0;
            slot.erase_count = 0;
            slot.text_length = 0;
        }
        self.head = 0;
        self.tail = 0;
        self.length = 0;
        self.deliveries = sequence_cursor::new();
        self.waits = sequence_cursor::new();
        self.readiness = sequence_cursor::new();
        self.revision = 0;
        self.wait = None;
    }
}

impl session_input_endpoint {
    pub(crate) fn new(binding: session_input_binding) -> Self {
        Self {
            binding,
            phase: session_input_phase::open_idle,
            queue: core::array::from_fn(|_| session_input_queue_slot::empty()),
            head: 0,
            tail: 0,
            length: 0,
            deliveries: sequence_cursor::new(),
            waits: sequence_cursor::new(),
            readiness: sequence_cursor::new(),
            revision: first_revision,
            wait: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> session_input_endpoint_test_snapshot {
        session_input_endpoint_test_snapshot {
            binding: self.binding,
            phase: self.phase,
            queue: core::array::from_fn(|index| {
                let slot = &self.queue[index];
                session_input_queue_slot_test_snapshot {
                    occupied: slot.occupied,
                    sequence: slot.sequence,
                    origin: slot.origin,
                    kind: slot.kind,
                    erase_count: slot.erase_count,
                    text_length: slot.text_length,
                    text: slot.text,
                }
            }),
            head: self.head,
            tail: self.tail,
            length: self.length,
            deliveries: self.deliveries,
            waits: self.waits,
            readiness: self.readiness,
            revision: self.revision,
            wait: self.wait,
        }
    }

    /// Returns freshness correlation only, never authority.
    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.binding
    }

    #[must_use]
    pub(crate) fn revision(&self) -> session_input_revision {
        let Some(revision) = NonZeroU64::new(self.revision) else {
            panic!("session input revision invariant violated");
        };
        session_input_revision(revision)
    }

    /// Prepares either an immediate receive or one exact empty-queue wait.
    ///
    /// The proposed wait and readiness cursors are not consumed here. This
    /// function validates the complete binding before inspecting private route
    /// state.
    pub(crate) fn prepare_receive(
        &self,
        binding: session_input_binding,
    ) -> Result<session_input_receive_preparation, session_input_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        match self.phase {
            session_input_phase::closed_idle | session_input_phase::closed_terminal => {
                return Err(session_input_error::session_closed);
            }
            session_input_phase::open_blocked | session_input_phase::open_input_ready => {
                return Err(session_input_error::wait_already_present);
            }
            session_input_phase::open_idle => {}
        }
        if self.length != 0 {
            return Ok(session_input_receive_preparation::ready);
        }
        let Some(wait) = self.waits.prepare() else {
            return Err(session_input_error::wait_sequence_exhausted);
        };
        let Some(readiness) = self.readiness.prepare() else {
            return Err(session_input_error::readiness_generation_exhausted);
        };
        Ok(session_input_receive_preparation::wait(
            session_input_prepared_wait {
                binding: self.binding,
                wait,
                readiness,
                revision: self.revision,
            },
        ))
    }

    /// Commits an exact empty-queue wait without a lost-wake window.
    ///
    /// Proposed wait/readiness cursor equality is checked before the nonempty
    /// fast path. Therefore an invalidated preparation cannot become valid just
    /// because input appears later. Actual ready input may dominate only a
    /// revision change after those cursor checks pass.
    pub(crate) fn commit_wait(
        &mut self,
        prepared: session_input_prepared_wait,
    ) -> Result<session_input_wait_commit, session_input_error> {
        self.validate_binding(prepared.binding)?;
        self.require_invariants()?;
        if self.phase != session_input_phase::open_idle || self.wait.is_some() {
            return match self.phase {
                session_input_phase::closed_idle | session_input_phase::closed_terminal => {
                    Err(session_input_error::session_closed)
                }
                _ => Err(session_input_error::wait_already_present),
            };
        }
        if self.waits.prepare() != Some(prepared.wait) {
            return Err(session_input_error::stale_wait);
        }
        if self.readiness.prepare() != Some(prepared.readiness) {
            return Err(session_input_error::stale_readiness);
        }
        if self.length != 0 {
            return Ok(session_input_wait_commit::ready);
        }
        if self.revision != prepared.revision {
            return Err(session_input_error::stale_preparation);
        }
        let next_revision = self.next_ordinary_revision()?;
        let Some(wait_sequence) = NonZeroU64::new(prepared.wait.value) else {
            return Err(session_input_error::invariant_violation);
        };
        let Some(readiness_generation) = NonZeroU64::new(prepared.readiness.value) else {
            return Err(session_input_error::invariant_violation);
        };
        let identity = session_input_wait_identity {
            binding: self.binding,
            sequence: wait_sequence,
        };
        let readiness = session_input_readiness_identity {
            binding: self.binding,
            generation: readiness_generation,
        };

        self.waits.commit(prepared.wait);
        self.readiness.commit(prepared.readiness);
        self.wait = Some(session_input_wait_record::blocked(identity, readiness));
        self.phase = session_input_phase::open_blocked;
        self.revision = next_revision;
        debug_assert!(self.invariants_hold());
        Ok(session_input_wait_commit::blocked(
            session_input_blocked_wait {
                identity,
                readiness,
            },
        ))
    }

    /// Trusted-router preparation that stamps private attribution.
    ///
    /// Origin is attribution data, not authentication. Only a future trusted
    /// route host may call this after authenticating and authorizing its source.
    /// Rejection returns the move-only private action unchanged.
    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_publication(
        &self,
        binding: session_input_binding,
        origin: session_input_origin,
        action: session_input_action,
    ) -> Result<session_input_prepared_publication, session_input_publication_rejection> {
        if let Err(error) = self.validate_binding(binding) {
            return Err(session_input_publication_rejection { error, action });
        }
        if let Err(error) = self.require_invariants() {
            return Err(session_input_publication_rejection { error, action });
        }
        if !action.invariants_hold() {
            return Err(session_input_publication_rejection {
                error: session_input_error::invariant_violation,
                action,
            });
        }
        if origin == session_input_origin::paste
            && action.expose_kind() != session_input_action_kind::insert_text
        {
            return Err(session_input_publication_rejection {
                error: session_input_error::paste_restricted,
                action,
            });
        }
        if matches!(
            self.phase,
            session_input_phase::closed_idle | session_input_phase::closed_terminal
        ) {
            return Err(session_input_publication_rejection {
                error: session_input_error::session_closed,
                action,
            });
        }
        if self.length == session_input_queue_capacity {
            return Err(session_input_publication_rejection {
                error: session_input_error::queue_full,
                action,
            });
        }
        if !self.queue[self.tail].storage_is_clear() {
            return Err(session_input_publication_rejection {
                error: session_input_error::invariant_violation,
                action,
            });
        }
        let Some(delivery) = self.deliveries.prepare() else {
            return Err(session_input_publication_rejection {
                error: session_input_error::delivery_sequence_exhausted,
                action,
            });
        };
        let next_revision = match self.next_publication_revision() {
            Ok(revision) => revision,
            Err(error) => return Err(session_input_publication_rejection { error, action }),
        };

        Ok(session_input_prepared_publication {
            binding: self.binding,
            origin,
            action,
            delivery,
            revision: self.revision,
            next_revision,
            phase: self.phase,
            wait: self.wait,
            tail: self.tail,
        })
    }

    /// Commits a previously prepared publication.
    ///
    /// On success the action is installed before an exact prepared wake is
    /// returned. The serialized integration must preflight scheduler placement
    /// before this call and consume that move-only wake in its infallible commit
    /// before releasing exclusion. Rejection returns private ownership.
    #[allow(clippy::result_large_err)]
    pub(crate) fn commit_publication(
        &mut self,
        mut prepared: session_input_prepared_publication,
    ) -> Result<session_input_publication_receipt, session_input_publication_rejection> {
        let error = if let Err(error) = self.validate_binding(prepared.binding) {
            Some(error)
        } else if self.require_invariants().is_err() {
            Some(session_input_error::invariant_violation)
        } else if self.revision != prepared.revision
            || self.phase != prepared.phase
            || self.wait != prepared.wait
            || self.tail != prepared.tail
        {
            Some(session_input_error::stale_preparation)
        } else if matches!(
            self.phase,
            session_input_phase::closed_idle | session_input_phase::closed_terminal
        ) {
            Some(session_input_error::session_closed)
        } else if self.length == session_input_queue_capacity {
            Some(session_input_error::queue_full)
        } else if !self.queue[self.tail].storage_is_clear() || !prepared.action.invariants_hold() {
            Some(session_input_error::invariant_violation)
        } else if self.deliveries.prepare() != Some(prepared.delivery) {
            Some(session_input_error::delivery_sequence_exhausted)
        } else if self.next_publication_revision() != Ok(prepared.next_revision) {
            Some(session_input_error::revision_exhausted)
        } else {
            None
        };
        if let Some(error) = error {
            return Err(session_input_publication_rejection {
                error,
                action: prepared.action,
            });
        }

        let sequence = prepared.delivery.value;
        self.queue[self.tail].install(sequence, prepared.origin, &mut prepared.action);
        self.tail = (self.tail + 1) % session_input_queue_capacity;
        self.length += 1;
        self.deliveries.commit(prepared.delivery);
        self.revision = prepared.next_revision;

        let wake = if self.phase == session_input_phase::open_blocked {
            let Some(wait) = self.wait else {
                panic!("validated blocked session input wait disappeared");
            };
            self.phase = session_input_phase::open_input_ready;
            Some(session_input_prepared_wake {
                wait: wait.identity,
                readiness: wait.readiness,
                outcome: session_input_wake_outcome::input_ready,
            })
        } else {
            None
        };
        debug_assert!(self.invariants_hold());

        let Some(sequence) = NonZeroU64::new(sequence) else {
            panic!("validated delivery sequence became zero");
        };
        Ok(session_input_publication_receipt {
            sequence: session_input_delivery_sequence(sequence),
            wake,
        })
    }

    /// Prepares an exact route close without mutating or clearing private data.
    pub(crate) fn prepare_close(
        &self,
        binding: session_input_binding,
        reason: session_input_close_reason,
    ) -> Result<session_input_close_preparation, session_input_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        if matches!(
            self.phase,
            session_input_phase::closed_idle | session_input_phase::closed_terminal
        ) {
            return Ok(session_input_close_preparation::already_closed);
        }
        let remaining = u64::from(self.wait.is_some());
        let next_revision = self.next_revision_with_reserve(remaining)?;
        Ok(session_input_close_preparation::prepared(
            session_input_prepared_close {
                binding: self.binding,
                reason,
                revision: self.revision,
                next_revision,
                phase: self.phase,
                wait: self.wait,
                discarded: self.length as u8,
            },
        ))
    }

    /// Commits one prepared close and clears all queued private typeahead.
    ///
    /// A blocked wait returns one terminal scheduler wake. An input-ready wait
    /// retains the same exact correlation but was already placed, so close does
    /// not enqueue it again.
    pub(crate) fn commit_close(
        &mut self,
        prepared: session_input_prepared_close,
    ) -> Result<session_input_close_receipt, session_input_error> {
        self.validate_binding(prepared.binding)?;
        self.require_invariants()?;
        if self.revision != prepared.revision
            || self.phase != prepared.phase
            || self.wait != prepared.wait
            || self.length as u8 != prepared.discarded
        {
            return Err(session_input_error::stale_preparation);
        }
        if matches!(
            self.phase,
            session_input_phase::closed_idle | session_input_phase::closed_terminal
        ) {
            return Err(session_input_error::session_closed);
        }
        let remaining = u64::from(self.wait.is_some());
        if self.next_revision_with_reserve(remaining)? != prepared.next_revision {
            return Err(session_input_error::stale_preparation);
        }

        let (next_phase, next_wait, wake) = match (prepared.phase, prepared.wait) {
            (session_input_phase::open_idle, None) => {
                (session_input_phase::closed_idle, None, None)
            }
            (session_input_phase::open_blocked, Some(wait)) => {
                let terminal = wait.terminal(prepared.reason);
                (
                    session_input_phase::closed_terminal,
                    Some(terminal),
                    Some(session_input_prepared_wake {
                        wait: terminal.identity,
                        readiness: terminal.readiness,
                        outcome: session_input_wake_outcome::terminal(prepared.reason),
                    }),
                )
            }
            (session_input_phase::open_input_ready, Some(wait)) => (
                session_input_phase::closed_terminal,
                Some(wait.terminal(prepared.reason)),
                None,
            ),
            _ => return Err(session_input_error::invariant_violation),
        };

        self.clear_queue();
        self.revision = prepared.next_revision;
        self.wait = next_wait;
        self.phase = next_phase;
        debug_assert!(self.invariants_hold());
        let terminal = next_wait.map(|wait| session_input_wake_correlation {
            wait: wait.identity,
            readiness: wait.readiness,
            outcome: session_input_wake_outcome::terminal(prepared.reason),
        });
        Ok(session_input_close_receipt {
            first_close: true,
            discarded: prepared.discarded,
            wake,
            terminal,
        })
    }

    /// Freeze one immediately available or exactly input-ready source head.
    ///
    /// `correlation` is required only for an input-ready wait. It includes the
    /// complete endpoint/session/consumer/route/task, wait, readiness, and
    /// outcome tuple retained by the pending Rite. No source byte or counter
    /// changes until [`Self::commit_prepared_delivery`].
    pub(crate) fn prepare_delivery(
        &self,
        binding: session_input_binding,
        correlation: Option<session_input_wake_correlation>,
    ) -> Result<session_input_prepared_delivery, session_input_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        match self.phase {
            session_input_phase::open_idle => {
                if correlation.is_some() {
                    return Err(session_input_error::stale_wait);
                }
            }
            session_input_phase::open_blocked => {
                return Err(session_input_error::wait_not_ready);
            }
            session_input_phase::open_input_ready => {
                let Some(correlation) = correlation else {
                    return Err(session_input_error::wake_acknowledgement_required);
                };
                let Some(wait) = self.wait else {
                    return Err(session_input_error::invariant_violation);
                };
                self.validate_wake_correlation(
                    correlation,
                    wait,
                    session_input_wake_outcome::input_ready,
                )?;
            }
            session_input_phase::closed_terminal => {
                return Err(session_input_error::terminal_acknowledgement_required);
            }
            session_input_phase::closed_idle => {
                return Err(session_input_error::session_closed);
            }
        }
        if self.length == 0 {
            return Err(session_input_error::queue_empty);
        }
        if !self.queue[self.head].occupied || !self.queue[self.head].invariants_hold() {
            return Err(session_input_error::invariant_violation);
        }
        let next_revision = self.next_delivery_revision(correlation.is_some())?;
        if NonZeroU64::new(self.queue[self.head].sequence).is_none() {
            return Err(session_input_error::invariant_violation);
        }
        Ok(session_input_prepared_delivery {
            binding: self.binding,
            correlation,
            revision: self.revision,
            next_revision,
            phase: self.phase,
            wait: self.wait,
            head: self.head,
            sequence: self.queue[self.head].sequence,
        })
    }

    /// Consume a fully copied source head after the exact consumer is current.
    ///
    /// Every fallible fact was checked by `prepare_delivery` and the prepared
    /// transport encoder. A changed endpoint or mismatched acknowledgement is
    /// therefore a kernel serialization defect and fails stop rather than
    /// returning after the scheduler has already selected the consumer. An
    /// input-ready acknowledgement plus consumption advances the revision by
    /// two in this one atomic commit, matching the two host-model transitions;
    /// an immediate poll delivery advances it by one.
    pub(crate) fn commit_prepared_delivery(
        &mut self,
        prepared: session_input_prepared_delivery,
        acknowledgement: Option<session_input_wait_acknowledgement>,
    ) -> session_input_delivery_receipt {
        self.validate_prepared_delivery(&prepared)
            .expect("prepared session input delivery changed after selection");
        match (prepared.correlation, acknowledgement.as_ref()) {
            (None, None) => {}
            (Some(correlation), Some(acknowledgement)) => {
                assert!(
                    acknowledgement.wait == correlation.wait
                        && acknowledgement.readiness == correlation.readiness,
                    "selected session input acknowledgement changed"
                );
            }
            _ => panic!("selected session input acknowledgement is missing or unexpected"),
        }

        let Some(sequence) = NonZeroU64::new(prepared.sequence) else {
            panic!("prepared session input sequence became zero");
        };
        self.queue[prepared.head].clear();
        self.head = (prepared.head + 1) % session_input_queue_capacity;
        self.length -= 1;
        let acknowledged_wait = prepared.correlation.is_some();
        if acknowledged_wait {
            self.wait = None;
            self.phase = session_input_phase::open_idle;
        }
        self.revision = prepared.next_revision;
        debug_assert!(self.invariants_hold());
        session_input_delivery_receipt {
            sequence: session_input_delivery_sequence(sequence),
            acknowledged_wait,
        }
    }

    /// Freeze the exact terminal tombstone before selecting its consumer.
    pub(crate) fn prepare_terminal(
        &self,
        binding: session_input_binding,
        correlation: session_input_wake_correlation,
    ) -> Result<session_input_prepared_terminal, session_input_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        if self.phase != session_input_phase::closed_terminal {
            return match self.phase {
                session_input_phase::closed_idle => Err(session_input_error::stale_wait),
                _ => Err(session_input_error::terminal_acknowledgement_required),
            };
        }
        let Some(wait) = self.wait else {
            return Err(session_input_error::invariant_violation);
        };
        let Some(reason) = wait.terminal_reason else {
            return Err(session_input_error::invariant_violation);
        };
        self.validate_wake_correlation(
            correlation,
            wait,
            session_input_wake_outcome::terminal(reason),
        )?;
        let next_revision = self.next_revision_with_reserve(0)?;
        Ok(session_input_prepared_terminal {
            binding: self.binding,
            correlation,
            revision: self.revision,
            next_revision,
            wait,
            reason,
        })
    }

    /// Complete a preflighted terminal tombstone after exact task selection.
    pub(crate) fn commit_prepared_terminal(
        &mut self,
        prepared: session_input_prepared_terminal,
        acknowledgement: session_input_wait_acknowledgement,
    ) -> session_input_terminal_acknowledgement {
        self.validate_prepared_terminal(&prepared)
            .expect("prepared session input terminal changed after selection");
        assert!(
            acknowledgement.wait == prepared.correlation.wait
                && acknowledgement.readiness == prepared.correlation.readiness,
            "selected terminal acknowledgement changed"
        );

        self.wait = None;
        self.phase = session_input_phase::closed_idle;
        self.revision = prepared.next_revision;
        debug_assert!(self.invariants_hold());
        session_input_terminal_acknowledgement {
            wait: acknowledgement.wait,
            readiness: acknowledgement.readiness,
            reason: prepared.reason,
        }
    }

    /// Acknowledges one input-ready wake, copies completely, then consumes.
    ///
    /// `preflight` may fail without mutation. Once it succeeds, `commit` must
    /// be infallible under the caller's serialized native exclusion. The source
    /// head, wait, readiness, revision, and cursor remain unchanged until that
    /// callback returns. Supplying `None` performs an immediate poll-mode
    /// receive and is rejected while any wait is blocked, input-ready, or
    /// terminal.
    pub(crate) fn receive_with<P, C>(
        &mut self,
        binding: session_input_binding,
        acknowledgement: Option<&session_input_wait_acknowledgement>,
        preflight: P,
        commit: C,
    ) -> Result<session_input_delivery_receipt, session_input_error>
    where
        P: FnOnce(&session_input_delivery<'_>) -> bool,
        C: FnOnce(&session_input_delivery<'_>),
    {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        let acknowledged_wait = match self.phase {
            session_input_phase::open_idle => {
                if acknowledgement.is_some() {
                    return Err(session_input_error::stale_wait);
                }
                false
            }
            session_input_phase::open_blocked => {
                return Err(session_input_error::wait_not_ready);
            }
            session_input_phase::open_input_ready => {
                let Some(acknowledgement) = acknowledgement else {
                    return Err(session_input_error::wake_acknowledgement_required);
                };
                let Some(wait) = self.wait else {
                    return Err(session_input_error::invariant_violation);
                };
                self.validate_acknowledgement(acknowledgement, wait)?;
                true
            }
            session_input_phase::closed_terminal => {
                return Err(session_input_error::terminal_acknowledgement_required);
            }
            session_input_phase::closed_idle => {
                return Err(session_input_error::session_closed);
            }
        };
        if self.length == 0 {
            return Err(session_input_error::queue_empty);
        }
        let head = self.head;
        if !self.queue[head].invariants_hold() || !self.queue[head].occupied {
            return Err(session_input_error::invariant_violation);
        }
        let next_revision = self.next_delivery_revision(acknowledged_wait)?;
        let Some(sequence) = NonZeroU64::new(self.queue[head].sequence) else {
            return Err(session_input_error::invariant_violation);
        };

        {
            let delivery = session_input_delivery {
                binding: self.binding,
                slot: &self.queue[head],
            };
            if !preflight(&delivery) {
                return Err(session_input_error::copy_failed);
            }
        }
        {
            let delivery = session_input_delivery {
                binding: self.binding,
                slot: &self.queue[head],
            };
            commit(&delivery);
        }

        self.queue[head].clear();
        self.head = (head + 1) % session_input_queue_capacity;
        self.length -= 1;
        if acknowledged_wait {
            self.wait = None;
            self.phase = session_input_phase::open_idle;
        }
        self.revision = next_revision;
        debug_assert!(self.invariants_hold());
        Ok(session_input_delivery_receipt {
            sequence: session_input_delivery_sequence(sequence),
            acknowledged_wait,
        })
    }

    /// Completes one exact closed-terminal tombstone.
    ///
    /// This operation never reopens the endpoint and therefore cannot enable a
    /// receive. It may consume the final revision value reserved by close.
    pub(crate) fn acknowledge_terminal(
        &mut self,
        binding: session_input_binding,
        acknowledgement: session_input_wait_acknowledgement,
    ) -> Result<session_input_terminal_acknowledgement, session_input_error> {
        self.validate_binding(binding)?;
        self.require_invariants()?;
        if self.phase != session_input_phase::closed_terminal {
            return match self.phase {
                session_input_phase::closed_idle => Err(session_input_error::stale_wait),
                _ => Err(session_input_error::terminal_acknowledgement_required),
            };
        }
        let Some(wait) = self.wait else {
            return Err(session_input_error::invariant_violation);
        };
        self.validate_acknowledgement(&acknowledgement, wait)?;
        let Some(reason) = wait.terminal_reason else {
            return Err(session_input_error::invariant_violation);
        };
        let next_revision = self.next_revision_with_reserve(0)?;

        self.wait = None;
        self.phase = session_input_phase::closed_idle;
        self.revision = next_revision;
        debug_assert!(self.invariants_hold());
        Ok(session_input_terminal_acknowledgement {
            wait: acknowledgement.wait,
            readiness: acknowledgement.readiness,
            reason,
        })
    }

    fn validate_binding(
        &self,
        candidate: session_input_binding,
    ) -> Result<(), session_input_error> {
        if candidate.endpoint != self.binding.endpoint {
            return Err(session_input_error::wrong_endpoint);
        }
        if candidate.session != self.binding.session {
            return Err(session_input_error::wrong_session);
        }
        if candidate.consumer != self.binding.consumer {
            return Err(session_input_error::wrong_consumer);
        }
        if candidate.route != self.binding.route {
            return Err(session_input_error::stale_route);
        }
        if candidate.task != self.binding.task {
            return Err(session_input_error::wrong_task);
        }
        Ok(())
    }

    fn validate_acknowledgement(
        &self,
        acknowledgement: &session_input_wait_acknowledgement,
        expected: session_input_wait_record,
    ) -> Result<(), session_input_error> {
        self.validate_binding(acknowledgement.wait.binding)?;
        if acknowledgement.wait.sequence != expected.identity.sequence {
            return Err(session_input_error::stale_wait);
        }
        self.validate_binding(acknowledgement.readiness.binding)?;
        if acknowledgement.readiness.generation != expected.readiness.generation {
            return Err(session_input_error::stale_readiness);
        }
        Ok(())
    }

    fn validate_wake_correlation(
        &self,
        correlation: session_input_wake_correlation,
        expected: session_input_wait_record,
        outcome: session_input_wake_outcome,
    ) -> Result<(), session_input_error> {
        self.validate_binding(correlation.wait.binding)?;
        if correlation.wait.sequence != expected.identity.sequence {
            return Err(session_input_error::stale_wait);
        }
        self.validate_binding(correlation.readiness.binding)?;
        if correlation.readiness.generation != expected.readiness.generation {
            return Err(session_input_error::stale_readiness);
        }
        if correlation.outcome != outcome {
            return Err(session_input_error::wrong_wake_outcome);
        }
        Ok(())
    }

    fn validate_prepared_delivery(
        &self,
        prepared: &session_input_prepared_delivery,
    ) -> Result<(), session_input_error> {
        self.validate_binding(prepared.binding)?;
        self.require_invariants()?;
        if self.revision != prepared.revision
            || self.phase != prepared.phase
            || self.wait != prepared.wait
            || self.head != prepared.head
            || self.length == 0
            || self.queue[prepared.head].sequence != prepared.sequence
            || !self.queue[prepared.head].occupied
            || !self.queue[prepared.head].invariants_hold()
            || self.next_delivery_revision(prepared.correlation.is_some())
                != Ok(prepared.next_revision)
        {
            return Err(session_input_error::stale_preparation);
        }
        match (prepared.phase, prepared.wait, prepared.correlation) {
            (session_input_phase::open_idle, None, None) => Ok(()),
            (session_input_phase::open_input_ready, Some(wait), Some(correlation)) => self
                .validate_wake_correlation(
                    correlation,
                    wait,
                    session_input_wake_outcome::input_ready,
                ),
            _ => Err(session_input_error::stale_preparation),
        }
    }

    fn validate_prepared_terminal(
        &self,
        prepared: &session_input_prepared_terminal,
    ) -> Result<(), session_input_error> {
        self.validate_binding(prepared.binding)?;
        self.require_invariants()?;
        if self.phase != session_input_phase::closed_terminal
            || self.revision != prepared.revision
            || self.wait != Some(prepared.wait)
            || prepared.wait.terminal_reason != Some(prepared.reason)
            || self.next_revision_with_reserve(0) != Ok(prepared.next_revision)
        {
            return Err(session_input_error::stale_preparation);
        }
        self.validate_wake_correlation(
            prepared.correlation,
            prepared.wait,
            session_input_wake_outcome::terminal(prepared.reason),
        )
    }

    fn next_ordinary_revision(&self) -> Result<u64, session_input_error> {
        self.next_revision_with_reserve(2)
    }

    fn next_publication_revision(&self) -> Result<u64, session_input_error> {
        let remaining = if self.wait.is_some() { 4 } else { 2 };
        self.next_revision_with_reserve(remaining)
    }

    fn next_delivery_revision(&self, acknowledges_wait: bool) -> Result<u64, session_input_error> {
        let advance = if acknowledges_wait { 2 } else { 1 };
        self.next_revision_after_with_reserve(advance, 2)
    }

    fn next_revision_with_reserve(&self, remaining: u64) -> Result<u64, session_input_error> {
        self.next_revision_after_with_reserve(1, remaining)
    }

    fn next_revision_after_with_reserve(
        &self,
        advance: u64,
        remaining: u64,
    ) -> Result<u64, session_input_error> {
        let Some(next) = self.revision.checked_add(advance) else {
            return Err(session_input_error::revision_exhausted);
        };
        if u64::MAX - next < remaining {
            return Err(session_input_error::revision_exhausted);
        }
        Ok(next)
    }

    fn clear_queue(&mut self) {
        for slot in &mut self.queue {
            slot.clear();
        }
        self.head = 0;
        self.tail = 0;
        self.length = 0;
    }

    fn clear_owned_private_storage(&mut self) {
        self.clear_queue();
        self.wait = None;
        self.phase = session_input_phase::closed_idle;
    }

    fn require_invariants(&self) -> Result<(), session_input_error> {
        if self.invariants_hold() {
            Ok(())
        } else {
            Err(session_input_error::invariant_violation)
        }
    }

    fn invariants_hold(&self) -> bool {
        if self.head >= session_input_queue_capacity
            || self.tail >= session_input_queue_capacity
            || self.length > session_input_queue_capacity
            || self.revision == 0
            || !self.deliveries.invariants_hold()
            || !self.waits.invariants_hold()
            || !self.readiness.invariants_hold()
        {
            return false;
        }
        match self.phase {
            session_input_phase::open_idle
            | session_input_phase::open_blocked
            | session_input_phase::open_input_ready => {
                if u64::MAX - self.revision < 2 {
                    return false;
                }
            }
            session_input_phase::closed_terminal => {
                if u64::MAX - self.revision < 1 {
                    return false;
                }
            }
            session_input_phase::closed_idle => {}
        }
        if self.queue.iter().filter(|slot| slot.occupied).count() != self.length {
            return false;
        }

        let mut previous_sequence = 0;
        for offset in 0..session_input_queue_capacity {
            let expected = offset < self.length;
            let index = (self.head + offset) % session_input_queue_capacity;
            let slot = &self.queue[index];
            if slot.occupied != expected {
                return false;
            }
            if expected {
                if !slot.invariants_hold()
                    || !self.deliveries.has_issued(slot.sequence)
                    || (previous_sequence != 0 && slot.sequence <= previous_sequence)
                {
                    return false;
                }
                previous_sequence = slot.sequence;
            } else if !slot.storage_is_clear() {
                return false;
            }
        }
        if self.tail != (self.head + self.length) % session_input_queue_capacity {
            return false;
        }

        let valid_wait = |wait: session_input_wait_record| {
            wait.identity.binding == self.binding
                && wait.readiness.binding == self.binding
                && self.waits.is_most_recent(wait.identity.sequence.get())
                && self
                    .readiness
                    .is_most_recent(wait.readiness.generation.get())
        };
        match (self.phase, self.wait) {
            (session_input_phase::open_idle, None) => true,
            (session_input_phase::open_blocked, Some(wait)) => {
                self.length == 0 && wait.terminal_reason.is_none() && valid_wait(wait)
            }
            (session_input_phase::open_input_ready, Some(wait)) => {
                self.length != 0 && wait.terminal_reason.is_none() && valid_wait(wait)
            }
            (session_input_phase::closed_idle, None) => self.length == 0,
            (session_input_phase::closed_terminal, Some(wait)) => {
                self.length == 0 && wait.terminal_reason.is_some() && valid_wait(wait)
            }
            _ => false,
        }
    }
}

/// Privacy fail-safe only; this is not a protocol close or scheduler wake.
impl Drop for session_input_endpoint {
    fn drop(&mut self) {
        self.clear_owned_private_storage();
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::identity::{circle_id, task_id};
    use anomalous_session as host;
    use std::string::{String, ToString};

    macro_rules! assert_not_impl {
        ($value:ty, $trait:path) => {
            const _: fn() = || {
                trait ambiguous_if_impl<marker> {
                    fn assertion() {}
                }

                impl<value: ?Sized> ambiguous_if_impl<()> for value {}

                #[allow(dead_code)]
                struct would_conflict;

                impl<value: ?Sized + $trait> ambiguous_if_impl<would_conflict> for value {}

                let _ = <$value as ambiguous_if_impl<_>>::assertion;
            };
        };
    }

    macro_rules! assert_private_owner_traits {
        ($value:ty) => {
            assert_not_impl!($value, core::clone::Clone);
            assert_not_impl!($value, core::marker::Copy);
            assert_not_impl!($value, core::fmt::Debug);
            assert_not_impl!($value, core::fmt::Display);
            assert_not_impl!($value, core::hash::Hash);
        };
    }

    assert_private_owner_traits!(session_input_action);
    assert_private_owner_traits!(session_input_queue_slot);
    assert_private_owner_traits!(session_input_prepared_wait);
    assert_private_owner_traits!(session_input_receive_preparation);
    assert_private_owner_traits!(session_input_blocked_wait);
    assert_private_owner_traits!(session_input_wait_commit);
    assert_private_owner_traits!(session_input_prepared_publication);
    assert_private_owner_traits!(session_input_publication_rejection);
    assert_private_owner_traits!(session_input_prepared_wake);
    assert_private_owner_traits!(session_input_publication_receipt);
    assert_private_owner_traits!(session_input_prepared_close);
    assert_private_owner_traits!(session_input_close_preparation);
    assert_private_owner_traits!(session_input_close_receipt);
    assert_private_owner_traits!(session_input_wait_acknowledgement);
    assert_private_owner_traits!(session_input_delivery<'static>);
    assert_private_owner_traits!(session_input_prepared_delivery);
    assert_private_owner_traits!(session_input_prepared_terminal);
    assert_private_owner_traits!(session_input_delivery_receipt);
    assert_private_owner_traits!(session_input_terminal_acknowledgement);
    assert_private_owner_traits!(session_input_endpoint);

    fn task(
        task_slot: usize,
        task_generation: u64,
        circle_slot: usize,
        circle_generation: u64,
    ) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(task_slot, task_generation).expect("task identity"),
            circle_id::try_from_parts(circle_slot, circle_generation).expect("circle identity"),
        )
    }

    fn binding() -> session_input_binding {
        session_input_binding::new(
            session_input_endpoint_identity::try_new(3, 1).expect("endpoint identity"),
            session_input_session_identity::try_new(5, 1).expect("session identity"),
            session_input_consumer_identity::try_new(7, 1).expect("consumer identity"),
            session_input_route_epoch::try_new(11).expect("route epoch"),
            task(13, 1, 17, 1),
        )
    }

    fn endpoint() -> session_input_endpoint {
        session_input_endpoint::new(binding())
    }

    fn insertion(value: &str) -> session_input_action {
        session_input_action::try_insert_text(value.as_bytes()).expect("valid insertion")
    }

    fn prepare_publication(
        endpoint: &session_input_endpoint,
        origin: session_input_origin,
        action: session_input_action,
    ) -> session_input_prepared_publication {
        match endpoint.prepare_publication(endpoint.binding(), origin, action) {
            Ok(prepared) => prepared,
            Err(rejection) => panic!("prepare publication: {}", rejection.error().code()),
        }
    }

    fn publish(
        endpoint: &mut session_input_endpoint,
        origin: session_input_origin,
        action: session_input_action,
    ) -> session_input_publication_receipt {
        let prepared = prepare_publication(endpoint, origin, action);
        match endpoint.commit_publication(prepared) {
            Ok(receipt) => receipt,
            Err(rejection) => panic!("commit publication: {}", rejection.error().code()),
        }
    }

    fn prepare_wait(endpoint: &session_input_endpoint) -> session_input_prepared_wait {
        match endpoint
            .prepare_receive(endpoint.binding())
            .unwrap_or_else(|error| panic!("prepare receive: {}", error.code()))
        {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("unexpected ready queue"),
        }
    }

    fn block(
        endpoint: &mut session_input_endpoint,
    ) -> (
        session_input_wait_identity,
        session_input_readiness_identity,
    ) {
        let prepared = prepare_wait(endpoint);
        assert!(prepared.binding() == endpoint.binding());
        let proposed_wait = prepared.proposed_wait();
        let proposed_readiness = prepared.proposed_readiness();
        match endpoint
            .commit_wait(prepared)
            .unwrap_or_else(|error| panic!("commit wait: {}", error.code()))
        {
            session_input_wait_commit::ready => panic!("unexpected ready queue"),
            session_input_wait_commit::blocked(blocked) => {
                assert!(blocked.binding() == endpoint.binding());
                assert!(blocked.identity() == proposed_wait);
                assert!(blocked.readiness() == proposed_readiness);
                (blocked.identity(), blocked.readiness())
            }
        }
    }

    fn acknowledgement(
        wait: session_input_wait_identity,
        readiness: session_input_readiness_identity,
    ) -> session_input_wait_acknowledgement {
        session_input_wait_acknowledgement::new(wait, readiness)
    }

    struct observed_delivery {
        sequence: u64,
        origin: session_input_origin,
        kind: session_input_action_kind,
        byte_len: u16,
        erase_count: Option<u16>,
        text: Option<String>,
        fixed_tail_clear: bool,
    }

    fn receive(
        endpoint: &mut session_input_endpoint,
        acknowledgement: Option<&session_input_wait_acknowledgement>,
    ) -> (session_input_delivery_receipt, observed_delivery) {
        let mut observed = None;
        let receipt = endpoint
            .receive_with(
                endpoint.binding(),
                acknowledgement,
                |delivery| {
                    delivery.binding() == binding()
                        && delivery.expose_sequence().get() != 0
                        && delivery.expose_fixed_text()[usize::from(delivery.expose_byte_len())..]
                            .iter()
                            .all(|byte| *byte == 0)
                },
                |delivery| {
                    observed = Some(observed_delivery {
                        sequence: delivery.expose_sequence().get(),
                        origin: delivery.expose_origin(),
                        kind: delivery.expose_action_kind(),
                        byte_len: delivery.expose_byte_len(),
                        erase_count: delivery.expose_erase_count(),
                        text: delivery.expose_text().map(ToString::to_string),
                        fixed_tail_clear: delivery.expose_fixed_text()
                            [usize::from(delivery.expose_byte_len())..]
                            .iter()
                            .all(|byte| *byte == 0),
                    });
                },
            )
            .unwrap_or_else(|error| panic!("receive: {}", error.code()));
        (receipt, observed.expect("copy commit observed delivery"))
    }

    fn prepared_close(
        endpoint: &session_input_endpoint,
        reason: session_input_close_reason,
    ) -> session_input_prepared_close {
        match endpoint
            .prepare_close(endpoint.binding(), reason)
            .unwrap_or_else(|error| panic!("prepare close: {}", error.code()))
        {
            session_input_close_preparation::already_closed => panic!("already closed"),
            session_input_close_preparation::prepared(prepared) => prepared,
        }
    }

    fn close(
        endpoint: &mut session_input_endpoint,
        reason: session_input_close_reason,
    ) -> session_input_close_receipt {
        let prepared = prepared_close(endpoint, reason);
        endpoint
            .commit_close(prepared)
            .unwrap_or_else(|error| panic!("commit close: {}", error.code()))
    }

    fn new_host_session() -> host::shell_session {
        host::shell_session::new(
            host::session_identity::try_new(5, 1).expect("host session"),
            host::consumer_identity::try_new(7, 1).expect("host consumer"),
            host::route_epoch::try_new(11).expect("host route"),
            host::language_version::new(0, 1),
        )
    }

    fn host_insertion(value: &str) -> host::semantic_action {
        host::semantic_action::insert_text(
            host::private_text::try_from_str(value).expect("host private text"),
        )
        .expect("host insertion")
    }

    fn host_publish(
        session: &mut host::shell_session,
        origin: host::submission_origin,
        action: host::semantic_action,
    ) -> host::publish_receipt {
        let prepared = session
            .prepare_submission(origin, action)
            .unwrap_or_else(|_| panic!("host prepare publication"));
        session
            .publish(prepared)
            .unwrap_or_else(|_| panic!("host commit publication"))
    }

    fn host_prepare_wait(session: &host::shell_session) -> host::prepared_wait {
        match session
            .prepare_receive(session.consumer())
            .unwrap_or_else(|error| panic!("host prepare wait: {}", error.code()))
        {
            host::receive_preparation::ready => panic!("host queue unexpectedly ready"),
            host::receive_preparation::wait(prepared) => prepared,
        }
    }

    fn host_block(session: &mut host::shell_session) -> host::wait_identity {
        let prepared = host_prepare_wait(session);
        match session
            .commit_wait(prepared)
            .unwrap_or_else(|error| panic!("host commit wait: {}", error.code()))
        {
            host::wait_commit::ready => panic!("host queue unexpectedly ready"),
            host::wait_commit::blocked(wait) => wait,
        }
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum conformance_result_class {
        published,
        published_wake,
        prepared,
        ready,
        blocked,
        delivered,
        terminal,
        closed_idle,
        closed_terminal,
        already_closed,
        queue_full,
        queue_empty,
        wait_present,
        waiting,
        stale,
        exhausted,
        no_preparation,
        host_unexpected,
        native_unexpected,
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    struct conformance_result {
        class: conformance_result_class,
        detail: u8,
    }

    impl conformance_result {
        const fn new(class: conformance_result_class) -> Self {
            Self { class, detail: 0 }
        }

        const fn close(class: conformance_result_class, discarded: u8) -> Self {
            Self {
                class,
                detail: discarded,
            }
        }
    }

    #[derive(Clone, Copy)]
    enum conformance_step {
        publish,
        prepare,
        commit,
        complete,
        close,
    }

    struct host_conformance_state {
        session: host::shell_session,
        prepared: Option<host::prepared_wait>,
        wait: Option<host::wait_identity>,
    }

    impl host_conformance_state {
        fn new() -> Self {
            Self {
                session: new_host_session(),
                prepared: None,
                wait: None,
            }
        }

        fn apply(&mut self, step: conformance_step) -> conformance_result {
            match step {
                conformance_step::publish => self.publish(),
                conformance_step::prepare => self.prepare(),
                conformance_step::commit => self.commit(),
                conformance_step::complete => self.complete(),
                conformance_step::close => self.close(),
            }
        }

        fn publish(&mut self) -> conformance_result {
            let prepared = match self
                .session
                .prepare_submission(host::submission_origin::human_text, host_insertion("x"))
            {
                Ok(prepared) => prepared,
                Err(rejection) => {
                    return match rejection.error() {
                        host::session_error::session_closed => {
                            conformance_result::new(conformance_result_class::already_closed)
                        }
                        _ => conformance_result::new(conformance_result_class::host_unexpected),
                    };
                }
            };
            match self.session.publish(prepared) {
                Ok(receipt) => {
                    if let Some(wait) = receipt.wake() {
                        self.wait = Some(wait);
                        conformance_result::new(conformance_result_class::published_wake)
                    } else {
                        conformance_result::new(conformance_result_class::published)
                    }
                }
                Err(rejection) => match rejection.error() {
                    host::session_error::queue_full => {
                        conformance_result::new(conformance_result_class::queue_full)
                    }
                    host::session_error::revision_exhausted
                    | host::session_error::submission_sequence_exhausted => {
                        conformance_result::new(conformance_result_class::exhausted)
                    }
                    _ => conformance_result::new(conformance_result_class::host_unexpected),
                },
            }
        }

        fn prepare(&mut self) -> conformance_result {
            match self.session.prepare_receive(self.session.consumer()) {
                Ok(host::receive_preparation::ready) => {
                    conformance_result::new(conformance_result_class::ready)
                }
                Ok(host::receive_preparation::wait(prepared)) => {
                    self.prepared = Some(prepared);
                    conformance_result::new(conformance_result_class::prepared)
                }
                Err(host::session_error::session_closed) => {
                    conformance_result::new(conformance_result_class::already_closed)
                }
                Err(host::session_error::wait_already_present) => {
                    conformance_result::new(conformance_result_class::wait_present)
                }
                Err(host::session_error::wait_sequence_exhausted) => {
                    conformance_result::new(conformance_result_class::exhausted)
                }
                Err(_) => conformance_result::new(conformance_result_class::host_unexpected),
            }
        }

        fn commit(&mut self) -> conformance_result {
            let Some(prepared) = self.prepared.take() else {
                return conformance_result::new(conformance_result_class::no_preparation);
            };
            match self.session.commit_wait(prepared) {
                Ok(host::wait_commit::ready) => {
                    conformance_result::new(conformance_result_class::ready)
                }
                Ok(host::wait_commit::blocked(wait)) => {
                    self.wait = Some(wait);
                    conformance_result::new(conformance_result_class::blocked)
                }
                Err(host::session_error::session_closed) => {
                    conformance_result::new(conformance_result_class::already_closed)
                }
                Err(host::session_error::wait_already_present) => {
                    conformance_result::new(conformance_result_class::wait_present)
                }
                Err(host::session_error::stale_revision | host::session_error::stale_wait) => {
                    conformance_result::new(conformance_result_class::stale)
                }
                Err(
                    host::session_error::revision_exhausted
                    | host::session_error::wait_sequence_exhausted,
                ) => conformance_result::new(conformance_result_class::exhausted),
                Err(_) => conformance_result::new(conformance_result_class::host_unexpected),
            }
        }

        fn complete(&mut self) -> conformance_result {
            let Some(wait) = self.wait else {
                return match self.session.receive(self.session.consumer()) {
                    Ok(delivery) if delivery.action().expose_text() == Some("x") => {
                        conformance_result::new(conformance_result_class::delivered)
                    }
                    Err(host::session_error::queue_empty) => {
                        conformance_result::new(conformance_result_class::queue_empty)
                    }
                    Err(host::session_error::session_closed) => {
                        conformance_result::new(conformance_result_class::already_closed)
                    }
                    _ => conformance_result::new(conformance_result_class::host_unexpected),
                };
            };
            match self.session.acknowledge_wake(self.session.consumer(), wait) {
                Ok(acknowledgement) => match acknowledgement.outcome() {
                    host::wake_outcome::input_ready => {
                        let delivered = self.session.receive(self.session.consumer());
                        self.wait = None;
                        match delivered {
                            Ok(delivery) if delivery.action().expose_text() == Some("x") => {
                                conformance_result::new(conformance_result_class::delivered)
                            }
                            _ => conformance_result::new(conformance_result_class::host_unexpected),
                        }
                    }
                    host::wake_outcome::terminal(_) => {
                        self.wait = None;
                        conformance_result::new(conformance_result_class::terminal)
                    }
                },
                Err(host::session_error::wake_not_pending) => {
                    conformance_result::new(conformance_result_class::waiting)
                }
                Err(host::session_error::stale_wait) => {
                    conformance_result::new(conformance_result_class::stale)
                }
                Err(host::session_error::revision_exhausted) => {
                    conformance_result::new(conformance_result_class::exhausted)
                }
                Err(_) => conformance_result::new(conformance_result_class::host_unexpected),
            }
        }

        fn close(&mut self) -> conformance_result {
            match self.session.close(host::close_reason::route_replaced) {
                Ok(receipt) if !receipt.was_first_close() => {
                    conformance_result::new(conformance_result_class::already_closed)
                }
                Ok(receipt) => {
                    if let Some(wait) = receipt.terminal_wake() {
                        self.wait = Some(wait);
                        conformance_result::close(
                            conformance_result_class::closed_terminal,
                            receipt.discarded_count(),
                        )
                    } else {
                        conformance_result::close(
                            conformance_result_class::closed_idle,
                            receipt.discarded_count(),
                        )
                    }
                }
                Err(host::session_error::revision_exhausted) => {
                    conformance_result::new(conformance_result_class::exhausted)
                }
                Err(_) => conformance_result::new(conformance_result_class::host_unexpected),
            }
        }
    }

    struct native_conformance_state {
        endpoint: session_input_endpoint,
        prepared: Option<session_input_prepared_wait>,
        wait: Option<(
            session_input_wait_identity,
            session_input_readiness_identity,
        )>,
        correlation: Option<session_input_wake_correlation>,
    }

    impl native_conformance_state {
        fn new() -> Self {
            Self {
                endpoint: endpoint(),
                prepared: None,
                wait: None,
                correlation: None,
            }
        }

        fn apply(&mut self, step: conformance_step) -> conformance_result {
            match step {
                conformance_step::publish => self.publish(),
                conformance_step::prepare => self.prepare(),
                conformance_step::commit => self.commit(),
                conformance_step::complete => self.complete(),
                conformance_step::close => self.close(),
            }
        }

        fn publish(&mut self) -> conformance_result {
            let prepared = match self.endpoint.prepare_publication(
                self.endpoint.binding(),
                session_input_origin::human_text,
                insertion("x"),
            ) {
                Ok(prepared) => prepared,
                Err(rejection) => {
                    return match rejection.error() {
                        session_input_error::session_closed => {
                            conformance_result::new(conformance_result_class::already_closed)
                        }
                        session_input_error::queue_full => {
                            conformance_result::new(conformance_result_class::queue_full)
                        }
                        session_input_error::delivery_sequence_exhausted
                        | session_input_error::revision_exhausted => {
                            conformance_result::new(conformance_result_class::exhausted)
                        }
                        _ => conformance_result::new(conformance_result_class::native_unexpected),
                    };
                }
            };
            match self.endpoint.commit_publication(prepared) {
                Ok(receipt) => {
                    if let Some(wake) = receipt.wake() {
                        let correlation = wake.correlation();
                        self.wait = Some((correlation.wait(), correlation.readiness()));
                        self.correlation = Some(correlation);
                        drop(receipt.into_wake());
                        conformance_result::new(conformance_result_class::published_wake)
                    } else {
                        conformance_result::new(conformance_result_class::published)
                    }
                }
                Err(rejection) => match rejection.error() {
                    session_input_error::queue_full => {
                        conformance_result::new(conformance_result_class::queue_full)
                    }
                    session_input_error::delivery_sequence_exhausted
                    | session_input_error::revision_exhausted => {
                        conformance_result::new(conformance_result_class::exhausted)
                    }
                    _ => conformance_result::new(conformance_result_class::native_unexpected),
                },
            }
        }

        fn prepare(&mut self) -> conformance_result {
            match self.endpoint.prepare_receive(self.endpoint.binding()) {
                Ok(session_input_receive_preparation::ready) => {
                    conformance_result::new(conformance_result_class::ready)
                }
                Ok(session_input_receive_preparation::wait(prepared)) => {
                    self.prepared = Some(prepared);
                    conformance_result::new(conformance_result_class::prepared)
                }
                Err(session_input_error::session_closed) => {
                    conformance_result::new(conformance_result_class::already_closed)
                }
                Err(session_input_error::wait_already_present) => {
                    conformance_result::new(conformance_result_class::wait_present)
                }
                Err(
                    session_input_error::wait_sequence_exhausted
                    | session_input_error::readiness_generation_exhausted,
                ) => conformance_result::new(conformance_result_class::exhausted),
                Err(_) => conformance_result::new(conformance_result_class::native_unexpected),
            }
        }

        fn commit(&mut self) -> conformance_result {
            let Some(prepared) = self.prepared.take() else {
                return conformance_result::new(conformance_result_class::no_preparation);
            };
            match self.endpoint.commit_wait(prepared) {
                Ok(session_input_wait_commit::ready) => {
                    conformance_result::new(conformance_result_class::ready)
                }
                Ok(session_input_wait_commit::blocked(blocked)) => {
                    self.wait = Some((blocked.identity(), blocked.readiness()));
                    conformance_result::new(conformance_result_class::blocked)
                }
                Err(session_input_error::session_closed) => {
                    conformance_result::new(conformance_result_class::already_closed)
                }
                Err(session_input_error::wait_already_present) => {
                    conformance_result::new(conformance_result_class::wait_present)
                }
                Err(
                    session_input_error::stale_preparation
                    | session_input_error::stale_wait
                    | session_input_error::stale_readiness,
                ) => conformance_result::new(conformance_result_class::stale),
                Err(
                    session_input_error::revision_exhausted
                    | session_input_error::wait_sequence_exhausted
                    | session_input_error::readiness_generation_exhausted,
                ) => conformance_result::new(conformance_result_class::exhausted),
                Err(_) => conformance_result::new(conformance_result_class::native_unexpected),
            }
        }

        fn complete(&mut self) -> conformance_result {
            if let Some(correlation) = self.correlation {
                return match correlation.outcome() {
                    session_input_wake_outcome::input_ready => {
                        let prepared = match self
                            .endpoint
                            .prepare_delivery(self.endpoint.binding(), Some(correlation))
                        {
                            Ok(prepared) => prepared,
                            Err(session_input_error::revision_exhausted) => {
                                return conformance_result::new(
                                    conformance_result_class::exhausted,
                                );
                            }
                            Err(_) => {
                                return conformance_result::new(
                                    conformance_result_class::native_unexpected,
                                );
                            }
                        };
                        if !prepared
                            .delivery(&self.endpoint)
                            .is_ok_and(|delivery| delivery.expose_text() == Some("x"))
                        {
                            return conformance_result::new(
                                conformance_result_class::native_unexpected,
                            );
                        }
                        let Some((wait, readiness)) = self.wait.take() else {
                            return conformance_result::new(
                                conformance_result_class::native_unexpected,
                            );
                        };
                        drop(self.endpoint.commit_prepared_delivery(
                            prepared,
                            Some(acknowledgement(wait, readiness)),
                        ));
                        self.correlation = None;
                        conformance_result::new(conformance_result_class::delivered)
                    }
                    session_input_wake_outcome::terminal(_) => {
                        let prepared = match self
                            .endpoint
                            .prepare_terminal(self.endpoint.binding(), correlation)
                        {
                            Ok(prepared) => prepared,
                            Err(_) => {
                                return conformance_result::new(
                                    conformance_result_class::native_unexpected,
                                );
                            }
                        };
                        let Some((wait, readiness)) = self.wait.take() else {
                            return conformance_result::new(
                                conformance_result_class::native_unexpected,
                            );
                        };
                        drop(
                            self.endpoint.commit_prepared_terminal(
                                prepared,
                                acknowledgement(wait, readiness),
                            ),
                        );
                        self.correlation = None;
                        conformance_result::new(conformance_result_class::terminal)
                    }
                };
            }
            if self.wait.is_some() {
                return match self
                    .endpoint
                    .prepare_delivery(self.endpoint.binding(), None)
                {
                    Err(session_input_error::wait_not_ready) => {
                        conformance_result::new(conformance_result_class::waiting)
                    }
                    _ => conformance_result::new(conformance_result_class::native_unexpected),
                };
            }
            let prepared = match self
                .endpoint
                .prepare_delivery(self.endpoint.binding(), None)
            {
                Ok(prepared) => prepared,
                Err(session_input_error::queue_empty) => {
                    return conformance_result::new(conformance_result_class::queue_empty);
                }
                Err(session_input_error::session_closed) => {
                    return conformance_result::new(conformance_result_class::already_closed);
                }
                Err(_) => {
                    return conformance_result::new(conformance_result_class::native_unexpected);
                }
            };
            if !prepared
                .delivery(&self.endpoint)
                .is_ok_and(|delivery| delivery.expose_text() == Some("x"))
            {
                return conformance_result::new(conformance_result_class::native_unexpected);
            }
            drop(self.endpoint.commit_prepared_delivery(prepared, None));
            conformance_result::new(conformance_result_class::delivered)
        }

        fn close(&mut self) -> conformance_result {
            let prepared = match self.endpoint.prepare_close(
                self.endpoint.binding(),
                session_input_close_reason::route_replaced,
            ) {
                Ok(session_input_close_preparation::already_closed) => {
                    return conformance_result::new(conformance_result_class::already_closed);
                }
                Ok(session_input_close_preparation::prepared(prepared)) => prepared,
                Err(session_input_error::revision_exhausted) => {
                    return conformance_result::new(conformance_result_class::exhausted);
                }
                Err(_) => {
                    return conformance_result::new(conformance_result_class::native_unexpected);
                }
            };
            let receipt = match self.endpoint.commit_close(prepared) {
                Ok(receipt) => receipt,
                Err(_) => {
                    return conformance_result::new(conformance_result_class::native_unexpected);
                }
            };
            self.correlation = receipt.terminal_correlation();
            let result = if self.correlation.is_some() {
                conformance_result::close(
                    conformance_result_class::closed_terminal,
                    receipt.discarded_count(),
                )
            } else {
                conformance_result::close(
                    conformance_result_class::closed_idle,
                    receipt.discarded_count(),
                )
            };
            drop(receipt.into_wake());
            result
        }

        fn is_open(&self) -> bool {
            matches!(
                self.endpoint.phase,
                session_input_phase::open_idle
                    | session_input_phase::open_blocked
                    | session_input_phase::open_input_ready
            )
        }
    }

    fn frozen_reference(value: u32) -> bool {
        matches!(
            value,
            0x0000..=0x0008
                | 0x000b..=0x001f
                | 0x007f..=0x009f
                | 0x00ad
                | 0x034f
                | 0x061c
                | 0x180e
                | 0xfeff
                | 0x200b..=0x200f
                | 0x202a..=0x202e
                | 0x2060..=0x206f
        )
    }

    #[test]
    fn identity_parts_are_nonzero_and_exact() {
        assert!(matches!(
            session_input_endpoint_identity::try_new(0, 0),
            Err(session_input_error::zero_endpoint_generation)
        ));
        assert!(matches!(
            session_input_session_identity::try_new(0, 1),
            Err(session_input_error::zero_session_slot)
        ));
        assert!(matches!(
            session_input_session_identity::try_new(1, 0),
            Err(session_input_error::zero_session_generation)
        ));
        assert!(matches!(
            session_input_consumer_identity::try_new(0, 1),
            Err(session_input_error::zero_consumer)
        ));
        assert!(matches!(
            session_input_consumer_identity::try_new(1, 0),
            Err(session_input_error::zero_consumer_generation)
        ));
        assert!(matches!(
            session_input_route_epoch::try_new(0),
            Err(session_input_error::zero_route_epoch)
        ));

        let binding = binding();
        assert!(binding.endpoint().slot() == 3);
        assert!(binding.endpoint().generation() == 1);
        assert!(binding.endpoint().kernel_identity().slot() == 3);
        assert!(binding.session().slot() == 5);
        assert!(binding.session().generation() == 1);
        assert!(binding.consumer().value() == 7);
        assert!(binding.consumer().generation() == 1);
        assert!(binding.route().get() == 11);
        assert!(binding.task() == task(13, 1, 17, 1));
    }

    #[test]
    fn frozen_private_text_policy_matches_every_scalar_and_public_constructor() {
        let mut rejected = 0;
        for value in 0..=0x10ffff {
            let Some(scalar) = char::from_u32(value) else {
                continue;
            };
            assert!(forbidden_scalar(scalar) == frozen_reference(value));
            if !frozen_reference(value) {
                continue;
            }
            let mut encoded = [0; 4];
            assert!(matches!(
                session_input_action::try_insert_text(scalar.encode_utf8(&mut encoded).as_bytes()),
                Err(session_input_error::forbidden_scalar)
            ));
            rejected += 1;
        }
        assert!(rejected == 94);
        assert!(matches!(
            session_input_action::try_insert_text("allowed λ \u{2066} denied".as_bytes()),
            Err(session_input_error::forbidden_scalar)
        ));
    }

    #[test]
    fn private_action_shapes_are_exact_and_bounded() {
        let text = session_input_action::try_insert_text("human λ\tmachine\n".as_bytes())
            .expect("valid private text");
        assert!(text.expose_kind() == session_input_action_kind::insert_text);
        assert!(text.expose_text() == Some("human λ\tmachine\n"));
        assert!(text.expose_byte_len() == Some(17));
        assert!(text.expose_erase_count().is_none());
        assert!(text.invariants_hold());

        let exact = [b'a'; session_input_text_capacity];
        assert!(
            session_input_action::try_insert_text(&exact)
                .expect("exact capacity")
                .invariants_hold()
        );
        assert!(matches!(
            session_input_action::try_insert_text(&[]),
            Err(session_input_error::empty_text)
        ));
        assert!(matches!(
            session_input_action::try_insert_text(&[0xf0, 0x28, 0x8c, 0x28]),
            Err(session_input_error::invalid_utf8)
        ));
        assert!(matches!(
            session_input_action::try_insert_text(&[b'a'; session_input_text_capacity + 1]),
            Err(session_input_error::text_limit_exceeded)
        ));
        assert!(matches!(
            session_input_action::erase_previous(0),
            Err(session_input_error::zero_erase_count)
        ));
        assert!(
            session_input_action::erase_previous(session_input_erase_limit)
                .expect("bounded erase")
                .invariants_hold()
        );
        assert!(matches!(
            session_input_action::erase_previous(session_input_erase_limit + 1),
            Err(session_input_error::erase_limit_exceeded)
        ));
        for action in [
            session_input_action::submit(),
            session_input_action::cancel_submission(),
            session_input_action::source_discontinuity(),
        ] {
            assert!(action.invariants_hold());
            assert!(action.expose_text().is_none());
            assert!(action.expose_erase_count().is_none());
        }
    }

    #[test]
    fn paste_is_restricted_to_insertion_and_line_feed_remains_data() {
        let mut endpoint = endpoint();
        let receipt = publish(
            &mut endpoint,
            session_input_origin::paste,
            insertion("first\nsecond"),
        );
        assert!(receipt.wake().is_none());
        let (_, observed) = receive(&mut endpoint, None);
        assert!(observed.kind == session_input_action_kind::insert_text);
        assert!(observed.text.as_deref() == Some("first\nsecond"));

        for action in [
            session_input_action::erase_previous(1).expect("erase"),
            session_input_action::submit(),
            session_input_action::cancel_submission(),
            session_input_action::source_discontinuity(),
        ] {
            let revision = endpoint.revision();
            let rejection = match endpoint.prepare_publication(
                endpoint.binding(),
                session_input_origin::paste,
                action,
            ) {
                Ok(_) => panic!("restricted paste accepted"),
                Err(rejection) => rejection,
            };
            assert!(rejection.error() == session_input_error::paste_restricted);
            assert!(rejection.action().expose_kind() != session_input_action_kind::insert_text);
            assert!(endpoint.revision() == revision);
            drop(rejection.into_action());
        }
    }

    #[test]
    fn every_origin_uses_one_immediate_fifo_and_delivery_shape() {
        for origin in [
            session_input_origin::human_text,
            session_input_origin::machine_text,
            session_input_origin::paste,
            session_input_origin::accessibility,
            session_input_origin::remote_text,
        ] {
            let mut endpoint = endpoint();
            let receipt = publish(&mut endpoint, origin, insertion("same private text"));
            assert!(receipt.sequence().get() == 1);
            assert!(receipt.wake().is_none());
            assert!(matches!(
                endpoint.prepare_receive(endpoint.binding()),
                Ok(session_input_receive_preparation::ready)
            ));
            let (delivery, observed) = receive(&mut endpoint, None);
            assert!(delivery.sequence().get() == 1);
            assert!(!delivery.acknowledged_wait());
            assert!(observed.sequence == 1);
            assert!(observed.origin == origin);
            assert!(observed.text.as_deref() == Some("same private text"));
            assert!(observed.byte_len == 17);
            assert!(observed.erase_count.is_none());
            assert!(observed.fixed_tail_clear);
            assert!(endpoint.invariants_hold());
        }
    }

    #[test]
    fn nontext_actions_retain_exact_delivery_shape() {
        let mut endpoint = endpoint();
        for action in [
            session_input_action::erase_previous(7).expect("erase"),
            session_input_action::submit(),
            session_input_action::cancel_submission(),
            session_input_action::source_discontinuity(),
        ] {
            drop(publish(
                &mut endpoint,
                session_input_origin::human_text,
                action,
            ));
        }
        let (_, erase) = receive(&mut endpoint, None);
        assert!(erase.kind == session_input_action_kind::erase_previous);
        assert!(erase.erase_count == Some(7));
        assert!(erase.byte_len == 0);
        assert!(erase.text.is_none());
        for expected in [
            session_input_action_kind::submit,
            session_input_action_kind::cancel_submission,
            session_input_action_kind::source_discontinuity,
        ] {
            let (_, action) = receive(&mut endpoint, None);
            assert!(action.kind == expected);
            assert!(action.byte_len == 0);
            assert!(action.erase_count.is_none());
            assert!(action.text.is_none());
        }
    }

    #[test]
    fn full_backpressure_returns_private_ownership_without_counter_use() {
        let mut endpoint = endpoint();
        for value in ["one", "two", "three", "four"] {
            drop(publish(
                &mut endpoint,
                session_input_origin::human_text,
                insertion(value),
            ));
        }
        let revision = endpoint.revision();
        let next_sequence = endpoint.deliveries;
        let rejection = match endpoint.prepare_publication(
            endpoint.binding(),
            session_input_origin::human_text,
            insertion("private fifth"),
        ) {
            Ok(_) => panic!("full queue accepted"),
            Err(rejection) => rejection,
        };
        assert!(rejection.error() == session_input_error::queue_full);
        assert!(rejection.action().expose_text() == Some("private fifth"));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.deliveries == next_sequence);
        assert!(endpoint.length == session_input_queue_capacity);
        drop(rejection.into_action());
    }

    #[test]
    fn wrap_straddle_fifo_order_is_exact() {
        let mut endpoint = endpoint();
        for value in ["one", "two", "three", "four"] {
            drop(publish(
                &mut endpoint,
                session_input_origin::human_text,
                insertion(value),
            ));
        }
        for expected in ["one", "two"] {
            let (_, observed) = receive(&mut endpoint, None);
            assert!(observed.text.as_deref() == Some(expected));
        }
        for value in ["five", "six"] {
            drop(publish(
                &mut endpoint,
                session_input_origin::machine_text,
                insertion(value),
            ));
        }
        for (expected_sequence, expected_text) in
            [(3, "three"), (4, "four"), (5, "five"), (6, "six")]
        {
            let (receipt, observed) = receive(&mut endpoint, None);
            assert!(receipt.sequence().get() == expected_sequence);
            assert!(observed.text.as_deref() == Some(expected_text));
            assert!(endpoint.invariants_hold());
        }
        assert!(endpoint.head == 2);
        assert!(endpoint.tail == 2);
        assert!(endpoint.length == 0);
    }

    #[test]
    fn input_between_prepare_and_commit_prevents_sleep() {
        let mut endpoint = endpoint();
        let prepared = prepare_wait(&endpoint);
        drop(publish(
            &mut endpoint,
            session_input_origin::machine_text,
            insertion("arrived"),
        ));
        let revision = endpoint.revision();
        assert!(matches!(
            endpoint.commit_wait(prepared),
            Ok(session_input_wait_commit::ready)
        ));
        assert!(endpoint.revision() == revision);
        let (_, observed) = receive(&mut endpoint, None);
        assert!(observed.text.as_deref() == Some("arrived"));
    }

    #[test]
    fn invalidated_wait_sequence_cannot_regain_validity_when_input_appears() {
        let mut endpoint = endpoint();
        let prepared = prepare_wait(&endpoint);
        let wait_issue = endpoint.waits.prepare().expect("wait issue");
        let readiness_issue = endpoint.readiness.prepare().expect("readiness issue");
        endpoint.waits.commit(wait_issue);
        endpoint.readiness.commit(readiness_issue);
        drop(publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("ready but not for stale preparation"),
        ));
        let revision = endpoint.revision();
        assert!(matches!(
            endpoint.commit_wait(prepared),
            Err(session_input_error::stale_wait)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.length == 1);
        assert!(endpoint.phase == session_input_phase::open_idle);
    }

    #[test]
    fn stale_empty_preparation_cannot_install_after_transient_input() {
        let mut endpoint = endpoint();
        let prepared = prepare_wait(&endpoint);
        drop(publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("transient"),
        ));
        drop(receive(&mut endpoint, None));
        let revision = endpoint.revision();
        assert!(matches!(
            endpoint.commit_wait(prepared),
            Err(session_input_error::stale_preparation)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.phase == session_input_phase::open_idle);
    }

    #[test]
    fn input_after_block_returns_one_exact_move_only_wake() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        assert!(wait.binding() == endpoint.binding());
        assert!(readiness.binding() == endpoint.binding());
        let blocked_revision = endpoint.revision();
        assert!(matches!(
            endpoint.receive_with(endpoint.binding(), None, |_| true, |_| {}),
            Err(session_input_error::wait_not_ready)
        ));
        assert!(endpoint.revision() == blocked_revision);

        let prepared = prepare_publication(
            &endpoint,
            session_input_origin::accessibility,
            insertion("wake"),
        );
        let correlation = prepared
            .wake_correlation()
            .expect("prospective input-ready wake");
        assert!(correlation.binding() == endpoint.binding());
        assert!(correlation.wait() == wait);
        assert!(correlation.readiness() == readiness);
        assert!(correlation.outcome() == session_input_wake_outcome::input_ready);
        let receipt = endpoint
            .commit_publication(prepared)
            .unwrap_or_else(|rejection| panic!("publication: {}", rejection.error().code()));
        let wake = receipt.into_wake().expect("one input-ready wake");
        assert!(wake.correlation() == correlation);
        assert!(wake.binding() == endpoint.binding());
        assert!(wake.endpoint() == endpoint.binding().endpoint());
        assert!(wake.session() == endpoint.binding().session());
        assert!(wake.consumer() == endpoint.binding().consumer());
        assert!(wake.route() == endpoint.binding().route());
        assert!(wake.task() == endpoint.binding().task());
        assert!(wake.wait() == wait);
        assert!(wake.readiness() == readiness);
        assert!(wake.outcome() == session_input_wake_outcome::input_ready);
        drop(wake);

        let ready_revision = endpoint.revision();
        assert!(matches!(
            endpoint.receive_with(endpoint.binding(), None, |_| true, |_| {}),
            Err(session_input_error::wake_acknowledgement_required)
        ));
        assert!(endpoint.revision() == ready_revision);
        let acknowledgement = acknowledgement(wait, readiness);
        assert!(acknowledgement.wait() == wait);
        assert!(acknowledgement.readiness() == readiness);
        let (delivery, observed) = receive(&mut endpoint, Some(&acknowledgement));
        assert!(delivery.acknowledged_wait());
        assert!(observed.text.as_deref() == Some("wake"));
        assert!(endpoint.phase == session_input_phase::open_idle);
        assert!(endpoint.wait.is_none());
    }

    #[test]
    fn prepared_delivery_freezes_without_consuming_and_commits_after_selection() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let publication = publish(
            &mut endpoint,
            session_input_origin::machine_text,
            insertion("selected only"),
        );
        let correlation = publication.wake().expect("input-ready wake").correlation();
        drop(publication.into_wake());

        let revision = endpoint.revision();
        let head = endpoint.head;
        let prepared = endpoint
            .prepare_delivery(endpoint.binding(), Some(correlation))
            .expect("prepare selected delivery");
        assert!(prepared.correlation() == Some(correlation));
        let delivery = prepared.delivery(&endpoint).expect("frozen delivery");
        assert!(delivery.expose_text() == Some("selected only"));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.head == head);
        assert!(endpoint.length == 1);
        drop(prepared);
        assert!(endpoint.revision() == revision);
        assert!(endpoint.head == head);
        assert!(endpoint.queue[head].invariants_hold());

        let prepared = endpoint
            .prepare_delivery(endpoint.binding(), Some(correlation))
            .expect("prepare retry");
        let receipt =
            endpoint.commit_prepared_delivery(prepared, Some(acknowledgement(wait, readiness)));
        assert!(receipt.sequence().get() == 1);
        assert!(receipt.acknowledged_wait());
        assert!(endpoint.revision().get() == revision.get() + 2);
        assert!(endpoint.length == 0);
        assert!(endpoint.queue[head].storage_is_clear());
        assert!(endpoint.phase == session_input_phase::open_idle);
        assert!(endpoint.wait.is_none());
    }

    #[test]
    fn prepared_delivery_rejects_wrong_outcome_before_private_head_disclosure() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        drop(
            publish(
                &mut endpoint,
                session_input_origin::human_text,
                insertion("undisclosed"),
            )
            .into_wake(),
        );
        let revision = endpoint.revision();
        let wrong = session_input_wake_correlation::new(
            wait,
            readiness,
            session_input_wake_outcome::terminal(session_input_close_reason::focus_lost),
        );
        assert!(matches!(
            endpoint.prepare_delivery(endpoint.binding(), Some(wrong)),
            Err(session_input_error::wrong_wake_outcome)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.length == 1);
        assert!(endpoint.phase == session_input_phase::open_input_ready);
    }

    #[test]
    fn prepared_immediate_delivery_is_drop_safe_and_uses_no_acknowledgement() {
        let mut endpoint = endpoint();
        drop(publish(
            &mut endpoint,
            session_input_origin::accessibility,
            insertion("poll"),
        ));
        let revision = endpoint.revision();
        let prepared = endpoint
            .prepare_delivery(endpoint.binding(), None)
            .expect("prepare immediate delivery");
        assert!(prepared.correlation().is_none());
        assert!(
            prepared
                .delivery(&endpoint)
                .is_ok_and(|delivery| delivery.expose_text() == Some("poll"))
        );
        drop(prepared);
        assert!(endpoint.revision() == revision);
        assert!(endpoint.length == 1);

        let prepared = endpoint
            .prepare_delivery(endpoint.binding(), None)
            .expect("prepare immediate retry");
        assert!(
            prepared
                .delivery(&endpoint)
                .is_ok_and(|delivery| delivery.expose_text() == Some("poll"))
        );
        let receipt = endpoint.commit_prepared_delivery(prepared, None);
        assert!(!receipt.acknowledged_wait());
        assert!(endpoint.length == 0);
    }

    #[test]
    fn one_ready_wait_covers_later_fifo_publications_without_duplicate_wake() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let first = publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("first"),
        );
        assert!(first.wake().is_some());
        drop(first.into_wake());
        let second = publish(
            &mut endpoint,
            session_input_origin::machine_text,
            insertion("second"),
        );
        assert!(second.wake().is_none());

        let acknowledgement = acknowledgement(wait, readiness);
        let (_, first) = receive(&mut endpoint, Some(&acknowledgement));
        let (_, second) = receive(&mut endpoint, None);
        assert!(first.text.as_deref() == Some("first"));
        assert!(second.text.as_deref() == Some("second"));
    }

    #[test]
    fn receive_copy_failure_is_failure_atomic_and_copy_precedes_consume() {
        let mut endpoint = endpoint();
        drop(publish(
            &mut endpoint,
            session_input_origin::remote_text,
            insertion("private copy"),
        ));
        let revision = endpoint.revision();
        let head = endpoint.head;
        let delivery_cursor = endpoint.deliveries;
        let mut preflight_saw = false;
        assert!(matches!(
            endpoint.receive_with(
                endpoint.binding(),
                None,
                |delivery| {
                    preflight_saw = delivery.expose_text() == Some("private copy");
                    false
                },
                |_| panic!("copy commit ran after rejected preflight"),
            ),
            Err(session_input_error::copy_failed)
        ));
        assert!(preflight_saw);
        assert!(endpoint.revision() == revision);
        assert!(endpoint.head == head);
        assert!(endpoint.length == 1);
        assert!(endpoint.deliveries == delivery_cursor);
        assert!(endpoint.queue[head].invariants_hold());

        let mut copied = false;
        let receipt = endpoint
            .receive_with(
                endpoint.binding(),
                None,
                |_| true,
                |delivery| {
                    copied = delivery.expose_text() == Some("private copy");
                    assert!(delivery.slot.occupied);
                },
            )
            .expect("successful copy-before-consume");
        assert!(copied);
        assert!(receipt.sequence().get() == 1);
        assert!(endpoint.queue[head].storage_is_clear());
        assert!(endpoint.length == 0);
    }

    #[test]
    fn failed_woken_copy_preserves_wait_for_exact_retry_or_focus_close() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        drop(
            publish(
                &mut endpoint,
                session_input_origin::machine_text,
                insertion("retry private"),
            )
            .into_wake(),
        );
        let acknowledgement = acknowledgement(wait, readiness);
        let revision = endpoint.revision();
        assert!(matches!(
            endpoint.receive_with(
                endpoint.binding(),
                Some(&acknowledgement),
                |_| false,
                |_| panic!("failed copy committed"),
            ),
            Err(session_input_error::copy_failed)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.phase == session_input_phase::open_input_ready);
        assert!(endpoint.wait.is_some());
        assert!(endpoint.length == 1);

        let (receipt, observed) = receive(&mut endpoint, Some(&acknowledgement));
        assert!(receipt.acknowledged_wait());
        assert!(observed.text.as_deref() == Some("retry private"));
        assert!(endpoint.phase == session_input_phase::open_idle);
        assert!(endpoint.wait.is_none());
    }

    #[test]
    fn wrong_binding_parts_fail_before_private_state_inspection() {
        let mut endpoint = endpoint();
        drop(publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("private"),
        ));
        let expected = endpoint.binding();
        let wrong_endpoint = session_input_binding {
            endpoint: session_input_endpoint_identity::try_new(4, 1).expect("wrong endpoint"),
            ..expected
        };
        let wrong_session = session_input_binding {
            session: session_input_session_identity::try_new(6, 1).expect("wrong session"),
            ..expected
        };
        let wrong_consumer = session_input_binding {
            consumer: session_input_consumer_identity::try_new(8, 1).expect("wrong consumer"),
            ..expected
        };
        let wrong_route = session_input_binding {
            route: session_input_route_epoch::try_new(12).expect("wrong route"),
            ..expected
        };
        let wrong_task = session_input_binding {
            task: task(13, 2, 17, 1),
            ..expected
        };
        let revision = endpoint.revision();
        let depth = endpoint.length;
        for (candidate, error) in [
            (wrong_endpoint, session_input_error::wrong_endpoint),
            (wrong_session, session_input_error::wrong_session),
            (wrong_consumer, session_input_error::wrong_consumer),
            (wrong_route, session_input_error::stale_route),
            (wrong_task, session_input_error::wrong_task),
        ] {
            assert!(matches!(
                endpoint.prepare_receive(candidate),
                Err(actual) if actual == error
            ));
            assert!(matches!(
                endpoint.receive_with(candidate, None, |_| panic!("private preflight"), |_| {}),
                Err(actual) if actual == error
            ));
            assert!(endpoint.revision() == revision);
            assert!(endpoint.length == depth);
        }
    }

    #[test]
    fn wrong_wait_and_readiness_parts_are_independently_failure_atomic() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        drop(
            publish(
                &mut endpoint,
                session_input_origin::human_text,
                insertion("pending"),
            )
            .into_wake(),
        );
        let revision = endpoint.revision();

        let wrong_wait_sequence = session_input_wait_identity {
            sequence: NonZeroU64::new(wait.sequence() + 1).expect("nonzero"),
            ..wait
        };
        let wrong_readiness_generation = session_input_readiness_identity {
            generation: NonZeroU64::new(readiness.generation() + 1).expect("nonzero"),
            ..readiness
        };
        let wrong_wait_route = session_input_wait_identity {
            binding: session_input_binding {
                route: session_input_route_epoch::try_new(99).expect("wrong route"),
                ..wait.binding()
            },
            ..wait
        };
        let wrong_readiness_task = session_input_readiness_identity {
            binding: session_input_binding {
                task: task(13, 2, 17, 1),
                ..readiness.binding()
            },
            ..readiness
        };
        let stale_wait = acknowledgement(wrong_wait_sequence, readiness);
        let stale_readiness = acknowledgement(wait, wrong_readiness_generation);
        let wrong_route = acknowledgement(wrong_wait_route, readiness);
        let wrong_task = acknowledgement(wait, wrong_readiness_task);
        for (candidate, expected) in [
            (stale_wait, session_input_error::stale_wait),
            (stale_readiness, session_input_error::stale_readiness),
            (wrong_route, session_input_error::stale_route),
            (wrong_task, session_input_error::wrong_task),
        ] {
            assert!(matches!(
                endpoint.receive_with(
                    endpoint.binding(),
                    Some(&candidate),
                    |_| panic!("stale acknowledgement reached private preflight"),
                    |_| {},
                ),
                Err(error) if error == expected
            ));
            assert!(endpoint.revision() == revision);
            assert!(endpoint.length == 1);
            assert!(endpoint.phase == session_input_phase::open_input_ready);
        }
    }

    #[test]
    fn stale_publication_commit_returns_private_ownership() {
        let mut endpoint = endpoint();
        let first = prepare_publication(
            &endpoint,
            session_input_origin::human_text,
            insertion("first"),
        );
        let stale = prepare_publication(
            &endpoint,
            session_input_origin::machine_text,
            insertion("returned"),
        );
        assert!(stale.expose_action_kind() == session_input_action_kind::insert_text);
        drop(
            endpoint
                .commit_publication(first)
                .unwrap_or_else(|rejection| panic!("first commit: {}", rejection.error().code())),
        );
        let revision = endpoint.revision();
        let rejection = match endpoint.commit_publication(stale) {
            Ok(_) => panic!("stale publication accepted"),
            Err(rejection) => rejection,
        };
        assert!(rejection.error() == session_input_error::stale_preparation);
        assert!(rejection.action().expose_text() == Some("returned"));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.length == 1);
        drop(rejection.into_action());
    }

    #[test]
    fn close_without_wait_clears_every_private_slot_and_is_idempotent() {
        let mut endpoint = endpoint();
        for value in ["private one", "private two"] {
            drop(publish(
                &mut endpoint,
                session_input_origin::remote_text,
                insertion(value),
            ));
        }
        let revision = endpoint.revision().get();
        let receipt = close(&mut endpoint, session_input_close_reason::focus_lost);
        assert!(receipt.was_first_close());
        assert!(receipt.discarded_count() == 2);
        assert!(receipt.wake().is_none());
        assert!(endpoint.revision().get() == revision + 1);
        assert!(endpoint.phase == session_input_phase::closed_idle);
        assert!(
            endpoint
                .queue
                .iter()
                .all(session_input_queue_slot::storage_is_clear)
        );
        assert!(matches!(
            endpoint.prepare_receive(endpoint.binding()),
            Err(session_input_error::session_closed)
        ));
        assert!(matches!(
            endpoint.prepare_close(
                endpoint.binding(),
                session_input_close_reason::source_failed
            ),
            Ok(session_input_close_preparation::already_closed)
        ));
        assert!(endpoint.revision().get() == revision + 1);
    }

    #[test]
    fn close_blocked_wait_returns_one_terminal_wake_and_exact_acknowledgement() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let revision = endpoint.revision().get();
        let prepared = prepared_close(&endpoint, session_input_close_reason::route_replaced);
        let correlation = prepared
            .wake_correlation()
            .expect("prospective terminal wake");
        assert!(prepared.terminal_correlation() == Some(correlation));
        assert!(correlation.binding() == endpoint.binding());
        assert!(correlation.wait() == wait);
        assert!(correlation.readiness() == readiness);
        assert!(
            correlation.outcome()
                == session_input_wake_outcome::terminal(session_input_close_reason::route_replaced)
        );
        let receipt = endpoint.commit_close(prepared).expect("commit close");
        assert!(receipt.discarded_count() == 0);
        assert!(receipt.terminal_correlation() == Some(correlation));
        let wake = receipt.into_wake().expect("terminal wake");
        assert!(wake.correlation() == correlation);
        assert!(wake.wait() == wait);
        assert!(wake.readiness() == readiness);
        assert!(
            wake.outcome()
                == session_input_wake_outcome::terminal(session_input_close_reason::route_replaced)
        );
        drop(wake);
        assert!(endpoint.revision().get() == revision + 1);
        assert!(endpoint.phase == session_input_phase::closed_terminal);
        assert!(matches!(
            endpoint.receive_with(endpoint.binding(), None, |_| true, |_| {}),
            Err(session_input_error::terminal_acknowledgement_required)
        ));

        let terminal = endpoint
            .acknowledge_terminal(endpoint.binding(), acknowledgement(wait, readiness))
            .expect("terminal acknowledgement");
        assert!(terminal.wait() == wait);
        assert!(terminal.readiness() == readiness);
        assert!(terminal.reason() == session_input_close_reason::route_replaced);
        assert!(endpoint.revision().get() == revision + 2);
        assert!(endpoint.phase == session_input_phase::closed_idle);
        assert!(matches!(
            endpoint.receive_with(endpoint.binding(), None, |_| true, |_| {}),
            Err(session_input_error::session_closed)
        ));
        assert!(matches!(
            endpoint.acknowledge_terminal(endpoint.binding(), acknowledgement(wait, readiness)),
            Err(session_input_error::stale_wait)
        ));
    }

    #[test]
    fn prepared_terminal_is_drop_safe_and_commits_only_after_exact_selection() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let receipt = close(&mut endpoint, session_input_close_reason::source_failed);
        let correlation = receipt.wake().expect("terminal wake").correlation();
        drop(receipt.into_wake());
        let revision = endpoint.revision();

        let wrong = session_input_wake_correlation::new(
            wait,
            readiness,
            session_input_wake_outcome::input_ready,
        );
        assert!(matches!(
            endpoint.prepare_terminal(endpoint.binding(), wrong),
            Err(session_input_error::wrong_wake_outcome)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.phase == session_input_phase::closed_terminal);

        let prepared = endpoint
            .prepare_terminal(endpoint.binding(), correlation)
            .expect("prepare terminal");
        assert!(prepared.correlation() == correlation);
        drop(prepared);
        assert!(endpoint.revision() == revision);
        assert!(endpoint.wait.is_some());

        let prepared = endpoint
            .prepare_terminal(endpoint.binding(), correlation)
            .expect("prepare terminal retry");
        let terminal =
            endpoint.commit_prepared_terminal(prepared, acknowledgement(wait, readiness));
        assert!(terminal.wait() == wait);
        assert!(terminal.readiness() == readiness);
        assert!(terminal.reason() == session_input_close_reason::source_failed);
        assert!(endpoint.revision().get() == revision.get() + 1);
        assert!(endpoint.phase == session_input_phase::closed_idle);
        assert!(endpoint.wait.is_none());
    }

    #[test]
    fn close_input_ready_discards_typeahead_without_duplicate_wake() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let ready = publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("must be discarded"),
        );
        assert!(ready.wake().is_some());
        drop(ready.into_wake());
        let receipt = close(&mut endpoint, session_input_close_reason::focus_lost);
        assert!(receipt.discarded_count() == 1);
        assert!(receipt.wake().is_none());
        let terminal_correlation = receipt
            .terminal_correlation()
            .expect("already-runnable terminal correlation");
        assert!(terminal_correlation.wait() == wait);
        assert!(terminal_correlation.readiness() == readiness);
        assert!(
            terminal_correlation.outcome()
                == session_input_wake_outcome::terminal(session_input_close_reason::focus_lost)
        );
        assert!(endpoint.phase == session_input_phase::closed_terminal);
        assert!(endpoint.length == 0);
        assert!(
            endpoint
                .queue
                .iter()
                .all(session_input_queue_slot::storage_is_clear)
        );
        assert!(matches!(
            endpoint.receive_with(
                endpoint.binding(),
                Some(&acknowledgement(wait, readiness)),
                |_| panic!("closed text reached preflight"),
                |_| {},
            ),
            Err(session_input_error::terminal_acknowledgement_required)
        ));
        let prepared = endpoint
            .prepare_terminal(endpoint.binding(), terminal_correlation)
            .expect("prepare already-runnable terminal");
        drop(endpoint.commit_prepared_terminal(prepared, acknowledgement(wait, readiness)));
        assert!(matches!(
            endpoint.prepare_receive(endpoint.binding()),
            Err(session_input_error::session_closed)
        ));
    }

    #[test]
    fn stale_close_preparation_is_failure_atomic() {
        let mut endpoint = endpoint();
        let stale = prepared_close(&endpoint, session_input_close_reason::focus_lost);
        drop(publish(
            &mut endpoint,
            session_input_origin::human_text,
            insertion("later"),
        ));
        let revision = endpoint.revision();
        assert!(matches!(
            endpoint.commit_close(stale),
            Err(session_input_error::stale_preparation)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.phase == session_input_phase::open_idle);
        assert!(endpoint.length == 1);
    }

    #[test]
    fn terminal_acknowledgement_rejects_stale_parts_without_mutation() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        drop(close(&mut endpoint, session_input_close_reason::consumer_replaced).into_wake());
        let revision = endpoint.revision();
        let stale_readiness = session_input_readiness_identity {
            generation: NonZeroU64::new(readiness.generation() + 1).expect("generation"),
            ..readiness
        };
        assert!(matches!(
            endpoint
                .acknowledge_terminal(endpoint.binding(), acknowledgement(wait, stale_readiness)),
            Err(session_input_error::stale_readiness)
        ));
        assert!(endpoint.revision() == revision);
        assert!(endpoint.phase == session_input_phase::closed_terminal);
    }

    #[test]
    fn delivery_wait_and_readiness_cursors_exhaust_without_wrap() {
        let mut delivery_endpoint = endpoint();
        delivery_endpoint.deliveries.next = u64::MAX;
        let receipt = publish(
            &mut delivery_endpoint,
            session_input_origin::human_text,
            insertion("last delivery"),
        );
        assert!(receipt.sequence().get() == u64::MAX);
        drop(receive(&mut delivery_endpoint, None));
        let revision = delivery_endpoint.revision();
        let rejection = match delivery_endpoint.prepare_publication(
            delivery_endpoint.binding(),
            session_input_origin::human_text,
            insertion("held"),
        ) {
            Ok(_) => panic!("wrapped delivery accepted"),
            Err(rejection) => rejection,
        };
        assert!(rejection.error() == session_input_error::delivery_sequence_exhausted);
        assert!(rejection.action().expose_text() == Some("held"));
        assert!(delivery_endpoint.revision() == revision);

        let mut wait_endpoint = endpoint();
        wait_endpoint.waits.next = u64::MAX;
        wait_endpoint.readiness.next = u64::MAX;
        let (wait, readiness) = block(&mut wait_endpoint);
        assert!(wait.sequence() == u64::MAX);
        assert!(readiness.generation() == u64::MAX);
        drop(close(&mut wait_endpoint, session_input_close_reason::requested).into_wake());
        drop(
            wait_endpoint
                .acknowledge_terminal(wait_endpoint.binding(), acknowledgement(wait, readiness))
                .expect("retire final wait"),
        );

        let mut exhausted_wait = endpoint();
        exhausted_wait.waits.next = u64::MAX;
        exhausted_wait.waits.exhausted = true;
        assert!(matches!(
            exhausted_wait.prepare_receive(exhausted_wait.binding()),
            Err(session_input_error::wait_sequence_exhausted)
        ));
        let mut exhausted_readiness = endpoint();
        exhausted_readiness.readiness.next = u64::MAX;
        exhausted_readiness.readiness.exhausted = true;
        let wait_cursor = exhausted_readiness.waits;
        assert!(matches!(
            exhausted_readiness.prepare_receive(exhausted_readiness.binding()),
            Err(session_input_error::readiness_generation_exhausted)
        ));
        assert!(exhausted_readiness.waits == wait_cursor);
    }

    #[test]
    fn revision_headroom_keeps_every_public_state_retireable() {
        let mut wait_endpoint = endpoint();
        wait_endpoint.revision = u64::MAX - 3;
        let (wait, readiness) = block(&mut wait_endpoint);
        assert!(wait_endpoint.revision().get() == u64::MAX - 2);
        drop(close(&mut wait_endpoint, session_input_close_reason::requested).into_wake());
        assert!(wait_endpoint.revision().get() == u64::MAX - 1);
        drop(
            wait_endpoint
                .acknowledge_terminal(wait_endpoint.binding(), acknowledgement(wait, readiness))
                .expect("terminal acknowledgement may use final revision"),
        );
        assert!(wait_endpoint.revision().get() == u64::MAX);

        let mut publish_endpoint = endpoint();
        publish_endpoint.revision = u64::MAX - 2;
        let rejection = match publish_endpoint.prepare_publication(
            publish_endpoint.binding(),
            session_input_origin::human_text,
            insertion("held"),
        ) {
            Ok(_) => panic!("ordinary publish consumed close headroom"),
            Err(rejection) => rejection,
        };
        assert!(rejection.error() == session_input_error::revision_exhausted);
        assert!(publish_endpoint.revision().get() == u64::MAX - 2);
        drop(rejection.into_action());
        let close_receipt = close(&mut publish_endpoint, session_input_close_reason::requested);
        assert!(close_receipt.wake().is_none());
        assert!(publish_endpoint.revision().get() == u64::MAX - 1);

        let mut receive_endpoint = endpoint();
        drop(publish(
            &mut receive_endpoint,
            session_input_origin::human_text,
            insertion("retained"),
        ));
        receive_endpoint.revision = u64::MAX - 2;
        assert!(matches!(
            receive_endpoint.receive_with(
                receive_endpoint.binding(),
                None,
                |_| panic!("exhausted receive reached private preflight"),
                |_| {},
            ),
            Err(session_input_error::revision_exhausted)
        ));
        assert!(receive_endpoint.length == 1);
        assert!(receive_endpoint.revision().get() == u64::MAX - 2);
        let receipt = close(
            &mut receive_endpoint,
            session_input_close_reason::focus_lost,
        );
        assert!(receipt.discarded_count() == 1);
        assert!(receive_endpoint.revision().get() == u64::MAX - 1);
    }

    #[test]
    fn close_and_terminal_ack_have_exact_single_revision_increments() {
        let mut endpoint = endpoint();
        let (wait, readiness) = block(&mut endpoint);
        let before_close = endpoint.revision().get();
        drop(close(&mut endpoint, session_input_close_reason::source_failed).into_wake());
        assert!(endpoint.revision().get() == before_close + 1);
        drop(
            endpoint
                .acknowledge_terminal(endpoint.binding(), acknowledgement(wait, readiness))
                .expect("terminal acknowledgement"),
        );
        assert!(endpoint.revision().get() == before_close + 2);
    }

    #[test]
    fn logical_clear_path_zeroes_text_metadata_and_wait_state() {
        let mut action = insertion("private action");
        action.clear();
        assert!(action.invariants_hold());

        let mut endpoint = endpoint();
        let _ = block(&mut endpoint);
        drop(
            publish(
                &mut endpoint,
                session_input_origin::machine_text,
                insertion("private endpoint"),
            )
            .into_wake(),
        );
        endpoint.clear_owned_private_storage();
        assert!(endpoint.phase == session_input_phase::closed_idle);
        assert!(endpoint.wait.is_none());
        assert!(endpoint.length == 0);
        assert!(
            endpoint
                .queue
                .iter()
                .all(session_input_queue_slot::storage_is_clear)
        );
    }

    #[test]
    fn strengthened_invariants_detect_queue_wait_and_cursor_corruption() {
        let mut bad_head = endpoint();
        bad_head.head = session_input_queue_capacity;
        assert!(!bad_head.invariants_hold());

        let mut bad_tail = endpoint();
        bad_tail.tail = session_input_queue_capacity;
        assert!(!bad_tail.invariants_hold());

        let mut bad_slot = endpoint();
        bad_slot.queue[0].occupied = true;
        bad_slot.length = 1;
        bad_slot.tail = 1;
        assert!(!bad_slot.invariants_hold());

        let mut bad_cursor = endpoint();
        bad_cursor.waits.next = 0;
        assert!(!bad_cursor.invariants_hold());

        let mut bad_wait = endpoint();
        let _ = block(&mut bad_wait);
        bad_wait.wait.as_mut().expect("wait").identity.binding.route =
            session_input_route_epoch::try_new(99).expect("route");
        assert!(!bad_wait.invariants_hold());
    }

    #[test]
    fn errors_format_only_as_static_content_free_codes() {
        let errors = [
            session_input_error::zero_endpoint_generation,
            session_input_error::zero_session_slot,
            session_input_error::zero_session_generation,
            session_input_error::zero_consumer,
            session_input_error::zero_consumer_generation,
            session_input_error::zero_route_epoch,
            session_input_error::empty_text,
            session_input_error::invalid_utf8,
            session_input_error::text_limit_exceeded,
            session_input_error::forbidden_scalar,
            session_input_error::zero_erase_count,
            session_input_error::erase_limit_exceeded,
            session_input_error::paste_restricted,
            session_input_error::wrong_endpoint,
            session_input_error::wrong_session,
            session_input_error::wrong_consumer,
            session_input_error::stale_route,
            session_input_error::wrong_task,
            session_input_error::session_closed,
            session_input_error::queue_full,
            session_input_error::queue_empty,
            session_input_error::wait_already_present,
            session_input_error::wait_not_ready,
            session_input_error::wake_acknowledgement_required,
            session_input_error::terminal_acknowledgement_required,
            session_input_error::stale_wait,
            session_input_error::stale_readiness,
            session_input_error::wrong_wake_outcome,
            session_input_error::stale_preparation,
            session_input_error::delivery_sequence_exhausted,
            session_input_error::wait_sequence_exhausted,
            session_input_error::readiness_generation_exhausted,
            session_input_error::revision_exhausted,
            session_input_error::copy_failed,
            session_input_error::invariant_violation,
        ];
        for error in errors {
            assert!(
                error
                    .code()
                    .bytes()
                    .all(|byte| byte == b'_' || byte.is_ascii_lowercase() || byte.is_ascii_digit())
            );
            assert!(std::format!("{error}") == error.code());
            assert!(std::format!("{error:?}") == error.code());
            assert!(!error.code().contains("private"));
        }
    }

    #[test]
    fn host_session_and_native_endpoint_share_lost_wake_and_transient_replay() {
        let mut host_session = new_host_session();
        let mut native = endpoint();
        let host_prepared = host_prepare_wait(&host_session);
        let native_prepared = prepare_wait(&native);

        drop(host_publish(
            &mut host_session,
            host::submission_origin::machine_text,
            host_insertion("arrived between"),
        ));
        drop(publish(
            &mut native,
            session_input_origin::machine_text,
            insertion("arrived between"),
        ));
        let host_revision = host_session.revision();
        let native_revision = native.revision();
        assert!(matches!(
            host_session.commit_wait(host_prepared),
            Ok(host::wait_commit::ready)
        ));
        assert!(matches!(
            native.commit_wait(native_prepared),
            Ok(session_input_wait_commit::ready)
        ));
        assert!(host_session.revision() == host_revision);
        assert!(native.revision() == native_revision);
        let host_delivery = host_session
            .receive(host_session.consumer())
            .expect("host between delivery");
        let native_prepared = native
            .prepare_delivery(native.binding(), None)
            .expect("native between delivery");
        assert!(
            native_prepared
                .delivery(&native)
                .is_ok_and(|delivery| delivery.expose_text() == Some("arrived between"))
        );
        drop(native.commit_prepared_delivery(native_prepared, None));
        assert!(host_delivery.action().expose_text() == Some("arrived between"));

        let mut host_session = new_host_session();
        let mut native = endpoint();
        let host_replay = host_prepare_wait(&host_session);
        let native_replay = prepare_wait(&native);
        drop(host_publish(
            &mut host_session,
            host::submission_origin::human_text,
            host_insertion("transient"),
        ));
        drop(publish(
            &mut native,
            session_input_origin::human_text,
            insertion("transient"),
        ));
        drop(
            host_session
                .receive(host_session.consumer())
                .expect("host transient receive"),
        );
        let native_prepared = native
            .prepare_delivery(native.binding(), None)
            .expect("native transient receive");
        assert!(
            native_prepared
                .delivery(&native)
                .is_ok_and(|delivery| delivery.expose_text() == Some("transient"))
        );
        drop(native.commit_prepared_delivery(native_prepared, None));

        let host_revision = host_session.revision();
        let native_revision = native.revision();
        assert!(matches!(
            host_session.commit_wait(host_replay),
            Err(host::session_error::stale_revision)
        ));
        assert!(matches!(
            native.commit_wait(native_replay),
            Err(session_input_error::stale_preparation)
        ));
        assert!(host_session.revision() == host_revision);
        assert!(native.revision() == native_revision);
        assert!(host_session.queue_depth() == 0);
        assert!(native.length == 0);
    }

    #[test]
    fn host_session_and_native_endpoint_share_full_return_and_wake_replay() {
        let mut host_session = new_host_session();
        let mut native = endpoint();
        for value in ["one", "two", "three", "four"] {
            drop(host_publish(
                &mut host_session,
                host::submission_origin::human_text,
                host_insertion(value),
            ));
            drop(publish(
                &mut native,
                session_input_origin::human_text,
                insertion(value),
            ));
        }
        let host_revision = host_session.revision();
        let native_revision = native.revision();
        let host_prepared = host_session
            .prepare_submission(
                host::submission_origin::remote_text,
                host_insertion("returned fifth"),
            )
            .unwrap_or_else(|_| panic!("host fifth preparation"));
        let host_rejection = match host_session.publish(host_prepared) {
            Ok(_) => panic!("host full queue accepted fifth"),
            Err(rejection) => rejection,
        };
        let native_rejection = match native.prepare_publication(
            native.binding(),
            session_input_origin::remote_text,
            insertion("returned fifth"),
        ) {
            Ok(_) => panic!("native full queue accepted fifth"),
            Err(rejection) => rejection,
        };
        assert!(host_rejection.error() == host::session_error::queue_full);
        assert!(native_rejection.error() == session_input_error::queue_full);
        assert!(host_rejection.submission().origin() == host::submission_origin::remote_text);
        assert!(
            host_rejection.submission().action_kind()
                == Some(host::semantic_action_kind::insert_text)
        );
        assert!(native_rejection.action().expose_text() == Some("returned fifth"));
        assert!(host_session.revision() == host_revision);
        assert!(native.revision() == native_revision);
        drop(host_rejection.into_submission());
        drop(native_rejection.into_action());

        for expected in ["one", "two", "three", "four"] {
            let host_delivery = host_session
                .receive(host_session.consumer())
                .expect("host full-queue drain");
            let native_prepared = native
                .prepare_delivery(native.binding(), None)
                .expect("native full-queue drain");
            assert!(
                native_prepared
                    .delivery(&native)
                    .is_ok_and(|delivery| delivery.expose_text() == Some(expected))
            );
            drop(native.commit_prepared_delivery(native_prepared, None));
            assert!(host_delivery.action().expose_text() == Some(expected));
        }
        let host_fifth = host_publish(
            &mut host_session,
            host::submission_origin::remote_text,
            host_insertion("accepted fifth"),
        );
        let native_fifth = publish(
            &mut native,
            session_input_origin::remote_text,
            insertion("accepted fifth"),
        );
        assert!(host_fifth.sequence().get() == 5);
        assert!(native_fifth.sequence().get() == 5);

        let mut host_session = new_host_session();
        let mut native = endpoint();
        let host_wait = host_block(&mut host_session);
        let (native_wait, native_readiness) = block(&mut native);
        let host_publication = host_publish(
            &mut host_session,
            host::submission_origin::machine_text,
            host_insertion("one wake"),
        );
        let native_publication = publish(
            &mut native,
            session_input_origin::machine_text,
            insertion("one wake"),
        );
        assert!(host_publication.wake() == Some(host_wait));
        let correlation = native_publication
            .wake()
            .expect("native wake")
            .correlation();
        drop(native_publication.into_wake());
        host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host wake acknowledgement");
        drop(
            host_session
                .receive(host_session.consumer())
                .expect("host woken delivery"),
        );
        let native_prepared = native
            .prepare_delivery(native.binding(), Some(correlation))
            .expect("native woken delivery");
        assert!(
            native_prepared
                .delivery(&native)
                .is_ok_and(|delivery| delivery.expose_text() == Some("one wake"))
        );
        drop(native.commit_prepared_delivery(
            native_prepared,
            Some(acknowledgement(native_wait, native_readiness)),
        ));
        assert!(host_session.revision().get() == native.revision().get());

        let host_revision = host_session.revision();
        let native_revision = native.revision();
        assert!(matches!(
            host_session.acknowledge_wake(host_session.consumer(), host_wait),
            Err(host::session_error::stale_wait)
        ));
        assert!(matches!(
            native.prepare_delivery(native.binding(), Some(correlation)),
            Err(session_input_error::stale_wait)
        ));
        assert!(host_session.revision() == host_revision);
        assert!(native.revision() == native_revision);
    }

    #[test]
    fn host_session_and_native_endpoint_share_sequence_and_revision_retirement() {
        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_final_submission_sequence_for_kernel_conformance()
            .expect("host final submission sequence");
        native.deliveries.next = u64::MAX;
        let host_last = host_publish(
            &mut host_session,
            host::submission_origin::human_text,
            host_insertion("last delivery"),
        );
        let native_last = publish(
            &mut native,
            session_input_origin::human_text,
            insertion("last delivery"),
        );
        assert!(host_last.sequence().get() == u64::MAX);
        assert!(native_last.sequence().get() == u64::MAX);
        drop(
            host_session
                .receive(host_session.consumer())
                .expect("host final delivery"),
        );
        let native_prepared = native
            .prepare_delivery(native.binding(), None)
            .expect("native final delivery");
        assert!(
            native_prepared
                .delivery(&native)
                .is_ok_and(|delivery| delivery.expose_text() == Some("last delivery"))
        );
        drop(native.commit_prepared_delivery(native_prepared, None));
        let host_revision = host_session.revision();
        let native_revision = native.revision();
        let host_prepared = host_session
            .prepare_submission(host::submission_origin::human_text, host_insertion("held"))
            .unwrap_or_else(|_| panic!("host exhausted preparation"));
        let host_rejection = match host_session.publish(host_prepared) {
            Ok(_) => panic!("host wrapped submission sequence"),
            Err(rejection) => rejection,
        };
        let native_rejection = match native.prepare_publication(
            native.binding(),
            session_input_origin::human_text,
            insertion("held"),
        ) {
            Ok(_) => panic!("native wrapped delivery sequence"),
            Err(rejection) => rejection,
        };
        assert!(host_rejection.error() == host::session_error::submission_sequence_exhausted);
        assert!(native_rejection.error() == session_input_error::delivery_sequence_exhausted);
        assert!(native_rejection.action().expose_text() == Some("held"));
        assert!(host_session.revision() == host_revision);
        assert!(native.revision() == native_revision);
        drop(host_rejection.into_submission());
        drop(native_rejection.into_action());

        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_final_wait_sequence_for_kernel_conformance()
            .expect("host final wait sequence");
        native.waits.next = u64::MAX;
        native.readiness.next = u64::MAX;
        let host_wait = host_block(&mut host_session);
        let (native_wait, native_readiness) = block(&mut native);
        assert!(host_wait.sequence() == u64::MAX);
        assert!(native_wait.sequence() == u64::MAX);
        assert!(native_readiness.generation() == u64::MAX);
        drop(host_publish(
            &mut host_session,
            host::submission_origin::human_text,
            host_insertion("final wait"),
        ));
        let native_publication = publish(
            &mut native,
            session_input_origin::human_text,
            insertion("final wait"),
        );
        let correlation = native_publication
            .wake()
            .expect("native final wake")
            .correlation();
        drop(native_publication.into_wake());
        host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host final wake acknowledgement");
        drop(
            host_session
                .receive(host_session.consumer())
                .expect("host final wait delivery"),
        );
        let native_prepared = native
            .prepare_delivery(native.binding(), Some(correlation))
            .expect("native final wait delivery");
        assert!(
            native_prepared
                .delivery(&native)
                .is_ok_and(|delivery| delivery.expose_text() == Some("final wait"))
        );
        drop(native.commit_prepared_delivery(
            native_prepared,
            Some(acknowledgement(native_wait, native_readiness)),
        ));
        assert!(matches!(
            host_session.prepare_receive(host_session.consumer()),
            Err(host::session_error::wait_sequence_exhausted)
        ));
        assert!(matches!(
            native.prepare_receive(native.binding()),
            Err(session_input_error::wait_sequence_exhausted)
        ));

        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_revision_retirement_for_kernel_conformance()
            .expect("host retirement revision");
        native.revision = u64::MAX - 3;
        let host_wait = host_block(&mut host_session);
        let (native_wait, native_readiness) = block(&mut native);
        assert!(host_session.revision().get() == u64::MAX - 2);
        assert!(native.revision().get() == u64::MAX - 2);
        let host_close = host_session
            .close(host::close_reason::requested)
            .expect("host retirement close");
        let native_close = close(&mut native, session_input_close_reason::requested);
        assert!(host_close.terminal_wake() == Some(host_wait));
        let terminal = native_close
            .terminal_correlation()
            .expect("native retirement terminal");
        drop(native_close.into_wake());
        assert!(host_session.revision().get() == u64::MAX - 1);
        assert!(native.revision().get() == u64::MAX - 1);
        let host_terminal = host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host retirement acknowledgement");
        assert!(
            host_terminal.outcome() == host::wake_outcome::terminal(host::close_reason::requested)
        );
        let native_prepared = native
            .prepare_terminal(native.binding(), terminal)
            .expect("native retirement acknowledgement");
        drop(native.commit_prepared_terminal(
            native_prepared,
            acknowledgement(native_wait, native_readiness),
        ));
        assert!(host_session.revision().get() == u64::MAX);
        assert!(native.revision().get() == u64::MAX);

        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_close_only_revision_for_kernel_conformance()
            .expect("host close-only revision");
        native.revision = u64::MAX - 2;
        let host_prepared = host_session
            .prepare_submission(
                host::submission_origin::human_text,
                host_insertion("retained"),
            )
            .unwrap_or_else(|_| panic!("host close-only preparation"));
        let host_rejection = match host_session.publish(host_prepared) {
            Ok(_) => panic!("host ordinary mutation consumed close headroom"),
            Err(rejection) => rejection,
        };
        let native_rejection = match native.prepare_publication(
            native.binding(),
            session_input_origin::human_text,
            insertion("retained"),
        ) {
            Ok(_) => panic!("native ordinary mutation consumed close headroom"),
            Err(rejection) => rejection,
        };
        assert!(host_rejection.error() == host::session_error::revision_exhausted);
        assert!(native_rejection.error() == session_input_error::revision_exhausted);
        assert!(native_rejection.action().expose_text() == Some("retained"));
        assert!(host_session.revision().get() == u64::MAX - 2);
        assert!(native.revision().get() == u64::MAX - 2);
        drop(host_rejection.into_submission());
        drop(native_rejection.into_action());
        let host_close = host_session
            .close(host::close_reason::requested)
            .expect("host close-only retirement");
        let native_close = close(&mut native, session_input_close_reason::requested);
        assert!(host_close.discarded_count() == 0);
        assert!(native_close.discarded_count() == 0);
        assert!(host_session.revision().get() == u64::MAX - 1);
        assert!(native.revision().get() == u64::MAX - 1);
    }

    #[test]
    fn host_and_native_woken_delivery_share_the_revision_exhaustion_frontier() {
        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_last_wake_delivery_revision_for_kernel_conformance()
            .expect("host last successful delivery revision");
        native.revision = u64::MAX - 6;

        let host_wait = host_block(&mut host_session);
        let (native_wait, native_readiness) = block(&mut native);
        assert!(host_session.revision().get() == u64::MAX - 5);
        assert!(native.revision().get() == u64::MAX - 5);
        let host_publication = host_publish(
            &mut host_session,
            host::submission_origin::human_text,
            host_insertion("last successful wake"),
        );
        let native_publication = publish(
            &mut native,
            session_input_origin::human_text,
            insertion("last successful wake"),
        );
        assert!(host_session.revision().get() == u64::MAX - 4);
        assert!(native.revision().get() == u64::MAX - 4);
        assert!(host_publication.wake() == Some(host_wait));
        let native_correlation = native_publication
            .wake()
            .expect("native last successful wake")
            .correlation();
        drop(native_publication.into_wake());

        host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host last wake acknowledgement");
        drop(
            host_session
                .receive(host_session.consumer())
                .expect("host last woken delivery"),
        );
        let native_delivery = native
            .prepare_delivery(native.binding(), Some(native_correlation))
            .expect("native last woken delivery");
        drop(native.commit_prepared_delivery(
            native_delivery,
            Some(acknowledgement(native_wait, native_readiness)),
        ));
        assert!(host_session.revision().get() == u64::MAX - 2);
        assert!(native.revision().get() == u64::MAX - 2);

        let mut host_session = new_host_session();
        let mut native = endpoint();
        host_session
            .prepare_rejected_wake_delivery_revision_for_kernel_conformance()
            .expect("host first rejected delivery revision");
        native.revision = u64::MAX - 5;
        let host_wait = host_block(&mut host_session);
        let (native_wait, native_readiness) = block(&mut native);
        assert!(host_session.revision().get() == u64::MAX - 4);
        assert!(native.revision().get() == u64::MAX - 4);

        let host_prepared = host_session
            .prepare_submission(
                host::submission_origin::machine_text,
                host_insertion("returned host wake"),
            )
            .unwrap_or_else(|_| panic!("host rejected-wake preparation"));
        let host_rejection = match host_session.publish(host_prepared) {
            Ok(_) => panic!("host accepted an unfinishable wake"),
            Err(rejection) => rejection,
        };
        let native_rejection = match native.prepare_publication(
            native.binding(),
            session_input_origin::machine_text,
            insertion("returned native wake"),
        ) {
            Ok(_) => panic!("native accepted an unfinishable wake"),
            Err(rejection) => rejection,
        };
        assert!(host_rejection.error() == host::session_error::revision_exhausted);
        assert!(native_rejection.error() == session_input_error::revision_exhausted);
        assert!(
            host_rejection.submission().action_kind()
                == Some(host::semantic_action_kind::insert_text)
        );
        assert!(native_rejection.action().expose_text() == Some("returned native wake"));
        assert!(host_session.revision().get() == u64::MAX - 4);
        assert!(native.revision().get() == u64::MAX - 4);
        assert!(host_session.queue_depth() == 0);
        assert!(native.length == 0);
        drop(host_rejection.into_submission());
        drop(native_rejection.into_action());

        let host_close = host_session
            .close(host::close_reason::requested)
            .expect("host rejected-wake retirement close");
        let native_close = close(&mut native, session_input_close_reason::requested);
        assert!(host_close.terminal_wake() == Some(host_wait));
        let native_terminal = native_close
            .terminal_correlation()
            .expect("native rejected-wake terminal");
        drop(native_close.into_wake());
        host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host rejected-wake terminal acknowledgement");
        let native_terminal = native
            .prepare_terminal(native.binding(), native_terminal)
            .expect("native rejected-wake terminal acknowledgement");
        drop(native.commit_prepared_terminal(
            native_terminal,
            acknowledgement(native_wait, native_readiness),
        ));
        assert!(host_session.revision().get() == u64::MAX - 2);
        assert!(native.revision().get() == u64::MAX - 2);
    }

    #[test]
    fn exhaustive_bounded_host_and_native_transition_traces_conform() {
        const steps: [conformance_step; 5] = [
            conformance_step::publish,
            conformance_step::prepare,
            conformance_step::commit,
            conformance_step::complete,
            conformance_step::close,
        ];
        let trace_count = steps.len().pow(4);
        for encoded in 0..trace_count {
            let mut host = host_conformance_state::new();
            let mut native = native_conformance_state::new();
            let mut cursor = encoded;
            for _ in 0..4 {
                let step = steps[cursor % steps.len()];
                cursor /= steps.len();
                let host_result = host.apply(step);
                let native_result = native.apply(step);
                assert!(
                    host_result == native_result,
                    "bounded model outcome drifted"
                );
                assert!(host.session.is_open() == native.is_open());
                assert!(usize::from(host.session.queue_depth()) == native.endpoint.length);
                assert!(host.prepared.is_some() == native.prepared.is_some());
                assert!(host.wait.is_some() == native.wait.is_some());
                assert!(native.endpoint.invariants_hold());
            }
        }
    }

    #[test]
    fn host_session_and_native_endpoint_share_every_action_origin_boundary() {
        let origins = [
            (
                host::submission_origin::human_text,
                session_input_origin::human_text,
            ),
            (
                host::submission_origin::machine_text,
                session_input_origin::machine_text,
            ),
            (host::submission_origin::paste, session_input_origin::paste),
            (
                host::submission_origin::accessibility,
                session_input_origin::accessibility,
            ),
            (
                host::submission_origin::remote_text,
                session_input_origin::remote_text,
            ),
        ];

        for kind in 0..5 {
            for (host_origin, native_origin) in origins {
                let mut host_session = host::shell_session::new(
                    host::session_identity::try_new(5, 1).expect("host session"),
                    host::consumer_identity::try_new(7, 1).expect("host consumer"),
                    host::route_epoch::try_new(11).expect("host route"),
                    host::language_version::new(0, 1),
                );
                let mut native = endpoint();
                let host_action = match kind {
                    0 => host::semantic_action::insert_text(
                        host::private_text::try_from_str("same\ntext").expect("host text"),
                    )
                    .expect("host insertion"),
                    1 => host::semantic_action::erase_previous(9).expect("host erase"),
                    2 => host::semantic_action::submit(),
                    3 => host::semantic_action::cancel_submission(),
                    4 => host::semantic_action::source_discontinuity(),
                    _ => unreachable!(),
                };
                let native_action = match kind {
                    0 => insertion("same\ntext"),
                    1 => session_input_action::erase_previous(9).expect("native erase"),
                    2 => session_input_action::submit(),
                    3 => session_input_action::cancel_submission(),
                    4 => session_input_action::source_discontinuity(),
                    _ => unreachable!(),
                };
                let host_prepared = host_session.prepare_submission(host_origin, host_action);
                let native_prepared =
                    native.prepare_publication(native.binding(), native_origin, native_action);
                if host_origin == host::submission_origin::paste && kind != 0 {
                    let host_rejection = match host_prepared {
                        Ok(_) => panic!("host accepted restricted paste"),
                        Err(rejection) => rejection,
                    };
                    let native_rejection = match native_prepared {
                        Ok(_) => panic!("native accepted restricted paste"),
                        Err(rejection) => rejection,
                    };
                    assert!(host_rejection.error() == host::session_error::paste_restricted);
                    assert!(native_rejection.error() == session_input_error::paste_restricted);
                    continue;
                }

                let host_receipt = host_session
                    .publish(host_prepared.unwrap_or_else(|_| panic!("host prepare")))
                    .unwrap_or_else(|_| panic!("host publish"));
                let native_receipt = native
                    .commit_publication(native_prepared.unwrap_or_else(|rejection| {
                        panic!("native prepare: {}", rejection.error().code())
                    }))
                    .unwrap_or_else(|rejection| {
                        panic!("native publish: {}", rejection.error().code())
                    });
                assert!(host_receipt.sequence().get() == native_receipt.sequence().get());
                assert!(host_receipt.wake().is_none());
                assert!(native_receipt.wake().is_none());

                let host_delivery = host_session
                    .receive(host_session.consumer())
                    .expect("host receive");
                let (_, native_delivery) = receive(&mut native, None);
                assert!(host_delivery.origin() == host_origin);
                assert!(native_delivery.origin == native_origin);
                match kind {
                    0 => {
                        assert!(
                            host_delivery.action().kind()
                                == Some(host::semantic_action_kind::insert_text)
                        );
                        assert!(native_delivery.kind == session_input_action_kind::insert_text);
                        assert!(
                            host_delivery.action().expose_text() == native_delivery.text.as_deref()
                        );
                    }
                    1 => {
                        assert!(
                            host_delivery.action().kind()
                                == Some(host::semantic_action_kind::erase_previous)
                        );
                        assert!(native_delivery.kind == session_input_action_kind::erase_previous);
                        assert!(
                            host_delivery.action().expose_erase_count()
                                == native_delivery.erase_count
                        );
                    }
                    2 => {
                        assert!(
                            host_delivery.action().kind()
                                == Some(host::semantic_action_kind::submit)
                        );
                        assert!(native_delivery.kind == session_input_action_kind::submit);
                    }
                    3 => {
                        assert!(
                            host_delivery.action().kind()
                                == Some(host::semantic_action_kind::cancel_submission)
                        );
                        assert!(
                            native_delivery.kind == session_input_action_kind::cancel_submission
                        );
                    }
                    4 => {
                        assert!(
                            host_delivery.action().kind()
                                == Some(host::semantic_action_kind::source_discontinuity)
                        );
                        assert!(
                            native_delivery.kind == session_input_action_kind::source_discontinuity
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    #[test]
    fn host_session_and_native_endpoint_share_fifo_wrap_order() {
        let mut host_session = host::shell_session::new(
            host::session_identity::try_new(5, 1).expect("host session"),
            host::consumer_identity::try_new(7, 1).expect("host consumer"),
            host::route_epoch::try_new(11).expect("host route"),
            host::language_version::new(0, 1),
        );
        let mut native = endpoint();
        for value in ["one", "two", "three", "four"] {
            let host_action = host::semantic_action::insert_text(
                host::private_text::try_from_str(value).expect("host text"),
            )
            .expect("host action");
            let host_prepared = host_session
                .prepare_submission(host::submission_origin::human_text, host_action)
                .unwrap_or_else(|_| panic!("host prepare"));
            drop(
                host_session
                    .publish(host_prepared)
                    .unwrap_or_else(|_| panic!("host publish")),
            );
            drop(publish(
                &mut native,
                session_input_origin::human_text,
                insertion(value),
            ));
        }
        for expected in ["one", "two"] {
            let host_delivery = host_session
                .receive(host_session.consumer())
                .expect("host receive");
            let (_, native_delivery) = receive(&mut native, None);
            assert!(host_delivery.action().expose_text() == Some(expected));
            assert!(native_delivery.text.as_deref() == Some(expected));
        }
        for value in ["five", "six"] {
            let host_action = host::semantic_action::insert_text(
                host::private_text::try_from_str(value).expect("host text"),
            )
            .expect("host action");
            let host_prepared = host_session
                .prepare_submission(host::submission_origin::machine_text, host_action)
                .unwrap_or_else(|_| panic!("host prepare"));
            drop(
                host_session
                    .publish(host_prepared)
                    .unwrap_or_else(|_| panic!("host publish")),
            );
            drop(publish(
                &mut native,
                session_input_origin::machine_text,
                insertion(value),
            ));
        }
        for (expected_sequence, expected_text) in
            [(3, "three"), (4, "four"), (5, "five"), (6, "six")]
        {
            let host_delivery = host_session
                .receive(host_session.consumer())
                .expect("host receive");
            let (native_receipt, native_delivery) = receive(&mut native, None);
            assert!(host_delivery.sequence().get() == expected_sequence);
            assert!(native_receipt.sequence().get() == expected_sequence);
            assert!(host_delivery.action().expose_text() == Some(expected_text));
            assert!(native_delivery.text.as_deref() == Some(expected_text));
        }
    }

    #[test]
    fn host_session_and_native_endpoint_share_ready_then_close_order() {
        let mut host_session = host::shell_session::new(
            host::session_identity::try_new(5, 1).expect("host session"),
            host::consumer_identity::try_new(7, 1).expect("host consumer"),
            host::route_epoch::try_new(11).expect("host route"),
            host::language_version::new(0, 1),
        );
        let mut native = endpoint();
        let host_prepared_wait = match host_session
            .prepare_receive(host_session.consumer())
            .expect("host prepare wait")
        {
            host::receive_preparation::ready => panic!("host unexpectedly ready"),
            host::receive_preparation::wait(wait) => wait,
        };
        let host_wait = match host_session
            .commit_wait(host_prepared_wait)
            .expect("host commit wait")
        {
            host::wait_commit::ready => panic!("host unexpectedly ready"),
            host::wait_commit::blocked(wait) => wait,
        };
        let (native_wait, native_readiness) = block(&mut native);

        let host_action = host::semantic_action::insert_text(
            host::private_text::try_from_str("close wins").expect("host text"),
        )
        .expect("host action");
        let host_prepared = host_session
            .prepare_submission(host::submission_origin::machine_text, host_action)
            .unwrap_or_else(|_| panic!("host prepare"));
        let host_publication = host_session
            .publish(host_prepared)
            .unwrap_or_else(|_| panic!("host publish"));
        assert!(host_publication.wake() == Some(host_wait));
        let native_publication = publish(
            &mut native,
            session_input_origin::machine_text,
            insertion("close wins"),
        );
        let input_ready = native_publication
            .wake()
            .expect("native input-ready wake")
            .correlation();
        drop(native_publication.into_wake());

        let host_close = host_session
            .close(host::close_reason::route_replaced)
            .expect("host close");
        let native_close = close(&mut native, session_input_close_reason::route_replaced);
        assert!(host_close.discarded_count() == native_close.discarded_count());
        assert!(host_close.terminal_wake() == Some(host_wait));
        assert!(native_close.wake().is_none());
        let terminal = native_close
            .terminal_correlation()
            .expect("native terminal correlation");
        assert!(terminal.wait() == native_wait);
        assert!(terminal.readiness() == native_readiness);
        assert!(
            terminal.outcome()
                == session_input_wake_outcome::terminal(session_input_close_reason::route_replaced)
        );
        assert!(matches!(
            native.prepare_delivery(native.binding(), Some(input_ready)),
            Err(session_input_error::terminal_acknowledgement_required)
        ));

        let host_terminal = host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host terminal acknowledgement");
        let native_prepared = native
            .prepare_terminal(native.binding(), terminal)
            .expect("native prepare terminal");
        let native_terminal = native.commit_prepared_terminal(
            native_prepared,
            acknowledgement(native_wait, native_readiness),
        );
        assert!(
            host_terminal.outcome()
                == host::wake_outcome::terminal(host::close_reason::route_replaced)
        );
        assert!(native_terminal.reason() == session_input_close_reason::route_replaced);
        assert!(matches!(
            host_session.acknowledge_wake(host_session.consumer(), host_wait),
            Err(host::session_error::stale_wait)
        ));
        assert!(matches!(
            native.prepare_terminal(native.binding(), terminal),
            Err(session_input_error::stale_wait)
        ));
    }

    #[test]
    fn host_session_and_native_endpoint_share_immediate_wait_and_close_outcomes() {
        let mut host_session = host::shell_session::new(
            host::session_identity::try_new(5, 1).expect("host session"),
            host::consumer_identity::try_new(7, 1).expect("host consumer"),
            host::route_epoch::try_new(11).expect("host route"),
            host::language_version::new(0, 1),
        );
        let mut native = endpoint();

        let host_action = host::semantic_action::insert_text(
            host::private_text::try_from_str("same immediate").expect("host text"),
        )
        .expect("host action");
        let host_prepared = host_session
            .prepare_submission(host::submission_origin::human_text, host_action)
            .unwrap_or_else(|_| panic!("host prepare"));
        let host_receipt = host_session
            .publish(host_prepared)
            .unwrap_or_else(|_| panic!("host publish"));
        let native_receipt = publish(
            &mut native,
            session_input_origin::human_text,
            insertion("same immediate"),
        );
        assert!(host_receipt.sequence().get() == native_receipt.sequence().get());
        let host_delivery = host_session
            .receive(host_session.consumer())
            .expect("host immediate receive");
        let (_, native_delivery) = receive(&mut native, None);
        assert!(host_delivery.action().expose_text() == native_delivery.text.as_deref());

        let host_prepared_wait = match host_session
            .prepare_receive(host_session.consumer())
            .expect("host prepare wait")
        {
            host::receive_preparation::ready => panic!("host unexpectedly ready"),
            host::receive_preparation::wait(wait) => wait,
        };
        let host_wait = match host_session
            .commit_wait(host_prepared_wait)
            .expect("host commit wait")
        {
            host::wait_commit::ready => panic!("host unexpectedly ready"),
            host::wait_commit::blocked(wait) => wait,
        };
        let (native_wait, native_readiness) = block(&mut native);

        let host_action = host::semantic_action::insert_text(
            host::private_text::try_from_str("same wake").expect("host text"),
        )
        .expect("host action");
        let host_prepared = host_session
            .prepare_submission(host::submission_origin::machine_text, host_action)
            .unwrap_or_else(|_| panic!("host prepare"));
        let host_receipt = host_session
            .publish(host_prepared)
            .unwrap_or_else(|_| panic!("host publish"));
        assert!(host_receipt.wake() == Some(host_wait));
        let native_receipt = publish(
            &mut native,
            session_input_origin::machine_text,
            insertion("same wake"),
        );
        let native_wake = native_receipt.into_wake().expect("native wake");
        assert!(native_wake.wait() == native_wait);
        drop(native_wake);

        host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host wake acknowledgement");
        let host_delivery = host_session
            .receive(host_session.consumer())
            .expect("host wake receive");
        let native_ack = acknowledgement(native_wait, native_readiness);
        let (_, native_delivery) = receive(&mut native, Some(&native_ack));
        assert!(host_delivery.action().expose_text() == native_delivery.text.as_deref());

        let host_prepared_wait = match host_session
            .prepare_receive(host_session.consumer())
            .expect("host prepare wait")
        {
            host::receive_preparation::ready => panic!("host unexpectedly ready"),
            host::receive_preparation::wait(wait) => wait,
        };
        let host_wait = match host_session
            .commit_wait(host_prepared_wait)
            .expect("host block")
        {
            host::wait_commit::ready => panic!("host unexpectedly ready"),
            host::wait_commit::blocked(wait) => wait,
        };
        let (native_wait, native_readiness) = block(&mut native);
        let host_close = host_session
            .close(host::close_reason::route_replaced)
            .expect("host close");
        let native_close = close(&mut native, session_input_close_reason::route_replaced);
        assert!(host_close.terminal_wake() == Some(host_wait));
        assert!(native_close.wake().is_some());
        drop(native_close.into_wake());
        let host_terminal = host_session
            .acknowledge_wake(host_session.consumer(), host_wait)
            .expect("host terminal acknowledgement");
        assert!(
            host_terminal.outcome()
                == host::wake_outcome::terminal(host::close_reason::route_replaced)
        );
        drop(
            native
                .acknowledge_terminal(
                    native.binding(),
                    acknowledgement(native_wait, native_readiness),
                )
                .expect("native terminal acknowledgement"),
        );
        assert!(matches!(
            host_session.receive(host_session.consumer()),
            Err(host::session_error::session_closed)
        ));
        assert!(matches!(
            native.receive_with(native.binding(), None, |_| true, |_| {}),
            Err(session_input_error::session_closed)
        ));
    }
}
