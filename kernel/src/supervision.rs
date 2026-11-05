#![cfg_attr(not(test), allow(dead_code))]

//! One bounded, architecture-neutral supervision relationship.
//!
//! A `supervision_id` attributes a kernel-owned relationship. It is not
//! authority. Every transition also checks the exact supervisor, the exact
//! vessel incarnation, their current task states, the relationship phase, and
//! the single observation slot. Scheduler-facing transitions use prepared
//! guards: preparation mutates nothing and holds an exclusive borrow until an
//! infallible commit, so an independent scheduler preflight may fail without
//! partially changing this table.

use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use crate::circle::{circle_child_kind, circle_error, circle_table};
use crate::identity::{circle_id, next_generation, supervision_id, task_id, task_identity};
use crate::task::{task_error, task_state, task_table};

/// ADR 0018 deliberately proves exactly one live relationship.
pub(crate) const supervision_capacity: usize = 1;

const restart_budget: u8 = 1;
const first_sequence: u64 = 1;
const inactive_record: u8 = 0;
const active_record: u8 = 1;
const owner_detached: u8 = 0;
const owner_attached: u8 = 1;
const sequence_available: u8 = 0;
const sequence_exhausted: u8 = 1;
const no_phase: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum supervision_error {
    invalid_identity,
    stale_identity,
    capacity_exhausted,
    generation_exhausted,
    corrupt_record,
    same_task,
    same_circle,
    wrong_supervisor,
    wrong_incarnation,
    invalid_phase,
    invalid_supervisor_state,
    invalid_incarnation_state,
    context_missing,
    context_still_attached,
    observation_pending,
    observation_missing,
    wrong_outcome,
    event_sequence_exhausted,
    restart_sequence_exhausted,
    restart_budget_exhausted,
    retired_incarnation_live,
    retired_circle_live,
    replacement_not_fresh,
    circle_accounting_mismatch,
    record_clearing_failed,
    task(task_error),
    circle(circle_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum supervision_phase {
    active_incarnation = 1,
    waiting_for_fault = 2,
    fault_observation_pending = 3,
    restart_authorized = 4,
    waiting_for_completion = 5,
    replacing_incarnation = 6,
    replacement_active = 7,
    completion_observation_pending = 8,
    completion_observed = 9,
    supervisor_completed = 10,
    incarnation_withdrawn = 11,
    supervisor_withdrawn = 12,
    waiting_for_initial_completion = 13,
}

impl supervision_phase {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::active_incarnation => "active_incarnation",
            Self::waiting_for_fault => "waiting_for_fault",
            Self::fault_observation_pending => "fault_observation_pending",
            Self::restart_authorized => "restart_authorized",
            Self::waiting_for_completion => "waiting_for_completion",
            Self::replacing_incarnation => "replacing_incarnation",
            Self::replacement_active => "replacement_active",
            Self::completion_observation_pending => "completion_observation_pending",
            Self::completion_observed => "completion_observed",
            Self::supervisor_completed => "supervisor_completed",
            Self::incarnation_withdrawn => "incarnation_withdrawn",
            Self::supervisor_withdrawn => "supervisor_withdrawn",
            Self::waiting_for_initial_completion => "waiting_for_initial_completion",
        }
    }

    const fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::active_incarnation),
            2 => Some(Self::waiting_for_fault),
            3 => Some(Self::fault_observation_pending),
            4 => Some(Self::restart_authorized),
            5 => Some(Self::waiting_for_completion),
            6 => Some(Self::replacing_incarnation),
            7 => Some(Self::replacement_active),
            8 => Some(Self::completion_observation_pending),
            9 => Some(Self::completion_observed),
            10 => Some(Self::supervisor_completed),
            11 => Some(Self::incarnation_withdrawn),
            12 => Some(Self::supervisor_withdrawn),
            13 => Some(Self::waiting_for_initial_completion),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum supervision_outcome {
    invalid_instruction_fault = 1,
    successful_exit = 2,
}

impl supervision_outcome {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::invalid_instruction_fault => "invalid_instruction_fault",
            Self::successful_exit => "successful_exit",
        }
    }

    const fn decode(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::invalid_instruction_fault),
            2 => Some(Self::successful_exit),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct supervision_observation {
    event_sequence: u64,
    incarnation: task_identity,
    outcome: supervision_outcome,
}

impl supervision_observation {
    pub(crate) const fn event_sequence(self) -> u64 {
        self.event_sequence
    }

    pub(crate) const fn incarnation(self) -> task_identity {
        self.incarnation
    }

    pub(crate) const fn outcome(self) -> supervision_outcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct supervision_replacement {
    restart_sequence: u64,
    retired_incarnation: task_identity,
}

impl supervision_replacement {
    pub(crate) const fn restart_sequence(self) -> u64 {
        self.restart_sequence
    }

    pub(crate) const fn retired_incarnation(self) -> task_identity {
        self.retired_incarnation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct supervision_snapshot {
    identity: supervision_id,
    supervisor: task_identity,
    active_incarnation: Option<task_identity>,
    retired_incarnation: Option<task_identity>,
    phase: supervision_phase,
    pending_observation: Option<supervision_observation>,
    restart_budget_remaining: u8,
    restarts_used: u8,
    next_event_sequence: Option<u64>,
    next_restart_sequence: Option<u64>,
    replacement_sequence: Option<u64>,
    supervisor_circle_owns_relationship: bool,
}

impl supervision_snapshot {
    pub(crate) const fn identity(self) -> supervision_id {
        self.identity
    }

    pub(crate) const fn supervisor(self) -> task_identity {
        self.supervisor
    }

    pub(crate) const fn active_incarnation(self) -> Option<task_identity> {
        self.active_incarnation
    }

    pub(crate) const fn retired_incarnation(self) -> Option<task_identity> {
        self.retired_incarnation
    }

    pub(crate) const fn phase(self) -> supervision_phase {
        self.phase
    }

    pub(crate) const fn pending_observation(self) -> Option<supervision_observation> {
        self.pending_observation
    }

    pub(crate) const fn restart_budget_remaining(self) -> u8 {
        self.restart_budget_remaining
    }

    pub(crate) const fn restarts_used(self) -> u8 {
        self.restarts_used
    }

    pub(crate) const fn next_event_sequence(self) -> Option<u64> {
        self.next_event_sequence
    }

    pub(crate) const fn next_restart_sequence(self) -> Option<u64> {
        self.next_restart_sequence
    }

    pub(crate) const fn replacement_sequence(self) -> Option<u64> {
        self.replacement_sequence
    }

    pub(crate) const fn supervisor_circle_owns_relationship(self) -> bool {
        self.supervisor_circle_owns_relationship
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
struct raw_task_identity {
    task_slot: u64,
    task_generation: u64,
    circle_slot: u64,
    circle_generation: u64,
}

impl raw_task_identity {
    const fn empty() -> Self {
        Self {
            task_slot: 0,
            task_generation: 0,
            circle_slot: 0,
            circle_generation: 0,
        }
    }

    fn new(identity: task_identity) -> Self {
        Self {
            task_slot: identity.task().slot() as u64,
            task_generation: identity.task().generation_value(),
            circle_slot: identity.circle().slot() as u64,
            circle_generation: identity.circle().generation_value(),
        }
    }

    fn identity(self) -> Option<task_identity> {
        let task_slot = usize::try_from(self.task_slot).ok()?;
        let circle_slot = usize::try_from(self.circle_slot).ok()?;
        let task = task_id::try_from_parts(task_slot, self.task_generation)?;
        let circle = circle_id::try_from_parts(circle_slot, self.circle_generation)?;
        Some(task_identity::new(task, circle))
    }

    const fn is_clear(self) -> bool {
        self.task_slot == 0
            && self.task_generation == 0
            && self.circle_slot == 0
            && self.circle_generation == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
struct raw_observation {
    present: u8,
    outcome: u8,
    reserved: [u8; 6],
    event_sequence: u64,
    incarnation: raw_task_identity,
}

impl raw_observation {
    const fn empty() -> Self {
        Self {
            present: 0,
            outcome: 0,
            reserved: [0; 6],
            event_sequence: 0,
            incarnation: raw_task_identity::empty(),
        }
    }

    fn new(observation: supervision_observation) -> Self {
        Self {
            present: 1,
            outcome: observation.outcome as u8,
            reserved: [0; 6],
            event_sequence: observation.event_sequence,
            incarnation: raw_task_identity::new(observation.incarnation),
        }
    }

    fn observation(self) -> Option<supervision_observation> {
        if self.present != 1 || self.reserved != [0; 6] || self.event_sequence == 0 {
            return None;
        }
        Some(supervision_observation {
            event_sequence: self.event_sequence,
            incarnation: self.incarnation.identity()?,
            outcome: supervision_outcome::decode(self.outcome)?,
        })
    }

    const fn is_clear(self) -> bool {
        self.present == 0
            && self.outcome == 0
            && self.reserved[0] == 0
            && self.reserved[1] == 0
            && self.reserved[2] == 0
            && self.reserved[3] == 0
            && self.reserved[4] == 0
            && self.reserved[5] == 0
            && self.event_sequence == 0
            && self.incarnation.is_clear()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct supervision_record {
    generation: u64,
    active: u8,
    phase: u8,
    owner_attached: u8,
    restart_budget_remaining: u8,
    restarts_used: u8,
    event_sequence_exhausted: u8,
    restart_sequence_exhausted: u8,
    reserved: u8,
    next_event_sequence: u64,
    next_restart_sequence: u64,
    replacement_sequence: u64,
    supervisor: raw_task_identity,
    incarnation: raw_task_identity,
    retired_incarnation: raw_task_identity,
    observation: raw_observation,
}

impl supervision_record {
    const fn empty() -> Self {
        Self {
            generation: 0,
            active: inactive_record,
            phase: no_phase,
            owner_attached: owner_detached,
            restart_budget_remaining: 0,
            restarts_used: 0,
            event_sequence_exhausted: sequence_available,
            restart_sequence_exhausted: sequence_available,
            reserved: 0,
            next_event_sequence: 0,
            next_restart_sequence: 0,
            replacement_sequence: 0,
            supervisor: raw_task_identity::empty(),
            incarnation: raw_task_identity::empty(),
            retired_incarnation: raw_task_identity::empty(),
            observation: raw_observation::empty(),
        }
    }

    fn new(generation: u64, supervisor: task_identity, incarnation: task_identity) -> Self {
        Self {
            generation,
            active: active_record,
            phase: supervision_phase::active_incarnation as u8,
            owner_attached,
            restart_budget_remaining: restart_budget,
            restarts_used: 0,
            event_sequence_exhausted: sequence_available,
            restart_sequence_exhausted: sequence_available,
            reserved: 0,
            next_event_sequence: first_sequence,
            next_restart_sequence: first_sequence,
            replacement_sequence: 0,
            supervisor: raw_task_identity::new(supervisor),
            incarnation: raw_task_identity::new(incarnation),
            retired_incarnation: raw_task_identity::empty(),
            observation: raw_observation::empty(),
        }
    }

    fn phase(self) -> Option<supervision_phase> {
        supervision_phase::decode(self.phase)
    }

    fn supervisor(self) -> Option<task_identity> {
        self.supervisor.identity()
    }

    fn incarnation(self) -> Option<task_identity> {
        self.incarnation.identity()
    }

    fn retired_incarnation(self) -> Option<task_identity> {
        self.retired_incarnation.identity()
    }

    fn pending_observation(self) -> Option<supervision_observation> {
        self.observation.observation()
    }

    fn is_clear_tombstone(self) -> bool {
        self.active == inactive_record
            && self.phase == no_phase
            && self.owner_attached == owner_detached
            && self.restart_budget_remaining == 0
            && self.restarts_used == 0
            && self.event_sequence_exhausted == sequence_available
            && self.restart_sequence_exhausted == sequence_available
            && self.reserved == 0
            && self.next_event_sequence == 0
            && self.next_restart_sequence == 0
            && self.replacement_sequence == 0
            && self.supervisor.is_clear()
            && self.incarnation.is_clear()
            && self.retired_incarnation.is_clear()
            && self.observation.is_clear()
    }

    fn is_structurally_valid(self, identity: supervision_id) -> bool {
        let Some(phase) = self.phase() else {
            return false;
        };
        let Some(supervisor) = self.supervisor() else {
            return false;
        };
        if self.active != active_record
            || self.generation != identity.generation_value()
            || self.reserved != 0
            || self.restart_budget_remaining > restart_budget
            || self.restarts_used > restart_budget
            || self.restart_budget_remaining + self.restarts_used != restart_budget
            || !sequence_state_is_valid(self.next_event_sequence, self.event_sequence_exhausted)
            || !sequence_state_is_valid(self.next_restart_sequence, self.restart_sequence_exhausted)
        {
            return false;
        }

        let incarnation = self.incarnation();
        if incarnation.is_some_and(|value| value.task() == supervisor.task()) {
            return false;
        }
        let retired = self.retired_incarnation();
        let observation = self.pending_observation();
        let owner_expected = phase != supervision_phase::supervisor_withdrawn;
        if self.owner_attached
            != if owner_expected {
                owner_attached
            } else {
                owner_detached
            }
        {
            return false;
        }

        let restart_fields_valid = if self.restarts_used == 0 {
            self.replacement_sequence == 0
        } else {
            self.replacement_sequence != 0
                && sequence_was_issued(
                    self.replacement_sequence,
                    self.next_restart_sequence,
                    self.restart_sequence_exhausted,
                )
        };
        if !restart_fields_valid {
            return false;
        }

        match phase {
            supervision_phase::active_incarnation
            | supervision_phase::waiting_for_fault
            | supervision_phase::waiting_for_initial_completion
            | supervision_phase::restart_authorized
            | supervision_phase::waiting_for_completion => {
                incarnation.is_some()
                    && retired.is_none()
                    && observation.is_none()
                    && self.observation.is_clear()
                    && self.restarts_used == 0
            }
            supervision_phase::fault_observation_pending => observation.is_some_and(|value| {
                retired.is_none()
                    && incarnation == Some(value.incarnation())
                    && value.outcome() == supervision_outcome::invalid_instruction_fault
                    && sequence_was_issued(
                        value.event_sequence(),
                        self.next_event_sequence,
                        self.event_sequence_exhausted,
                    )
                    && self.restarts_used == 0
            }),
            supervision_phase::replacing_incarnation => {
                incarnation.is_none()
                    && self.incarnation.is_clear()
                    && retired.is_some()
                    && observation.is_none()
                    && self.observation.is_clear()
                    && self.restarts_used == 1
            }
            supervision_phase::replacement_active => {
                incarnation.is_some()
                    && retired.is_none()
                    && self.retired_incarnation.is_clear()
                    && observation.is_none()
                    && self.observation.is_clear()
                    && self.restarts_used == 1
            }
            supervision_phase::completion_observed | supervision_phase::supervisor_completed => {
                incarnation.is_some()
                    && retired.is_none()
                    && self.retired_incarnation.is_clear()
                    && observation.is_none()
                    && self.observation.is_clear()
                    && self.restarts_used <= 1
            }
            supervision_phase::completion_observation_pending => observation.is_some_and(|value| {
                retired.is_none()
                    && incarnation == Some(value.incarnation())
                    && value.outcome() == supervision_outcome::successful_exit
                    && sequence_was_issued(
                        value.event_sequence(),
                        self.next_event_sequence,
                        self.event_sequence_exhausted,
                    )
                    && self.restarts_used <= 1
            }),
            supervision_phase::incarnation_withdrawn | supervision_phase::supervisor_withdrawn => {
                incarnation.is_none()
                    && self.incarnation.is_clear()
                    && retired.is_none()
                    && self.retired_incarnation.is_clear()
                    && observation.is_none()
                    && self.observation.is_clear()
                    && self.restarts_used <= 1
            }
        }
    }
}

#[must_use = "dropping a prepared wait leaves the relationship unchanged"]
pub(crate) struct prepared_supervision_wait<'a> {
    record: &'a mut supervision_record,
    supervisor: task_identity,
    incarnation: task_identity,
    expected_outcome: supervision_outcome,
    next_phase: supervision_phase,
}

impl prepared_supervision_wait<'_> {
    pub(crate) const fn supervisor(&self) -> task_identity {
        self.supervisor
    }

    pub(crate) const fn incarnation(&self) -> task_identity {
        self.incarnation
    }

    pub(crate) const fn expected_outcome(&self) -> supervision_outcome {
        self.expected_outcome
    }

    pub(crate) fn commit(self) {
        self.record.phase = self.next_phase as u8;
    }
}

#[must_use = "dropping a prepared publication leaves the relationship unchanged"]
pub(crate) struct prepared_supervision_publication<'a> {
    record: &'a mut supervision_record,
    supervisor: task_identity,
    observation: supervision_observation,
    sequence: sequence_issue,
    next_phase: supervision_phase,
}

impl prepared_supervision_publication<'_> {
    pub(crate) const fn supervisor(&self) -> task_identity {
        self.supervisor
    }

    pub(crate) const fn observation(&self) -> supervision_observation {
        self.observation
    }

    pub(crate) fn commit(self) {
        self.record.observation = raw_observation::new(self.observation);
        self.record.next_event_sequence = self.sequence.next;
        self.record.event_sequence_exhausted = self.sequence.exhausted;
        self.record.phase = self.next_phase as u8;
    }
}

#[must_use = "dropping a prepared admission leaves the relationship unchanged"]
pub(crate) struct prepared_replacement_admission<'a> {
    record: &'a mut supervision_record,
    incarnation: task_identity,
    restart_sequence: u64,
}

impl prepared_replacement_admission<'_> {
    pub(crate) const fn incarnation(&self) -> task_identity {
        self.incarnation
    }

    pub(crate) const fn restart_sequence(&self) -> u64 {
        self.restart_sequence
    }

    pub(crate) fn commit(self) {
        self.record.incarnation = raw_task_identity::new(self.incarnation);
        self.record.retired_incarnation = raw_task_identity::empty();
        self.record.phase = supervision_phase::replacement_active as u8;
    }
}

#[must_use = "dropping a prepared completion leaves the relationship unchanged"]
pub(crate) struct prepared_supervisor_completion<'a> {
    record: &'a mut supervision_record,
    supervisor: task_identity,
}

impl prepared_supervisor_completion<'_> {
    pub(crate) const fn supervisor(&self) -> task_identity {
        self.supervisor
    }

    pub(crate) fn commit(self) {
        self.record.phase = supervision_phase::supervisor_completed as u8;
    }
}

pub(crate) struct supervision_table {
    records: [supervision_record; supervision_capacity],
}

impl supervision_table {
    pub(crate) const fn new() -> Self {
        Self {
            records: [supervision_record::empty(); supervision_capacity],
        }
    }

    pub(crate) fn active_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.active == active_record)
            .count()
    }

    pub(crate) fn create(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<supervision_id, supervision_error> {
        if supervisor.task() == incarnation.task() {
            return Err(supervision_error::same_task);
        }
        if supervisor.circle() == incarnation.circle() {
            return Err(supervision_error::same_circle);
        }
        require_task(tasks, supervisor, task_state::ready, Some(true))?;
        require_task(tasks, incarnation, task_state::ready, Some(true))?;
        circles
            .validate(supervisor.circle())
            .map_err(supervision_error::circle)?;
        circles
            .validate(incarnation.circle())
            .map_err(supervision_error::circle)?;
        let (slot, generation) = self.next_record()?;

        circles
            .attach_child(supervisor.circle(), circle_child_kind::supervision)
            .map_err(supervision_error::circle)?;
        self.records[slot] = supervision_record::new(generation.get(), supervisor, incarnation);
        Ok(supervision_id::from_parts(slot, generation))
    }

    pub(crate) fn validate(&self, identity: supervision_id) -> Result<(), supervision_error> {
        self.record(identity).map(|_| ())
    }

    pub(crate) fn snapshot(
        &self,
        identity: supervision_id,
    ) -> Result<supervision_snapshot, supervision_error> {
        let record = self.record(identity)?;
        Ok(supervision_snapshot {
            identity,
            supervisor: record
                .supervisor()
                .ok_or(supervision_error::corrupt_record)?,
            active_incarnation: record.incarnation(),
            retired_incarnation: record.retired_incarnation(),
            phase: record.phase().ok_or(supervision_error::corrupt_record)?,
            pending_observation: record.pending_observation(),
            restart_budget_remaining: record.restart_budget_remaining,
            restarts_used: record.restarts_used,
            next_event_sequence: available_sequence(
                record.next_event_sequence,
                record.event_sequence_exhausted,
            ),
            next_restart_sequence: available_sequence(
                record.next_restart_sequence,
                record.restart_sequence_exhausted,
            ),
            replacement_sequence: (record.replacement_sequence != 0)
                .then_some(record.replacement_sequence),
            supervisor_circle_owns_relationship: record.owner_attached == owner_attached,
        })
    }

    pub(crate) fn prepare_wait<'a>(
        &'a mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<prepared_supervision_wait<'a>, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        let stored_incarnation = require_incarnation(record, incarnation)?;
        require_task(tasks, stored_supervisor, task_state::running, Some(true))?;

        let phase = record.phase().ok_or(supervision_error::corrupt_record)?;
        let (required_incarnation_state, expected_outcome, next_phase) = match phase {
            supervision_phase::active_incarnation => (
                task_state::suspended,
                supervision_outcome::invalid_instruction_fault,
                supervision_phase::waiting_for_fault,
            ),
            supervision_phase::restart_authorized => (
                task_state::faulted,
                supervision_outcome::successful_exit,
                supervision_phase::waiting_for_completion,
            ),
            supervision_phase::fault_observation_pending
            | supervision_phase::completion_observation_pending => {
                return Err(supervision_error::observation_pending);
            }
            _ => return Err(supervision_error::invalid_phase),
        };
        require_task(
            tasks,
            stored_incarnation,
            required_incarnation_state,
            Some(true),
        )?;

        Ok(prepared_supervision_wait {
            record: &mut self.records[slot],
            supervisor: stored_supervisor,
            incarnation: stored_incarnation,
            expected_outcome,
            next_phase,
        })
    }

    /// Prepare one bounded wait for the successful exit of the initially
    /// admitted incarnation.
    ///
    /// This is the completion-only sibling of [`Self::prepare_wait`]. It does
    /// not authorize a restart, accept a fault outcome, or configure a general
    /// policy. The exact supervisor must be running and the exact incarnation
    /// must be ready for its first dispatch by an independent scheduler
    /// admission. A
    /// dropped guard leaves the relationship unchanged.
    pub(crate) fn prepare_completion_wait<'a>(
        &'a mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<prepared_supervision_wait<'a>, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        let stored_incarnation = require_incarnation(record, incarnation)?;
        require_task(tasks, stored_supervisor, task_state::running, Some(true))?;
        require_task(tasks, stored_incarnation, task_state::ready, Some(true))?;
        if record.phase() != Some(supervision_phase::active_incarnation) {
            return Err(supervision_error::invalid_phase);
        }
        Ok(prepared_supervision_wait {
            record: &mut self.records[slot],
            supervisor: stored_supervisor,
            incarnation: stored_incarnation,
            expected_outcome: supervision_outcome::successful_exit,
            next_phase: supervision_phase::waiting_for_initial_completion,
        })
    }

    pub(crate) fn prepare_publication<'a>(
        &'a mut self,
        tasks: &task_table,
        identity: supervision_id,
        incarnation: task_identity,
        outcome: supervision_outcome,
    ) -> Result<prepared_supervision_publication<'a>, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_incarnation = require_incarnation(record, incarnation)?;
        let supervisor = record
            .supervisor()
            .ok_or(supervision_error::corrupt_record)?;
        let phase = record.phase().ok_or(supervision_error::corrupt_record)?;
        let (expected_outcome, next_phase) = match phase {
            supervision_phase::waiting_for_fault => (
                supervision_outcome::invalid_instruction_fault,
                supervision_phase::fault_observation_pending,
            ),
            supervision_phase::replacement_active => (
                supervision_outcome::successful_exit,
                supervision_phase::completion_observation_pending,
            ),
            supervision_phase::waiting_for_initial_completion => (
                supervision_outcome::successful_exit,
                supervision_phase::completion_observation_pending,
            ),
            supervision_phase::fault_observation_pending
            | supervision_phase::completion_observation_pending => {
                return Err(supervision_error::observation_pending);
            }
            _ => return Err(supervision_error::invalid_phase),
        };
        if outcome != expected_outcome {
            return Err(supervision_error::wrong_outcome);
        }
        require_task(tasks, stored_incarnation, task_state::running, Some(true))?;
        require_task(tasks, supervisor, task_state::blocked, Some(true))?;
        let sequence = issued_sequence(record.next_event_sequence, record.event_sequence_exhausted)
            .ok_or(supervision_error::event_sequence_exhausted)?;
        let observation = supervision_observation {
            event_sequence: sequence.issued,
            incarnation: stored_incarnation,
            outcome,
        };

        Ok(prepared_supervision_publication {
            record: &mut self.records[slot],
            supervisor,
            observation,
            sequence,
            next_phase,
        })
    }

    pub(crate) fn take_observation(
        &mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
    ) -> Result<supervision_observation, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        require_task(tasks, stored_supervisor, task_state::running, Some(true))?;
        let phase = record.phase().ok_or(supervision_error::corrupt_record)?;
        let (outcome, incarnation_state, next_phase) = match phase {
            supervision_phase::fault_observation_pending => (
                supervision_outcome::invalid_instruction_fault,
                task_state::faulted,
                supervision_phase::restart_authorized,
            ),
            supervision_phase::completion_observation_pending => (
                supervision_outcome::successful_exit,
                task_state::exited,
                supervision_phase::completion_observed,
            ),
            _ => return Err(supervision_error::observation_missing),
        };
        let observation = record
            .pending_observation()
            .ok_or(supervision_error::corrupt_record)?;
        if observation.outcome() != outcome {
            return Err(supervision_error::corrupt_record);
        }
        require_task(
            tasks,
            observation.incarnation(),
            incarnation_state,
            Some(true),
        )?;

        if !volatile_zero_and_verify(&mut self.records[slot].observation) {
            return Err(supervision_error::record_clearing_failed);
        }
        self.records[slot].phase = next_phase as u8;
        Ok(observation)
    }

    pub(crate) fn begin_replacement(
        &mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<supervision_replacement, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        let stored_incarnation = require_incarnation(record, incarnation)?;
        if record.phase() != Some(supervision_phase::waiting_for_completion) {
            return Err(supervision_error::invalid_phase);
        }
        require_task(tasks, stored_supervisor, task_state::blocked, Some(true))?;
        require_task(tasks, stored_incarnation, task_state::faulted, Some(true))?;
        if record.restart_budget_remaining == 0 {
            return Err(supervision_error::restart_budget_exhausted);
        }
        let sequence = issued_sequence(
            record.next_restart_sequence,
            record.restart_sequence_exhausted,
        )
        .ok_or(supervision_error::restart_sequence_exhausted)?;

        self.records[slot].retired_incarnation = raw_task_identity::new(stored_incarnation);
        self.records[slot].incarnation = raw_task_identity::empty();
        self.records[slot].restart_budget_remaining -= 1;
        self.records[slot].restarts_used += 1;
        self.records[slot].next_restart_sequence = sequence.next;
        self.records[slot].restart_sequence_exhausted = sequence.exhausted;
        self.records[slot].replacement_sequence = sequence.issued;
        self.records[slot].phase = supervision_phase::replacing_incarnation as u8;
        Ok(supervision_replacement {
            restart_sequence: sequence.issued,
            retired_incarnation: stored_incarnation,
        })
    }

    pub(crate) fn prepare_admit_replacement<'a>(
        &'a mut self,
        circles: &circle_table,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        replacement: task_identity,
    ) -> Result<prepared_replacement_admission<'a>, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        if record.phase() != Some(supervision_phase::replacing_incarnation) {
            return Err(supervision_error::invalid_phase);
        }
        let retired = record
            .retired_incarnation()
            .ok_or(supervision_error::corrupt_record)?;
        require_task(tasks, stored_supervisor, task_state::blocked, Some(true))?;
        if !task_is_stale(tasks, retired) {
            return Err(supervision_error::retired_incarnation_live);
        }
        if !circle_is_stale(circles, retired.circle()) {
            return Err(supervision_error::retired_circle_live);
        }
        if replacement.task() == stored_supervisor.task()
            || replacement.circle() == stored_supervisor.circle()
        {
            return Err(supervision_error::replacement_not_fresh);
        }
        if !is_exact_next_incarnation(retired, replacement) {
            return Err(supervision_error::replacement_not_fresh);
        }
        require_task(tasks, replacement, task_state::ready, Some(false))?;
        circles
            .validate(replacement.circle())
            .map_err(supervision_error::circle)?;

        Ok(prepared_replacement_admission {
            record: &mut self.records[slot],
            incarnation: replacement,
            restart_sequence: record.replacement_sequence,
        })
    }

    pub(crate) fn prepare_supervisor_completion<'a>(
        &'a mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<prepared_supervisor_completion<'a>, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        let stored_incarnation = require_incarnation(record, incarnation)?;
        if record.phase() != Some(supervision_phase::completion_observed) {
            return Err(supervision_error::invalid_phase);
        }
        require_task(tasks, stored_supervisor, task_state::running, Some(true))?;
        require_task(tasks, stored_incarnation, task_state::exited, Some(true))?;

        Ok(prepared_supervisor_completion {
            record: &mut self.records[slot],
            supervisor: stored_supervisor,
        })
    }

    pub(crate) fn withdraw_incarnation(
        &mut self,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
        incarnation: task_identity,
    ) -> Result<task_identity, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        let stored_incarnation = require_incarnation(record, incarnation)?;
        if record.phase() != Some(supervision_phase::supervisor_completed) {
            return Err(supervision_error::invalid_phase);
        }
        require_task(tasks, stored_supervisor, task_state::exited, None)?;
        require_task(tasks, stored_incarnation, task_state::exited, Some(false))?;

        self.records[slot].incarnation = raw_task_identity::empty();
        self.records[slot].phase = supervision_phase::incarnation_withdrawn as u8;
        Ok(stored_incarnation)
    }

    pub(crate) fn withdraw_supervisor(
        &mut self,
        circles: &mut circle_table,
        tasks: &task_table,
        identity: supervision_id,
        supervisor: task_identity,
    ) -> Result<task_identity, supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        let stored_supervisor = require_supervisor(record, supervisor)?;
        if record.phase() != Some(supervision_phase::incarnation_withdrawn) {
            return Err(supervision_error::invalid_phase);
        }
        require_task(tasks, stored_supervisor, task_state::exited, Some(false))?;
        let owner = circles
            .snapshot(stored_supervisor.circle())
            .map_err(supervision_error::circle)?;
        if owner.children().supervisions() != 1 {
            return Err(supervision_error::circle_accounting_mismatch);
        }

        circles
            .detach_child(stored_supervisor.circle(), circle_child_kind::supervision)
            .map_err(supervision_error::circle)?;
        self.records[slot].owner_attached = owner_detached;
        self.records[slot].phase = supervision_phase::supervisor_withdrawn as u8;
        Ok(stored_supervisor)
    }

    pub(crate) fn release(
        &mut self,
        circles: &circle_table,
        tasks: &task_table,
        identity: supervision_id,
    ) -> Result<(), supervision_error> {
        let slot = self.record_index(identity)?;
        let record = self.records[slot];
        if record.phase() != Some(supervision_phase::supervisor_withdrawn) {
            return Err(supervision_error::invalid_phase);
        }
        let supervisor = record
            .supervisor()
            .ok_or(supervision_error::corrupt_record)?;
        if !task_is_stale(tasks, supervisor) {
            return Err(supervision_error::invalid_supervisor_state);
        }
        if !circle_is_stale(circles, supervisor.circle()) {
            return Err(supervision_error::retired_circle_live);
        }
        if !clear_record_and_preserve_generation(&mut self.records[slot]) {
            return Err(supervision_error::record_clearing_failed);
        }
        Ok(())
    }

    fn next_record(&self) -> Result<(usize, core::num::NonZeroU64), supervision_error> {
        let mut saw_inactive = false;
        for (slot, record) in self.records.iter().copied().enumerate() {
            if record.active == active_record {
                let Some(generation) = core::num::NonZeroU64::new(record.generation) else {
                    return Err(supervision_error::corrupt_record);
                };
                let identity = supervision_id::from_parts(slot, generation);
                if !record.is_structurally_valid(identity) {
                    return Err(supervision_error::corrupt_record);
                }
                continue;
            }
            if record.active != inactive_record || !record.is_clear_tombstone() {
                return Err(supervision_error::corrupt_record);
            }
            saw_inactive = true;
            if let Some(generation) = next_generation(record.generation) {
                return Ok((slot, generation));
            }
        }
        Err(if saw_inactive {
            supervision_error::generation_exhausted
        } else {
            supervision_error::capacity_exhausted
        })
    }

    fn record(&self, identity: supervision_id) -> Result<&supervision_record, supervision_error> {
        let slot = self.record_index(identity)?;
        Ok(&self.records[slot])
    }

    fn record_index(&self, identity: supervision_id) -> Result<usize, supervision_error> {
        let slot = identity.slot();
        let Some(record) = self.records.get(slot) else {
            return Err(supervision_error::invalid_identity);
        };
        if record.active == inactive_record || record.generation != identity.generation_value() {
            return Err(supervision_error::stale_identity);
        }
        if record.active != active_record || !record.is_structurally_valid(identity) {
            return Err(supervision_error::corrupt_record);
        }
        Ok(slot)
    }
}

fn require_supervisor(
    record: supervision_record,
    supervisor: task_identity,
) -> Result<task_identity, supervision_error> {
    let stored = record
        .supervisor()
        .ok_or(supervision_error::corrupt_record)?;
    if stored != supervisor {
        return Err(supervision_error::wrong_supervisor);
    }
    Ok(stored)
}

fn require_incarnation(
    record: supervision_record,
    incarnation: task_identity,
) -> Result<task_identity, supervision_error> {
    let stored = record
        .incarnation()
        .ok_or(supervision_error::wrong_incarnation)?;
    if stored != incarnation {
        return Err(supervision_error::wrong_incarnation);
    }
    Ok(stored)
}

fn require_task(
    tasks: &task_table,
    identity: task_identity,
    state: task_state,
    context_attached: Option<bool>,
) -> Result<(), supervision_error> {
    let snapshot = tasks.snapshot(identity).map_err(supervision_error::task)?;
    if snapshot.state() != state {
        return Err(if identity.circle() == snapshot.circle() {
            supervision_error::invalid_incarnation_state
        } else {
            supervision_error::invalid_supervisor_state
        });
    }
    if let Some(expected) = context_attached
        && snapshot.context_attached() != expected
    {
        return Err(if expected {
            supervision_error::context_missing
        } else {
            supervision_error::context_still_attached
        });
    }
    Ok(())
}

fn task_is_stale(tasks: &task_table, identity: task_identity) -> bool {
    matches!(tasks.snapshot(identity), Err(task_error::stale_identity))
}

fn circle_is_stale(circles: &circle_table, identity: circle_id) -> bool {
    matches!(
        circles.validate(identity),
        Err(circle_error::stale_identity)
    )
}

fn is_exact_next_incarnation(retired: task_identity, replacement: task_identity) -> bool {
    retired.task().slot() == replacement.task().slot()
        && retired.circle().slot() == replacement.circle().slot()
        && retired
            .task()
            .generation_value()
            .checked_add(1)
            .is_some_and(|generation| generation == replacement.task().generation_value())
        && retired
            .circle()
            .generation_value()
            .checked_add(1)
            .is_some_and(|generation| generation == replacement.circle().generation_value())
}

#[derive(Clone, Copy)]
struct sequence_issue {
    issued: u64,
    next: u64,
    exhausted: u8,
}

const fn issued_sequence(next: u64, exhausted: u8) -> Option<sequence_issue> {
    if next == 0 || exhausted != sequence_available {
        return None;
    }
    Some(if next == u64::MAX {
        sequence_issue {
            issued: next,
            next,
            exhausted: sequence_exhausted,
        }
    } else {
        sequence_issue {
            issued: next,
            next: next + 1,
            exhausted: sequence_available,
        }
    })
}

const fn available_sequence(next: u64, exhausted: u8) -> Option<u64> {
    if next == 0 || exhausted != sequence_available {
        None
    } else {
        Some(next)
    }
}

const fn sequence_state_is_valid(next: u64, exhausted: u8) -> bool {
    next != 0
        && (exhausted == sequence_available
            || (exhausted == sequence_exhausted && next == u64::MAX))
}

const fn sequence_was_issued(issued: u64, next: u64, exhausted: u8) -> bool {
    if issued == 0 {
        return false;
    }
    if exhausted == sequence_exhausted && issued == u64::MAX && next == u64::MAX {
        return true;
    }
    if exhausted != sequence_available {
        return false;
    }
    match issued.checked_add(1) {
        Some(value) => value == next,
        None => false,
    }
}

fn volatile_zero_and_verify<T>(value: &mut T) -> bool {
    let bytes = ptr::from_mut(value).cast::<u8>();
    for offset in 0..core::mem::size_of::<T>() {
        // SAFETY: `bytes` spans one uniquely borrowed integer-only record;
        // every all-zero field representation is valid.
        unsafe {
            ptr::write_volatile(bytes.add(offset), 0);
        }
    }
    compiler_fence(Ordering::SeqCst);
    (0..core::mem::size_of::<T>()).all(|offset| {
        // SAFETY: Every byte was initialized above and remains borrowed.
        unsafe { ptr::read_volatile(bytes.add(offset)) == 0 }
    })
}

fn clear_record_and_preserve_generation(record: &mut supervision_record) -> bool {
    let generation = record.generation;
    if !volatile_zero_and_verify(record) {
        return false;
    }
    // SAFETY: The record remains uniquely borrowed. Restoring only its raw
    // generation leaves the canonical inactive tombstone representation.
    unsafe {
        ptr::write_volatile(ptr::addr_of_mut!(record.generation), generation);
    }
    compiler_fence(Ordering::SeqCst);
    record.generation == generation && record.is_clear_tombstone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::fixture_principal_id;
    use crate::task::{synthetic_origin_metadata, task_binding, task_bounds, task_table};

    struct direct_completion_fixture {
        circles: circle_table,
        tasks: task_table,
        supervisions: supervision_table,
        supervisor_circle: circle_id,
        incarnation_circle: circle_id,
        supervisor: task_identity,
        incarnation: task_identity,
        relationship: supervision_id,
    }

    fn bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("valid task bounds")
    }

    fn create_task(
        circles: &mut circle_table,
        tasks: &mut task_table,
        circle: circle_id,
        principal: u64,
    ) -> task_identity {
        tasks
            .create(
                circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(principal).expect("fixture principal"),
                    synthetic_origin_metadata::machine_role_fixture,
                    bounds(),
                ),
            )
            .expect("task")
    }

    fn fixture() -> direct_completion_fixture {
        let mut circles = circle_table::new();
        let supervisor_circle = circles.create().expect("supervisor circle");
        let incarnation_circle = circles.create().expect("incarnation circle");
        let mut tasks = task_table::new();
        let supervisor = create_task(&mut circles, &mut tasks, supervisor_circle, 1);
        let incarnation = create_task(&mut circles, &mut tasks, incarnation_circle, 2);
        tasks
            .attach_context(&mut circles, supervisor)
            .expect("supervisor context");
        tasks
            .attach_context(&mut circles, incarnation)
            .expect("incarnation context");
        let mut supervisions = supervision_table::new();
        let relationship = supervisions
            .create(&mut circles, &tasks, supervisor, incarnation)
            .expect("relationship");
        direct_completion_fixture {
            circles,
            tasks,
            supervisions,
            supervisor_circle,
            incarnation_circle,
            supervisor,
            incarnation,
            relationship,
        }
    }

    fn begin_wait(fixture: &mut direct_completion_fixture) {
        fixture
            .tasks
            .dispatch(fixture.supervisor)
            .expect("dispatch supervisor");
        let wait = fixture
            .supervisions
            .prepare_completion_wait(
                &fixture.tasks,
                fixture.relationship,
                fixture.supervisor,
                fixture.incarnation,
            )
            .expect("completion wait");
        let switch = fixture
            .tasks
            .prepare_block_and_dispatch(fixture.supervisor, fixture.incarnation)
            .expect("block supervisor and dispatch incarnation");
        let _wake = switch.commit();
        wait.commit();
    }

    #[test]
    fn direct_completion_observes_one_success_and_fully_tears_down() {
        let mut fixture = fixture();
        begin_wait(&mut fixture);
        let publication = fixture
            .supervisions
            .prepare_publication(
                &fixture.tasks,
                fixture.relationship,
                fixture.incarnation,
                supervision_outcome::successful_exit,
            )
            .expect("success publication");
        let expected = publication.observation();
        assert_eq!(expected.event_sequence(), 1);
        assert_eq!(expected.incarnation(), fixture.incarnation);
        assert_eq!(expected.outcome(), supervision_outcome::successful_exit);
        fixture
            .tasks
            .complete_and_wake(fixture.incarnation, fixture.supervisor)
            .expect("complete and wake supervisor");
        publication.commit();
        let observed = fixture
            .supervisions
            .take_observation(&fixture.tasks, fixture.relationship, fixture.supervisor)
            .expect("take observation");
        assert_eq!(observed, expected);
        let completion = fixture
            .supervisions
            .prepare_supervisor_completion(
                &fixture.tasks,
                fixture.relationship,
                fixture.supervisor,
                fixture.incarnation,
            )
            .expect("supervisor completion");
        fixture
            .tasks
            .complete_scheduled(fixture.supervisor)
            .expect("complete supervisor");
        completion.commit();

        fixture
            .tasks
            .withdraw_context(&mut fixture.circles, fixture.incarnation)
            .expect("withdraw incarnation context");
        fixture
            .tasks
            .withdraw_context(&mut fixture.circles, fixture.supervisor)
            .expect("withdraw supervisor context");
        fixture
            .supervisions
            .withdraw_incarnation(
                &fixture.tasks,
                fixture.relationship,
                fixture.supervisor,
                fixture.incarnation,
            )
            .expect("withdraw incarnation");
        fixture
            .tasks
            .destroy(&mut fixture.circles, fixture.incarnation)
            .expect("destroy incarnation");
        fixture
            .circles
            .destroy(fixture.incarnation_circle)
            .expect("destroy incarnation circle");
        fixture
            .supervisions
            .withdraw_supervisor(
                &mut fixture.circles,
                &fixture.tasks,
                fixture.relationship,
                fixture.supervisor,
            )
            .expect("withdraw supervisor");
        fixture
            .tasks
            .destroy(&mut fixture.circles, fixture.supervisor)
            .expect("destroy supervisor");
        fixture
            .circles
            .destroy(fixture.supervisor_circle)
            .expect("destroy supervisor circle");
        fixture
            .supervisions
            .release(&fixture.circles, &fixture.tasks, fixture.relationship)
            .expect("release relationship");
        assert_eq!(fixture.supervisions.active_count(), 0);
        assert_eq!(fixture.tasks.active_count(), 0);
        assert_eq!(fixture.circles.accounting().circles, 0);
    }

    #[test]
    fn direct_completion_rejects_wrong_state_identity_phase_and_outcome() {
        let mut fixture = fixture();
        assert!(
            fixture
                .supervisions
                .prepare_completion_wait(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.supervisor,
                    fixture.incarnation,
                )
                .is_err()
        );
        fixture
            .tasks
            .dispatch(fixture.supervisor)
            .expect("dispatch supervisor");
        assert_eq!(
            fixture
                .supervisions
                .prepare_completion_wait(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.incarnation,
                    fixture.supervisor,
                )
                .err(),
            Some(supervision_error::wrong_supervisor)
        );
        let stale_incarnation = task_identity::new(
            task_id::try_from_parts(
                fixture.incarnation.task().slot(),
                fixture.incarnation.task().generation_value() + 1,
            )
            .expect("stale task identity"),
            fixture.incarnation.circle(),
        );
        assert_eq!(
            fixture
                .supervisions
                .prepare_completion_wait(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.supervisor,
                    stale_incarnation,
                )
                .err(),
            Some(supervision_error::wrong_incarnation)
        );
        assert_eq!(
            fixture
                .supervisions
                .prepare_admit_replacement(
                    &fixture.circles,
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.supervisor,
                    fixture.incarnation,
                )
                .err(),
            Some(supervision_error::invalid_phase)
        );

        let before = fixture
            .supervisions
            .snapshot(fixture.relationship)
            .expect("snapshot before dropped guard");
        drop(
            fixture
                .supervisions
                .prepare_completion_wait(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.supervisor,
                    fixture.incarnation,
                )
                .expect("prepared wait"),
        );
        assert_eq!(
            fixture
                .supervisions
                .snapshot(fixture.relationship)
                .expect("snapshot after dropped guard"),
            before
        );

        let mut phase_fixture = self::fixture();
        phase_fixture
            .tasks
            .dispatch(phase_fixture.supervisor)
            .expect("dispatch phase supervisor");
        phase_fixture
            .supervisions
            .prepare_completion_wait(
                &phase_fixture.tasks,
                phase_fixture.relationship,
                phase_fixture.supervisor,
                phase_fixture.incarnation,
            )
            .expect("phase wait")
            .commit();
        assert_eq!(
            phase_fixture
                .supervisions
                .prepare_completion_wait(
                    &phase_fixture.tasks,
                    phase_fixture.relationship,
                    phase_fixture.supervisor,
                    phase_fixture.incarnation,
                )
                .err(),
            Some(supervision_error::invalid_phase)
        );

        let wait = fixture
            .supervisions
            .prepare_completion_wait(
                &fixture.tasks,
                fixture.relationship,
                fixture.supervisor,
                fixture.incarnation,
            )
            .expect("completion wait");
        let switch = fixture
            .tasks
            .prepare_block_and_dispatch(fixture.supervisor, fixture.incarnation)
            .expect("block and dispatch");
        let _wake = switch.commit();
        wait.commit();
        assert_eq!(
            fixture
                .supervisions
                .prepare_publication(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.incarnation,
                    supervision_outcome::invalid_instruction_fault,
                )
                .err(),
            Some(supervision_error::wrong_outcome)
        );
        assert_eq!(
            fixture
                .supervisions
                .prepare_publication(
                    &fixture.tasks,
                    fixture.relationship,
                    fixture.supervisor,
                    supervision_outcome::successful_exit,
                )
                .err(),
            Some(supervision_error::wrong_incarnation)
        );
    }
}
