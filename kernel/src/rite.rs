#![cfg_attr(not(test), allow(dead_code))]

//! Architecture-neutral admission for the first supervised-vessel rites.
//!
//! This module validates only an already-copied ABI prefix. It never receives a
//! user pointer and never copies or interprets payload bytes. An architecture
//! gate must recover the calling task identity from kernel state before entering
//! this layer. Operation-specific code may touch payload bytes only while
//! holding the single pending admission returned here.

use anomalous_abi::{
    id_128, rite_call, rite_display_patch_header_size, rite_display_patch_max_payload_size,
    rite_display_patch_min_payload_size, rite_exit_payload, rite_operation, rite_payload_kind,
    rite_reply, rite_rights, rite_semantic_surface_request_v1,
    rite_semantic_surface_snapshot_v1_size, rite_session_input_delivery_v1,
    rite_session_input_request_v1, rite_status,
};
use core::mem::size_of;
#[cfg(any(test, feature = "input_wait_test"))]
use core::num::NonZeroU64;

use crate::identity::task_identity;
#[cfg(any(test, feature = "input_wait_test"))]
use crate::session_input::{
    session_input_binding, session_input_blocked_wait, session_input_prepared_wait,
    session_input_readiness_identity, session_input_wait_identity,
};
use crate::task::{task_state, task_table};

const first_rite_sequence: u64 = 1;
const task_exit_payload_size: u32 = size_of::<rite_exit_payload>() as u32;
const session_input_request_size: u32 = size_of::<rite_session_input_request_v1>() as u32;
const session_input_delivery_size: u32 = size_of::<rite_session_input_delivery_v1>() as u32;
const semantic_surface_request_size: u32 = size_of::<rite_semantic_surface_request_v1>() as u32;
const semantic_surface_snapshot_size: u32 = rite_semantic_surface_snapshot_v1_size;

/// Independent kernel cap for every variable-length first-wave rite payload.
pub(crate) const max_rite_payload_size: u32 = 4 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum rite_error {
    invalid_grant,
    invalid_sequence,
    channel_closed,
    admission_pending,
    sequence_exhausted,
    stale_admission,
    invalid_completion,
    invalid_mapping_identity,
    session_input_not_pending,
    wrong_pending_correlation,
}

/// Run the one bounded byte-copy loop shared by the native mailbox reader and
/// the host source-copy fault model.  The caller supplies the byte source so
/// raw reads remain confined to architecture code.
pub(crate) fn copy_bounded_body_bytes(
    destination: &mut [u8],
    mut source: impl FnMut(usize) -> Result<u8, &'static str>,
) -> Result<(), &'static str> {
    for (index, byte) in destination.iter_mut().enumerate() {
        *byte = source(index)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum rite_channel_state {
    idle,
    admitted,
    session_input_pending,
    closed,
}

/// Rights recovered from trusted kernel state for one exact task incarnation.
///
/// The caller-supplied rights mask can narrow this grant for one request, but it
/// can never add authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct rite_grant {
    vessel: task_identity,
    rights: u64,
}

impl rite_grant {
    const fn try_new(vessel: task_identity, rights: u64) -> Result<Self, rite_error> {
        if rights & !rite_rights::known != 0 {
            return Err(rite_error::invalid_grant);
        }
        Ok(Self { vessel, rights })
    }

    fn admits(self, caller: task_identity, claimed: u64, required: u64) -> bool {
        caller == self.vessel
            && claimed & required != 0
            && claimed & !self.rights == 0
            && self.rights & required != 0
    }
}

/// Monotonic attribution sequence owned by one vessel-incarnation channel.
///
/// Rejected calls consume a sequence too. An admitted call exclusively borrows
/// its channel until completion, preventing a later attempt from overtaking it.
/// Exhaustion is terminal and never wraps to zero.
#[derive(Debug, Eq, PartialEq)]
struct rite_sequencer {
    next: u64,
    exhausted: bool,
}

impl rite_sequencer {
    const fn new() -> Self {
        Self {
            next: first_rite_sequence,
            exhausted: false,
        }
    }

    #[cfg(test)]
    const fn try_from_next(next: u64) -> Result<Self, rite_error> {
        if next == 0 {
            return Err(rite_error::invalid_sequence);
        }
        Ok(Self {
            next,
            exhausted: false,
        })
    }

    fn issue(&mut self) -> Result<u64, rite_error> {
        if self.exhausted {
            return Err(rite_error::sequence_exhausted);
        }

        let issued = self.next;
        if issued == u64::MAX {
            self.exhausted = true;
        } else {
            self.next = issued + 1;
        }
        Ok(issued)
    }
}

/// Generational identity of one task-owned Rite mailbox mapping.
///
/// This value contains no user pointer or Rust reference. Architecture code
/// must rederive the fixed virtual address and supervisor physical alias from
/// the exact live task-space owner before every copy.
#[cfg(any(test, feature = "input_wait_test"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct rite_mailbox_mapping_identity {
    vessel: task_identity,
    generation: NonZeroU64,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl rite_mailbox_mapping_identity {
    pub(crate) const fn try_new(
        vessel: task_identity,
        generation: u64,
    ) -> Result<Self, rite_error> {
        let Some(generation) = NonZeroU64::new(generation) else {
            return Err(rite_error::invalid_mapping_identity);
        };
        Ok(Self { vessel, generation })
    }

    #[must_use]
    pub(crate) const fn vessel(self) -> task_identity {
        self.vessel
    }

    #[must_use]
    pub(crate) const fn generation(self) -> u64 {
        self.generation.get()
    }
}

/// Copyable freshness view of one kernel-owned pending semantic read.
///
/// Equality proves correlation only. Completion authority remains the mutable
/// channel owner plus the exact endpoint and scheduler tokens retained by the
/// stable outer runtime.
#[cfg(any(test, feature = "input_wait_test"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct rite_session_input_pending_view {
    vessel: task_identity,
    binding: session_input_binding,
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    mapping: rite_mailbox_mapping_identity,
    sequence: u64,
    trace_id: id_128,
    reply_capacity: u32,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl rite_session_input_pending_view {
    #[must_use]
    pub(crate) const fn vessel(self) -> task_identity {
        self.vessel
    }

    #[must_use]
    pub(crate) const fn binding(self) -> session_input_binding {
        self.binding
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
    pub(crate) const fn mapping(self) -> rite_mailbox_mapping_identity {
        self.mapping
    }

    #[must_use]
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    #[must_use]
    pub(crate) const fn trace_id(self) -> id_128 {
        self.trace_id
    }

    #[must_use]
    pub(crate) const fn reply_capacity(self) -> u32 {
        self.reply_capacity
    }
}

#[cfg(any(test, feature = "input_wait_test"))]
struct rite_session_input_pending {
    blocked_wait: session_input_blocked_wait,
    mapping: rite_mailbox_mapping_identity,
    sequence: u64,
    trace_id: id_128,
    reply_capacity: u32,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl rite_session_input_pending {
    fn view(&self) -> rite_session_input_pending_view {
        rite_session_input_pending_view {
            vessel: self.blocked_wait.binding().task(),
            binding: self.blocked_wait.binding(),
            wait: self.blocked_wait.identity(),
            readiness: self.blocked_wait.readiness(),
            mapping: self.mapping,
            sequence: self.sequence,
            trace_id: self.trace_id,
            reply_capacity: self.reply_capacity,
        }
    }

    fn clear(&mut self) {
        self.sequence = 0;
        self.trace_id.bytes.fill(0);
        self.reply_capacity = 0;
    }
}

#[cfg(any(test, feature = "input_wait_test"))]
impl Drop for rite_session_input_pending {
    fn drop(&mut self) {
        self.clear();
    }
}

/// Kernel-owned rite state for one exact vessel incarnation.
///
/// A replacement incarnation receives a fresh channel, grant, and sequence. The
/// channel is intentionally neither Copy nor Clone.
pub(crate) struct rite_channel {
    grant: rite_grant,
    sequencer: rite_sequencer,
    state: rite_channel_state,
    #[cfg(any(test, feature = "input_wait_test"))]
    session_input_pending: Option<rite_session_input_pending>,
}

/// Exact logical channel state used only by failure-atomic unit tests.
#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct rite_channel_test_snapshot {
    grant: rite_grant,
    next_sequence: u64,
    sequence_exhausted: bool,
    state: rite_channel_state,
    session_input_pending: Option<rite_session_input_pending_view>,
}

impl rite_channel {
    pub(crate) const fn try_new(vessel: task_identity, rights: u64) -> Result<Self, rite_error> {
        let grant = match rite_grant::try_new(vessel, rights) {
            Ok(grant) => grant,
            Err(error) => return Err(error),
        };
        Ok(Self {
            grant,
            sequencer: rite_sequencer::new(),
            state: rite_channel_state::idle,
            #[cfg(any(test, feature = "input_wait_test"))]
            session_input_pending: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> rite_channel_test_snapshot {
        rite_channel_test_snapshot {
            grant: self.grant,
            next_sequence: self.sequencer.next,
            sequence_exhausted: self.sequencer.exhausted,
            state: self.state,
            session_input_pending: self
                .session_input_pending
                .as_ref()
                .map(rite_session_input_pending::view),
        }
    }

    /// Validate one call without reading payload bytes or producing side effects.
    ///
    /// The task table must still contain the exact caller in the running state.
    /// An admitted result holds the only mutable channel borrow and leaves a
    /// persistent pending marker. It must be completed or rejected explicitly.
    pub(crate) fn assess(
        &mut self,
        tasks: &task_table,
        caller: task_identity,
        call: rite_call,
    ) -> Result<rite_decision<'_>, rite_error> {
        match self.state {
            rite_channel_state::idle => {}
            rite_channel_state::admitted => {
                self.state = rite_channel_state::closed;
                return Err(rite_error::admission_pending);
            }
            rite_channel_state::session_input_pending => {
                return Err(rite_error::admission_pending);
            }
            rite_channel_state::closed => return Err(rite_error::channel_closed),
        }

        let sequence = self.sequencer.issue()?;
        let status = validate_call(&call, tasks, caller, self.grant);

        if status == rite_status::ok {
            self.state = rite_channel_state::admitted;
            return Ok(rite_decision::admitted(rite_admission {
                channel: self,
                vessel: caller,
                operation: call.operation,
                payload_kind: call.payload_kind,
                payload_size: call.payload_size,
                sequence,
                trace_id: call.trace_id,
                settled: false,
            }));
        }

        Ok(rite_decision::rejected(rite_reply::new(
            status,
            rite_payload_kind::none,
            0,
            sequence,
            call.trace_id,
        )))
    }

    /// Return the complete freshness correlation for one live pending read.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn session_input_pending(
        &self,
    ) -> Result<rite_session_input_pending_view, rite_error> {
        if self.state != rite_channel_state::session_input_pending {
            return Err(rite_error::session_input_not_pending);
        }
        self.session_input_pending
            .as_ref()
            .map(rite_session_input_pending::view)
            .ok_or(rite_error::session_input_not_pending)
    }

    /// Preflight settlement of one exact pending semantic-session read.
    ///
    /// The returned guard borrows the stable channel. Dropping it leaves the
    /// pending owner intact; only `commit` consumes that owner and reopens the
    /// channel after the endpoint transaction has completed.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_session_input_completion(
        &mut self,
        expected: rite_session_input_pending_view,
        outcome: rite_session_input_completion_outcome,
    ) -> Result<prepared_rite_session_input_completion<'_>, rite_error> {
        let actual = self.session_input_pending()?;
        if actual != expected {
            return Err(rite_error::wrong_pending_correlation);
        }
        let reply = match outcome {
            rite_session_input_completion_outcome::delivery => rite_reply::new(
                rite_status::ok,
                rite_payload_kind::session_input_delivery,
                session_input_delivery_size,
                actual.sequence,
                actual.trace_id,
            ),
            rite_session_input_completion_outcome::interrupted => rite_reply::new(
                rite_status::interrupted,
                rite_payload_kind::none,
                0,
                actual.sequence,
                actual.trace_id,
            ),
        };
        Ok(prepared_rite_session_input_completion {
            channel: self,
            expected,
            outcome,
            reply,
        })
    }

    pub(crate) fn close_idle(&mut self) -> Result<(), rite_error> {
        if self.state != rite_channel_state::idle {
            return Err(rite_error::invalid_completion);
        }
        #[cfg(any(test, feature = "input_wait_test"))]
        if self.session_input_pending.is_some() {
            return Err(rite_error::invalid_completion);
        }
        self.state = rite_channel_state::closed;
        Ok(())
    }

    /// Retire after the isolated owner has stopped. A dropped admission has
    /// already closed the channel; neither outcome retains a pending borrow.
    #[cfg(any(test, feature = "workspace_dev"))]
    pub(crate) fn close_retired(&mut self) -> Result<(), rite_error> {
        match self.state {
            rite_channel_state::closed => Ok(()),
            rite_channel_state::idle => self.close_idle(),
            _ => Err(rite_error::admission_pending),
        }
    }
}

#[cfg(any(test, feature = "input_wait_test"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum rite_session_input_completion_outcome {
    delivery,
    interrupted,
}

#[cfg(any(test, feature = "input_wait_test"))]
#[must_use = "dropping a prepared completion preserves the pending Rite"]
pub(crate) struct prepared_rite_session_input_completion<'a> {
    channel: &'a mut rite_channel,
    expected: rite_session_input_pending_view,
    outcome: rite_session_input_completion_outcome,
    reply: rite_reply,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_rite_session_input_completion<'_> {
    #[must_use]
    pub(crate) const fn reply(&self) -> rite_reply {
        self.reply
    }

    #[must_use]
    pub(crate) const fn outcome(&self) -> rite_session_input_completion_outcome {
        self.outcome
    }

    /// Settle after the exact scheduler selection and endpoint acknowledgement.
    pub(crate) fn commit(self) -> rite_reply {
        debug_assert!(
            self.channel.session_input_pending().ok() == Some(self.expected),
            "prepared Rite session completion correlation changed"
        );
        let pending = self
            .channel
            .session_input_pending
            .take()
            .expect("prepared pending Rite disappeared");
        drop(pending);
        self.channel.state = rite_channel_state::idle;
        self.reply
    }
}

/// Sanitized metadata for one single-pending operation-specific preflight.
///
/// This is not a completed operation or a success reply. No claimed-rights
/// mask, payload pointer, or payload bytes cross the admission boundary.
#[must_use = "an admitted rite must be completed or rejected explicitly"]
pub(crate) struct rite_admission<'a> {
    channel: &'a mut rite_channel,
    vessel: task_identity,
    operation: rite_operation,
    payload_kind: rite_payload_kind,
    payload_size: u32,
    sequence: u64,
    trace_id: id_128,
    settled: bool,
}

/// A submit completion whose exact running-task revalidation has already
/// succeeded.  The semantic-surface endpoint may commit only while this token
/// is live, so the architecture path cannot publish endpoint ownership before
/// a fallible Rite revalidation.
#[must_use = "commit or drop the prepared semantic-surface submit completion"]
pub(crate) struct prepared_semantic_surface_submit_completion<'a> {
    admission: rite_admission<'a>,
}

impl<'a> rite_admission<'a> {
    pub(crate) const fn vessel(&self) -> task_identity {
        self.vessel
    }

    pub(crate) const fn operation(&self) -> rite_operation {
        self.operation
    }

    pub(crate) const fn payload_kind(&self) -> rite_payload_kind {
        self.payload_kind
    }

    pub(crate) const fn payload_size(&self) -> u32 {
        self.payload_size
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn trace_id(&self) -> id_128 {
        self.trace_id
    }

    /// Convert a fully staged, exact semantic-session wait request into a
    /// preparation that can be committed without retaining any user pointer.
    ///
    /// The request body must already have been copied into kernel scratch. A
    /// malformed body should normally be completed through
    /// `reject_after_revalidation`; invoking this method with one is an
    /// integration error and closes the channel.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_session_input_pending(
        mut self,
        request: &rite_session_input_request_v1,
        binding: session_input_binding,
        prepared_wait: &session_input_prepared_wait,
        mapping: rite_mailbox_mapping_identity,
    ) -> Result<prepared_rite_session_input_pending<'a>, rite_error> {
        if self.operation != rite_operation::session_input_read
            || self.payload_kind != rite_payload_kind::session_input_request
            || self.payload_size != session_input_request_size
            || !request.has_supported_body()
            || !request.wait_if_empty()
            || request.reply_capacity() != session_input_delivery_size
            || binding.task() != self.vessel
            || prepared_wait.binding() != binding
            || mapping.vessel() != self.vessel
        {
            return self.close_with(rite_error::invalid_completion);
        }

        Ok(prepared_rite_session_input_pending {
            admission: self,
            binding,
            wait: prepared_wait.proposed_wait(),
            readiness: prepared_wait.proposed_readiness(),
            mapping,
            reply_capacity: request.reply_capacity(),
        })
    }

    /// Commit a successful operation whose reply carries no payload.
    ///
    /// All fallible payload validation and execution preflight must finish
    /// before this call. Input reads cannot use this path because their
    /// payload-free completion is would-block, not a committed effect. This
    /// method validates the completion shape, revalidates that the exact vessel
    /// is running, invokes one infallible bounded commit, and only then
    /// constructs the success reply.
    pub(crate) fn finish_ok<F>(
        mut self,
        tasks: &mut task_table,
        commit: F,
    ) -> Result<rite_reply, rite_error>
    where
        F: FnOnce(&mut task_table, task_identity),
    {
        if self.operation == rite_operation::input_read
            || self.operation == rite_operation::session_input_read
            || self.operation == rite_operation::semantic_surface_read
        {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        commit(tasks, self.vessel);
        Ok(self.settle_empty(rite_status::ok))
    }

    /// Complete an input read that currently has no data without committing an
    /// operation-specific effect.
    pub(crate) fn finish_would_block(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::input_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        Ok(self.settle_empty(rite_status::would_block))
    }

    #[cfg(any(test, feature = "workspace_dev"))]
    pub(crate) fn finish_physical_input_delivery(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::input_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::physical_input_delivery,
            anomalous_abi::rite_physical_input_v1_size as u32,
            self.sequence,
            self.trace_id,
        );
        self.channel.state = rite_channel_state::idle;
        self.settled = true;
        Ok(reply)
    }

    /// Complete an empty semantic-session poll without installing a wait.
    pub(crate) fn finish_session_input_would_block(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::session_input_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        Ok(self.settle_empty(rite_status::would_block))
    }

    /// Complete an empty semantic-surface poll without consuming endpoint state.
    pub(crate) fn finish_semantic_surface_would_block(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::semantic_surface_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        Ok(self.settle_empty(rite_status::would_block))
    }

    /// Report exact one-slot backpressure for a semantic-surface submission.
    pub(crate) fn finish_semantic_surface_submit_would_block(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::semantic_surface_submit {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        Ok(self.settle_empty(rite_status::would_block))
    }

    /// Revalidate the exact source task before its prepared endpoint body is
    /// committed.  No endpoint mutation occurs before this method succeeds.
    pub(crate) fn prepare_semantic_surface_submit_ok(
        mut self,
        tasks: &task_table,
    ) -> Result<prepared_semantic_surface_submit_completion<'a>, rite_error> {
        if self.operation != rite_operation::semantic_surface_submit {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }
        Ok(prepared_semantic_surface_submit_completion { admission: self })
    }

    /// Complete an authorized semantic-session read after exact route closure.
    pub(crate) fn finish_session_input_interrupted(
        mut self,
        tasks: &task_table,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::session_input_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }

        Ok(self.settle_empty(rite_status::interrupted))
    }

    /// Construct the exact nonempty reply prefix before the caller's complete
    /// body-copy/source-consume transaction.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn session_input_delivery_reply(&self) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::session_input_read {
            return Err(rite_error::invalid_completion);
        }
        Ok(rite_reply::new(
            rite_status::ok,
            rite_payload_kind::session_input_delivery,
            session_input_delivery_size,
            self.sequence,
            self.trace_id,
        ))
    }

    /// Construct the exact nonempty semantic-surface reply before copy-out.
    pub(crate) fn semantic_surface_delivery_reply(&self) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::semantic_surface_read {
            return Err(rite_error::invalid_completion);
        }
        Ok(rite_reply::new(
            rite_status::ok,
            rite_payload_kind::semantic_surface_snapshot,
            semantic_surface_snapshot_size,
            self.sequence,
            self.trace_id,
        ))
    }

    /// Construct an exact payload-free semantic-surface outcome before its
    /// fixed reply area is staged with an invalid header.
    pub(crate) fn semantic_surface_empty_reply(
        &self,
        status: rite_status,
    ) -> Result<rite_reply, rite_error> {
        if !matches!(
            self.operation,
            rite_operation::semantic_surface_submit | rite_operation::semantic_surface_read
        ) || !matches!(
            status,
            rite_status::would_block | rite_status::denied | rite_status::bad_payload
        ) {
            return Err(rite_error::invalid_completion);
        }
        Ok(rite_reply::new(
            status,
            rite_payload_kind::none,
            0,
            self.sequence,
            self.trace_id,
        ))
    }

    /// Construct a payload-free semantic-session reply before the fixed body
    /// area is cleared and its header is published last.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn session_input_empty_reply(
        &self,
        status: rite_status,
    ) -> Result<rite_reply, rite_error> {
        if self.operation != rite_operation::session_input_read
            || !matches!(status, rite_status::would_block | rite_status::interrupted)
        {
            return Err(rite_error::invalid_completion);
        }
        Ok(rite_reply::new(
            status,
            rite_payload_kind::none,
            0,
            self.sequence,
            self.trace_id,
        ))
    }

    /// Settle one synchronous semantic-session delivery after its complete
    /// copy and source commit. The callback is the infallible linearization
    /// point supplied by the serialized endpoint/transport integration.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn finish_session_input_delivery<F>(
        mut self,
        tasks: &task_table,
        commit: F,
    ) -> Result<rite_reply, rite_error>
    where
        F: FnOnce(),
    {
        if self.operation != rite_operation::session_input_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }
        commit();
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::session_input_delivery,
            session_input_delivery_size,
            self.sequence,
            self.trace_id,
        );
        self.channel.state = rite_channel_state::idle;
        self.settled = true;
        Ok(reply)
    }

    /// Settle one synchronous semantic-surface delivery after complete copy-out.
    pub(crate) fn finish_semantic_surface_delivery<F>(
        mut self,
        tasks: &task_table,
        commit: F,
    ) -> Result<rite_reply, rite_error>
    where
        F: FnOnce(),
    {
        if self.operation != rite_operation::semantic_surface_read {
            return self.close_with(rite_error::invalid_completion);
        }
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }
        commit();
        let reply = rite_reply::new(
            rite_status::ok,
            rite_payload_kind::semantic_surface_snapshot,
            semantic_surface_snapshot_size,
            self.sequence,
            self.trace_id,
        );
        self.channel.state = rite_channel_state::idle;
        self.settled = true;
        Ok(reply)
    }

    /// Return a canonical payload-free error after body copy or preflight fails.
    ///
    /// This path revalidates the exact running incarnation and cannot be used to
    /// report success or would-block.
    pub(crate) fn reject_after_revalidation(
        mut self,
        tasks: &task_table,
        status: rite_status,
    ) -> Result<rite_reply, rite_error> {
        if !task_is_running(tasks, self.vessel) {
            return self.close_with(rite_error::stale_admission);
        }
        if !rejection_status_is_valid(status) {
            return self.close_with(rite_error::invalid_completion);
        }

        Ok(self.settle_empty(status))
    }

    fn settle_empty(&mut self, status: rite_status) -> rite_reply {
        let reply = rite_reply::new(
            status,
            rite_payload_kind::none,
            0,
            self.sequence,
            self.trace_id,
        );
        self.channel.state = rite_channel_state::idle;
        self.settled = true;
        reply
    }

    fn close_with<T>(&mut self, error: rite_error) -> Result<T, rite_error> {
        self.channel.state = rite_channel_state::closed;
        self.settled = true;
        Err(error)
    }
}

impl prepared_semantic_surface_submit_completion<'_> {
    /// Construct the exact payload-free success reply before the source
    /// mailbox is staged.  This does not settle the Rite channel.
    pub(crate) const fn reply(&self) -> rite_reply {
        rite_reply::new(
            rite_status::ok,
            rite_payload_kind::none,
            0,
            self.admission.sequence,
            self.admission.trace_id,
        )
    }

    /// Linearize one already-prevalidated endpoint submit and settle its Rite
    /// channel.  The callback is deliberately infallible: all admission,
    /// body, binding, endpoint-preparation, and task-incarnation checks must
    /// have completed before it is called.
    pub(crate) fn commit<F>(mut self, commit: F) -> rite_reply
    where
        F: FnOnce(),
    {
        let reply = self.reply();
        commit();
        let settled = self.admission.settle_empty(rite_status::ok);
        debug_assert_eq!(settled, reply);
        settled
    }
}

/// Synchronous admission transformed into stable, pointer-free pending state.
///
/// The wrapper remains stack-local only until the endpoint and scheduler
/// preparations are ready. Its commit consumes the admission and stores all
/// future correlation inside the channel before any selected-frame return.
#[cfg(any(test, feature = "input_wait_test"))]
#[must_use = "commit pending state before returning another task frame"]
pub(crate) struct prepared_rite_session_input_pending<'a> {
    admission: rite_admission<'a>,
    binding: session_input_binding,
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
    mapping: rite_mailbox_mapping_identity,
    reply_capacity: u32,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_rite_session_input_pending<'_> {
    #[must_use]
    pub(crate) const fn binding(&self) -> session_input_binding {
        self.binding
    }

    #[must_use]
    pub(crate) const fn mapping(&self) -> rite_mailbox_mapping_identity {
        self.mapping
    }

    #[must_use]
    pub(crate) const fn wait(&self) -> session_input_wait_identity {
        self.wait
    }

    #[must_use]
    pub(crate) const fn readiness(&self) -> session_input_readiness_identity {
        self.readiness
    }

    /// Install the endpoint-owned exact wait into the stable channel.
    ///
    /// The exact binding, wait sequence, and readiness generation are captured
    /// from the same endpoint preparation used by the scheduler. A mismatch is
    /// a terminal kernel integration defect, not an untrusted recoverable
    /// condition after the endpoint has mutated.
    pub(crate) fn commit(mut self, blocked_wait: session_input_blocked_wait) {
        assert!(
            blocked_wait.binding() == self.binding
                && blocked_wait.identity() == self.wait
                && blocked_wait.readiness() == self.readiness,
            "pending Rite and endpoint wait correlation diverged"
        );
        assert!(
            self.admission.channel.state == rite_channel_state::admitted
                && self.admission.channel.session_input_pending.is_none(),
            "Rite channel changed during pending conversion"
        );
        self.admission.channel.session_input_pending = Some(rite_session_input_pending {
            blocked_wait,
            mapping: self.mapping,
            sequence: self.admission.sequence,
            trace_id: self.admission.trace_id,
            reply_capacity: self.reply_capacity,
        });
        self.admission.channel.state = rite_channel_state::session_input_pending;
        self.admission.settled = true;
    }
}

impl Drop for rite_admission<'_> {
    fn drop(&mut self) {
        if !self.settled {
            self.channel.state = rite_channel_state::closed;
        }
    }
}

#[must_use = "a rite decision must be rejected or completed explicitly"]
pub(crate) enum rite_decision<'a> {
    admitted(rite_admission<'a>),
    rejected(rite_reply),
}

/// Validation order is fixed: header, operation, structural metadata, caller
/// declaration, trusted grant and live incarnation, payload metadata, bounds.
fn validate_call(
    call: &rite_call,
    tasks: &task_table,
    caller: task_identity,
    grant: rite_grant,
) -> rite_status {
    if call.header_size as usize != size_of::<rite_call>() || call.reserved != 0 {
        return rite_status::bad_payload;
    }

    let Some(required_right) = call.operation.required_right() else {
        return rite_status::bad_operation;
    };

    if !call.payload_kind.is_known() || call.rights_present & !rite_rights::known != 0 {
        return rite_status::bad_payload;
    }

    if call.rights_present & required_right == 0 {
        return rite_status::denied;
    }

    if !grant.admits(caller, call.rights_present, required_right) || !task_is_running(tasks, caller)
    {
        return rite_status::denied;
    }

    if !payload_metadata_shape_is_valid(call.operation, call.payload_kind, call.payload_size) {
        return rite_status::bad_payload;
    }

    let Some(_envelope_size) = u32::from(call.header_size).checked_add(call.payload_size) else {
        return rite_status::limit_exceeded;
    };
    if call.payload_size > call.payload_limit
        || call.payload_size > operation_payload_cap(call.operation)
    {
        return rite_status::limit_exceeded;
    }
    if call.operation == rite_operation::session_input_read
        && call.payload_limit != session_input_request_size
    {
        return rite_status::bad_payload;
    }
    if matches!(
        call.operation,
        rite_operation::display_update | rite_operation::semantic_surface_submit
    ) && call.payload_limit != call.payload_size
    {
        return rite_status::bad_payload;
    }
    if call.operation == rite_operation::semantic_surface_read
        && call.payload_limit != semantic_surface_request_size
    {
        return rite_status::bad_payload;
    }

    rite_status::ok
}

fn task_is_running(tasks: &task_table, identity: task_identity) -> bool {
    matches!(
        tasks.snapshot(identity),
        Ok(snapshot) if snapshot.state() == task_state::running
    )
}

fn payload_metadata_shape_is_valid(
    operation: rite_operation,
    payload_kind: rite_payload_kind,
    payload_size: u32,
) -> bool {
    match operation.raw() {
        value if value == rite_operation::console_write.raw() => {
            payload_kind == rite_payload_kind::utf8_text && payload_size > 0
        }
        value if value == rite_operation::input_read.raw() => {
            payload_kind == rite_payload_kind::input_request && payload_size == 0
        }
        value if value == rite_operation::task_exit.raw() => {
            payload_kind == rite_payload_kind::exit_status && payload_size == task_exit_payload_size
        }
        value if value == rite_operation::task_yield.raw() => {
            payload_kind == rite_payload_kind::none && payload_size == 0
        }
        value if value == rite_operation::display_update.raw() => {
            payload_kind == rite_payload_kind::display_patch
                && payload_size >= rite_display_patch_min_payload_size
                && (payload_size - rite_display_patch_header_size).is_multiple_of(3)
        }
        value if value == rite_operation::session_input_read.raw() => {
            payload_kind == rite_payload_kind::session_input_request
                && payload_size == session_input_request_size
        }
        value if value == rite_operation::semantic_surface_submit.raw() => {
            payload_kind == rite_payload_kind::semantic_surface_snapshot
                && payload_size == semantic_surface_snapshot_size
        }
        value if value == rite_operation::semantic_surface_read.raw() => {
            payload_kind == rite_payload_kind::semantic_surface_request
                && payload_size == semantic_surface_request_size
        }
        _ => false,
    }
}

const fn operation_payload_cap(operation: rite_operation) -> u32 {
    match operation.raw() {
        value if value == rite_operation::input_read.raw() => 0,
        value if value == rite_operation::task_exit.raw() => task_exit_payload_size,
        value if value == rite_operation::task_yield.raw() => 0,
        value if value == rite_operation::display_update.raw() => {
            rite_display_patch_max_payload_size
        }
        value if value == rite_operation::session_input_read.raw() => session_input_request_size,
        value if value == rite_operation::semantic_surface_submit.raw() => {
            semantic_surface_snapshot_size
        }
        value if value == rite_operation::semantic_surface_read.raw() => {
            semantic_surface_request_size
        }
        _ => max_rite_payload_size,
    }
}

fn rejection_status_is_valid(status: rite_status) -> bool {
    status.is_known() && status != rite_status::ok && status != rite_status::would_block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::circle_table;
    use crate::identity::{fixture_principal_id, task_id};
    use crate::task::{synthetic_origin_metadata, task_binding, task_bounds};

    fn running_fixture() -> (circle_table, task_table, task_identity) {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let bounds =
            task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("bounds");
        let binding = task_binding::new(
            circle,
            fixture_principal_id::new(1).expect("principal"),
            synthetic_origin_metadata::human_role_fixture,
            bounds,
        );
        let mut tasks = task_table::new();
        let vessel = tasks.create(&mut circles, binding).expect("task");
        tasks.start(vessel).expect("running task");
        (circles, tasks, vessel)
    }

    fn stale_identity(vessel: task_identity) -> task_identity {
        task_identity::new(
            task_id::try_from_parts(vessel.task().slot(), vessel.task().generation_value() + 1)
                .expect("stale task"),
            vessel.circle(),
        )
    }

    fn trace(value: u8) -> id_128 {
        id_128::from_bytes([value; 16])
    }

    fn call_with_limit(
        operation: rite_operation,
        payload_kind: rite_payload_kind,
        rights_present: u64,
        payload_size: u32,
        payload_limit: u32,
    ) -> rite_call {
        rite_call::new(
            operation,
            payload_kind,
            rights_present,
            payload_size,
            payload_limit,
            trace(0x5a),
        )
    }

    fn call(
        operation: rite_operation,
        payload_kind: rite_payload_kind,
        rights_present: u64,
        payload_size: u32,
    ) -> rite_call {
        call_with_limit(
            operation,
            payload_kind,
            rights_present,
            payload_size,
            max_rite_payload_size,
        )
    }

    fn admitted<'a>(decision: rite_decision<'a>) -> rite_admission<'a> {
        let rite_decision::admitted(admission) = decision else {
            panic!("expected admitted rite");
        };
        admission
    }

    fn rejected(decision: rite_decision<'_>) -> rite_reply {
        let rite_decision::rejected(reply) = decision else {
            panic!("expected rejected rite");
        };
        reply
    }

    #[test]
    fn shell_console_reply_is_created_after_commit_for_the_live_incarnation() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let request = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            12,
        );

        let admission = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("sequence available"),
        );
        assert_eq!(admission.vessel(), vessel);
        assert_eq!(admission.operation(), rite_operation::console_write);
        assert_eq!(admission.payload_kind(), rite_payload_kind::utf8_text);
        assert_eq!(admission.payload_size(), 12);
        assert_eq!(admission.sequence(), first_rite_sequence);
        assert_eq!(admission.trace_id(), trace(0x5a));

        let mut committed = false;
        let reply = admission
            .finish_ok(&mut tasks, |_, exact_vessel| {
                assert_eq!(exact_vessel, vessel);
                committed = true;
            })
            .expect("live completion");
        assert!(committed);
        assert_eq!(reply.status, rite_status::ok);
        assert_eq!(reply.sequence, first_rite_sequence);
        assert_eq!(reply.trace_id, trace(0x5a));
    }

    #[test]
    fn caller_claims_and_stale_incarnations_never_expand_authority() {
        let (_circles, tasks, vessel) = running_fixture();
        let stale_vessel = stale_identity(vessel);
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");

        let display_request = call(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            16,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, display_request)
                    .expect("sequence available")
            )
            .status,
            rite_status::denied
        );

        let overclaimed_console = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write | rite_rights::display_update,
            4,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, overclaimed_console)
                    .expect("sequence available")
            )
            .status,
            rite_status::denied
        );

        let valid_console = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            4,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, stale_vessel, valid_console)
                    .expect("sequence available")
            )
            .status,
            rite_status::denied
        );
    }

    #[test]
    fn every_known_grant_subset_obeys_the_required_console_right() {
        let (_circles, mut tasks, vessel) = running_fixture();
        for granted in 0..=rite_rights::known {
            let mut channel = rite_channel::try_new(vessel, granted).expect("known subset");
            let request = call(
                rite_operation::console_write,
                rite_payload_kind::utf8_text,
                rite_rights::console_write,
                1,
            );
            let decision = channel.assess(&tasks, vessel, request).expect("sequence");
            if granted & rite_rights::console_write != 0 {
                let reply = admitted(decision)
                    .finish_ok(&mut tasks, |_, _| {})
                    .expect("authorized completion");
                assert_eq!(reply.status, rite_status::ok);
            } else {
                assert_eq!(rejected(decision).status, rite_status::denied);
            }
        }
    }

    #[test]
    fn validation_order_is_stable_and_fail_closed() {
        let (_circles, tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let base = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            4,
        );

        let mut mutation = base;
        mutation.header_size = 0;
        mutation.operation = rite_operation::from_raw(u16::MAX);
        mutation.payload_kind = rite_payload_kind::from_raw(u16::MAX);
        mutation.rights_present = 0;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::bad_payload
        );

        mutation = base;
        mutation.operation = rite_operation::from_raw(u16::MAX);
        mutation.payload_kind = rite_payload_kind::from_raw(u16::MAX);
        mutation.rights_present = 0;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::bad_operation
        );

        mutation = base;
        mutation.payload_kind = rite_payload_kind::from_raw(u16::MAX);
        mutation.rights_present = 0;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::bad_payload
        );

        mutation = base;
        mutation.rights_present = 0;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::denied
        );

        mutation = base;
        mutation.payload_kind = rite_payload_kind::exit_status;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::bad_payload
        );

        mutation = base;
        mutation.payload_size = u32::MAX;
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, mutation).expect("sequence")).status,
            rite_status::limit_exceeded
        );
    }

    #[test]
    fn initial_operations_have_explicit_payload_metadata_shapes() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let mut channel = rite_channel::try_new(vessel, rite_rights::known).expect("valid grant");

        let console = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            1,
        );
        admitted(channel.assess(&tasks, vessel, console).expect("console"))
            .finish_ok(&mut tasks, |_, _| {})
            .expect("console completion");

        let input = call_with_limit(
            rite_operation::input_read,
            rite_payload_kind::input_request,
            rite_rights::input_read,
            0,
            0,
        );
        admitted(channel.assess(&tasks, vessel, input).expect("input"))
            .finish_would_block(&tasks)
            .expect("input completion");

        let task_yield = call_with_limit(
            rite_operation::task_yield,
            rite_payload_kind::none,
            rite_rights::task_yield,
            0,
            0,
        );
        admitted(channel.assess(&tasks, vessel, task_yield).expect("yield"))
            .finish_ok(&mut tasks, |_, _| {})
            .expect("yield completion");

        let display = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size,
        );
        admitted(channel.assess(&tasks, vessel, display).expect("display"))
            .reject_after_revalidation(&tasks, rite_status::bad_payload)
            .expect("body rejection");

        let structured_console = call(
            rite_operation::console_write,
            rite_payload_kind::structured_event,
            rite_rights::console_write,
            1,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, structured_console)
                    .expect("sequence")
            )
            .status,
            rite_status::bad_payload
        );

        let (_exit_circles, mut exit_tasks, exit_vessel) = running_fixture();
        let mut exit_channel =
            rite_channel::try_new(exit_vessel, rite_rights::task_exit).expect("exit grant");
        let exit = call_with_limit(
            rite_operation::task_exit,
            rite_payload_kind::exit_status,
            rite_rights::task_exit,
            task_exit_payload_size,
            task_exit_payload_size,
        );
        let exit_reply = admitted(
            exit_channel
                .assess(&exit_tasks, exit_vessel, exit)
                .expect("exit"),
        )
        .finish_ok(&mut exit_tasks, |tasks, exact_vessel| {
            tasks.exit(exact_vessel).expect("task exit commit");
        })
        .expect("exit completion");
        assert_eq!(exit_reply.status, rite_status::ok);
        assert_eq!(
            exit_tasks
                .snapshot(exit_vessel)
                .expect("exited task")
                .state(),
            task_state::exited
        );
    }

    #[test]
    fn copied_exit_body_failure_uses_the_admission_sequence() {
        let (_circles, tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::task_exit).expect("exit grant");
        let exit = call_with_limit(
            rite_operation::task_exit,
            rite_payload_kind::exit_status,
            rite_rights::task_exit,
            task_exit_payload_size,
            task_exit_payload_size,
        );
        let invalid_body = rite_exit_payload {
            code: 0,
            reserved: 1,
        };
        assert!(!invalid_body.has_supported_body());

        let reply = admitted(
            channel
                .assess(&tasks, vessel, exit)
                .expect("exit admission"),
        )
        .reject_after_revalidation(&tasks, rite_status::bad_payload)
        .expect("body rejection");
        assert_eq!(reply.status, rite_status::bad_payload);
        assert_eq!(reply.sequence, first_rite_sequence);
        assert_eq!(reply.payload_kind, rite_payload_kind::none);
        assert_eq!(reply.payload_size, 0);
    }

    #[test]
    fn rejected_admitted_and_rejected_attempts_stay_in_sequence() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");

        let denied = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            0,
            1,
        );
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, denied).expect("sequence")).sequence,
            1
        );

        let accepted = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            1,
        );
        let completion = admitted(channel.assess(&tasks, vessel, accepted).expect("sequence"))
            .finish_ok(&mut tasks, |_, _| {})
            .expect("completion");
        assert_eq!(completion.sequence, 2);

        let malformed = call(
            rite_operation::console_write,
            rite_payload_kind::exit_status,
            rite_rights::console_write,
            1,
        );
        assert_eq!(
            rejected(channel.assess(&tasks, vessel, malformed).expect("sequence")).sequence,
            3
        );
    }

    #[test]
    fn caller_limit_never_replaces_the_kernel_payload_cap() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");

        let boundary = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            max_rite_payload_size,
        );
        let boundary_reply = admitted(channel.assess(&tasks, vessel, boundary).expect("sequence"))
            .finish_ok(&mut tasks, |_, _| {})
            .expect("boundary completion");
        assert_eq!(boundary_reply.status, rite_status::ok);

        let oversized = call_with_limit(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            max_rite_payload_size + 1,
            u32::MAX,
        );
        let oversized_reply = rejected(
            channel
                .assess(&tasks, vessel, oversized)
                .expect("sequence available"),
        );
        assert_eq!(oversized_reply.status, rite_status::limit_exceeded);
        assert_eq!(oversized_reply.payload_kind, rite_payload_kind::none);
        assert_eq!(oversized_reply.payload_size, 0);
        assert_eq!(oversized_reply.sequence, 2);
        assert_eq!(oversized_reply.trace_id, trace(0x5a));
        assert!(oversized_reply.has_supported_header());

        let caller_overflow = call_with_limit(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            4,
            3,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, caller_overflow)
                    .expect("sequence available")
            )
            .status,
            rite_status::limit_exceeded
        );
    }

    #[test]
    fn display_admission_requires_exact_rgb8_envelope_metadata() {
        let (_circles, tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::display_update).expect("display grant");

        for payload_size in [
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size + 3,
            rite_display_patch_max_payload_size - 2,
        ] {
            let request = call_with_limit(
                rite_operation::display_update,
                rite_payload_kind::display_patch,
                rite_rights::display_update,
                payload_size,
                payload_size,
            );
            admitted(channel.assess(&tasks, vessel, request).expect("sequence"))
                .reject_after_revalidation(&tasks, rite_status::bad_payload)
                .expect("body rejection");
        }

        for payload_size in [
            0,
            rite_display_patch_header_size,
            rite_display_patch_min_payload_size + 1,
            rite_display_patch_min_payload_size + 2,
            rite_display_patch_max_payload_size - 1,
            rite_display_patch_max_payload_size,
        ] {
            let request = call_with_limit(
                rite_operation::display_update,
                rite_payload_kind::display_patch,
                rite_rights::display_update,
                payload_size,
                payload_size,
            );
            assert_eq!(
                rejected(channel.assess(&tasks, vessel, request).expect("sequence")).status,
                rite_status::bad_payload
            );
        }

        let crossed_kind = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::utf8_text,
            rite_rights::display_update,
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, crossed_kind)
                    .expect("sequence")
            )
            .status,
            rite_status::bad_payload
        );
    }

    #[test]
    fn display_admission_enforces_exact_caller_limit_and_kernel_cap() {
        let (_circles, tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::display_update).expect("display grant");

        let too_small_limit = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size - 1,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, too_small_limit)
                    .expect("sequence")
            )
            .status,
            rite_status::limit_exceeded
        );

        let too_large_limit = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size + 1,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, too_large_limit)
                    .expect("sequence")
            )
            .status,
            rite_status::bad_payload
        );

        let over_kernel_cap = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_max_payload_size + 1,
            rite_display_patch_max_payload_size + 1,
        );
        assert_eq!(
            rejected(
                channel
                    .assess(&tasks, vessel, over_kernel_cap)
                    .expect("sequence")
            )
            .status,
            rite_status::limit_exceeded
        );
    }

    #[test]
    fn display_error_precedence_stays_fail_closed_for_combined_mutations() {
        let (_circles, tasks, vessel) = running_fixture();
        let stale_vessel = stale_identity(vessel);
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::display_update).expect("display grant");

        let over_cap_size = rite_display_patch_max_payload_size + 1;
        assert_eq!((over_cap_size - rite_display_patch_header_size) % 3, 0);

        let mut invalid_header = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            0,
            over_cap_size,
            over_cap_size + 1,
        );
        invalid_header.header_size = 0;

        let unknown_operation = call_with_limit(
            rite_operation::from_raw(u16::MAX),
            rite_payload_kind::from_raw(u16::MAX),
            0,
            over_cap_size,
            over_cap_size + 1,
        );
        let unknown_kind = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::from_raw(u16::MAX),
            0,
            over_cap_size,
            over_cap_size + 1,
        );
        let denied = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            0,
            over_cap_size,
            over_cap_size + 1,
        );
        let stale = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            over_cap_size,
            over_cap_size + 1,
        );
        let malformed_shape = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_header_size,
            u32::MAX,
        );
        let over_cap = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            over_cap_size,
            over_cap_size + 1,
        );
        let unequal_limit = call_with_limit(
            rite_operation::display_update,
            rite_payload_kind::display_patch,
            rite_rights::display_update,
            rite_display_patch_min_payload_size,
            rite_display_patch_min_payload_size + 1,
        );

        for (caller, request, expected) in [
            (vessel, invalid_header, rite_status::bad_payload),
            (vessel, unknown_operation, rite_status::bad_operation),
            (vessel, unknown_kind, rite_status::bad_payload),
            (vessel, denied, rite_status::denied),
            (stale_vessel, stale, rite_status::denied),
            (vessel, malformed_shape, rite_status::bad_payload),
            (vessel, over_cap, rite_status::limit_exceeded),
            (vessel, unequal_limit, rite_status::bad_payload),
        ] {
            assert_eq!(
                rejected(channel.assess(&tasks, caller, request).expect("sequence")).status,
                expected
            );
        }
    }

    #[test]
    fn every_known_grant_subset_obeys_the_display_update_right() {
        let (_circles, tasks, vessel) = running_fixture();
        for granted in 0..=rite_rights::known {
            let mut channel = rite_channel::try_new(vessel, granted).expect("known subset");
            let request = call_with_limit(
                rite_operation::display_update,
                rite_payload_kind::display_patch,
                rite_rights::display_update,
                rite_display_patch_min_payload_size,
                rite_display_patch_min_payload_size,
            );
            let decision = channel.assess(&tasks, vessel, request).expect("sequence");
            if granted & rite_rights::display_update != 0 {
                admitted(decision)
                    .reject_after_revalidation(&tasks, rite_status::bad_payload)
                    .expect("authorized body rejection");
            } else {
                assert_eq!(rejected(decision).status, rite_status::denied);
            }
        }
    }

    #[test]
    fn admitted_max_sequence_completes_once_then_never_wraps() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let grant = rite_grant::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let mut channel = rite_channel {
            grant,
            sequencer: rite_sequencer::try_from_next(u64::MAX).expect("nonzero sequence"),
            state: rite_channel_state::idle,
            session_input_pending: None,
        };
        let request = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            1,
        );

        let reply = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("last sequence remains available"),
        )
        .finish_ok(&mut tasks, |_, _| {})
        .expect("last completion");
        assert_eq!(reply.sequence, u64::MAX);
        assert!(matches!(
            channel.assess(&tasks, vessel, request),
            Err(rite_error::sequence_exhausted)
        ));
    }

    #[test]
    fn dropped_forgotten_or_stale_admission_is_terminal() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let request = call(
            rite_operation::console_write,
            rite_payload_kind::utf8_text,
            rite_rights::console_write,
            1,
        );

        let mut dropped_channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let admission = admitted(
            dropped_channel
                .assess(&tasks, vessel, request)
                .expect("sequence available"),
        );
        drop(admission);
        assert!(matches!(
            dropped_channel.assess(&tasks, vessel, request),
            Err(rite_error::channel_closed)
        ));

        let mut forgotten_channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let admission = admitted(
            forgotten_channel
                .assess(&tasks, vessel, request)
                .expect("sequence available"),
        );
        core::mem::forget(admission);
        assert!(matches!(
            forgotten_channel.assess(&tasks, vessel, request),
            Err(rite_error::admission_pending)
        ));
        assert!(matches!(
            forgotten_channel.assess(&tasks, vessel, request),
            Err(rite_error::channel_closed)
        ));

        let mut stale_channel =
            rite_channel::try_new(vessel, rite_rights::shell_minimum).expect("valid grant");
        let admission = admitted(
            stale_channel
                .assess(&tasks, vessel, request)
                .expect("sequence available"),
        );
        tasks.exit(vessel).expect("task exits before commit");
        assert_eq!(
            admission.finish_ok(&mut tasks, |_, _| {}),
            Err(rite_error::stale_admission)
        );
        assert!(matches!(
            stale_channel.assess(&tasks, vessel, request),
            Err(rite_error::channel_closed)
        ));
    }

    #[test]
    fn ready_task_invalid_completion_and_zero_sequence_fail_closed() {
        assert_eq!(
            rite_sequencer::try_from_next(0),
            Err(rite_error::invalid_sequence)
        );

        let (mut circles, mut tasks, _) = running_fixture();
        let circle = circles.create().expect("ready circle");
        let bounds =
            task_bounds::new(0x5000, 0x6000, 0x5000, 0x7000, 0x8000, 0x8000).expect("bounds");
        let binding = task_binding::new(
            circle,
            fixture_principal_id::new(2).expect("principal"),
            synthetic_origin_metadata::machine_role_fixture,
            bounds,
        );
        let ready_vessel = tasks.create(&mut circles, binding).expect("ready task");
        let input_request = call_with_limit(
            rite_operation::input_read,
            rite_payload_kind::input_request,
            rite_rights::input_read,
            0,
            0,
        );
        let mut ready_channel =
            rite_channel::try_new(ready_vessel, rite_rights::input_read).expect("grant");
        assert_eq!(
            rejected(
                ready_channel
                    .assess(&tasks, ready_vessel, input_request)
                    .expect("sequence")
            )
            .status,
            rite_status::denied
        );

        let (_other_circles, mut other_tasks, other_vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(other_vessel, rite_rights::input_read).expect("grant");
        let admission = admitted(
            channel
                .assess(&other_tasks, other_vessel, input_request)
                .expect("sequence"),
        );
        let mut committed = false;
        assert_eq!(
            admission.finish_ok(&mut other_tasks, |_, _| committed = true),
            Err(rite_error::invalid_completion)
        );
        assert!(!committed);
        assert!(matches!(
            channel.assess(&other_tasks, other_vessel, input_request),
            Err(rite_error::channel_closed)
        ));
    }

    #[test]
    fn semantic_surface_outer_shapes_and_completions_are_exact() {
        let (_circles, tasks, vessel) = running_fixture();
        let submit = call_with_limit(
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            semantic_surface_snapshot_size,
            semantic_surface_snapshot_size,
        );
        let mut submit_channel =
            rite_channel::try_new(vessel, rite_rights::semantic_surface_submit).expect("grant");
        let mut submitted = false;
        let completion = admitted(
            submit_channel
                .assess(&tasks, vessel, submit)
                .expect("submit admission"),
        )
        .prepare_semantic_surface_submit_ok(&tasks)
        .expect("submit revalidation");
        let submitted_reply = completion.commit(|| submitted = true);
        assert!(submitted);
        assert_eq!(submitted_reply.status, rite_status::ok);
        let full = admitted(
            submit_channel
                .assess(&tasks, vessel, submit)
                .expect("full submit admission"),
        )
        .finish_semantic_surface_submit_would_block(&tasks)
        .expect("full submit completion");
        assert_eq!(full.status, rite_status::would_block);
        assert_eq!(full.payload_kind, rite_payload_kind::none);
        assert_eq!(full.payload_size, 0);

        let read = call_with_limit(
            rite_operation::semantic_surface_read,
            rite_payload_kind::semantic_surface_request,
            rite_rights::semantic_surface_read,
            semantic_surface_request_size,
            semantic_surface_request_size,
        );
        let mut read_channel =
            rite_channel::try_new(vessel, rite_rights::semantic_surface_read).expect("grant");
        let admission = admitted(
            read_channel
                .assess(&tasks, vessel, read)
                .expect("read admission"),
        );
        let prepared_reply = admission
            .semantic_surface_delivery_reply()
            .expect("delivery reply");
        assert_eq!(prepared_reply.status, rite_status::ok);
        assert_eq!(
            prepared_reply.payload_kind,
            rite_payload_kind::semantic_surface_snapshot
        );
        assert_eq!(prepared_reply.payload_size, semantic_surface_snapshot_size);
        let mut consumed = false;
        let committed_reply = admission
            .finish_semantic_surface_delivery(&tasks, || consumed = true)
            .expect("delivery completion");
        assert!(consumed);
        assert_eq!(committed_reply, prepared_reply);

        let empty = admitted(
            read_channel
                .assess(&tasks, vessel, read)
                .expect("empty read admission"),
        )
        .finish_semantic_surface_would_block(&tasks)
        .expect("empty poll completion");
        assert_eq!(empty.status, rite_status::would_block);
        assert_eq!(empty.payload_kind, rite_payload_kind::none);
        assert_eq!(empty.payload_size, 0);

        for malformed in [
            call_with_limit(
                rite_operation::semantic_surface_submit,
                rite_payload_kind::semantic_surface_request,
                rite_rights::semantic_surface_submit,
                semantic_surface_snapshot_size,
                semantic_surface_snapshot_size,
            ),
            call_with_limit(
                rite_operation::semantic_surface_submit,
                rite_payload_kind::semantic_surface_snapshot,
                rite_rights::semantic_surface_submit,
                semantic_surface_snapshot_size - 1,
                semantic_surface_snapshot_size,
            ),
            call_with_limit(
                rite_operation::semantic_surface_submit,
                rite_payload_kind::semantic_surface_snapshot,
                rite_rights::semantic_surface_submit,
                semantic_surface_snapshot_size,
                semantic_surface_snapshot_size + 1,
            ),
            call_with_limit(
                rite_operation::semantic_surface_read,
                rite_payload_kind::semantic_surface_snapshot,
                rite_rights::semantic_surface_read,
                semantic_surface_request_size,
                semantic_surface_request_size,
            ),
            call_with_limit(
                rite_operation::semantic_surface_read,
                rite_payload_kind::semantic_surface_request,
                rite_rights::semantic_surface_read,
                semantic_surface_request_size + 1,
                semantic_surface_request_size + 1,
            ),
        ] {
            let grant = malformed.rights_present;
            let mut channel = rite_channel::try_new(vessel, grant).expect("exact grant");
            assert_eq!(
                rejected(channel.assess(&tasks, vessel, malformed).expect("sequence")).status,
                rite_status::bad_payload
            );
        }
    }

    #[test]
    fn semantic_surface_submit_revalidates_before_endpoint_commit() {
        let (_circles, mut tasks, vessel) = running_fixture();
        let request = call_with_limit(
            rite_operation::semantic_surface_submit,
            rite_payload_kind::semantic_surface_snapshot,
            rite_rights::semantic_surface_submit,
            semantic_surface_snapshot_size,
            semantic_surface_snapshot_size,
        );
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::semantic_surface_submit).expect("grant");
        let admission = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("submit admission"),
        );
        tasks.exit(vessel).expect("source exits before commit");

        assert!(matches!(
            admission.prepare_semantic_surface_submit_ok(&tasks),
            Err(rite_error::stale_admission)
        ));
        assert!(matches!(
            channel.assess(&tasks, vessel, request),
            Err(rite_error::channel_closed)
        ));
    }

    #[test]
    fn session_input_outer_shape_and_error_precedence_are_exact() {
        let (_circles, tasks, vessel) = running_fixture();
        let valid = call_with_limit(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            session_input_request_size,
            session_input_request_size,
        );

        let mut accepted_channel =
            rite_channel::try_new(vessel, rite_rights::session_input_read).expect("grant");
        admitted(
            accepted_channel
                .assess(&tasks, vessel, valid)
                .expect("valid outer request"),
        )
        .finish_session_input_would_block(&tasks)
        .expect("empty poll completion");

        for (mutated, expected) in [
            (
                call_with_limit(
                    rite_operation::session_input_read,
                    rite_payload_kind::input_request,
                    rite_rights::session_input_read,
                    session_input_request_size,
                    session_input_request_size,
                ),
                rite_status::bad_payload,
            ),
            (
                call_with_limit(
                    rite_operation::session_input_read,
                    rite_payload_kind::session_input_request,
                    rite_rights::session_input_read,
                    session_input_request_size - 1,
                    session_input_request_size,
                ),
                rite_status::bad_payload,
            ),
            (
                call_with_limit(
                    rite_operation::session_input_read,
                    rite_payload_kind::session_input_request,
                    rite_rights::session_input_read,
                    session_input_request_size,
                    session_input_request_size + 1,
                ),
                rite_status::bad_payload,
            ),
            (
                call_with_limit(
                    rite_operation::session_input_read,
                    rite_payload_kind::session_input_request,
                    rite_rights::session_input_read,
                    session_input_request_size,
                    session_input_request_size - 1,
                ),
                rite_status::limit_exceeded,
            ),
        ] {
            let mut channel =
                rite_channel::try_new(vessel, rite_rights::session_input_read).expect("grant");
            assert_eq!(
                rejected(
                    channel
                        .assess(&tasks, vessel, mutated)
                        .expect("rejection sequence")
                )
                .status,
                expected
            );
        }

        let mut denied_channel = rite_channel::try_new(vessel, 0).expect("empty grant");
        let mut malformed_denied = valid;
        malformed_denied.payload_kind = rite_payload_kind::from_raw(u16::MAX);
        malformed_denied.payload_size = u32::MAX;
        assert_eq!(
            rejected(
                denied_channel
                    .assess(&tasks, vessel, malformed_denied)
                    .expect("denied sequence")
            )
            .status,
            rite_status::bad_payload,
            "structural metadata precedes uniform authority denial"
        );
        let mut denied = valid;
        denied.rights_present = 0;
        assert_eq!(
            rejected(
                denied_channel
                    .assess(&tasks, vessel, denied)
                    .expect("denied sequence")
            )
            .status,
            rite_status::denied
        );
    }

    #[test]
    fn session_input_admission_becomes_owned_exact_pending_state() {
        use crate::session_input::{
            session_input_consumer_identity, session_input_endpoint,
            session_input_endpoint_identity, session_input_receive_preparation,
            session_input_route_epoch, session_input_session_identity, session_input_wait_commit,
        };

        let (_circles, tasks, vessel) = running_fixture();
        let binding = session_input_binding::new(
            session_input_endpoint_identity::try_new(0, 1).expect("endpoint"),
            session_input_session_identity::try_new(1, 1).expect("session"),
            session_input_consumer_identity::try_new(1, 1).expect("consumer"),
            session_input_route_epoch::try_new(1).expect("route"),
            vessel,
        );
        let mapping = rite_mailbox_mapping_identity::try_new(vessel, 1).expect("mapping");
        let request_body = rite_session_input_request_v1::new(true);
        let request = call_with_limit(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            session_input_request_size,
            session_input_request_size,
        );
        let mut endpoint = session_input_endpoint::new(binding);
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::session_input_read).expect("channel");

        let admission = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("outer admission"),
        );
        let wait_preparation = match endpoint.prepare_receive(binding).expect("receive prepare") {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("unexpected input"),
        };
        let expected_wait = wait_preparation.proposed_wait();
        let expected_readiness = wait_preparation.proposed_readiness();
        let pending_preparation = admission
            .prepare_session_input_pending(&request_body, binding, &wait_preparation, mapping)
            .expect("pending preparation");
        assert!(pending_preparation.wait() == expected_wait);
        assert!(pending_preparation.readiness() == expected_readiness);
        let blocked_wait = match endpoint.commit_wait(wait_preparation).expect("wait commit") {
            session_input_wait_commit::blocked(blocked) => blocked,
            session_input_wait_commit::ready => panic!("unexpected input"),
        };
        pending_preparation.commit(blocked_wait);

        let pending = channel.session_input_pending().expect("owned pending read");
        assert!(pending.vessel() == vessel);
        assert!(pending.binding() == binding);
        assert!(pending.wait().binding() == binding);
        assert!(pending.readiness().binding() == binding);
        assert!(pending.mapping() == mapping);
        assert_eq!(pending.mapping().generation(), 1);
        assert_eq!(pending.sequence(), 1);
        assert_eq!(pending.trace_id(), trace(0x5a));
        assert_eq!(pending.reply_capacity(), session_input_delivery_size);

        assert!(matches!(
            channel.assess(&tasks, vessel, request),
            Err(rite_error::admission_pending)
        ));
        assert!(channel.session_input_pending().is_ok());

        let mut wrong = pending;
        wrong.sequence = 2;
        assert!(matches!(
            channel.prepare_session_input_completion(
                wrong,
                rite_session_input_completion_outcome::delivery,
            ),
            Err(rite_error::wrong_pending_correlation)
        ));
        let prepared = channel
            .prepare_session_input_completion(
                pending,
                rite_session_input_completion_outcome::delivery,
            )
            .expect("completion preparation");
        assert_eq!(prepared.reply().status, rite_status::ok);
        assert_eq!(
            prepared.reply().payload_kind,
            rite_payload_kind::session_input_delivery
        );
        assert_eq!(prepared.reply().payload_size, session_input_delivery_size);
        drop(prepared);
        assert!(channel.session_input_pending().is_ok());

        let reply = channel
            .prepare_session_input_completion(
                pending,
                rite_session_input_completion_outcome::delivery,
            )
            .expect("completion preparation")
            .commit();
        assert_eq!(reply.sequence, 1);
        assert_eq!(reply.trace_id, trace(0x5a));
        assert!(matches!(
            channel.session_input_pending(),
            Err(rite_error::session_input_not_pending)
        ));
        channel.close_idle().expect("idle retirement");
        assert!(matches!(
            channel.assess(&tasks, vessel, request),
            Err(rite_error::channel_closed)
        ));
    }

    #[test]
    #[should_panic(expected = "pending Rite and endpoint wait correlation diverged")]
    fn pending_session_input_rejects_a_later_same_binding_wait() {
        use crate::session_input::{
            session_input_action, session_input_consumer_identity, session_input_endpoint,
            session_input_endpoint_identity, session_input_origin,
            session_input_receive_preparation, session_input_route_epoch,
            session_input_session_identity, session_input_wait_acknowledgement,
            session_input_wait_commit,
        };

        let (_circles, tasks, vessel) = running_fixture();
        let binding = session_input_binding::new(
            session_input_endpoint_identity::try_new(0, 1).expect("endpoint"),
            session_input_session_identity::try_new(1, 1).expect("session"),
            session_input_consumer_identity::try_new(1, 1).expect("consumer"),
            session_input_route_epoch::try_new(1).expect("route"),
            vessel,
        );
        let mapping = rite_mailbox_mapping_identity::try_new(vessel, 1).expect("mapping");
        let request_body = rite_session_input_request_v1::new(true);
        let request = call_with_limit(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            session_input_request_size,
            session_input_request_size,
        );
        let mut endpoint = session_input_endpoint::new(binding);
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::session_input_read).expect("channel");

        let first_preparation = match endpoint.prepare_receive(binding).expect("first prepare") {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("unexpected first input"),
        };
        let first_wait = first_preparation.proposed_wait();
        let first_readiness = first_preparation.proposed_readiness();
        let admission = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("outer admission"),
        );
        let pending_preparation = admission
            .prepare_session_input_pending(&request_body, binding, &first_preparation, mapping)
            .expect("first pending preparation");
        let _first_blocked = match endpoint
            .commit_wait(first_preparation)
            .expect("first wait commit")
        {
            session_input_wait_commit::blocked(blocked) => blocked,
            session_input_wait_commit::ready => panic!("unexpected first ready"),
        };
        let publication = endpoint
            .prepare_publication(
                binding,
                session_input_origin::human_text,
                session_input_action::try_insert_text(b"advance").expect("valid private action"),
            )
            .unwrap_or_else(|rejection| {
                panic!("publication preparation: {}", rejection.error().code())
            });
        let publication = endpoint
            .commit_publication(publication)
            .unwrap_or_else(|rejection| panic!("publication commit: {}", rejection.error().code()));
        drop(publication.into_wake());
        let first_acknowledgement =
            session_input_wait_acknowledgement::new(first_wait, first_readiness);
        drop(
            endpoint
                .receive_with(binding, Some(&first_acknowledgement), |_| true, |_| {})
                .expect("complete first wait"),
        );
        let second_preparation = match endpoint.prepare_receive(binding).expect("second prepare") {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("unexpected second input"),
        };
        let second_wait = second_preparation.proposed_wait();
        let second_readiness = second_preparation.proposed_readiness();
        assert!(second_wait.sequence() == first_wait.sequence() + 1);
        assert!(second_readiness.generation() == first_readiness.generation() + 1);
        let second_blocked = match endpoint
            .commit_wait(second_preparation)
            .expect("second wait commit")
        {
            session_input_wait_commit::blocked(blocked) => blocked,
            session_input_wait_commit::ready => panic!("unexpected second ready"),
        };

        pending_preparation.commit(second_blocked);
    }

    #[test]
    fn malformed_session_body_is_rejected_before_pending_conversion() {
        let (_circles, tasks, vessel) = running_fixture();
        let mut channel =
            rite_channel::try_new(vessel, rite_rights::session_input_read).expect("channel");
        let request = call_with_limit(
            rite_operation::session_input_read,
            rite_payload_kind::session_input_request,
            rite_rights::session_input_read,
            session_input_request_size,
            session_input_request_size,
        );
        let malformed = rite_session_input_request_v1::from_raw_parts(16, 1, 1 << 31, 280, 0);
        assert!(!malformed.has_supported_body());
        let reply = admitted(
            channel
                .assess(&tasks, vessel, request)
                .expect("outer admission"),
        )
        .reject_after_revalidation(&tasks, rite_status::bad_payload)
        .expect("body rejection");
        assert_eq!(reply.status, rite_status::bad_payload);
        assert!(matches!(
            channel.session_input_pending(),
            Err(rite_error::session_input_not_pending)
        ));
    }

    #[test]
    fn unknown_kernel_grants_are_rejected_at_construction() {
        let (_circles, _tasks, vessel) = running_fixture();
        assert!(matches!(
            rite_channel::try_new(vessel, rite_rights::known | (1 << 63)),
            Err(rite_error::invalid_grant)
        ));
        assert!(rite_channel::try_new(vessel, 0).is_ok());
    }
}
