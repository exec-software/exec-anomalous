#![cfg_attr(not(test), allow(dead_code))]

//! Fixed-capacity, architecture-neutral scheduling state for bounded proofs.
//!
//! This module owns runnable order and failure-atomic task-state transitions.
//! Architecture code remains responsible for validating, saving, restoring,
//! and clearing the actual processor context before committing the matching
//! transition here.

use crate::circle::circle_table;
use crate::identity::task_identity;
#[cfg(any(test, feature = "input_wait_test"))]
use crate::session_input::{
    session_input_prepared_wake, session_input_readiness_identity, session_input_wait_identity,
    session_input_wake_correlation,
};
#[cfg(any(test, feature = "input_wait_test", feature = "veil_application_test"))]
use crate::task::task_wake_authorization;
#[cfg(any(test, feature = "input_wait_test"))]
use crate::task::{prepared_task_block, prepared_task_switch, prepared_task_wake};
#[cfg(any(test, feature = "veil_application_test"))]
use crate::task::{
    prepared_task_call_exchange, prepared_task_call_fault, prepared_task_service_fault,
};
use crate::task::{task_capacity, task_error, task_state, task_table};
#[cfg(any(test, feature = "veil_application_test"))]
use core::sync::atomic::{AtomicU64, Ordering};

pub(crate) const scheduler_capacity: usize = task_capacity;

const no_context: u8 = 0;
const initial_context: u8 = 1;
const saved_context: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum scheduler_context {
    initial,
    saved,
}

impl scheduler_context {
    const fn encode(self) -> u8 {
        match self {
            Self::initial => initial_context,
            Self::saved => saved_context,
        }
    }

    const fn decode(value: u8) -> Option<Self> {
        match value {
            initial_context => Some(Self::initial),
            saved_context => Some(Self::saved),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum scheduler_error {
    invalid_capacity,
    capacity_exhausted,
    duplicate_task,
    task_not_admitted,
    no_current_task,
    wrong_current_task,
    no_runnable_task,
    no_runnable_peer,
    wrong_runnable_task,
    runnable_peer_present,
    dispatch_sequence_exhausted,
    preemption_sequence_exhausted,
    dispatch_reservation_pending,
    dispatch_reservation_missing,
    wrong_dispatch_reservation,
    authorization_missing,
    authorization_slot_occupied,
    wrong_endpoint_wake,
    #[cfg(any(test, feature = "veil_application_test"))]
    call_owner_exhausted,
    corrupt_state,
    task(task_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_dispatch {
    task: task_identity,
    sequence: u64,
    context: scheduler_context,
}

impl scheduler_dispatch {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn context(self) -> scheduler_context {
        self.context
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_preemption {
    task: task_identity,
    sequence: u64,
    dispatch: scheduler_dispatch,
}

impl scheduler_preemption {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn dispatch(self) -> scheduler_dispatch {
        self.dispatch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_fault {
    task: task_identity,
    dispatch: scheduler_dispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_block {
    task: task_identity,
    dispatch: scheduler_dispatch,
}

impl scheduler_block {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn dispatch(self) -> scheduler_dispatch {
        self.dispatch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_park {
    task: task_identity,
}

impl scheduler_park {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_completion {
    task: task_identity,
    dispatch: scheduler_dispatch,
}

impl scheduler_completion {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn dispatch(self) -> scheduler_dispatch {
        self.dispatch
    }
}

impl scheduler_fault {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn dispatch(self) -> scheduler_dispatch {
        self.dispatch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_snapshot {
    capacity: usize,
    admitted: usize,
    runnable: usize,
    current: Option<task_identity>,
    next_dispatch_sequence: Option<u64>,
    next_preemption_sequence: Option<u64>,
    reservation: Option<scheduler_reservation_snapshot>,
}

impl scheduler_snapshot {
    pub(crate) const fn capacity(self) -> usize {
        self.capacity
    }

    pub(crate) const fn admitted(self) -> usize {
        self.admitted
    }

    pub(crate) const fn runnable(self) -> usize {
        self.runnable
    }

    pub(crate) const fn current(self) -> Option<task_identity> {
        self.current
    }

    pub(crate) const fn next_dispatch_sequence(self) -> Option<u64> {
        self.next_dispatch_sequence
    }

    pub(crate) const fn next_preemption_sequence(self) -> Option<u64> {
        self.next_preemption_sequence
    }

    pub(crate) const fn reservation(self) -> Option<scheduler_reservation_snapshot> {
        self.reservation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_reservation_snapshot {
    task: task_identity,
    current: task_identity,
    sequence: u64,
    runnable: bool,
}

impl scheduler_reservation_snapshot {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }

    pub(crate) const fn current(self) -> task_identity {
        self.current
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn runnable(self) -> bool {
        self.runnable
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct scheduler_record {
    task: Option<task_identity>,
    context: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct dispatch_reservation {
    task: task_identity,
    current: task_identity,
    sequence: u64,
}

impl scheduler_record {
    const fn empty() -> Self {
        Self {
            task: None,
            context: no_context,
        }
    }

    const fn new(task: task_identity) -> Self {
        Self {
            task: Some(task),
            context: initial_context,
        }
    }

    const fn context(self) -> Option<scheduler_context> {
        scheduler_context::decode(self.context)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct scheduler_table {
    capacity: usize,
    records: [scheduler_record; scheduler_capacity],
    queue: [Option<task_identity>; scheduler_capacity],
    queue_head: usize,
    queue_depth: usize,
    current: Option<task_identity>,
    next_dispatch_sequence: u64,
    dispatch_sequence_exhausted: bool,
    reservation: Option<dispatch_reservation>,
    next_preemption_sequence: u64,
    preemption_sequence_exhausted: bool,
    #[cfg(any(test, feature = "veil_application_test"))]
    call_owner: u64,
    #[cfg(any(test, feature = "veil_application_test"))]
    call_service: u64,
    #[cfg(any(test, feature = "veil_application_test"))]
    call: Option<call_reservation>,
}

/// Non-executable structural evidence used only for failure-atomic unit tests.
///
/// Keeping this separate from `scheduler_table` lets tests compare every byte
/// of logical state without making a live scheduler or reservation cloneable.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_test_snapshot {
    capacity: usize,
    records: [scheduler_record; scheduler_capacity],
    queue: [Option<task_identity>; scheduler_capacity],
    queue_head: usize,
    queue_depth: usize,
    current: Option<task_identity>,
    next_dispatch_sequence: u64,
    dispatch_sequence_exhausted: bool,
    reservation: Option<dispatch_reservation>,
    next_preemption_sequence: u64,
    preemption_sequence_exhausted: bool,
    call_owner: u64,
    call_service: u64,
    call: Option<call_reservation>,
}

#[cfg(any(test, feature = "veil_application_test"))]
static next_call_owner: AtomicU64 = AtomicU64::new(1);

#[cfg(any(test, feature = "veil_application_test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct call_reservation {
    owner: u64,
    service: u64,
    sequence: u64,
    return_sequence: u64,
    caller: task_identity,
    veil: task_identity,
}

/// Single-use authority for one suspended caller. Numbers alone do not confer
/// access: every copy/reply must validate this against its original scheduler.
#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "retain through exact reply or dependency-fault containment"]
pub(crate) struct scheduler_call_authorization {
    reservation: call_reservation,
    wake: task_wake_authorization,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl scheduler_call_authorization {
    pub(crate) const fn caller(&self) -> task_identity {
        self.reservation.caller
    }
    pub(crate) const fn veil(&self) -> task_identity {
        self.reservation.veil
    }
    pub(crate) const fn sequence(&self) -> u64 {
        self.reservation.sequence
    }

    /// Inert correlation metadata for a supervisor-held staged TLS effect.
    /// These words cannot construct call authority or identify another caller.
    #[cfg(any(test, feature = "linux_tls_test"))]
    pub(crate) const fn correlation_words(&self) -> [u64; 4] {
        [
            self.reservation.owner,
            self.reservation.service,
            self.reservation.sequence,
            self.reservation.return_sequence,
        ]
    }
}

/// The veil is blocked between calls; only its original caller relation can
/// wake it again. Terminal dependency cleanup consumes this value instead.
#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "retain until the next call or explicit waiting-veil retirement"]
pub(crate) struct scheduler_veil_wait_authorization {
    owner: u64,
    service: u64,
    caller: task_identity,
    veil: task_identity,
    wake: task_wake_authorization,
}

/// Controller-held membership authority for one fixed shared native service.
/// The scheduler retains only its globally unique nonce. This move-only value
/// retains the exact task generations and cannot be reconstructed from wire IDs.
#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "retain until explicit retirement or shared-service containment"]
pub(crate) struct scheduler_call_service {
    owner: u64,
    nonce: u64,
    veil: task_identity,
    callers: [Option<task_identity>; 2],
}

#[cfg(any(test, feature = "veil_application_test"))]
impl scheduler_call_service {
    pub(crate) fn contains(&self, caller: task_identity) -> bool {
        self.nonce != 0 && self.callers.contains(&Some(caller))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.callers.iter().all(Option::is_none)
    }

    pub(crate) const fn is_revoked(&self) -> bool {
        self.nonce == 0
    }
}

#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "dropping preparation leaves caller, veil and service queue unchanged"]
pub(crate) struct prepared_scheduler_call<'a> {
    scheduler: &'a mut scheduler_table,
    tasks: prepared_task_call_exchange<'a>,
    waiting: &'a mut Option<scheduler_veil_wait_authorization>,
    pending: &'a mut Option<scheduler_call_authorization>,
    queue: [Option<task_identity>; scheduler_capacity],
    queue_depth: usize,
    reservation: call_reservation,
    next_sequence: sequence_issue,
    context: scheduler_context,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_scheduler_call<'_> {
    pub(crate) const fn transition(&self) -> scheduler_dispatch {
        scheduler_dispatch {
            task: self.reservation.veil,
            sequence: self.reservation.sequence,
            context: self.context,
        }
    }

    pub(crate) fn commit(self) -> scheduler_dispatch {
        let transition = self.transition();
        let wake = self
            .tasks
            .commit(self.waiting.take().map(|value| value.wake));
        self.scheduler.queue = self.queue;
        self.scheduler.queue_depth = self.queue_depth;
        self.scheduler
            .record_mut(self.reservation.caller)
            .unwrap()
            .context = saved_context;
        self.scheduler.current = Some(self.reservation.veil);
        self.scheduler.call = Some(self.reservation);
        self.scheduler.commit_dispatch_sequence(self.next_sequence);
        *self.pending = Some(scheduler_call_authorization {
            reservation: self.reservation,
            wake,
        });
        transition
    }
}

#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "commit only after all caller effects are fully prepared"]
pub(crate) struct prepared_scheduler_call_reply<'a> {
    scheduler: &'a mut scheduler_table,
    tasks: prepared_task_call_exchange<'a>,
    pending: &'a mut Option<scheduler_call_authorization>,
    waiting: &'a mut Option<scheduler_veil_wait_authorization>,
    reservation: call_reservation,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_scheduler_call_reply<'_> {
    pub(crate) fn scheduler(&self) -> &scheduler_table {
        self.scheduler
    }
    pub(crate) fn tasks(&self) -> &task_table {
        self.tasks.tasks()
    }
    pub(crate) fn authorization(&self) -> &scheduler_call_authorization {
        self.pending.as_ref().expect("preflighted call authority")
    }
    pub(crate) const fn transition(&self) -> scheduler_dispatch {
        scheduler_dispatch {
            task: self.reservation.caller,
            sequence: self.reservation.return_sequence,
            context: scheduler_context::saved,
        }
    }
    /// Exclusive preparation borrows prevent task/scheduler/token mutation;
    /// every fallible check precedes this effect-publication boundary.
    pub(crate) fn commit(self) -> scheduler_dispatch {
        let transition = self.transition();
        let pending = self.pending.take().expect("preflighted call authority");
        let wake = self.tasks.commit(Some(pending.wake));
        self.scheduler
            .record_mut(self.reservation.veil)
            .unwrap()
            .context = saved_context;
        self.scheduler.current = Some(self.reservation.caller);
        self.scheduler.call = None;
        *self.waiting = Some(scheduler_veil_wait_authorization {
            owner: self.reservation.owner,
            service: self.reservation.service,
            caller: self.reservation.caller,
            veil: self.reservation.veil,
            wake,
        });
        transition
    }
}

#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "abort staged effects before committing dependent-task containment"]
pub(crate) struct prepared_scheduler_call_fault<'a> {
    scheduler: &'a mut scheduler_table,
    tasks: prepared_task_call_fault<'a>,
    pending: &'a mut Option<scheduler_call_authorization>,
    reservation: call_reservation,
    transition: scheduler_dispatch,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_scheduler_call_fault<'_> {
    pub(crate) const fn transition(&self) -> scheduler_dispatch {
        self.transition
    }
    pub(crate) fn commit(self) -> scheduler_dispatch {
        let pending = self.pending.take().expect("preflighted call authority");
        self.tasks.commit(pending.wake);
        self.scheduler
            .record_mut(self.reservation.veil)
            .unwrap()
            .context = saved_context;
        self.scheduler.pop_queue();
        self.scheduler.current = Some(self.transition.task);
        self.scheduler.call = None;
        self.transition
    }
}

#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "abort staged effects before committing shared-service containment"]
pub(crate) struct prepared_scheduler_service_fault<'a> {
    scheduler: &'a mut scheduler_table,
    tasks: prepared_task_service_fault<'a>,
    service: &'a mut scheduler_call_service,
    pending: &'a mut Option<scheduler_call_authorization>,
    queue: [Option<task_identity>; scheduler_capacity],
    queue_depth: usize,
    transition: scheduler_dispatch,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_scheduler_service_fault<'_> {
    pub(crate) const fn transition(&self) -> scheduler_dispatch {
        self.transition
    }

    pub(crate) fn commit(self) -> scheduler_dispatch {
        let pending = self
            .pending
            .take()
            .expect("preflighted shared call authority");
        self.tasks.commit(pending.wake);
        self.scheduler
            .record_mut(self.service.veil)
            .unwrap()
            .context = saved_context;
        // A never-dispatched dependent also leaves the initial-ready class.
        // Its retained context is terminal and can only be withdrawn, never
        // dispatched as an initial context after the service has failed.
        for caller in self.service.callers.iter().flatten() {
            self.scheduler.record_mut(*caller).unwrap().context = saved_context;
        }
        self.scheduler.queue = self.queue;
        self.scheduler.queue_depth = self.queue_depth;
        self.scheduler.pop_queue();
        self.scheduler.current = Some(self.transition.task);
        self.scheduler.call = None;
        self.scheduler.call_service = 0;
        self.service.nonce = 0;
        self.service.callers = [None; 2];
        self.transition
    }
}

/// Single-use authority returned only by a committed reserved block.
///
/// The future session-input endpoint must retain this move-only value inside
/// the exact pending wait. Its numeric fields are deliberately private: task
/// identity or dispatch sequence equality alone is not wake authority.
#[must_use = "dropping wake authority strands its dispatch reservation"]
#[cfg(any(test, feature = "input_wait_test"))]
pub(crate) struct scheduler_wake_authorization {
    reservation: dispatch_reservation,
    task_authorization: task_wake_authorization,
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
}

/// Endpoint-composed wake authority installed in stable runtime state.
///
/// Neither numeric correlation nor scheduler authority alone can construct
/// this value. [`compose_endpoint_wake_authorization`] requires the sealed,
/// move-only token issued by the exact session-input endpoint.
#[cfg(any(test, feature = "input_wait_test"))]
#[must_use = "the exact scheduler wake must consume composed endpoint authority"]
pub(crate) struct scheduler_endpoint_wake_authorization {
    reservation: dispatch_reservation,
    task_authorization: task_wake_authorization,
    endpoint_authorization: session_input_prepared_wake,
    preflight: scheduler_endpoint_wake_preflight,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl scheduler_endpoint_wake_authorization {
    pub(crate) const fn endpoint(&self) -> &session_input_prepared_wake {
        &self.endpoint_authorization
    }
}

/// Copyable evidence that scheduler placement was checked before endpoint
/// publication or closure committed. This is correlation, never authority.
#[cfg(any(test, feature = "input_wait_test"))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct scheduler_endpoint_wake_preflight {
    reservation: dispatch_reservation,
    correlation: session_input_wake_correlation,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl scheduler_endpoint_wake_preflight {
    pub(crate) const fn correlation(self) -> session_input_wake_correlation {
        self.correlation
    }
}

/// Single-use authority returned only after the exact blocked task is queued.
#[must_use = "dropping selection authority strands its runnable reservation"]
#[cfg(any(test, feature = "input_wait_test"))]
pub(crate) struct scheduler_selection_authorization {
    reservation: dispatch_reservation,
}

/// Exact retained selection authority used only by failure-atomic unit tests.
#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct scheduler_selection_test_snapshot {
    reservation: dispatch_reservation,
}

#[cfg(test)]
impl scheduler_selection_authorization {
    pub(crate) const fn test_snapshot(&self) -> scheduler_selection_test_snapshot {
        scheduler_selection_test_snapshot {
            reservation: self.reservation,
        }
    }
}

#[cfg(any(test, feature = "input_wait_test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct scheduler_wake {
    task: task_identity,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl scheduler_wake {
    pub(crate) const fn task(self) -> task_identity {
        self.task
    }
}

/// Rejected endpoint composition returns the private endpoint token intact.
#[cfg(any(test, feature = "input_wait_test"))]
#[must_use = "a rejected composition retains move-only endpoint authority"]
pub(crate) struct scheduler_endpoint_composition_rejection {
    error: scheduler_error,
    endpoint_authorization: session_input_prepared_wake,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl scheduler_endpoint_composition_rejection {
    pub(crate) const fn error(&self) -> scheduler_error {
        self.error
    }

    pub(crate) fn into_endpoint_authorization(self) -> session_input_prepared_wake {
        self.endpoint_authorization
    }
}

/// Atomically compose stable scheduler authority with an endpoint-issued wake.
///
/// The scheduler source slot and destination slot remain unchanged on every
/// rejection. Success moves both authorities directly into the caller-owned
/// destination slot, leaving no droppable intermediate owner.
#[cfg(any(test, feature = "input_wait_test"))]
#[allow(clippy::result_large_err)]
pub(crate) fn compose_endpoint_wake_authorization(
    scheduler_authorization: &mut Option<scheduler_wake_authorization>,
    endpoint_authorization: session_input_prepared_wake,
    preflight: scheduler_endpoint_wake_preflight,
    composed_authorization: &mut Option<scheduler_endpoint_wake_authorization>,
) -> Result<(), scheduler_endpoint_composition_rejection> {
    let error = if composed_authorization.is_some() {
        Some(scheduler_error::authorization_slot_occupied)
    } else {
        match scheduler_authorization.as_ref() {
            None => Some(scheduler_error::authorization_missing),
            Some(authorization) if authorization.reservation != preflight.reservation => {
                Some(scheduler_error::wrong_dispatch_reservation)
            }
            Some(_)
                if endpoint_authorization.correlation() != preflight.correlation
                    || endpoint_authorization.task() != preflight.reservation.task =>
            {
                Some(scheduler_error::wrong_endpoint_wake)
            }
            Some(_) => None,
        }
    };
    if let Some(error) = error {
        return Err(scheduler_endpoint_composition_rejection {
            error,
            endpoint_authorization,
        });
    }

    let scheduler_authorization = scheduler_authorization
        .take()
        .expect("preflighted scheduler wake authority");
    *composed_authorization = Some(scheduler_endpoint_wake_authorization {
        reservation: scheduler_authorization.reservation,
        task_authorization: scheduler_authorization.task_authorization,
        endpoint_authorization,
        preflight,
    });
    Ok(())
}

#[must_use = "dropping a prepared reserved block leaves scheduler and tasks unchanged"]
#[cfg(any(test, feature = "input_wait_test"))]
pub(crate) struct prepared_scheduler_block<'a> {
    scheduler: &'a mut scheduler_table,
    task_transition: prepared_task_block<'a>,
    authorization: &'a mut Option<scheduler_wake_authorization>,
    current: task_identity,
    next: task_identity,
    next_context: scheduler_context,
    dispatch_sequence: sequence_issue,
    reserved_sequence: sequence_issue,
    wait: session_input_wait_identity,
    readiness: session_input_readiness_identity,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_scheduler_block<'_> {
    pub(crate) const fn transition(&self) -> scheduler_block {
        scheduler_block {
            task: self.current,
            dispatch: scheduler_dispatch {
                task: self.next,
                sequence: self.dispatch_sequence.issued,
                context: self.next_context,
            },
        }
    }

    pub(crate) fn commit(self) -> scheduler_block {
        let Self {
            scheduler,
            task_transition,
            authorization,
            current,
            next,
            next_context,
            dispatch_sequence,
            reserved_sequence,
            wait,
            readiness,
        } = self;
        let task_authorization = task_transition.commit();
        scheduler.pop_queue();
        scheduler
            .record_mut(current)
            .expect("prepared current scheduler record")
            .context = scheduler_context::saved.encode();
        scheduler.current = Some(next);
        let reservation = dispatch_reservation {
            task: current,
            current: next,
            sequence: reserved_sequence.issued,
        };
        scheduler.reservation = Some(reservation);
        scheduler.commit_dispatch_sequence(reserved_sequence);
        *authorization = Some(scheduler_wake_authorization {
            reservation,
            task_authorization,
            wait,
            readiness,
        });
        scheduler_block {
            task: current,
            dispatch: scheduler_dispatch {
                task: next,
                sequence: dispatch_sequence.issued,
                context: next_context,
            },
        }
    }
}

#[cfg(any(test, feature = "input_wait_test"))]
#[must_use = "dropping a prepared wake leaves scheduler and tasks unchanged"]
pub(crate) struct prepared_scheduler_wake<'a> {
    scheduler: &'a mut scheduler_table,
    task_transition: prepared_task_wake<'a>,
    authorization: &'a mut Option<scheduler_endpoint_wake_authorization>,
    selection_authorization: &'a mut Option<scheduler_selection_authorization>,
    reservation: dispatch_reservation,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_scheduler_wake<'_> {
    pub(crate) const fn task(&self) -> task_identity {
        self.reservation.task
    }

    pub(crate) fn commit(self) -> scheduler_wake {
        let Self {
            scheduler,
            task_transition,
            authorization,
            selection_authorization,
            reservation,
        } = self;
        let authorization = authorization
            .take()
            .expect("preflighted endpoint-composed wake authority");
        assert!(
            authorization.reservation == reservation,
            "prepared scheduler wake authority changed after preflight"
        );
        task_transition.commit(authorization.task_authorization);
        drop(authorization.endpoint_authorization);
        scheduler.push_queue(reservation.task);
        *selection_authorization = Some(scheduler_selection_authorization { reservation });
        scheduler_wake {
            task: reservation.task,
        }
    }
}

#[must_use = "dropping a prepared reserved preemption leaves scheduler and tasks unchanged"]
#[cfg(any(test, feature = "input_wait_test"))]
pub(crate) struct prepared_reserved_preemption<'a> {
    scheduler: &'a mut scheduler_table,
    task_transition: prepared_task_switch<'a>,
    authorization: &'a mut Option<scheduler_selection_authorization>,
    reservation: dispatch_reservation,
    preemption_sequence: sequence_issue,
    next_context: scheduler_context,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_reserved_preemption<'_> {
    pub(crate) const fn transition(&self) -> scheduler_preemption {
        let reservation = self.reservation;
        scheduler_preemption {
            task: reservation.current,
            sequence: self.preemption_sequence.issued,
            dispatch: scheduler_dispatch {
                task: reservation.task,
                sequence: reservation.sequence,
                context: self.next_context,
            },
        }
    }

    pub(crate) fn commit(self) -> scheduler_preemption {
        let Self {
            scheduler,
            task_transition,
            authorization,
            reservation,
            preemption_sequence,
            next_context,
        } = self;
        let authorization = authorization
            .take()
            .expect("preflighted reserved-selection authority");
        assert!(
            authorization.reservation == reservation,
            "prepared reserved-preemption authority changed after preflight"
        );
        task_transition.commit();
        scheduler.pop_queue();
        scheduler.push_queue(reservation.current);
        scheduler
            .record_mut(reservation.current)
            .expect("prepared current scheduler record")
            .context = scheduler_context::saved.encode();
        scheduler.current = Some(reservation.task);
        scheduler.reservation = None;
        scheduler.commit_preemption_sequence(preemption_sequence);
        scheduler_preemption {
            task: reservation.current,
            sequence: preemption_sequence.issued,
            dispatch: scheduler_dispatch {
                task: reservation.task,
                sequence: reservation.sequence,
                context: next_context,
            },
        }
    }
}

#[must_use = "dropping a prepared reserved fault leaves scheduler and tasks unchanged"]
#[cfg(any(test, feature = "input_wait_test"))]
pub(crate) struct prepared_reserved_fault<'a> {
    scheduler: &'a mut scheduler_table,
    task_transition: prepared_task_switch<'a>,
    authorization: &'a mut Option<scheduler_selection_authorization>,
    reservation: dispatch_reservation,
    next_context: scheduler_context,
}

#[cfg(any(test, feature = "input_wait_test"))]
impl prepared_reserved_fault<'_> {
    pub(crate) const fn transition(&self) -> scheduler_fault {
        let reservation = self.reservation;
        scheduler_fault {
            task: reservation.current,
            dispatch: scheduler_dispatch {
                task: reservation.task,
                sequence: reservation.sequence,
                context: self.next_context,
            },
        }
    }

    pub(crate) fn commit(self) -> scheduler_fault {
        let Self {
            scheduler,
            task_transition,
            authorization,
            reservation,
            next_context,
        } = self;
        let authorization = authorization
            .take()
            .expect("preflighted reserved-selection authority");
        assert!(
            authorization.reservation == reservation,
            "prepared reserved-fault authority changed after preflight"
        );
        task_transition.commit();
        scheduler.pop_queue();
        scheduler
            .record_mut(reservation.current)
            .expect("prepared current scheduler record")
            .context = scheduler_context::saved.encode();
        scheduler.current = Some(reservation.task);
        scheduler.reservation = None;
        scheduler_fault {
            task: reservation.current,
            dispatch: scheduler_dispatch {
                task: reservation.task,
                sequence: reservation.sequence,
                context: next_context,
            },
        }
    }
}

impl scheduler_table {
    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> scheduler_test_snapshot {
        scheduler_test_snapshot {
            capacity: self.capacity,
            records: self.records,
            queue: self.queue,
            queue_head: self.queue_head,
            queue_depth: self.queue_depth,
            current: self.current,
            next_dispatch_sequence: self.next_dispatch_sequence,
            dispatch_sequence_exhausted: self.dispatch_sequence_exhausted,
            reservation: self.reservation,
            next_preemption_sequence: self.next_preemption_sequence,
            preemption_sequence_exhausted: self.preemption_sequence_exhausted,
            call_owner: self.call_owner,
            call_service: self.call_service,
            call: self.call,
        }
    }

    #[cfg(test)]
    fn restore_test_snapshot(&mut self, snapshot: scheduler_test_snapshot) {
        self.capacity = snapshot.capacity;
        self.records = snapshot.records;
        self.queue = snapshot.queue;
        self.queue_head = snapshot.queue_head;
        self.queue_depth = snapshot.queue_depth;
        self.current = snapshot.current;
        self.next_dispatch_sequence = snapshot.next_dispatch_sequence;
        self.dispatch_sequence_exhausted = snapshot.dispatch_sequence_exhausted;
        self.reservation = snapshot.reservation;
        self.next_preemption_sequence = snapshot.next_preemption_sequence;
        self.preemption_sequence_exhausted = snapshot.preemption_sequence_exhausted;
        self.call_owner = snapshot.call_owner;
        self.call_service = snapshot.call_service;
        self.call = snapshot.call;
    }

    pub(crate) fn new(capacity: usize) -> Result<Self, scheduler_error> {
        if capacity > scheduler_capacity {
            return Err(scheduler_error::invalid_capacity);
        }
        Ok(Self {
            capacity,
            records: [scheduler_record::empty(); scheduler_capacity],
            queue: [None; scheduler_capacity],
            queue_head: 0,
            queue_depth: 0,
            current: None,
            next_dispatch_sequence: 0,
            dispatch_sequence_exhausted: false,
            reservation: None,
            next_preemption_sequence: 0,
            preemption_sequence_exhausted: false,
            #[cfg(any(test, feature = "veil_application_test"))]
            call_owner: next_call_owner
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    if value == 0 {
                        None
                    } else {
                        value.checked_add(1)
                    }
                })
                .map_err(|_| scheduler_error::call_owner_exhausted)?,
            #[cfg(any(test, feature = "veil_application_test"))]
            call_service: 0,
            #[cfg(any(test, feature = "veil_application_test"))]
            call: None,
        })
    }

    /// Suspend the current caller and enter its exact veil, preserving every
    /// unrelated queued peer. The first veil must be initially ready; later
    /// entries require the sealed authority produced by its preceding reply.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_call_handoff<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        caller: task_identity,
        veil: task_identity,
        waiting: &'a mut Option<scheduler_veil_wait_authorization>,
        pending: &'a mut Option<scheduler_call_authorization>,
    ) -> Result<prepared_scheduler_call<'a>, scheduler_error> {
        if self.call_service != 0 {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        self.prepare_bound_call_handoff(tasks, caller, veil, 0, waiting, pending)
    }

    /// Register an exact two-caller relationship before the first dispatch.
    /// A second registration cannot mint another usable membership authority.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn register_call_service(
        &mut self,
        tasks: &task_table,
        veil: task_identity,
        callers: [task_identity; 2],
    ) -> Result<scheduler_call_service, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        if self.call_service != 0 || self.current.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        if callers[0] == callers[1] || callers.contains(&veil) {
            return Err(scheduler_error::task(task_error::same_task));
        }
        for identity in [veil, callers[0], callers[1]] {
            if self.record(identity).and_then(scheduler_record::context)
                != Some(scheduler_context::initial)
                || !tasks.snapshot(identity).is_ok_and(|snapshot| {
                    snapshot.state() == task_state::ready && snapshot.context_attached()
                })
            {
                return Err(scheduler_error::task_not_admitted);
            }
        }
        let nonce = next_call_owner
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                if value == 0 {
                    None
                } else {
                    value.checked_add(1)
                }
            })
            .map_err(|_| scheduler_error::call_owner_exhausted)?;
        self.call_service = nonce;
        Ok(scheduler_call_service {
            owner: self.call_owner,
            nonce,
            veil,
            callers: callers.map(Some),
        })
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    fn validate_call_service(
        &self,
        tasks: &task_table,
        service: &scheduler_call_service,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        if service.owner != self.call_owner
            || service.nonce == 0
            || service.nonce != self.call_service
            || self.record(service.veil).is_none()
            || tasks.snapshot(service.veil).is_err()
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        for caller in service.callers.iter().flatten() {
            if *caller == service.veil
                || self.record(*caller).is_none()
                || tasks.snapshot(*caller).is_err()
            {
                return Err(scheduler_error::wrong_dispatch_reservation);
            }
        }
        Ok(())
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_service_call_handoff<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        caller: task_identity,
        service: &scheduler_call_service,
        waiting: &'a mut Option<scheduler_veil_wait_authorization>,
        pending: &'a mut Option<scheduler_call_authorization>,
    ) -> Result<prepared_scheduler_call<'a>, scheduler_error> {
        self.validate_call_service(tasks, service)?;
        if !service.contains(caller) {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        self.prepare_bound_call_handoff(
            tasks,
            caller,
            service.veil,
            service.nonce,
            waiting,
            pending,
        )
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    fn prepare_bound_call_handoff<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        caller: task_identity,
        veil: task_identity,
        service: u64,
        waiting: &'a mut Option<scheduler_veil_wait_authorization>,
        pending: &'a mut Option<scheduler_call_authorization>,
    ) -> Result<prepared_scheduler_call<'a>, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(caller)?;
        if pending.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        if caller == veil {
            return Err(scheduler_error::task(task_error::same_task));
        }
        let context = self
            .record(veil)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::task_not_admitted)?;
        let mut queue = self.queue;
        let mut queue_depth = self.queue_depth;
        if let Some(authority) = waiting.as_ref() {
            self.validate_veil_wait(tasks, authority)?;
            if authority.veil != veil
                || authority.service != service
                || (service == 0 && authority.caller != caller)
            {
                return Err(scheduler_error::wrong_dispatch_reservation);
            }
        } else {
            let snapshot = tasks.snapshot(veil).map_err(scheduler_error::task)?;
            if snapshot.state() != task_state::ready || context != scheduler_context::initial {
                return Err(scheduler_error::authorization_missing);
            }
            let offset = (0..self.queue_depth)
                .find(|offset| {
                    self.queue_index(*offset)
                        .is_some_and(|index| queue[index] == Some(veil))
                })
                .ok_or(scheduler_error::wrong_runnable_task)?;
            for index in offset..self.queue_depth - 1 {
                queue[self.queue_index(index).unwrap()] =
                    queue[self.queue_index(index + 1).unwrap()];
            }
            queue[self.queue_index(self.queue_depth - 1).unwrap()] = None;
            queue_depth -= 1;
        }
        let (dispatch, reply) = issued_sequence_pair(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;
        let reservation = call_reservation {
            owner: self.call_owner,
            service,
            sequence: dispatch.issued,
            return_sequence: reply.issued,
            caller,
            veil,
        };
        let transition = tasks
            .prepare_call_exchange(caller, veil, waiting.as_ref().map(|value| &value.wake))
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_call {
            scheduler: self,
            tasks: transition,
            waiting,
            pending,
            queue,
            queue_depth,
            reservation,
            next_sequence: reply,
            context,
        })
    }

    /// Copy authority applies only while this exact veil runs for this exact
    /// blocked caller. Architecture code must additionally validate retained
    /// frames, roots, aliases, bounds and complete memory permissions.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn validate_call_authorization(
        &self,
        tasks: &task_table,
        authority: &scheduler_call_authorization,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        if authority.reservation.owner != self.call_owner
            || self.call != Some(authority.reservation)
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        self.require_current(authority.veil())?;
        if tasks
            .validate_scheduled_wake_authorization(&authority.wake)
            .map_err(scheduler_error::task)?
            != authority.caller()
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        Ok(())
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_call_reply<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        pending: &'a mut Option<scheduler_call_authorization>,
        waiting: &'a mut Option<scheduler_veil_wait_authorization>,
    ) -> Result<prepared_scheduler_call_reply<'a>, scheduler_error> {
        let authority = pending
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        self.validate_call_authorization(tasks, authority)?;
        if waiting.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        let reservation = authority.reservation;
        let transition = tasks
            .prepare_call_exchange(reservation.veil, reservation.caller, Some(&authority.wake))
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_call_reply {
            scheduler: self,
            tasks: transition,
            pending,
            waiting,
            reservation,
        })
    }

    /// Contain the failed veil and its suspended dependent caller, then select
    /// the unchanged runnable queue head. No caller wake authority survives.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_call_fault<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        pending: &'a mut Option<scheduler_call_authorization>,
    ) -> Result<prepared_scheduler_call_fault<'a>, scheduler_error> {
        let authority = pending
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        self.validate_call_authorization(tasks, authority)?;
        let reservation = authority.reservation;
        if reservation.service != 0 {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        let next = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        let context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let transition = tasks
            .prepare_call_fault(reservation.veil, &authority.wake, next)
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_call_fault {
            scheduler: self,
            tasks: transition,
            pending,
            reservation,
            transition: scheduler_dispatch {
                task: next,
                sequence: reservation.return_sequence,
                context,
            },
        })
    }

    /// Retire only the exact terminal member. The other member retains its
    /// relationship and can reuse the same blocked veil on a later call.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn retire_service_caller(
        &mut self,
        tasks: &task_table,
        service: &mut scheduler_call_service,
        caller: task_identity,
    ) -> Result<(), scheduler_error> {
        self.validate_call_service(tasks, service)?;
        self.require_no_reservation()?;
        let index = service
            .callers
            .iter()
            .position(|member| *member == Some(caller))
            .ok_or(scheduler_error::wrong_dispatch_reservation)?;
        if !tasks
            .snapshot(caller)
            .map_err(scheduler_error::task)?
            .state()
            .is_terminal()
        {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        service.callers[index] = None;
        Ok(())
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn retire_call_service(
        &mut self,
        tasks: &task_table,
        service: &mut scheduler_call_service,
    ) -> Result<(), scheduler_error> {
        self.validate_call_service(tasks, service)?;
        self.require_no_reservation()?;
        if !service.is_empty()
            || !tasks
                .snapshot(service.veil)
                .map_err(scheduler_error::task)?
                .state()
                .is_terminal()
        {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        self.call_service = 0;
        service.nonce = 0;
        Ok(())
    }

    /// Fault every live dependent of this bounded service before selecting any
    /// unrelated queue peer. A merely queued caller cannot later enter a dead
    /// veil; no reply or membership authority survives commit.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_service_call_fault<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        service: &'a mut scheduler_call_service,
        pending: &'a mut Option<scheduler_call_authorization>,
    ) -> Result<prepared_scheduler_service_fault<'a>, scheduler_error> {
        self.validate_call_service(tasks, service)?;
        let authority = pending
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        self.validate_call_authorization(tasks, authority)?;
        let reservation = authority.reservation;
        if reservation.service != service.nonce
            || reservation.veil != service.veil
            || !service.contains(reservation.caller)
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        let peer = service
            .callers
            .iter()
            .flatten()
            .copied()
            .find(|caller| *caller != reservation.caller);
        let mut queue = self.queue;
        let mut queue_depth = self.queue_depth;
        if let Some(peer) = peer {
            let offset = (0..queue_depth)
                .find(|offset| {
                    self.queue_index(*offset)
                        .is_some_and(|index| queue[index] == Some(peer))
                })
                .ok_or(scheduler_error::wrong_runnable_task)?;
            for index in offset..queue_depth - 1 {
                queue[self.queue_index(index).unwrap()] =
                    queue[self.queue_index(index + 1).unwrap()];
            }
            queue[self.queue_index(queue_depth - 1).unwrap()] = None;
            queue_depth -= 1;
        }
        let next = if queue_depth == 0 {
            return Err(scheduler_error::no_runnable_peer);
        } else {
            queue[self.queue_head].ok_or(scheduler_error::corrupt_state)?
        };
        let context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let transition = tasks
            .prepare_service_fault(reservation.veil, &authority.wake, peer, next)
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_service_fault {
            scheduler: self,
            tasks: transition,
            service,
            pending,
            queue,
            queue_depth,
            transition: scheduler_dispatch {
                task: next,
                sequence: reservation.return_sequence,
                context,
            },
        })
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    fn validate_veil_wait(
        &self,
        tasks: &task_table,
        authority: &scheduler_veil_wait_authorization,
    ) -> Result<(), scheduler_error> {
        if authority.owner != self.call_owner
            || authority.service != self.call_service
            || tasks
                .validate_scheduled_wake_authorization(&authority.wake)
                .map_err(scheduler_error::task)?
                != authority.veil
            || self
                .record(authority.veil)
                .and_then(scheduler_record::context)
                != Some(scheduler_context::saved)
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        Ok(())
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn retire_waiting_veil(
        &mut self,
        tasks: &mut task_table,
        waiting: &mut Option<scheduler_veil_wait_authorization>,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        let authority = waiting
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        self.validate_veil_wait(tasks, authority)?;
        if authority.service != 0 {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        tasks
            .retire_authorized_wait(&authority.wake)
            .map_err(scheduler_error::task)?;
        waiting.take();
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(())
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn retire_service_veil(
        &mut self,
        tasks: &mut task_table,
        service: &scheduler_call_service,
        waiting: &mut Option<scheduler_veil_wait_authorization>,
    ) -> Result<(), scheduler_error> {
        self.validate_call_service(tasks, service)?;
        self.require_no_reservation()?;
        let authority = waiting
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        self.validate_veil_wait(tasks, authority)?;
        if !service.is_empty()
            || authority.service != service.nonce
            || authority.veil != service.veil
        {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        tasks
            .retire_authorized_wait(&authority.wake)
            .map_err(scheduler_error::task)?;
        waiting.take();
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(())
    }

    pub(crate) fn validate(&self, tasks: &task_table) -> Result<(), scheduler_error> {
        if self.is_structurally_valid(tasks) {
            Ok(())
        } else {
            Err(scheduler_error::corrupt_state)
        }
    }

    pub(crate) fn snapshot(
        &self,
        tasks: &task_table,
    ) -> Result<scheduler_snapshot, scheduler_error> {
        self.validate(tasks)?;
        Ok(scheduler_snapshot {
            capacity: self.capacity,
            admitted: self.active_count(),
            runnable: self.queue_depth,
            current: self.current,
            next_dispatch_sequence: available_sequence(
                self.next_dispatch_sequence,
                self.dispatch_sequence_exhausted,
            ),
            next_preemption_sequence: available_sequence(
                self.next_preemption_sequence,
                self.preemption_sequence_exhausted,
            ),
            reservation: self.reservation.map(|reservation| {
                let runnable = tasks
                    .snapshot(reservation.task)
                    .is_ok_and(|snapshot| snapshot.state() == task_state::suspended);
                scheduler_reservation_snapshot {
                    task: reservation.task,
                    current: reservation.current,
                    sequence: reservation.sequence,
                    runnable,
                }
            }),
        })
    }

    pub(crate) fn context(
        &self,
        tasks: &task_table,
        identity: task_identity,
    ) -> Result<scheduler_context, scheduler_error> {
        self.validate(tasks)?;
        self.record(identity)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::task_not_admitted)
    }

    pub(crate) fn admit(
        &mut self,
        circles: &mut circle_table,
        tasks: &mut task_table,
        identity: task_identity,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        if self.record(identity).is_some() {
            return Err(scheduler_error::duplicate_task);
        }
        let snapshot = tasks.snapshot(identity).map_err(scheduler_error::task)?;
        if snapshot.state() != task_state::ready {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        if snapshot.context_attached() {
            return Err(scheduler_error::task(task_error::context_already_attached));
        }
        if self.active_count() == self.capacity || self.queue_depth == self.capacity {
            return Err(scheduler_error::capacity_exhausted);
        }
        let record_slot = self
            .records
            .iter()
            .position(|record| record.task.is_none())
            .ok_or(scheduler_error::corrupt_state)?;
        let queue_slot = self.queue_tail().ok_or(scheduler_error::corrupt_state)?;

        tasks
            .attach_context(circles, identity)
            .map_err(scheduler_error::task)?;
        self.records[record_slot] = scheduler_record::new(identity);
        self.queue[queue_slot] = Some(identity);
        self.queue_depth += 1;
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(())
    }

    pub(crate) fn start_next(
        &mut self,
        tasks: &mut task_table,
    ) -> Result<scheduler_dispatch, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        if self.current.is_some() {
            return Err(scheduler_error::wrong_current_task);
        }
        let next = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_task)?;
        let context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks.dispatch(next).map_err(scheduler_error::task)?;
        self.pop_queue();
        self.current = Some(next);
        self.commit_dispatch_sequence(sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_dispatch {
            task: next,
            sequence: sequence.issued,
            context,
        })
    }

    pub(crate) fn preempt_current(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<scheduler_preemption, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let next = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        let next_context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let preemption_sequence = issued_sequence(
            self.next_preemption_sequence,
            self.preemption_sequence_exhausted,
        )
        .ok_or(scheduler_error::preemption_sequence_exhausted)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .suspend_and_dispatch(current, next)
            .map_err(scheduler_error::task)?;
        self.pop_queue();
        self.push_queue(current);
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(next);
        self.commit_preemption_sequence(preemption_sequence);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_preemption {
            task: current,
            sequence: preemption_sequence.issued,
            dispatch: scheduler_dispatch {
                task: next,
                sequence: dispatch_sequence.issued,
                context: next_context,
            },
        })
    }

    /// Block the exact current task and dispatch the exact runnable head.
    pub(crate) fn block_current_and_dispatch(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
        next: task_identity,
    ) -> Result<scheduler_block, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let runnable = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        if runnable != next {
            return Err(scheduler_error::wrong_runnable_task);
        }
        let next_context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .block_and_dispatch(current, next)
            .map_err(scheduler_error::task)?;
        self.pop_queue();
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(next);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_block {
            task: current,
            dispatch: scheduler_dispatch {
                task: next,
                sequence: dispatch_sequence.issued,
                context: next_context,
            },
        })
    }

    /// Prepare one exact block/peer dispatch and reserve the following dispatch.
    ///
    /// This fixed proof accepts exactly one runnable peer. The committed result
    /// returns the only safe wake authority for the blocked task; the reserved
    /// dispatch cannot be consumed through an ordinary scheduler path.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_block_current_and_reserve<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        current: task_identity,
        next: task_identity,
        wait: session_input_wait_identity,
        readiness: session_input_readiness_identity,
        authorization: &'a mut Option<scheduler_wake_authorization>,
    ) -> Result<prepared_scheduler_block<'a>, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        if authorization.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        if wait.binding() != readiness.binding() || wait.binding().task() != current {
            return Err(scheduler_error::wrong_endpoint_wake);
        }
        self.require_current(current)?;
        if self.queue_depth != 1 {
            return Err(scheduler_error::wrong_runnable_task);
        }
        let runnable = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        if runnable != next {
            return Err(scheduler_error::wrong_runnable_task);
        }
        let next_context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let (dispatch_sequence, reserved_sequence) = issued_sequence_pair(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;
        if issued_sequence(
            self.next_preemption_sequence,
            self.preemption_sequence_exhausted,
        )
        .is_none()
        {
            return Err(scheduler_error::preemption_sequence_exhausted);
        }
        let task_transition = tasks
            .prepare_block_and_dispatch(current, next)
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_block {
            scheduler: self,
            task_transition,
            authorization,
            current,
            next,
            next_context,
            dispatch_sequence,
            reserved_sequence,
            wait,
            readiness,
        })
    }

    /// Preflight exact FIFO-tail wake placement before endpoint mutation.
    ///
    /// The returned copyable value is correlation only. Under the caller's
    /// serialized exclusion, a successful endpoint commit returning the same
    /// correlation can be composed and committed without a new fallible fact.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn preflight_endpoint_wake(
        &self,
        tasks: &task_table,
        scheduler_authorization: &Option<scheduler_wake_authorization>,
        correlation: session_input_wake_correlation,
        composed_authorization: &Option<scheduler_endpoint_wake_authorization>,
        selection_authorization: &Option<scheduler_selection_authorization>,
    ) -> Result<scheduler_endpoint_wake_preflight, scheduler_error> {
        self.validate(tasks)?;
        if composed_authorization.is_some() || selection_authorization.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        let authorization = scheduler_authorization
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        let reservation = authorization.reservation;
        if correlation.wait() != authorization.wait
            || correlation.readiness() != authorization.readiness
            || correlation.binding().task() != reservation.task
        {
            return Err(scheduler_error::wrong_endpoint_wake);
        }
        self.require_reservation(reservation)?;
        self.require_current(reservation.current)?;
        self.preflight_reserved_wake(tasks, reservation)?;
        let authorized_task = tasks
            .validate_scheduled_wake_authorization(&authorization.task_authorization)
            .map_err(scheduler_error::task)?;
        if authorized_task != reservation.task {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        Ok(scheduler_endpoint_wake_preflight {
            reservation,
            correlation,
        })
    }

    /// Prepare the endpoint-authorized half of one exact nonterminal wake.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_endpoint_wake<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        authorization: &'a mut Option<scheduler_endpoint_wake_authorization>,
        selection_authorization: &'a mut Option<scheduler_selection_authorization>,
    ) -> Result<prepared_scheduler_wake<'a>, scheduler_error> {
        self.validate(tasks)?;
        if selection_authorization.is_some() {
            return Err(scheduler_error::authorization_slot_occupied);
        }
        let composed = authorization
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?;
        let reservation = composed.reservation;
        if composed.preflight.reservation != reservation {
            return Err(scheduler_error::wrong_dispatch_reservation);
        }
        if composed.endpoint_authorization.correlation() != composed.preflight.correlation
            || composed.endpoint_authorization.task() != reservation.task
        {
            return Err(scheduler_error::wrong_endpoint_wake);
        }
        self.require_reservation(reservation)?;
        self.require_current(reservation.current)?;
        self.preflight_reserved_wake(tasks, reservation)?;
        let task_transition = tasks
            .prepare_scheduled_wake(&composed.task_authorization)
            .map_err(scheduler_error::task)?;
        Ok(prepared_scheduler_wake {
            scheduler: self,
            task_transition,
            authorization,
            selection_authorization,
            reservation,
        })
    }

    /// Prepare a normal preemption that selects only the reserved runnable task.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_reserved_preemption<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        current: task_identity,
        authorization: &'a mut Option<scheduler_selection_authorization>,
    ) -> Result<prepared_reserved_preemption<'a>, scheduler_error> {
        self.validate(tasks)?;
        let reservation = authorization
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?
            .reservation;
        self.require_reservation(reservation)?;
        self.require_reserved_selection(current, reservation)?;
        let Some(preemption_sequence) = issued_sequence(
            self.next_preemption_sequence,
            self.preemption_sequence_exhausted,
        ) else {
            return Err(scheduler_error::preemption_sequence_exhausted);
        };
        let next_context = self
            .record(reservation.task)
            .and_then(scheduler_record::context)
            .expect("validated reserved scheduler context");
        let task_transition = tasks
            .prepare_suspend_and_dispatch(reservation.current, reservation.task)
            .map_err(scheduler_error::task)?;
        Ok(prepared_reserved_preemption {
            scheduler: self,
            task_transition,
            authorization,
            reservation,
            preemption_sequence,
            next_context,
        })
    }

    /// Prepare an exact current-task fault that selects the reserved task.
    #[cfg(any(test, feature = "input_wait_test"))]
    pub(crate) fn prepare_reserved_fault<'a>(
        &'a mut self,
        tasks: &'a mut task_table,
        current: task_identity,
        authorization: &'a mut Option<scheduler_selection_authorization>,
    ) -> Result<prepared_reserved_fault<'a>, scheduler_error> {
        self.validate(tasks)?;
        let reservation = authorization
            .as_ref()
            .ok_or(scheduler_error::authorization_missing)?
            .reservation;
        self.require_reservation(reservation)?;
        self.require_reserved_selection(current, reservation)?;
        let next_context = self
            .record(reservation.task)
            .and_then(scheduler_record::context)
            .expect("validated reserved scheduler context");
        let task_transition = tasks
            .prepare_fault_and_dispatch(reservation.current, reservation.task)
            .map_err(scheduler_error::task)?;
        Ok(prepared_reserved_fault {
            scheduler: self,
            task_transition,
            authorization,
            reservation,
            next_context,
        })
    }

    /// Park the exact current task after proving that no runnable peer exists.
    pub(crate) fn park_current(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<scheduler_park, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        if self.queue_depth != 0 {
            return Err(scheduler_error::runnable_peer_present);
        }

        tasks
            .park_scheduled(current)
            .map_err(scheduler_error::task)?;
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = None;
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_park { task: current })
    }

    pub(crate) fn fault_current(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<scheduler_fault, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let next = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        let next_context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .fault_and_dispatch(current, next)
            .map_err(scheduler_error::task)?;
        self.pop_queue();
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(next);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_fault {
            task: current,
            dispatch: scheduler_dispatch {
                task: next,
                sequence: dispatch_sequence.issued,
                context: next_context,
            },
        })
    }

    /// Complete one exact current task while a runnable peer keeps progressing.
    #[cfg(any(
        test,
        feature = "native_application_test",
        feature = "mixed_workspace_test"
    ))]
    pub(crate) fn complete_current_and_dispatch(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<scheduler_completion, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let next = self
            .queue_front()
            .ok_or(scheduler_error::no_runnable_peer)?;
        let next_context = self
            .record(next)
            .and_then(scheduler_record::context)
            .ok_or(scheduler_error::corrupt_state)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .complete_and_dispatch(current, next)
            .map_err(scheduler_error::task)?;
        self.pop_queue();
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(next);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_completion {
            task: current,
            dispatch: scheduler_dispatch {
                task: next,
                sequence: dispatch_sequence.issued,
                context: next_context,
            },
        })
    }

    /// Fault the exact current task and directly wake one exact blocked task.
    pub(crate) fn fault_current_and_wake(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
        wake: task_identity,
    ) -> Result<scheduler_fault, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let wake_context = self.preflight_direct_wake(tasks, current, wake)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .fault_and_wake(current, wake)
            .map_err(scheduler_error::task)?;
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(wake);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_fault {
            task: current,
            dispatch: scheduler_dispatch {
                task: wake,
                sequence: dispatch_sequence.issued,
                context: wake_context,
            },
        })
    }

    /// Complete the exact current task and directly wake one exact blocked task.
    pub(crate) fn complete_current_and_wake(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
        wake: task_identity,
    ) -> Result<scheduler_completion, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        let wake_context = self.preflight_direct_wake(tasks, current, wake)?;
        let dispatch_sequence = issued_sequence(
            self.next_dispatch_sequence,
            self.dispatch_sequence_exhausted,
        )
        .ok_or(scheduler_error::dispatch_sequence_exhausted)?;

        tasks
            .complete_and_wake(current, wake)
            .map_err(scheduler_error::task)?;
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = Some(wake);
        self.commit_dispatch_sequence(dispatch_sequence);
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(scheduler_completion {
            task: current,
            dispatch: scheduler_dispatch {
                task: wake,
                sequence: dispatch_sequence.issued,
                context: wake_context,
            },
        })
    }

    pub(crate) fn complete_current(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        if self.queue_depth != 0 {
            return Err(scheduler_error::runnable_peer_present);
        }

        tasks
            .complete_scheduled(current)
            .map_err(scheduler_error::task)?;
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = None;
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(())
    }

    /// Fault the sole current task without fabricating a dispatch. Its saved
    /// context stays attached until architecture code resumes the bootstrap
    /// continuation and can safely withdraw the abandoned exception stack.
    #[cfg(any(test, feature = "application_profile_test"))]
    pub(crate) fn fault_last_current(
        &mut self,
        tasks: &mut task_table,
        current: task_identity,
    ) -> Result<(), scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        self.require_current(current)?;
        if self.queue_depth != 0 {
            return Err(scheduler_error::runnable_peer_present);
        }
        tasks
            .fault_scheduled(current)
            .map_err(scheduler_error::task)?;
        self.record_mut(current)
            .expect("validated current scheduler record")
            .context = scheduler_context::saved.encode();
        self.current = None;
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(())
    }

    /// Roll back one initial queued context before this scheduler has a current
    /// task. A dispatch reservation or any evidence of prior task execution is
    /// rejected. The architecture caller retains and clears its actual initial
    /// context; this transition only withdraws scheduler/task/circle ownership.
    ///
    /// Every rejection leaves all three tables unchanged. Queue survivors keep
    /// their relative order and dispatch/preemption sequences are not consumed.
    pub(crate) fn withdraw_unstarted_context(
        &mut self,
        circles: &mut circle_table,
        tasks: &mut task_table,
        identity: task_identity,
    ) -> Result<scheduler_context, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        if self.current.is_some() {
            return Err(scheduler_error::wrong_current_task);
        }
        let record_slot = self
            .record_index(identity)
            .ok_or(scheduler_error::task_not_admitted)?;
        let context = self.records[record_slot]
            .context()
            .ok_or(scheduler_error::corrupt_state)?;
        let snapshot = tasks.snapshot(identity).map_err(scheduler_error::task)?;
        if snapshot.state() != task_state::ready || context != scheduler_context::initial {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        let queue_offset = (0..self.queue_depth)
            .find(|offset| {
                self.queue_index(*offset)
                    .is_some_and(|index| self.queue[index] == Some(identity))
            })
            .ok_or(scheduler_error::corrupt_state)?;
        // Precompute the complete ring update before the final fallible task
        // transition. The validated topology guarantees exactly one occurrence,
        // a nonzero capacity, and an initialized entry at every shifted offset.
        let mut queue = self.queue;
        for offset in queue_offset..self.queue_depth - 1 {
            let destination = self
                .queue_index(offset)
                .ok_or(scheduler_error::corrupt_state)?;
            let source = self
                .queue_index(offset + 1)
                .ok_or(scheduler_error::corrupt_state)?;
            queue[destination] = self.queue[source];
        }
        let tail = self
            .queue_index(self.queue_depth - 1)
            .ok_or(scheduler_error::corrupt_state)?;
        queue[tail] = None;

        tasks
            .withdraw_unstarted_context(circles, identity)
            .map_err(scheduler_error::task)?;
        self.queue = queue;
        self.queue_depth -= 1;
        self.records[record_slot] = scheduler_record::empty();
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(context)
    }

    pub(crate) fn withdraw_terminal_context(
        &mut self,
        circles: &mut circle_table,
        tasks: &mut task_table,
        identity: task_identity,
    ) -> Result<scheduler_context, scheduler_error> {
        self.validate(tasks)?;
        self.require_no_reservation()?;
        let record_slot = self
            .record_index(identity)
            .ok_or(scheduler_error::task_not_admitted)?;
        let context = self.records[record_slot]
            .context()
            .ok_or(scheduler_error::corrupt_state)?;
        let snapshot = tasks.snapshot(identity).map_err(scheduler_error::task)?;
        if !snapshot.state().is_terminal() {
            return Err(scheduler_error::task(task_error::invalid_state));
        }

        tasks
            .withdraw_context(circles, identity)
            .map_err(scheduler_error::task)?;
        self.records[record_slot] = scheduler_record::empty();
        debug_assert!(self.is_structurally_valid(tasks));
        Ok(context)
    }

    fn is_structurally_valid(&self, tasks: &task_table) -> bool {
        #[cfg(any(test, feature = "veil_application_test"))]
        if let Some(call) = self.call {
            let cursor_matches = if call.return_sequence == u64::MAX {
                self.dispatch_sequence_exhausted && self.next_dispatch_sequence == u64::MAX
            } else {
                !self.dispatch_sequence_exhausted
                    && self.next_dispatch_sequence == call.return_sequence + 1
            };
            if call.owner != self.call_owner
                || call.service != self.call_service
                || call.caller == call.veil
                || call.sequence.checked_add(1) != Some(call.return_sequence)
                || self.reservation.is_some()
                || self.current != Some(call.veil)
                || !cursor_matches
                || self.record(call.caller).and_then(scheduler_record::context)
                    != Some(scheduler_context::saved)
                || !tasks.snapshot(call.caller).is_ok_and(|value| {
                    value.state() == task_state::blocked && value.context_attached()
                })
            {
                return false;
            }
        }
        if self.capacity > scheduler_capacity
            || self.queue_depth > self.capacity
            || (self.capacity == 0 && (self.queue_head != 0 || self.queue_depth != 0))
            || (self.capacity != 0 && self.queue_head >= self.capacity)
            || (self.dispatch_sequence_exhausted && self.next_dispatch_sequence != u64::MAX)
            || (self.preemption_sequence_exhausted && self.next_preemption_sequence != u64::MAX)
        {
            return false;
        }

        if let Some(reservation) = self.reservation {
            let cursor_follows_reservation = if reservation.sequence == u64::MAX {
                self.dispatch_sequence_exhausted && self.next_dispatch_sequence == u64::MAX
            } else {
                !self.dispatch_sequence_exhausted
                    && self.next_dispatch_sequence == reservation.sequence + 1
            };
            if !cursor_follows_reservation
                || reservation.task == reservation.current
                || self.current != Some(reservation.current)
                || issued_sequence(
                    self.next_preemption_sequence,
                    self.preemption_sequence_exhausted,
                )
                .is_none()
            {
                return false;
            }
        }

        let active_count = self.active_count();
        if active_count > self.capacity || tasks.attached_context_count() != active_count {
            return false;
        }

        for (index, queued) in self.queue.iter().enumerate() {
            let expected_live = index < self.capacity
                && (0..self.queue_depth).any(|offset| self.queue_index(offset) == Some(index));
            if queued.is_some() != expected_live {
                return false;
            }
        }

        for (first, record) in self.records.iter().enumerate() {
            let Some(identity) = record.task else {
                if record.context != no_context {
                    return false;
                }
                continue;
            };
            let Some(context) = record.context() else {
                return false;
            };
            if self.records[first + 1..]
                .iter()
                .any(|other| other.task == Some(identity))
            {
                return false;
            }
            let Ok(snapshot) = tasks.snapshot(identity) else {
                return false;
            };
            if !snapshot.context_attached() {
                return false;
            }
            let queue_occurrences = self
                .queue
                .iter()
                .filter(|queued| **queued == Some(identity))
                .count();
            let is_current = self.current == Some(identity);
            let placement_valid = match snapshot.state() {
                task_state::ready => {
                    context == scheduler_context::initial && queue_occurrences == 1 && !is_current
                }
                task_state::suspended => {
                    context == scheduler_context::saved && queue_occurrences == 1 && !is_current
                }
                task_state::blocked => {
                    context == scheduler_context::saved && queue_occurrences == 0 && !is_current
                }
                task_state::running => queue_occurrences == 0 && is_current,
                task_state::faulted | task_state::exited => {
                    context == scheduler_context::saved && queue_occurrences == 0 && !is_current
                }
            };
            if !placement_valid {
                return false;
            }
        }

        if let Some(current) = self.current
            && self.record(current).is_none()
        {
            return false;
        }
        if let Some(reservation) = self.reservation {
            let Ok(snapshot) = tasks.snapshot(reservation.task) else {
                return false;
            };
            let reservation_placement_valid = match snapshot.state() {
                task_state::blocked => self.queue_depth == 0,
                task_state::suspended => {
                    self.queue_depth == 1 && self.queue_front() == Some(reservation.task)
                }
                _ => false,
            };
            if !reservation_placement_valid {
                return false;
            }
        }
        self.queue
            .iter()
            .flatten()
            .all(|identity| self.record(*identity).is_some())
    }

    fn require_no_reservation(&self) -> Result<(), scheduler_error> {
        #[cfg(any(test, feature = "veil_application_test"))]
        if self.call.is_some() {
            return Err(scheduler_error::dispatch_reservation_pending);
        }
        if self.reservation.is_some() {
            Err(scheduler_error::dispatch_reservation_pending)
        } else {
            Ok(())
        }
    }

    fn require_reservation(
        &self,
        reservation: dispatch_reservation,
    ) -> Result<(), scheduler_error> {
        match self.reservation {
            None => Err(scheduler_error::dispatch_reservation_missing),
            Some(expected) if expected != reservation => {
                Err(scheduler_error::wrong_dispatch_reservation)
            }
            Some(_) => Ok(()),
        }
    }

    fn preflight_reserved_wake(
        &self,
        tasks: &task_table,
        reservation: dispatch_reservation,
    ) -> Result<(), scheduler_error> {
        if self.queue_depth != 0 || self.queue_depth == self.capacity {
            return Err(scheduler_error::runnable_peer_present);
        }
        let tail = self.queue_tail().ok_or(scheduler_error::corrupt_state)?;
        if self.queue[tail].is_some() {
            return Err(scheduler_error::corrupt_state);
        }
        let record = self
            .record(reservation.task)
            .ok_or(scheduler_error::task_not_admitted)?;
        if record.context() != Some(scheduler_context::saved) {
            return Err(scheduler_error::corrupt_state);
        }
        let snapshot = tasks
            .snapshot(reservation.task)
            .map_err(scheduler_error::task)?;
        if snapshot.state() != task_state::blocked {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        Ok(())
    }

    fn require_reserved_selection(
        &self,
        current: task_identity,
        reservation: dispatch_reservation,
    ) -> Result<(), scheduler_error> {
        self.require_current(current)?;
        if current != reservation.current {
            return Err(scheduler_error::wrong_current_task);
        }
        if self.queue_depth != 1 || self.queue_front() != Some(reservation.task) {
            return Err(scheduler_error::wrong_runnable_task);
        }
        let record = self
            .record(reservation.task)
            .ok_or(scheduler_error::task_not_admitted)?;
        if record.context() != Some(scheduler_context::saved) {
            return Err(scheduler_error::corrupt_state);
        }
        Ok(())
    }

    fn require_current(&self, identity: task_identity) -> Result<(), scheduler_error> {
        match self.current {
            None => Err(scheduler_error::no_current_task),
            Some(current) if current != identity => Err(scheduler_error::wrong_current_task),
            Some(_) => Ok(()),
        }
    }

    fn preflight_direct_wake(
        &self,
        tasks: &task_table,
        current: task_identity,
        wake: task_identity,
    ) -> Result<scheduler_context, scheduler_error> {
        if current == wake {
            return Err(scheduler_error::task(task_error::same_task));
        }
        if self.queue_depth != 0 {
            return Err(scheduler_error::runnable_peer_present);
        }
        let wake_record = self
            .record(wake)
            .ok_or(scheduler_error::task_not_admitted)?;
        let wake_context = wake_record
            .context()
            .ok_or(scheduler_error::corrupt_state)?;
        if wake_context != scheduler_context::saved {
            return Err(scheduler_error::corrupt_state);
        }
        let wake_snapshot = tasks.snapshot(wake).map_err(scheduler_error::task)?;
        if wake_snapshot.state() != task_state::blocked {
            return Err(scheduler_error::task(task_error::invalid_state));
        }
        Ok(wake_context)
    }

    fn active_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.task.is_some())
            .count()
    }

    fn record(&self, identity: task_identity) -> Option<scheduler_record> {
        self.record_index(identity).map(|index| self.records[index])
    }

    fn record_mut(&mut self, identity: task_identity) -> Option<&mut scheduler_record> {
        let index = self.record_index(identity)?;
        self.records.get_mut(index)
    }

    fn record_index(&self, identity: task_identity) -> Option<usize> {
        self.records
            .iter()
            .position(|record| record.task == Some(identity))
    }

    fn queue_front(&self) -> Option<task_identity> {
        if self.queue_depth == 0 {
            None
        } else {
            self.queue[self.queue_head]
        }
    }

    fn queue_tail(&self) -> Option<usize> {
        self.queue_index(self.queue_depth)
    }

    fn queue_index(&self, offset: usize) -> Option<usize> {
        if self.capacity == 0 {
            None
        } else {
            Some((self.queue_head + offset) % self.capacity)
        }
    }

    fn pop_queue(&mut self) {
        debug_assert!(self.queue_depth != 0 && self.capacity != 0);
        self.queue[self.queue_head] = None;
        self.queue_head = (self.queue_head + 1) % self.capacity;
        self.queue_depth -= 1;
    }

    fn push_queue(&mut self, identity: task_identity) {
        debug_assert!(self.queue_depth < self.capacity);
        let tail = self.queue_tail().expect("nonzero validated queue capacity");
        debug_assert!(self.queue[tail].is_none());
        self.queue[tail] = Some(identity);
        self.queue_depth += 1;
    }

    fn commit_dispatch_sequence(&mut self, sequence: sequence_issue) {
        self.next_dispatch_sequence = sequence.next;
        self.dispatch_sequence_exhausted = sequence.exhausted;
    }

    fn commit_preemption_sequence(&mut self, sequence: sequence_issue) {
        self.next_preemption_sequence = sequence.next;
        self.preemption_sequence_exhausted = sequence.exhausted;
    }
}

#[derive(Clone, Copy)]
struct sequence_issue {
    issued: u64,
    next: u64,
    exhausted: bool,
}

const fn issued_sequence(next: u64, exhausted: bool) -> Option<sequence_issue> {
    if exhausted {
        return None;
    }
    Some(if next == u64::MAX {
        sequence_issue {
            issued: next,
            next,
            exhausted: true,
        }
    } else {
        sequence_issue {
            issued: next,
            next: next + 1,
            exhausted: false,
        }
    })
}

const fn issued_sequence_pair(
    next: u64,
    exhausted: bool,
) -> Option<(sequence_issue, sequence_issue)> {
    let Some(first) = issued_sequence(next, exhausted) else {
        return None;
    };
    let Some(second) = issued_sequence(first.next, first.exhausted) else {
        return None;
    };
    Some((first, second))
}

const fn available_sequence(next: u64, exhausted: bool) -> Option<u64> {
    if exhausted { None } else { Some(next) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circle::{circle_child_kind, circle_table};
    use crate::identity::{fixture_principal_id, task_id};
    use crate::session_input::{
        session_input_action, session_input_binding, session_input_consumer_identity,
        session_input_endpoint, session_input_endpoint_identity, session_input_origin,
        session_input_prepared_publication, session_input_receive_preparation,
        session_input_route_epoch, session_input_session_identity, session_input_wait_commit,
    };
    use crate::task::{synthetic_origin_metadata, task_binding, task_bounds};

    fn call_fixture() -> (
        circle_table,
        task_table,
        scheduler_table,
        [task_identity; 3],
    ) {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let identities =
            core::array::from_fn(|index| create_task(&mut circles, &mut tasks, index as u64 + 1));
        let mut scheduler = scheduler_table::new(3).unwrap();
        // The service deliberately precedes the veil in FIFO order.
        for identity in identities {
            scheduler.admit(&mut circles, &mut tasks, identity).unwrap();
        }
        scheduler.start_next(&mut tasks).unwrap();
        (circles, tasks, scheduler, identities)
    }

    fn shared_call_fixture() -> (
        circle_table,
        task_table,
        scheduler_table,
        [task_identity; 5],
        scheduler_call_service,
    ) {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let identities =
            core::array::from_fn(|index| create_task(&mut circles, &mut tasks, index as u64 + 1));
        let mut scheduler = scheduler_table::new(5).unwrap();
        for identity in identities {
            scheduler.admit(&mut circles, &mut tasks, identity).unwrap();
        }
        let service = scheduler
            .register_call_service(&tasks, identities[4], [identities[0], identities[2]])
            .unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        (circles, tasks, scheduler, identities, service)
    }

    #[test]
    fn shared_service_alternates_exact_callers_preserving_unrelated_fifo_and_rejecting_replay() {
        let (_, mut tasks, mut scheduler, [a, a_service, b, b_service, veil], service) =
            shared_call_fixture();
        let mut waiting = None;
        let mut pending = None;
        scheduler
            .prepare_service_call_handoff(&mut tasks, a, &service, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let first = pending.as_ref().unwrap().reservation;
        assert_eq!(scheduler.queue_front(), Some(a_service));
        assert_eq!(scheduler.queue_depth, 3);
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, a)
                .unwrap()
                .dispatch()
                .task(),
            a_service
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, a_service)
                .unwrap()
                .dispatch()
                .task(),
            b
        );
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        {
            let prepared = scheduler
                .prepare_service_call_handoff(&mut tasks, b, &service, &mut waiting, &mut pending)
                .unwrap();
            assert_eq!(prepared.transition().task(), veil);
        }
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(waiting.is_some() && pending.is_none());
        scheduler
            .prepare_service_call_handoff(&mut tasks, b, &service, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        assert_eq!(scheduler.queue_front(), Some(b_service));
        assert_eq!(scheduler.queue_depth, 3);
        assert_eq!(pending.as_ref().unwrap().caller(), b);
        let second = pending.as_ref().unwrap().reservation;
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        pending.as_mut().unwrap().reservation = first;
        assert!(
            scheduler
                .validate_call_authorization(&tasks, pending.as_ref().unwrap())
                .is_err()
        );
        assert!(
            scheduler
                .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                .is_err()
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        pending.as_mut().unwrap().reservation = second;
        assert_eq!(
            scheduler
                .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                .unwrap()
                .commit()
                .task(),
            b
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, b)
                .unwrap()
                .dispatch()
                .task(),
            b_service
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, b_service)
                .unwrap()
                .dispatch()
                .task(),
            a
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, a)
                .unwrap()
                .dispatch()
                .task(),
            a_service
        );
        scheduler.validate(&tasks).unwrap();
    }

    #[test]
    fn shared_service_rejects_foreign_membership_stale_generation_and_legacy_bypass_atomically() {
        let (_, mut tasks, mut scheduler, [a, native, b, _, veil], mut service) =
            shared_call_fixture();
        let (_, _, _, _, foreign) = shared_call_fixture();
        let mut pending = None;
        let mut waiting = None;
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(
            scheduler
                .prepare_service_call_handoff(&mut tasks, a, &foreign, &mut waiting, &mut pending)
                .is_err()
        );
        assert!(
            scheduler
                .prepare_call_handoff(&mut tasks, a, veil, &mut waiting, &mut pending)
                .is_err()
        );
        assert!(
            scheduler
                .register_call_service(&tasks, veil, [a, b])
                .is_err()
        );
        let stale = task_identity::new(
            task_id::try_from_parts(a.task().slot(), a.task().generation().get() + 1).unwrap(),
            a.circle(),
        );
        assert!(
            scheduler
                .prepare_service_call_handoff(
                    &mut tasks,
                    stale,
                    &service,
                    &mut waiting,
                    &mut pending
                )
                .is_err()
        );
        let original = service.callers;
        service.callers[0] = Some(stale);
        assert!(
            scheduler
                .prepare_service_call_handoff(
                    &mut tasks,
                    stale,
                    &service,
                    &mut waiting,
                    &mut pending
                )
                .is_err()
        );
        service.callers = original;
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(pending.is_none() && waiting.is_none());
        scheduler.preempt_current(&mut tasks, a).unwrap();
        let before = scheduler.test_snapshot();
        assert!(
            scheduler
                .prepare_service_call_handoff(
                    &mut tasks,
                    native,
                    &service,
                    &mut waiting,
                    &mut pending
                )
                .is_err()
        );
        assert_eq!(scheduler.test_snapshot(), before);
    }

    #[test]
    fn shared_service_member_retirement_preserves_survivor_and_finally_revokes_membership() {
        let (_, mut tasks, mut scheduler, [a, a_service, b, b_service, veil], mut service) =
            shared_call_fixture();
        let mut waiting = None;
        let mut pending = None;
        scheduler
            .prepare_service_call_handoff(&mut tasks, a, &service, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(
            scheduler
                .retire_service_caller(&tasks, &mut service, a)
                .is_err()
        );
        assert!(
            scheduler
                .retire_service_veil(&mut tasks, &service, &mut waiting)
                .is_err()
        );
        assert!(
            scheduler
                .retire_waiting_veil(&mut tasks, &mut waiting)
                .is_err()
        );
        assert!(scheduler.retire_call_service(&tasks, &mut service).is_err());
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        scheduler.fault_current(&mut tasks, a).unwrap();
        scheduler
            .retire_service_caller(&tasks, &mut service, a)
            .unwrap();
        assert!(!service.contains(a) && service.contains(b) && !service.is_empty());
        assert!(
            scheduler
                .retire_service_caller(&tasks, &mut service, a)
                .is_err()
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, a_service)
                .unwrap()
                .dispatch()
                .task(),
            b
        );
        scheduler
            .prepare_service_call_handoff(&mut tasks, b, &service, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        assert_eq!(
            scheduler
                .complete_current_and_dispatch(&mut tasks, b)
                .unwrap()
                .dispatch()
                .task(),
            b_service
        );
        scheduler
            .retire_service_caller(&tasks, &mut service, b)
            .unwrap();
        assert!(service.is_empty() && !service.is_revoked());
        scheduler
            .retire_service_veil(&mut tasks, &service, &mut waiting)
            .unwrap();
        assert_eq!(tasks.snapshot(veil).unwrap().state(), task_state::exited);
        scheduler.retire_call_service(&tasks, &mut service).unwrap();
        assert!(service.is_revoked() && waiting.is_none() && pending.is_none());
        assert!(scheduler.retire_call_service(&tasks, &mut service).is_err());
        scheduler.validate(&tasks).unwrap();
    }

    #[test]
    fn shared_service_fault_revokes_both_dependents_and_preserves_survivor_fifo() {
        for peer_has_run in [false, true] {
            let (_, mut tasks, mut scheduler, [a, a_service, b, b_service, veil], mut service) =
                shared_call_fixture();
            let mut waiting = None;
            let mut pending = None;
            if peer_has_run {
                scheduler
                    .prepare_service_call_handoff(
                        &mut tasks,
                        a,
                        &service,
                        &mut waiting,
                        &mut pending,
                    )
                    .unwrap()
                    .commit();
                scheduler
                    .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                    .unwrap()
                    .commit();
                for current in [a, a_service, b, b_service] {
                    scheduler.preempt_current(&mut tasks, current).unwrap();
                }
                assert_eq!(tasks.snapshot(b).unwrap().state(), task_state::suspended);
            }
            scheduler
                .prepare_service_call_handoff(&mut tasks, a, &service, &mut waiting, &mut pending)
                .unwrap()
                .commit();
            let before = scheduler.test_snapshot();
            let tasks_before = tasks.test_snapshot();
            assert!(
                scheduler
                    .prepare_call_fault(&mut tasks, &mut pending)
                    .is_err()
            );
            {
                let prepared = scheduler
                    .prepare_service_call_fault(&mut tasks, &mut service, &mut pending)
                    .unwrap();
                assert_eq!(prepared.transition().task(), a_service);
            }
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
            assert!(pending.is_some() && service.contains(a) && service.contains(b));
            let dispatch = scheduler
                .prepare_service_call_fault(&mut tasks, &mut service, &mut pending)
                .unwrap()
                .commit();
            assert_eq!(dispatch.task(), a_service);
            for identity in [a, b, veil] {
                assert_eq!(
                    tasks.snapshot(identity).unwrap().state(),
                    task_state::faulted
                );
            }
            assert_eq!(scheduler.queue_front(), Some(b_service));
            assert_eq!(scheduler.queue_depth, 1);
            assert!(
                pending.is_none()
                    && waiting.is_none()
                    && service.is_revoked()
                    && service.is_empty()
            );
            assert!(
                scheduler
                    .prepare_service_call_fault(&mut tasks, &mut service, &mut pending)
                    .is_err()
            );
            scheduler.validate(&tasks).unwrap();
            assert_eq!(
                scheduler
                    .complete_current_and_dispatch(&mut tasks, a_service)
                    .unwrap()
                    .dispatch()
                    .task(),
                b_service
            );
            scheduler.complete_current(&mut tasks, b_service).unwrap();
            scheduler.validate(&tasks).unwrap();
        }
    }

    #[test]
    fn shared_service_registration_rejects_aliases_and_stale_tasks_before_minting_authority() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let [a, b, veil] =
            core::array::from_fn(|index| create_task(&mut circles, &mut tasks, index as u64 + 1));
        let mut scheduler = scheduler_table::new(3).unwrap();
        for identity in [a, b, veil] {
            scheduler.admit(&mut circles, &mut tasks, identity).unwrap();
        }
        let stale = task_identity::new(
            task_id::try_from_parts(b.task().slot(), b.task().generation().get() + 1).unwrap(),
            b.circle(),
        );
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        for (selected_veil, members) in [
            (veil, [a, a]),
            (veil, [a, veil]),
            (veil, [a, stale]),
            (stale, [a, b]),
        ] {
            assert!(
                scheduler
                    .register_call_service(&tasks, selected_veil, members)
                    .is_err()
            );
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
        }
        let service = scheduler
            .register_call_service(&tasks, veil, [a, b])
            .unwrap();
        let before = scheduler.test_snapshot();
        assert!(
            scheduler
                .register_call_service(&tasks, veil, [a, b])
                .is_err()
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(service.contains(a) && service.contains(b) && !service.contains(veil));
    }

    #[test]
    fn shared_service_fault_without_unrelated_survivor_is_failure_atomic() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let [a, b, veil] =
            core::array::from_fn(|index| create_task(&mut circles, &mut tasks, index as u64 + 1));
        let mut scheduler = scheduler_table::new(3).unwrap();
        for identity in [a, b, veil] {
            scheduler.admit(&mut circles, &mut tasks, identity).unwrap();
        }
        let mut service = scheduler
            .register_call_service(&tasks, veil, [a, b])
            .unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        let mut waiting = None;
        let mut pending = None;
        scheduler
            .prepare_service_call_handoff(&mut tasks, a, &service, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(matches!(
            scheduler.prepare_service_call_fault(&mut tasks, &mut service, &mut pending),
            Err(scheduler_error::no_runnable_peer)
        ));
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(
            pending.is_some() && waiting.is_none() && service.contains(a) && service.contains(b)
        );
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        scheduler.validate(&tasks).unwrap();
    }

    #[test]
    fn call_reply_preserves_service_queue_and_reuses_only_sealed_waiting_veil() {
        let (_circles, mut tasks, mut scheduler, [caller, service, veil]) = call_fixture();
        let mut pending = None;
        let mut waiting = None;
        let mut last_sequence = 0;
        for turn in 0..3 {
            let before = scheduler.test_snapshot();
            let tasks_before = tasks.test_snapshot();
            {
                let prepared = scheduler
                    .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
                    .unwrap();
                assert_eq!(prepared.transition().task(), veil);
            }
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
            assert!(pending.is_none());
            assert_eq!(waiting.is_some(), turn != 0);
            let dispatch = scheduler
                .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
                .unwrap()
                .commit();
            assert_eq!(
                dispatch.context(),
                if turn == 0 {
                    scheduler_context::initial
                } else {
                    scheduler_context::saved
                }
            );
            let authority = pending.as_ref().unwrap();
            assert_eq!(authority.caller(), caller);
            assert_eq!(authority.veil(), veil);
            assert!(authority.sequence() > last_sequence);
            last_sequence = authority.sequence();
            scheduler
                .validate_call_authorization(&tasks, authority)
                .unwrap();
            assert_eq!(scheduler.queue_front(), Some(service));
            assert_eq!(scheduler.queue_depth, 1);
            assert_eq!(tasks.snapshot(caller).unwrap().state(), task_state::blocked);
            assert!(waiting.is_none());
            let before = scheduler.test_snapshot();
            let tasks_before = tasks.test_snapshot();
            assert_eq!(
                scheduler.preempt_current(&mut tasks, veil),
                Err(scheduler_error::dispatch_reservation_pending)
            );
            {
                let prepared = scheduler
                    .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                    .unwrap();
                prepared
                    .scheduler()
                    .validate_call_authorization(prepared.tasks(), prepared.authorization())
                    .unwrap();
                assert_eq!(prepared.transition().task(), caller);
                assert_eq!(prepared.transition().sequence(), last_sequence + 1);
            }
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
            assert!(pending.is_some());
            assert!(waiting.is_none());
            scheduler
                .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                .unwrap()
                .commit();
            assert!(pending.is_none());
            assert!(waiting.is_some());
            assert_eq!(scheduler.queue_front(), Some(service));
            assert_eq!(scheduler.queue_depth, 1);
            assert_eq!(tasks.snapshot(caller).unwrap().state(), task_state::running);
            assert_eq!(tasks.snapshot(veil).unwrap().state(), task_state::blocked);
            let before = scheduler.test_snapshot();
            assert!(matches!(
                scheduler.prepare_call_reply(&mut tasks, &mut pending, &mut waiting,),
                Err(scheduler_error::authorization_missing)
            ));
            assert_eq!(scheduler.test_snapshot(), before);
        }
        // Ordinary application yield still selects the untouched native service.
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, caller)
                .unwrap()
                .dispatch()
                .task(),
            service
        );
        assert_eq!(
            scheduler
                .preempt_current(&mut tasks, service)
                .unwrap()
                .dispatch()
                .task(),
            caller
        );
        scheduler
            .complete_current_and_dispatch(&mut tasks, caller)
            .unwrap();
        scheduler
            .retire_waiting_veil(&mut tasks, &mut waiting)
            .unwrap();
        assert_eq!(tasks.snapshot(veil).unwrap().state(), task_state::exited);
        assert!(waiting.is_none());
        scheduler.validate(&tasks).unwrap();
    }

    #[test]
    fn call_authority_rejects_foreign_owner_and_every_binding_mutation_without_changes() {
        let (_, mut tasks, mut scheduler, [caller, _, veil]) = call_fixture();
        let (_, mut other_tasks, mut other, [other_caller, _, other_veil]) = call_fixture();
        assert_eq!(caller, other_caller);
        assert_eq!(veil, other_veil);
        let (mut pending, mut waiting, mut foreign, mut foreign_wait) = (None, None, None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        other
            .prepare_call_handoff(
                &mut other_tasks,
                other_caller,
                other_veil,
                &mut foreign_wait,
                &mut foreign,
            )
            .unwrap()
            .commit();
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert_eq!(
            scheduler.validate_call_authorization(&tasks, foreign.as_ref().unwrap()),
            Err(scheduler_error::wrong_dispatch_reservation)
        );
        assert!(matches!(
            scheduler.prepare_call_reply(&mut tasks, &mut foreign, &mut waiting),
            Err(scheduler_error::wrong_dispatch_reservation)
        ));
        let original = pending.as_ref().unwrap().reservation;
        let changed_caller = task_identity::new(
            crate::identity::task_id::try_from_parts(
                caller.task().slot(),
                caller.task().generation().get() + 1,
            )
            .unwrap(),
            caller.circle(),
        );
        let variants = [
            call_reservation {
                owner: original.owner.wrapping_add(1),
                ..original
            },
            call_reservation {
                sequence: original.sequence.wrapping_add(1),
                ..original
            },
            call_reservation {
                return_sequence: original.return_sequence.wrapping_add(1),
                ..original
            },
            call_reservation {
                caller: changed_caller,
                ..original
            },
            call_reservation {
                caller: veil,
                ..original
            },
            call_reservation {
                veil: caller,
                ..original
            },
        ];
        for changed in variants {
            pending.as_mut().unwrap().reservation = changed;
            assert!(
                scheduler
                    .validate_call_authorization(&tasks, pending.as_ref().unwrap())
                    .is_err()
            );
            assert!(
                scheduler
                    .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
                    .is_err()
            );
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
            assert!(waiting.is_none());
            assert!(pending.is_some());
        }
        pending.as_mut().unwrap().reservation = original;
        scheduler
            .validate_call_authorization(&tasks, pending.as_ref().unwrap())
            .unwrap();
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        // A matching numeric veil identity from a different owner cannot wake it.
        other
            .prepare_call_reply(&mut other_tasks, &mut foreign, &mut foreign_wait)
            .unwrap()
            .commit();
        let before = scheduler.test_snapshot();
        assert!(
            scheduler
                .prepare_call_handoff(&mut tasks, caller, veil, &mut foreign_wait, &mut pending)
                .is_err()
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(foreign_wait.is_some());
        assert!(pending.is_none());
        // Losing the sealed waiting token never turns a blocked veil into ready.
        let mut absent = None;
        assert!(matches!(
            scheduler.prepare_call_handoff(&mut tasks, caller, veil, &mut absent, &mut pending),
            Err(scheduler_error::authorization_missing)
        ));
    }

    #[test]
    fn call_fault_contains_both_dependencies_and_preserves_service_progress() {
        let (mut circles, mut tasks, mut scheduler, [caller, service, veil]) = call_fixture();
        let (mut pending, mut waiting) = (None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        {
            let prepared = scheduler
                .prepare_call_fault(&mut tasks, &mut pending)
                .unwrap();
            assert_eq!(prepared.transition().task(), service);
        }
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(pending.is_some());
        let transition = scheduler
            .prepare_call_fault(&mut tasks, &mut pending)
            .unwrap()
            .commit();
        assert_eq!(transition.task(), service);
        assert_eq!(transition.context(), scheduler_context::initial);
        assert_eq!(tasks.snapshot(caller).unwrap().state(), task_state::faulted);
        assert_eq!(tasks.snapshot(veil).unwrap().state(), task_state::faulted);
        assert_eq!(
            tasks.snapshot(service).unwrap().state(),
            task_state::running
        );
        assert!(pending.is_none() && waiting.is_none());
        assert_eq!(scheduler.queue_depth, 0);
        assert!(scheduler.call.is_none());
        scheduler.complete_current(&mut tasks, service).unwrap();
        for task in [caller, service, veil] {
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, task)
                .unwrap();
            tasks.destroy(&mut circles, task).unwrap();
            circles.destroy(task.circle()).unwrap();
        }
        assert_eq!(tasks.attached_context_count(), 0);
        assert_eq!(scheduler.active_count(), 0);
    }

    #[test]
    fn call_reserves_last_dispatch_for_reply_and_rejects_overflow_atomically() {
        let (_, mut tasks, mut scheduler, [caller, service, veil]) = call_fixture();
        let (mut pending, mut waiting) = (None, None);
        scheduler.next_dispatch_sequence = u64::MAX;
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(matches!(
            scheduler.prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending),
            Err(scheduler_error::dispatch_sequence_exhausted)
        ));
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(pending.is_none() && waiting.is_none());
        scheduler.next_dispatch_sequence = u64::MAX - 1;
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        assert!(scheduler.dispatch_sequence_exhausted);
        let reply = scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        assert_eq!(reply.sequence(), u64::MAX);
        assert_eq!(scheduler.queue_front(), Some(service));
        let before = scheduler.test_snapshot();
        assert!(matches!(
            scheduler.prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending),
            Err(scheduler_error::dispatch_sequence_exhausted)
        ));
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(waiting.is_some() && pending.is_none());
    }

    #[test]
    fn call_handoff_rejects_wrong_current_generation_and_occupied_slot_atomically() {
        let (_, mut tasks, mut scheduler, [caller, service, veil]) = call_fixture();
        let (mut pending, mut waiting) = (None, None);
        let stale = task_identity::new(
            crate::identity::task_id::try_from_parts(
                veil.task().slot(),
                veil.task().generation().get() + 1,
            )
            .unwrap(),
            veil.circle(),
        );
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        for (source, target) in [(service, veil), (caller, caller), (caller, stale)] {
            assert!(
                scheduler
                    .prepare_call_handoff(&mut tasks, source, target, &mut waiting, &mut pending)
                    .is_err()
            );
            assert_eq!(scheduler.test_snapshot(), before);
            assert!(tasks.test_snapshot() == tasks_before);
        }
        let (_, mut other_tasks, mut other, [other_caller, _, other_veil]) = call_fixture();
        other
            .prepare_call_handoff(
                &mut other_tasks,
                other_caller,
                other_veil,
                &mut waiting,
                &mut pending,
            )
            .unwrap()
            .commit();
        assert!(matches!(
            scheduler.prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending),
            Err(scheduler_error::authorization_slot_occupied)
        ));
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
    }

    fn bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("task bounds")
    }

    fn create_task(
        circles: &mut circle_table,
        tasks: &mut task_table,
        principal: u64,
    ) -> task_identity {
        let circle = circles.create().expect("circle");
        tasks
            .create(
                circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(principal).expect("principal"),
                    synthetic_origin_metadata::human_role_fixture,
                    bounds(),
                ),
            )
            .expect("task")
    }

    fn two_tasks() -> (circle_table, task_table, task_identity, task_identity) {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let first = create_task(&mut circles, &mut tasks, 1);
        let second = create_task(&mut circles, &mut tasks, 2);
        (circles, tasks, first, second)
    }

    fn commit_reserved_block(
        scheduler: &mut scheduler_table,
        tasks: &mut task_table,
        current: task_identity,
        next: task_identity,
        wait: session_input_wait_identity,
        readiness: session_input_readiness_identity,
    ) -> (scheduler_block, Option<scheduler_wake_authorization>) {
        let mut authorization = None;
        let prepared = scheduler
            .prepare_block_current_and_reserve(
                tasks,
                current,
                next,
                wait,
                readiness,
                &mut authorization,
            )
            .expect("prepare reserved block");
        let expected = prepared.transition();
        let committed = prepared.commit();
        assert_eq!(committed, expected);
        assert!(authorization.is_some());
        (expected, authorization)
    }

    fn prepare_endpoint_publication(
        task: task_identity,
        seed: usize,
    ) -> (
        session_input_endpoint,
        session_input_prepared_publication,
        session_input_wait_identity,
        session_input_readiness_identity,
    ) {
        let binding = session_input_binding::new(
            session_input_endpoint_identity::try_new(seed, 1).expect("endpoint identity"),
            session_input_session_identity::try_new(seed as u64 + 1, 1).expect("session identity"),
            session_input_consumer_identity::try_new(seed as u64 + 2, 1)
                .expect("consumer identity"),
            session_input_route_epoch::try_new(seed as u64 + 3).expect("route identity"),
            task,
        );
        let mut endpoint = session_input_endpoint::new(binding);
        let prepared_wait = match endpoint
            .prepare_receive(binding)
            .expect("prepare endpoint wait")
        {
            session_input_receive_preparation::wait(prepared) => prepared,
            session_input_receive_preparation::ready => panic!("empty endpoint reported ready"),
        };
        let blocked = match endpoint
            .commit_wait(prepared_wait)
            .expect("commit endpoint wait")
        {
            session_input_wait_commit::blocked(blocked) => blocked,
            session_input_wait_commit::ready => panic!("empty endpoint avoided wait"),
        };
        let wait = blocked.identity();
        let readiness = blocked.readiness();
        let action = session_input_action::try_insert_text(b"x").expect("private input action");
        let publication =
            match endpoint.prepare_publication(binding, session_input_origin::human_text, action) {
                Ok(prepared) => prepared,
                Err(rejection) => panic!("prepare publication: {}", rejection.error().code()),
            };
        assert!(publication.wake_correlation().is_some());
        (endpoint, publication, wait, readiness)
    }

    fn commit_reserved_wake(
        scheduler: &mut scheduler_table,
        tasks: &mut task_table,
        scheduler_authorization: &mut Option<scheduler_wake_authorization>,
        endpoint: &mut session_input_endpoint,
        publication: session_input_prepared_publication,
    ) -> (task_identity, Option<scheduler_selection_authorization>) {
        let correlation = publication
            .wake_correlation()
            .expect("publication wake correlation");
        let mut composed_authorization = None;
        let mut selection_authorization = None;
        let preflight = scheduler
            .preflight_endpoint_wake(
                tasks,
                scheduler_authorization,
                correlation,
                &composed_authorization,
                &selection_authorization,
            )
            .expect("preflight endpoint wake");
        assert!(preflight.correlation() == correlation);
        let receipt = match endpoint.commit_publication(publication) {
            Ok(receipt) => receipt,
            Err(rejection) => panic!("commit publication: {}", rejection.error().code()),
        };
        let endpoint_authorization = receipt.into_wake().expect("endpoint wake authority");
        if let Err(rejection) = compose_endpoint_wake_authorization(
            scheduler_authorization,
            endpoint_authorization,
            preflight,
            &mut composed_authorization,
        ) {
            panic!("compose endpoint wake: {:?}", rejection.error());
        }
        let prepared = match scheduler.prepare_endpoint_wake(
            tasks,
            &mut composed_authorization,
            &mut selection_authorization,
        ) {
            Ok(prepared) => prepared,
            Err(error) => panic!("wake rejected: {error:?}"),
        };
        let expected = prepared.task();
        let committed = prepared.commit();
        assert_eq!(committed.task(), expected);
        assert!(composed_authorization.is_none());
        assert!(selection_authorization.is_some());
        (expected, selection_authorization)
    }

    #[test]
    fn capacity_is_bounded_and_zero_capacity_is_inert() {
        assert_eq!(
            scheduler_table::new(scheduler_capacity + 1),
            Err(scheduler_error::invalid_capacity)
        );
        let (mut circles, mut tasks, first, _) = two_tasks();
        let mut scheduler = scheduler_table::new(0).expect("zero capacity");
        let before = scheduler.test_snapshot();

        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, first),
            Err(scheduler_error::capacity_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(!tasks.snapshot(first).expect("task").context_attached());
        assert_eq!(
            scheduler.start_next(&mut tasks),
            Err(scheduler_error::no_runnable_task)
        );
        assert!(scheduler.validate(&tasks).is_ok());

        tasks.start(first).expect("ordinary start");
        tasks.exit(first).expect("ordinary exit");
        tasks
            .destroy(&mut circles, first)
            .expect("ordinary destroy");
    }

    #[test]
    fn completion_dispatch_preserves_peer_context_and_terminal_owner() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        scheduler.admit(&mut circles, &mut tasks, first).unwrap();
        scheduler.admit(&mut circles, &mut tasks, second).unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        scheduler.preempt_current(&mut tasks, first).unwrap();
        let transition = scheduler
            .complete_current_and_dispatch(&mut tasks, second)
            .unwrap();
        assert_eq!(transition.task(), second);
        assert_eq!(transition.dispatch().task(), first);
        assert_eq!(transition.dispatch().context(), scheduler_context::saved);
        assert_eq!(transition.dispatch().sequence(), 2);
        assert_eq!(tasks.snapshot(second).unwrap().state(), task_state::exited);
        assert!(tasks.snapshot(second).unwrap().context_attached());
        assert_eq!(tasks.snapshot(first).unwrap().state(), task_state::running);
        assert_eq!(scheduler.snapshot(&tasks).unwrap().runnable(), 0);
        scheduler.complete_current(&mut tasks, first).unwrap();
        for identity in [first, second] {
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, identity)
                .unwrap();
            tasks.destroy(&mut circles, identity).unwrap();
        }
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(tasks.attached_context_count(), 0);
    }

    #[test]
    fn completion_dispatch_rejections_preserve_tasks_and_sequences() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        scheduler.admit(&mut circles, &mut tasks, first).unwrap();
        scheduler.admit(&mut circles, &mut tasks, second).unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).unwrap();
        let second_before = tasks.snapshot(second).unwrap();
        assert!(
            scheduler
                .complete_current_and_dispatch(&mut tasks, second)
                .is_err()
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first).unwrap(), first_before);
        assert_eq!(tasks.snapshot(second).unwrap(), second_before);
        scheduler.next_dispatch_sequence = u64::MAX;
        scheduler.dispatch_sequence_exhausted = true;
        let before = scheduler.test_snapshot();
        assert_eq!(
            scheduler.complete_current_and_dispatch(&mut tasks, first),
            Err(scheduler_error::dispatch_sequence_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first).unwrap(), first_before);
        assert_eq!(tasks.snapshot(second).unwrap(), second_before);
        scheduler.dispatch_sequence_exhausted = false;
        scheduler
            .complete_current_and_dispatch(&mut tasks, first)
            .unwrap();
        let before = scheduler.test_snapshot();
        assert_eq!(
            scheduler.complete_current_and_dispatch(&mut tasks, second),
            Err(scheduler_error::no_runnable_peer)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(second).unwrap().state(), task_state::running);
    }

    #[test]
    fn two_task_round_robin_fault_and_completion_are_exact() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("admit second");

        let first_dispatch = scheduler.start_next(&mut tasks).expect("start first");
        assert_eq!(first_dispatch.task(), first);
        assert_eq!(first_dispatch.sequence(), 0);
        assert_eq!(first_dispatch.context(), scheduler_context::initial);

        let first_preemption = scheduler
            .preempt_current(&mut tasks, first)
            .expect("preempt first");
        assert_eq!(first_preemption.task(), first);
        assert_eq!(first_preemption.sequence(), 0);
        assert_eq!(first_preemption.dispatch().task(), second);
        assert_eq!(first_preemption.dispatch().sequence(), 1);
        assert_eq!(
            first_preemption.dispatch().context(),
            scheduler_context::initial
        );
        assert_eq!(
            tasks.snapshot(first).expect("first").state(),
            task_state::suspended
        );

        let second_preemption = scheduler
            .preempt_current(&mut tasks, second)
            .expect("preempt second");
        assert_eq!(second_preemption.sequence(), 1);
        assert_eq!(second_preemption.dispatch().task(), first);
        assert_eq!(second_preemption.dispatch().sequence(), 2);
        assert_eq!(
            second_preemption.dispatch().context(),
            scheduler_context::saved
        );

        let fault = scheduler
            .fault_current(&mut tasks, first)
            .expect("fault first");
        assert_eq!(fault.task(), first);
        assert_eq!(fault.dispatch().task(), second);
        assert_eq!(fault.dispatch().sequence(), 3);
        assert_eq!(fault.dispatch().context(), scheduler_context::saved);
        assert_eq!(
            tasks.snapshot(first).expect("first").state(),
            task_state::faulted
        );
        assert_eq!(
            tasks.snapshot(second).expect("second").state(),
            task_state::running
        );

        scheduler
            .complete_current(&mut tasks, second)
            .expect("complete peer");
        assert_eq!(
            tasks.snapshot(second).expect("second").state(),
            task_state::exited
        );
        assert_eq!(
            scheduler.context(&tasks, first),
            Ok(scheduler_context::saved)
        );
        assert_eq!(
            scheduler.context(&tasks, second),
            Ok(scheduler_context::saved)
        );

        assert_eq!(
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, first)
                .expect("withdraw fault context"),
            scheduler_context::saved
        );
        assert_eq!(
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, second)
                .expect("withdraw exit context"),
            scheduler_context::saved
        );
        tasks.destroy(&mut circles, first).expect("destroy faulted");
        tasks.destroy(&mut circles, second).expect("destroy exited");
        let snapshot = scheduler.snapshot(&tasks).expect("empty scheduler");
        assert_eq!(snapshot.capacity(), 2);
        assert_eq!(snapshot.admitted(), 0);
        assert_eq!(snapshot.runnable(), 0);
        assert_eq!(snapshot.current(), None);
        assert_eq!(snapshot.next_dispatch_sequence(), Some(4));
        assert_eq!(snapshot.next_preemption_sequence(), Some(2));
    }

    #[test]
    fn admission_rejects_duplicate_stale_and_capacity_without_mutation() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(1).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit first");
        let before = scheduler.test_snapshot();

        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, first),
            Err(scheduler_error::duplicate_task)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, second),
            Err(scheduler_error::capacity_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(!tasks.snapshot(second).expect("second").context_attached());

        tasks.start(second).expect("ordinary start");
        tasks.exit(second).expect("ordinary exit");
        tasks
            .destroy(&mut circles, second)
            .expect("destroy ordinary task");
        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, second),
            Err(scheduler_error::task(task_error::stale_identity))
        );
        assert_eq!(scheduler.test_snapshot(), before);
    }

    #[test]
    fn wrong_current_and_missing_peer_fail_without_mutation() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit first");
        scheduler.start_next(&mut tasks).expect("start first");
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first");

        assert_eq!(
            scheduler.preempt_current(&mut tasks, second),
            Err(scheduler_error::wrong_current_task)
        );
        assert_eq!(
            scheduler.preempt_current(&mut tasks, first),
            Err(scheduler_error::no_runnable_peer)
        );
        assert_eq!(
            scheduler.fault_current(&mut tasks, first),
            Err(scheduler_error::no_runnable_peer)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
    }

    #[test]
    fn completion_refuses_to_abandon_a_runnable_peer() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("second");
        scheduler.start_next(&mut tasks).expect("start first");
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first");

        assert_eq!(
            scheduler.complete_current(&mut tasks, first),
            Err(scheduler_error::runnable_peer_present)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
    }

    #[test]
    fn queue_indexes_wrap_without_changing_round_robin_order() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("second");
        assert_eq!(
            scheduler.start_next(&mut tasks).expect("start").task(),
            first
        );

        for expected in [second, first, second, first, second, first] {
            let current = scheduler.current.expect("current");
            let switched = scheduler
                .preempt_current(&mut tasks, current)
                .expect("preemption");
            assert_eq!(switched.dispatch().task(), expected);
        }
        assert_eq!(scheduler.queue_head, 1);
        assert!(scheduler.validate(&tasks).is_ok());
    }

    #[test]
    fn sequence_exhaustion_is_nonwrapping_and_failure_atomic() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("second");
        scheduler.next_dispatch_sequence = u64::MAX;
        let dispatch = scheduler.start_next(&mut tasks).expect("last dispatch");
        assert_eq!(dispatch.sequence(), u64::MAX);
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("snapshot")
                .next_dispatch_sequence(),
            None
        );
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first");
        let second_before = tasks.snapshot(second).expect("second");
        assert_eq!(
            scheduler.preempt_current(&mut tasks, first),
            Err(scheduler_error::dispatch_sequence_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("second");
        scheduler.start_next(&mut tasks).expect("start");
        scheduler.next_preemption_sequence = u64::MAX;
        let last = scheduler
            .preempt_current(&mut tasks, first)
            .expect("last preemption");
        assert_eq!(last.sequence(), u64::MAX);
        let before = scheduler.test_snapshot();
        assert_eq!(
            scheduler.preempt_current(&mut tasks, second),
            Err(scheduler_error::preemption_sequence_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
    }

    #[test]
    fn malformed_queue_record_and_sequence_topology_fail_closed() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("second");

        let valid = scheduler.test_snapshot();
        scheduler.queue_depth = 3;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        scheduler.queue[1] = None;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        scheduler.queue[1] = Some(first);
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        scheduler.records[0].context = 7;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        scheduler.dispatch_sequence_exhausted = true;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        scheduler.preemption_sequence_exhausted = true;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(valid);

        let stale_task = task_id::try_from_parts(first.task().slot(), 2).expect("stale task");
        let stale = task_identity::new(stale_task, first.circle());
        scheduler.records[0].task = Some(stale);
        scheduler.queue[0] = Some(stale);
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
    }

    #[test]
    fn last_current_fault_retains_context_and_never_consumes_sequences() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        for task in [first, second] {
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
        }
        scheduler.start_next(&mut tasks).unwrap();
        scheduler.fault_current(&mut tasks, first).unwrap();
        let before = scheduler.snapshot(&tasks).unwrap();
        let children = circles.accounting();
        scheduler.fault_last_current(&mut tasks, second).unwrap();
        let after = scheduler.snapshot(&tasks).unwrap();
        assert_eq!(after.current(), None);
        assert_eq!(after.runnable(), 0);
        assert_eq!(after.admitted(), 2);
        assert_eq!(
            after.next_dispatch_sequence(),
            before.next_dispatch_sequence()
        );
        assert_eq!(
            after.next_preemption_sequence(),
            before.next_preemption_sequence()
        );
        assert_eq!(circles.accounting(), children);
        for task in [first, second] {
            assert_eq!(tasks.snapshot(task).unwrap().state(), task_state::faulted);
            assert_eq!(
                scheduler.context(&tasks, task),
                Ok(scheduler_context::saved)
            );
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, task)
                .unwrap();
            tasks.destroy(&mut circles, task).unwrap();
            circles.destroy(task.circle()).unwrap();
        }
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(circles.accounting(), circle_table::new().accounting());
    }

    #[test]
    fn last_current_fault_rejects_peer_wrong_stale_and_terminal_without_mutation() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        for task in [first, second] {
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
        }
        let stale = task_identity::new(
            task_id::try_from_parts(first.task().slot(), first.task().generation().get() + 1)
                .unwrap(),
            first.circle(),
        );
        for started in [false, true] {
            if started {
                scheduler.start_next(&mut tasks).unwrap();
            }
            for identity in [
                first,
                second,
                stale,
                task_identity::new(first.task(), second.circle()),
            ] {
                let scheduler_before = scheduler.test_snapshot();
                let tasks_before = tasks.test_snapshot();
                let circles_before = circles.accounting();
                assert!(scheduler.fault_last_current(&mut tasks, identity).is_err());
                assert_eq!(scheduler.test_snapshot(), scheduler_before);
                assert!(tasks.test_snapshot() == tasks_before);
                assert_eq!(circles.accounting(), circles_before);
            }
        }
        scheduler.fault_current(&mut tasks, first).unwrap();
        // Terminal retirement itself needs no new sequence, even at exhaustion.
        scheduler.next_dispatch_sequence = u64::MAX;
        scheduler.dispatch_sequence_exhausted = true;
        scheduler.fault_last_current(&mut tasks, second).unwrap();
        let scheduler_before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(scheduler.fault_last_current(&mut tasks, second).is_err());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert!(tasks.test_snapshot() == tasks_before);
    }

    #[test]
    fn last_current_fault_rejects_corrupt_topology_before_task_mutation() {
        let (mut circles, mut tasks, first, _) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        scheduler.admit(&mut circles, &mut tasks, first).unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        scheduler.queue_depth = 3;
        let before = scheduler.test_snapshot();
        let task_before = tasks.test_snapshot();
        assert_eq!(
            scheduler.fault_last_current(&mut tasks, first),
            Err(scheduler_error::corrupt_state)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == task_before);
    }

    #[test]
    fn last_current_fault_rejects_outstanding_dispatch_reservation() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        for task in [first, second] {
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
        }
        scheduler.start_next(&mut tasks).unwrap();
        let (_, _, wait, readiness) = prepare_endpoint_publication(first, 71);
        let (_, authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);
        let before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        assert!(scheduler.fault_last_current(&mut tasks, second).is_err());
        assert_eq!(scheduler.test_snapshot(), before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert!(authorization.is_some());
    }

    fn assert_unstarted_withdrawal_rejected(
        scheduler: &mut scheduler_table,
        circles: &mut circle_table,
        tasks: &mut task_table,
        identity: task_identity,
        error: scheduler_error,
    ) {
        let scheduler_before = scheduler.test_snapshot();
        let tasks_before = tasks.test_snapshot();
        let children = circles.accounting();
        assert_eq!(
            scheduler.withdraw_unstarted_context(circles, tasks, identity),
            Err(error)
        );
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert!(tasks.test_snapshot() == tasks_before);
        assert_eq!(circles.accounting(), children);
    }

    #[test]
    fn unstarted_withdrawal_rolls_back_partial_pair_admission_exactly() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        for identity in [first, second] {
            circles
                .attach_child(identity.circle(), circle_child_kind::address_space)
                .unwrap();
            circles
                .attach_children(identity.circle(), circle_child_kind::mapping, 3)
                .unwrap();
        }
        let mut scheduler = scheduler_table::new(1).unwrap();
        let original_scheduler = scheduler.test_snapshot();
        let original_tasks = tasks.test_snapshot();
        let original_children = circles.accounting();
        scheduler.admit(&mut circles, &mut tasks, first).unwrap();
        let partial = scheduler.test_snapshot();
        let attached = tasks.test_snapshot();
        let children = circles.accounting();
        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, second),
            Err(scheduler_error::capacity_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), partial);
        assert!(tasks.test_snapshot() == attached);
        assert_eq!(circles.accounting(), children);
        assert_eq!(
            scheduler.withdraw_unstarted_context(&mut circles, &mut tasks, first),
            Ok(scheduler_context::initial)
        );
        assert_eq!(scheduler.test_snapshot(), original_scheduler);
        assert!(tasks.test_snapshot() == original_tasks);
        assert_eq!(circles.accounting(), original_children);
        for identity in [first, second] {
            tasks.cancel_unstarted(identity).unwrap();
            circles
                .detach_children(identity.circle(), circle_child_kind::mapping, 3)
                .unwrap();
            circles
                .detach_child(identity.circle(), circle_child_kind::address_space)
                .unwrap();
            tasks.destroy(&mut circles, identity).unwrap();
            circles.destroy(identity.circle()).unwrap();
        }
        assert_eq!(tasks.active_count(), 0);
        assert_eq!(circles.accounting(), circle_table::new().accounting());
    }

    #[test]
    fn unstarted_withdrawal_preserves_head_middle_tail_and_wrapped_queue_order() {
        for wrapped in [false, true] {
            for removed_index in 0..4 {
                let mut circles = circle_table::new();
                let mut tasks = task_table::new();
                let mut scheduler = scheduler_table::new(4).unwrap();
                if wrapped {
                    // Advance the head through real dispatch/terminal withdrawal
                    // before admitting the untouched four-task initial queue.
                    for _ in 0..3 {
                        let task = create_task(&mut circles, &mut tasks, 99);
                        scheduler.admit(&mut circles, &mut tasks, task).unwrap();
                        scheduler.start_next(&mut tasks).unwrap();
                        scheduler.complete_current(&mut tasks, task).unwrap();
                        scheduler
                            .withdraw_terminal_context(&mut circles, &mut tasks, task)
                            .unwrap();
                        tasks.destroy(&mut circles, task).unwrap();
                        circles.destroy(task.circle()).unwrap();
                    }
                    assert_eq!(scheduler.queue_head, 3);
                }
                let identities = core::array::from_fn::<_, 4, _>(|index| {
                    create_task(&mut circles, &mut tasks, index as u64 + 1)
                });
                for identity in identities {
                    scheduler.admit(&mut circles, &mut tasks, identity).unwrap();
                }
                let before = scheduler.test_snapshot();
                let task_snapshots = identities.map(|task| tasks.snapshot(task).unwrap());
                let removed = identities[removed_index];
                assert_eq!(
                    scheduler.withdraw_unstarted_context(&mut circles, &mut tasks, removed),
                    Ok(scheduler_context::initial)
                );
                assert_eq!(scheduler.queue_head, before.queue_head);
                assert_eq!(scheduler.queue_depth, 3);
                assert_eq!(
                    scheduler.next_dispatch_sequence,
                    before.next_dispatch_sequence
                );
                assert_eq!(
                    scheduler.dispatch_sequence_exhausted,
                    before.dispatch_sequence_exhausted
                );
                assert_eq!(
                    scheduler.next_preemption_sequence,
                    before.next_preemption_sequence
                );
                assert_eq!(
                    scheduler.preemption_sequence_exhausted,
                    before.preemption_sequence_exhausted
                );
                assert!(scheduler.validate(&tasks).is_ok());
                assert_eq!(scheduler.active_count(), 3);
                assert_eq!(circles.accounting().task_contexts, 3);
                let survivors =
                    identities.map(|identity| (identity != removed).then_some(identity));
                for (offset, survivor) in survivors.into_iter().flatten().enumerate() {
                    assert_eq!(
                        scheduler.queue[scheduler.queue_index(offset).unwrap()],
                        Some(survivor)
                    );
                }
                for (index, identity) in identities.into_iter().enumerate() {
                    let snapshot = tasks.snapshot(identity).unwrap();
                    if identity == removed {
                        assert_eq!(snapshot.state(), task_state::ready);
                        assert!(!snapshot.context_attached());
                        assert_eq!(snapshot.bounds(), task_snapshots[index].bounds());
                        assert_eq!(snapshot.principal(), task_snapshots[index].principal());
                    } else {
                        assert_eq!(snapshot, task_snapshots[index]);
                    }
                }
                // A rejected repeated withdrawal cannot disturb survivors.
                assert_unstarted_withdrawal_rejected(
                    &mut scheduler,
                    &mut circles,
                    &mut tasks,
                    removed,
                    scheduler_error::task_not_admitted,
                );
                let mut dispatch = scheduler.start_next(&mut tasks).unwrap();
                for (index, expected) in survivors.into_iter().flatten().enumerate() {
                    assert_eq!(dispatch.task(), expected);
                    assert_eq!(dispatch.context(), scheduler_context::initial);
                    assert_eq!(
                        dispatch.sequence(),
                        before.next_dispatch_sequence + index as u64
                    );
                    if index < 2 {
                        dispatch = scheduler
                            .fault_current(&mut tasks, expected)
                            .unwrap()
                            .dispatch();
                    } else {
                        scheduler.complete_current(&mut tasks, expected).unwrap();
                    }
                }
                for survivor in survivors.into_iter().flatten() {
                    scheduler
                        .withdraw_terminal_context(&mut circles, &mut tasks, survivor)
                        .unwrap();
                    tasks.destroy(&mut circles, survivor).unwrap();
                    circles.destroy(survivor.circle()).unwrap();
                }
                tasks.cancel_unstarted(removed).unwrap();
                tasks.destroy(&mut circles, removed).unwrap();
                circles.destroy(removed.circle()).unwrap();
                assert_eq!(scheduler.active_count(), 0);
                assert_eq!(scheduler.queue_depth, 0);
                assert_eq!(tasks.active_count(), 0);
            }
        }
    }

    #[test]
    fn unstarted_withdrawal_rejects_unadmitted_stale_and_wrong_circle_identities() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        scheduler.admit(&mut circles, &mut tasks, first).unwrap();
        let stale = task_identity::new(
            task_id::try_from_parts(first.task().slot(), 2).unwrap(),
            first.circle(),
        );
        let wrong_circle = task_identity::new(first.task(), second.circle());
        let invalid = task_identity::new(
            task_id::try_from_parts(task_capacity, 1).unwrap(),
            first.circle(),
        );
        for candidate in [second, stale, wrong_circle, invalid] {
            assert_unstarted_withdrawal_rejected(
                &mut scheduler,
                &mut circles,
                &mut tasks,
                candidate,
                scheduler_error::task_not_admitted,
            );
        }
    }

    #[test]
    fn unstarted_withdrawal_rejects_current_or_previously_executed_contexts() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        for task in [first, second] {
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
        }
        scheduler.start_next(&mut tasks).unwrap();
        for identity in [first, second] {
            assert_unstarted_withdrawal_rejected(
                &mut scheduler,
                &mut circles,
                &mut tasks,
                identity,
                scheduler_error::wrong_current_task,
            );
        }
        scheduler.preempt_current(&mut tasks, first).unwrap();
        assert_eq!(
            tasks.snapshot(first).unwrap().state(),
            task_state::suspended
        );
        assert_unstarted_withdrawal_rejected(
            &mut scheduler,
            &mut circles,
            &mut tasks,
            first,
            scheduler_error::wrong_current_task,
        );

        for terminal in [false, true] {
            let (mut circles, mut tasks, first, _) = two_tasks();
            let mut scheduler = scheduler_table::new(1).unwrap();
            scheduler.admit(&mut circles, &mut tasks, first).unwrap();
            scheduler.start_next(&mut tasks).unwrap();
            if terminal {
                scheduler.complete_current(&mut tasks, first).unwrap();
            } else {
                scheduler.park_current(&mut tasks, first).unwrap();
            }
            assert!(scheduler.current.is_none());
            assert_unstarted_withdrawal_rejected(
                &mut scheduler,
                &mut circles,
                &mut tasks,
                first,
                scheduler_error::task(task_error::invalid_state),
            );
        }
    }

    #[test]
    fn unstarted_withdrawal_rejects_pending_reservation_without_consuming_it() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).unwrap();
        for task in [first, second] {
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
        }
        scheduler.start_next(&mut tasks).unwrap();
        let (_endpoint, _publication, wait, readiness) = prepare_endpoint_publication(first, 101);
        let (_transition, authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);
        let reservation = authorization.as_ref().unwrap().reservation;
        for identity in [first, second] {
            assert_unstarted_withdrawal_rejected(
                &mut scheduler,
                &mut circles,
                &mut tasks,
                identity,
                scheduler_error::dispatch_reservation_pending,
            );
            assert_eq!(scheduler.reservation, Some(reservation));
        }
    }

    #[test]
    fn unstarted_withdrawal_rejects_corrupt_topology_and_attachment_bookkeeping() {
        for mutation in 0..9 {
            let (mut circles, mut tasks, first, second) = two_tasks();
            let mut scheduler = scheduler_table::new(2).unwrap();
            for task in [first, second] {
                scheduler.admit(&mut circles, &mut tasks, task).unwrap();
            }
            let error = match mutation {
                0 => {
                    scheduler.queue[0] = None;
                    scheduler_error::corrupt_state
                }
                1 => {
                    scheduler.queue[1] = Some(first);
                    scheduler_error::corrupt_state
                }
                2 => {
                    scheduler.records[0].context = saved_context;
                    scheduler_error::corrupt_state
                }
                3 => {
                    scheduler.records[0].context = 7;
                    scheduler_error::corrupt_state
                }
                4 => {
                    scheduler.queue_head = 2;
                    scheduler_error::corrupt_state
                }
                5 => {
                    scheduler.queue_depth = 3;
                    scheduler_error::corrupt_state
                }
                6 => {
                    tasks
                        .withdraw_unstarted_context(&mut circles, first)
                        .unwrap();
                    scheduler_error::corrupt_state
                }
                7 => {
                    circles
                        .detach_child(first.circle(), circle_child_kind::task_context)
                        .unwrap();
                    scheduler_error::task(task_error::circle_accounting_mismatch)
                }
                8 => {
                    circles
                        .detach_child(first.circle(), circle_child_kind::task)
                        .unwrap();
                    scheduler_error::task(task_error::circle_accounting_mismatch)
                }
                _ => unreachable!(),
            };
            assert_unstarted_withdrawal_rejected(
                &mut scheduler,
                &mut circles,
                &mut tasks,
                first,
                error,
            );
        }
    }

    #[test]
    fn terminal_context_must_be_withdrawn_once_before_task_destruction() {
        let (mut circles, mut tasks, first, _) = two_tasks();
        let mut scheduler = scheduler_table::new(1).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit");
        scheduler.start_next(&mut tasks).expect("start");
        scheduler
            .complete_current(&mut tasks, first)
            .expect("complete");
        assert_eq!(
            tasks.destroy(&mut circles, first),
            Err(task_error::context_still_attached)
        );
        scheduler
            .withdraw_terminal_context(&mut circles, &mut tasks, first)
            .expect("withdraw");
        let before = scheduler.test_snapshot();
        assert_eq!(
            scheduler.withdraw_terminal_context(&mut circles, &mut tasks, first),
            Err(scheduler_error::task_not_admitted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        tasks.destroy(&mut circles, first).expect("destroy task");
    }

    #[test]
    fn block_dispatch_and_completion_wake_are_exact() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("admit second");
        assert_eq!(
            scheduler.start_next(&mut tasks).expect("start").task(),
            first
        );

        let blocked = scheduler
            .block_current_and_dispatch(&mut tasks, first, second)
            .expect("block and dispatch");
        assert_eq!(blocked.task(), first);
        assert_eq!(blocked.dispatch().task(), second);
        assert_eq!(blocked.dispatch().sequence(), 1);
        assert_eq!(blocked.dispatch().context(), scheduler_context::initial);
        assert_eq!(
            tasks.snapshot(first).expect("blocked first").state(),
            task_state::blocked
        );
        assert_eq!(
            scheduler.context(&tasks, first),
            Ok(scheduler_context::saved)
        );
        let blocked_snapshot = scheduler.snapshot(&tasks).expect("blocked topology");
        assert_eq!(blocked_snapshot.admitted(), 2);
        assert_eq!(blocked_snapshot.runnable(), 0);
        assert_eq!(blocked_snapshot.current(), Some(second));

        let completion = scheduler
            .complete_current_and_wake(&mut tasks, second, first)
            .expect("complete and wake");
        assert_eq!(completion.task(), second);
        assert_eq!(completion.dispatch().task(), first);
        assert_eq!(completion.dispatch().sequence(), 2);
        assert_eq!(completion.dispatch().context(), scheduler_context::saved);
        assert_eq!(
            tasks.snapshot(second).expect("completed second").state(),
            task_state::exited
        );
        assert_eq!(
            tasks.snapshot(first).expect("woken first").state(),
            task_state::running
        );
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("woken topology")
                .current(),
            Some(first)
        );
    }

    #[test]
    fn park_then_fault_wake_preserves_supervisor_context() {
        let (mut circles, mut tasks, supervisor, vessel) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, supervisor)
            .expect("admit supervisor");
        scheduler.start_next(&mut tasks).expect("start supervisor");

        let parked = scheduler
            .park_current(&mut tasks, supervisor)
            .expect("park supervisor");
        assert_eq!(parked.task(), supervisor);
        assert_eq!(
            tasks
                .snapshot(supervisor)
                .expect("parked supervisor")
                .state(),
            task_state::blocked
        );
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("parked topology")
                .current(),
            None
        );
        assert_eq!(
            scheduler.context(&tasks, supervisor),
            Ok(scheduler_context::saved)
        );

        scheduler
            .admit(&mut circles, &mut tasks, vessel)
            .expect("admit vessel");
        let vessel_dispatch = scheduler.start_next(&mut tasks).expect("start vessel");
        assert_eq!(vessel_dispatch.task(), vessel);
        assert_eq!(vessel_dispatch.sequence(), 1);
        let fault = scheduler
            .fault_current_and_wake(&mut tasks, vessel, supervisor)
            .expect("fault vessel and wake supervisor");
        assert_eq!(fault.task(), vessel);
        assert_eq!(fault.dispatch().task(), supervisor);
        assert_eq!(fault.dispatch().sequence(), 2);
        assert_eq!(fault.dispatch().context(), scheduler_context::saved);
        assert_eq!(
            tasks.snapshot(vessel).expect("faulted vessel").state(),
            task_state::faulted
        );
        assert_eq!(
            tasks
                .snapshot(supervisor)
                .expect("running supervisor")
                .state(),
            task_state::running
        );
        assert!(scheduler.validate(&tasks).is_ok());
    }

    #[test]
    fn blocked_transitions_reject_wrong_topology_without_mutation() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let first = create_task(&mut circles, &mut tasks, 1);
        let second = create_task(&mut circles, &mut tasks, 2);
        let third = create_task(&mut circles, &mut tasks, 3);
        let mut scheduler = scheduler_table::new(3).expect("scheduler");
        for identity in [first, second] {
            scheduler
                .admit(&mut circles, &mut tasks, identity)
                .expect("admit task");
        }
        scheduler.start_next(&mut tasks).expect("start first");
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first");
        let second_before = tasks.snapshot(second).expect("second");

        assert_eq!(
            scheduler.block_current_and_dispatch(&mut tasks, first, third),
            Err(scheduler_error::wrong_runnable_task)
        );
        assert_eq!(
            scheduler.park_current(&mut tasks, first),
            Err(scheduler_error::runnable_peer_present)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        scheduler
            .block_current_and_dispatch(&mut tasks, first, second)
            .expect("block first");
        scheduler
            .admit(&mut circles, &mut tasks, third)
            .expect("admit third");
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("blocked first");
        let second_before = tasks.snapshot(second).expect("running second");
        let third_before = tasks.snapshot(third).expect("ready third");

        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::runnable_peer_present)
        );
        assert_eq!(
            scheduler.complete_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::runnable_peer_present)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));
        assert_eq!(tasks.snapshot(third), Ok(third_before));
    }

    #[test]
    fn direct_wake_requires_exact_admitted_blocked_target() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let supervisor = create_task(&mut circles, &mut tasks, 1);
        let vessel = create_task(&mut circles, &mut tasks, 2);
        let unadmitted = create_task(&mut circles, &mut tasks, 3);
        let mut scheduler = scheduler_table::new(3).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, supervisor)
            .expect("admit supervisor");
        scheduler.start_next(&mut tasks).expect("start supervisor");
        scheduler
            .park_current(&mut tasks, supervisor)
            .expect("park supervisor");
        scheduler
            .admit(&mut circles, &mut tasks, vessel)
            .expect("admit vessel");
        scheduler.start_next(&mut tasks).expect("start vessel");
        let before = scheduler.test_snapshot();
        let supervisor_before = tasks.snapshot(supervisor).expect("supervisor");
        let vessel_before = tasks.snapshot(vessel).expect("vessel");

        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, supervisor, vessel),
            Err(scheduler_error::wrong_current_task)
        );
        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, vessel, vessel),
            Err(scheduler_error::task(task_error::same_task))
        );
        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, vessel, unadmitted),
            Err(scheduler_error::task_not_admitted)
        );
        assert_eq!(
            scheduler.complete_current_and_wake(&mut tasks, vessel, unadmitted),
            Err(scheduler_error::task_not_admitted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(supervisor), Ok(supervisor_before));
        assert_eq!(tasks.snapshot(vessel), Ok(vessel_before));

        let supervisor_slot = scheduler
            .record_index(supervisor)
            .expect("supervisor record");
        scheduler.records[supervisor_slot].context = initial_context;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
    }

    #[test]
    fn blocked_dispatch_sequence_exhaustion_is_failure_atomic() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        scheduler
            .admit(&mut circles, &mut tasks, first)
            .expect("admit first");
        scheduler
            .admit(&mut circles, &mut tasks, second)
            .expect("admit second");
        scheduler.start_next(&mut tasks).expect("start first");
        scheduler.next_dispatch_sequence = u64::MAX;

        let last = scheduler
            .block_current_and_dispatch(&mut tasks, first, second)
            .expect("last block dispatch");
        assert_eq!(last.dispatch().sequence(), u64::MAX);
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("exhausted snapshot")
                .next_dispatch_sequence(),
            None
        );
        let before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("blocked first");
        let second_before = tasks.snapshot(second).expect("running second");

        assert_eq!(
            scheduler.complete_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::dispatch_sequence_exhausted)
        );
        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::dispatch_sequence_exhausted)
        );
        assert_eq!(scheduler.test_snapshot(), before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        scheduler.queue[scheduler.queue_head] = Some(first);
        scheduler.queue_depth = 1;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
    }

    #[test]
    fn reserved_block_wake_and_preemption_are_atomic_and_exclusive() {
        let mut circles = circle_table::new();
        let mut tasks = task_table::new();
        let first = create_task(&mut circles, &mut tasks, 1);
        let second = create_task(&mut circles, &mut tasks, 2);
        let third = create_task(&mut circles, &mut tasks, 3);
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        for identity in [first, second] {
            scheduler
                .admit(&mut circles, &mut tasks, identity)
                .expect("admit task");
        }
        scheduler.start_next(&mut tasks).expect("start first");
        assert_eq!(scheduler.queue_head, 1);
        let (mut endpoint, publication, wait, readiness) = prepare_endpoint_publication(first, 3);

        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first before prepare");
        let second_before = tasks.snapshot(second).expect("second before prepare");
        let mut prepared_authorization = None;
        let prepared = scheduler
            .prepare_block_current_and_reserve(
                &mut tasks,
                first,
                second,
                wait,
                readiness,
                &mut prepared_authorization,
            )
            .expect("prepare reserved block");
        let predicted = prepared.transition();
        assert_eq!(predicted.task(), first);
        assert_eq!(predicted.dispatch().task(), second);
        assert_eq!(predicted.dispatch().sequence(), 1);
        drop(prepared);
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));
        assert!(prepared_authorization.is_none());

        let (blocked, mut wake_authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);
        assert_eq!(blocked, predicted);
        assert_eq!(scheduler.queue_head, 0);
        assert_eq!(scheduler.queue_depth, 0);
        let reserved = scheduler
            .snapshot(&tasks)
            .expect("reserved snapshot")
            .reservation()
            .expect("dispatch reservation");
        assert_eq!(reserved.task(), first);
        assert_eq!(reserved.current(), second);
        assert_eq!(reserved.sequence(), 2);
        assert!(!reserved.runnable());
        assert_eq!(scheduler.snapshot(&tasks).expect("snapshot").capacity(), 2);
        assert_eq!(scheduler.snapshot(&tasks).expect("snapshot").admitted(), 2);
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("reserved snapshot")
                .next_dispatch_sequence(),
            Some(3)
        );

        let reserved_scheduler = scheduler.test_snapshot();
        let first_blocked = tasks.snapshot(first).expect("blocked first");
        let second_running = tasks.snapshot(second).expect("running second");
        assert_eq!(
            scheduler.start_next(&mut tasks),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.preempt_current(&mut tasks, second),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.block_current_and_dispatch(&mut tasks, second, first),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.park_current(&mut tasks, second),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.fault_current(&mut tasks, second),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.fault_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.complete_current_and_wake(&mut tasks, second, first),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.complete_current(&mut tasks, second),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.admit(&mut circles, &mut tasks, third),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(
            scheduler.withdraw_terminal_context(&mut circles, &mut tasks, first),
            Err(scheduler_error::dispatch_reservation_pending)
        );
        assert_eq!(scheduler.test_snapshot(), reserved_scheduler);
        assert_eq!(tasks.snapshot(first), Ok(first_blocked));
        assert_eq!(tasks.snapshot(second), Ok(second_running));

        let correlation = publication
            .wake_correlation()
            .expect("publication correlation");
        let mut composed_authorization = None;
        let mut selection_authorization = None;
        let preflight = scheduler
            .preflight_endpoint_wake(
                &tasks,
                &wake_authorization,
                correlation,
                &composed_authorization,
                &selection_authorization,
            )
            .expect("preflight endpoint wake");
        let receipt = endpoint
            .commit_publication(publication)
            .unwrap_or_else(|rejection| panic!("publication: {}", rejection.error().code()));
        let endpoint_authorization = receipt.into_wake().expect("endpoint wake authority");
        compose_endpoint_wake_authorization(
            &mut wake_authorization,
            endpoint_authorization,
            preflight,
            &mut composed_authorization,
        )
        .unwrap_or_else(|rejection| panic!("composition: {:?}", rejection.error()));
        assert!(wake_authorization.is_none());
        assert!(
            composed_authorization
                .as_ref()
                .expect("composed authority")
                .endpoint()
                .correlation()
                == correlation
        );
        let wake_prepared = scheduler
            .prepare_endpoint_wake(
                &mut tasks,
                &mut composed_authorization,
                &mut selection_authorization,
            )
            .expect("prepare endpoint wake");
        assert_eq!(wake_prepared.task(), first);
        drop(wake_prepared);
        assert_eq!(scheduler.test_snapshot(), reserved_scheduler);
        assert_eq!(tasks.snapshot(first), Ok(first_blocked));
        assert!(composed_authorization.is_some());
        assert!(selection_authorization.is_none());

        let wake_prepared = scheduler
            .prepare_endpoint_wake(
                &mut tasks,
                &mut composed_authorization,
                &mut selection_authorization,
            )
            .expect("prepare endpoint wake");
        assert_eq!(wake_prepared.commit().task(), first);
        assert!(composed_authorization.is_none());
        assert!(selection_authorization.is_some());
        assert_eq!(scheduler.queue_head, 0);
        assert_eq!(scheduler.queue_depth, 1);
        assert_eq!(scheduler.queue[0], Some(first));
        let ready_snapshot = scheduler.snapshot(&tasks).expect("ready snapshot");
        assert_eq!(ready_snapshot.current(), Some(second));
        assert_eq!(ready_snapshot.runnable(), 1);
        assert!(
            ready_snapshot
                .reservation()
                .expect("dispatch reservation")
                .runnable()
        );
        assert_eq!(
            tasks.snapshot(first).expect("suspended first").state(),
            task_state::suspended
        );
        assert_eq!(
            scheduler.preempt_current(&mut tasks, second),
            Err(scheduler_error::dispatch_reservation_pending)
        );

        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("first before selection");
        let second_before = tasks.snapshot(second).expect("second before selection");
        let prepared = match scheduler.prepare_reserved_preemption(
            &mut tasks,
            second,
            &mut selection_authorization,
        ) {
            Ok(prepared) => prepared,
            Err(error) => panic!("selection rejected: {error:?}"),
        };
        let predicted = prepared.transition();
        assert_eq!(predicted.task(), second);
        assert_eq!(predicted.sequence(), 0);
        assert_eq!(predicted.dispatch().task(), first);
        assert_eq!(predicted.dispatch().sequence(), 2);
        assert_eq!(predicted.dispatch().context(), scheduler_context::saved);
        drop(prepared);
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));
        assert!(selection_authorization.is_some());

        let prepared = match scheduler.prepare_reserved_preemption(
            &mut tasks,
            second,
            &mut selection_authorization,
        ) {
            Ok(prepared) => prepared,
            Err(error) => panic!("selection rejected: {error:?}"),
        };
        assert_eq!(prepared.transition(), predicted);
        assert_eq!(prepared.commit(), predicted);
        assert!(selection_authorization.is_none());
        let final_snapshot = scheduler.snapshot(&tasks).expect("selected snapshot");
        assert_eq!(final_snapshot.current(), Some(first));
        assert_eq!(final_snapshot.runnable(), 1);
        assert_eq!(final_snapshot.reservation(), None);
        assert_eq!(final_snapshot.next_dispatch_sequence(), Some(3));
        assert_eq!(final_snapshot.next_preemption_sequence(), Some(1));
        assert_eq!(scheduler.queue_head, 1);
        assert_eq!(scheduler.queue[1], Some(second));
        assert_eq!(
            tasks.snapshot(first).expect("running first").state(),
            task_state::running
        );
        assert_eq!(
            tasks.snapshot(second).expect("suspended second").state(),
            task_state::suspended
        );
        assert!(scheduler.validate(&tasks).is_ok());
    }

    #[test]
    fn reserved_authority_mismatch_replay_and_fault_are_failure_atomic() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        for identity in [first, second] {
            scheduler
                .admit(&mut circles, &mut tasks, identity)
                .expect("admit task");
        }
        scheduler.start_next(&mut tasks).expect("start first");
        let (mut endpoint, publication, wait, readiness) = prepare_endpoint_publication(first, 11);
        let (_, mut wake_authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);
        let reservation = wake_authorization
            .as_ref()
            .expect("wake authority")
            .reservation;

        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("blocked first");
        let second_before = tasks.snapshot(second).expect("running second");
        let (wrong_endpoint, wrong_publication, _, _) = prepare_endpoint_publication(first, 7);
        let wrong_revision = wrong_endpoint.revision();
        let wrong_correlation = wrong_publication
            .wake_correlation()
            .expect("wrong correlation");
        let mut composed_authorization = None;
        let mut selection_authorization = None;
        assert_eq!(
            scheduler
                .preflight_endpoint_wake(
                    &tasks,
                    &wake_authorization,
                    wrong_correlation,
                    &composed_authorization,
                    &selection_authorization,
                )
                .err(),
            Some(scheduler_error::wrong_endpoint_wake)
        );
        assert_eq!(wrong_endpoint.revision().get(), wrong_revision.get());
        assert!(wake_authorization.is_some());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        let correlation = publication.wake_correlation().expect("correct correlation");
        let preflight = scheduler
            .preflight_endpoint_wake(
                &tasks,
                &wake_authorization,
                correlation,
                &composed_authorization,
                &selection_authorization,
            )
            .expect("correct preflight");
        let receipt = endpoint
            .commit_publication(publication)
            .unwrap_or_else(|rejection| panic!("publication: {}", rejection.error().code()));
        let endpoint_authorization = receipt.into_wake().expect("endpoint authority");
        let rejection = compose_endpoint_wake_authorization(
            &mut wake_authorization,
            endpoint_authorization,
            scheduler_endpoint_wake_preflight {
                reservation,
                correlation: wrong_correlation,
            },
            &mut composed_authorization,
        )
        .expect_err("wrong preflight correlation must reject composition");
        assert_eq!(rejection.error(), scheduler_error::wrong_endpoint_wake);
        let endpoint_authorization = rejection.into_endpoint_authorization();
        assert!(wake_authorization.is_some());
        assert!(composed_authorization.is_none());
        compose_endpoint_wake_authorization(
            &mut wake_authorization,
            endpoint_authorization,
            preflight,
            &mut composed_authorization,
        )
        .unwrap_or_else(|rejection| panic!("composition: {:?}", rejection.error()));
        assert!(wake_authorization.is_none());

        let prepared = scheduler
            .prepare_endpoint_wake(
                &mut tasks,
                &mut composed_authorization,
                &mut selection_authorization,
            )
            .expect("prepare endpoint wake");
        prepared.commit();
        assert!(selection_authorization.is_some());
        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("suspended first");
        let second_before = tasks.snapshot(second).expect("running second");
        assert_eq!(
            scheduler
                .prepare_reserved_fault(&mut tasks, first, &mut selection_authorization)
                .err(),
            Some(scheduler_error::wrong_current_task)
        );
        assert!(selection_authorization.is_some());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        let prepared = scheduler
            .prepare_reserved_fault(&mut tasks, second, &mut selection_authorization)
            .expect("prepare reserved fault");
        let predicted = prepared.transition();
        assert_eq!(predicted.task(), second);
        assert_eq!(predicted.dispatch().task(), first);
        assert_eq!(predicted.dispatch().sequence(), 2);
        drop(prepared);
        assert!(selection_authorization.is_some());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        let prepared = scheduler
            .prepare_reserved_fault(&mut tasks, second, &mut selection_authorization)
            .expect("prepare reserved fault");
        assert_eq!(prepared.transition(), predicted);
        assert_eq!(prepared.commit(), predicted);
        assert!(selection_authorization.is_none());
        assert_eq!(
            tasks.snapshot(second).expect("faulted second").state(),
            task_state::faulted
        );
        assert_eq!(
            tasks.snapshot(first).expect("running first").state(),
            task_state::running
        );
        assert_eq!(
            scheduler.snapshot(&tasks).expect("snapshot").reservation(),
            None
        );

        let mut replayed_selection = Some(scheduler_selection_authorization { reservation });
        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("running first");
        let second_before = tasks.snapshot(second).expect("faulted second");
        assert_eq!(
            scheduler
                .prepare_reserved_fault(&mut tasks, first, &mut replayed_selection)
                .err(),
            Some(scheduler_error::dispatch_reservation_missing)
        );
        assert!(replayed_selection.is_some());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));
        scheduler
            .complete_current(&mut tasks, first)
            .expect("complete selected task");
        assert!(scheduler.validate(&tasks).is_ok());
    }

    #[test]
    fn reserved_dispatch_exhaustion_and_maximum_retirement_are_exact() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        for identity in [first, second] {
            scheduler
                .admit(&mut circles, &mut tasks, identity)
                .expect("admit task");
        }
        scheduler.start_next(&mut tasks).expect("start first");
        let (mut endpoint, publication, wait, readiness) = prepare_endpoint_publication(first, 13);
        scheduler.next_dispatch_sequence = u64::MAX;
        let scheduler_before = scheduler.test_snapshot();
        let first_before = tasks.snapshot(first).expect("running first");
        let second_before = tasks.snapshot(second).expect("suspended second");
        let mut wake_authorization = None;
        assert_eq!(
            scheduler
                .prepare_block_current_and_reserve(
                    &mut tasks,
                    first,
                    second,
                    wait,
                    readiness,
                    &mut wake_authorization,
                )
                .err(),
            Some(scheduler_error::dispatch_sequence_exhausted)
        );
        assert!(wake_authorization.is_none());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        scheduler.next_dispatch_sequence = 1;
        scheduler.next_preemption_sequence = u64::MAX;
        scheduler.preemption_sequence_exhausted = true;
        let scheduler_before = scheduler.test_snapshot();
        assert_eq!(
            scheduler
                .prepare_block_current_and_reserve(
                    &mut tasks,
                    first,
                    second,
                    wait,
                    readiness,
                    &mut wake_authorization,
                )
                .err(),
            Some(scheduler_error::preemption_sequence_exhausted)
        );
        assert!(wake_authorization.is_none());
        assert_eq!(scheduler.test_snapshot(), scheduler_before);
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        scheduler.next_dispatch_sequence = u64::MAX - 1;
        scheduler.preemption_sequence_exhausted = false;
        let (blocked, mut wake_authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);
        assert_eq!(blocked.dispatch().sequence(), u64::MAX - 1);
        let reservation = scheduler
            .snapshot(&tasks)
            .expect("reserved snapshot")
            .reservation()
            .expect("dispatch reservation");
        assert_eq!(reservation.sequence(), u64::MAX);
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("reserved snapshot")
                .next_dispatch_sequence(),
            None
        );
        assert_eq!(
            scheduler
                .snapshot(&tasks)
                .expect("reserved snapshot")
                .next_preemption_sequence(),
            Some(u64::MAX)
        );

        let (_, mut selection_authorization) = commit_reserved_wake(
            &mut scheduler,
            &mut tasks,
            &mut wake_authorization,
            &mut endpoint,
            publication,
        );
        let prepared = scheduler
            .prepare_reserved_preemption(&mut tasks, second, &mut selection_authorization)
            .expect("maximum preemption remains available");
        assert_eq!(prepared.transition().sequence(), u64::MAX);
        assert_eq!(prepared.transition().dispatch().sequence(), u64::MAX);
        let preemption = prepared.commit();
        assert_eq!(preemption.sequence(), u64::MAX);
        assert_eq!(preemption.dispatch().sequence(), u64::MAX);
        assert!(selection_authorization.is_none());
        let retired = scheduler.snapshot(&tasks).expect("retired snapshot");
        assert_eq!(retired.current(), Some(first));
        assert_eq!(retired.reservation(), None);
        assert_eq!(retired.next_dispatch_sequence(), None);
        assert_eq!(retired.next_preemption_sequence(), None);
        assert!(scheduler.validate(&tasks).is_ok());
    }

    #[test]
    fn reservation_structure_rejects_cursor_placement_and_lineage_corruption() {
        let (mut circles, mut tasks, first, second) = two_tasks();
        let mut scheduler = scheduler_table::new(2).expect("scheduler");
        for identity in [first, second] {
            scheduler
                .admit(&mut circles, &mut tasks, identity)
                .expect("admit task");
        }
        scheduler.start_next(&mut tasks).expect("start first");
        let (mut endpoint, publication, wait, readiness) = prepare_endpoint_publication(first, 17);
        let (_, mut wake_authorization) =
            commit_reserved_block(&mut scheduler, &mut tasks, first, second, wait, readiness);

        let blocked = scheduler.test_snapshot();
        scheduler.next_dispatch_sequence += 1;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(blocked);

        scheduler.reservation = Some(dispatch_reservation {
            task: first,
            current: first,
            sequence: 2,
        });
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(blocked);

        scheduler.queue[scheduler.queue_head] = Some(first);
        scheduler.queue_depth = 1;
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(blocked);

        let (_, mut selection_authorization) = commit_reserved_wake(
            &mut scheduler,
            &mut tasks,
            &mut wake_authorization,
            &mut endpoint,
            publication,
        );
        let runnable = scheduler.test_snapshot();
        scheduler.queue[scheduler.queue_head] = Some(second);
        assert_eq!(
            scheduler.validate(&tasks),
            Err(scheduler_error::corrupt_state)
        );
        scheduler.restore_test_snapshot(runnable);

        let prepared = match scheduler.prepare_reserved_preemption(
            &mut tasks,
            second,
            &mut selection_authorization,
        ) {
            Ok(prepared) => prepared,
            Err(error) => panic!("reserved selection rejected: {error:?}"),
        };
        prepared.commit();
        assert!(selection_authorization.is_none());
        assert!(scheduler.validate(&tasks).is_ok());
    }
}
