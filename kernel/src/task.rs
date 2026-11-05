#![cfg_attr(not(test), allow(dead_code))]

//! Fixed-capacity task lifecycle records for bounded control and scheduling
//! proofs.
//!
//! These records bind one finite execution attempt immutably to its circle,
//! synthetic fixture metadata, and validated code and stack bounds. Scheduling
//! policy remains outside this table; the composite transitions below let that
//! policy change two task records without exposing a half-completed switch.

use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use crate::circle::{circle_child_kind, circle_error, circle_table};
use crate::identity::{fixture_principal_id, next_generation, task_id};

pub(crate) use crate::identity::task_identity;

pub(crate) const task_capacity: usize = 16;

/// An operation budget for the opt-in owned multi-region proof, not an ABI or
/// path limit. Every interval retains its exact initialized-byte boundaries.
#[cfg(any(test, feature = "owned_multi_region_test"))]
pub(crate) const executable_interval_capacity: usize = 16;

#[cfg(any(test, feature = "owned_multi_region_test"))]
const executable_address_limit: u64 = 1 << 47;

/// Raw declared initialized executable bytes. Validation establishes geometry,
/// not copied contents, page permissions, mapping ownership or entry authority.
#[cfg(any(test, feature = "owned_multi_region_test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct executable_interval {
    pub(crate) start: u64,
    pub(crate) end: u64,
}

#[cfg(any(test, feature = "owned_multi_region_test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum executable_interval_error {
    empty,
    capacity_exhausted,
    invalid_extent,
    overlap,
}

/// Owned canonical disjoint intervals; unused slots cannot admit addresses.
#[cfg(any(test, feature = "owned_multi_region_test"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct executable_interval_set {
    intervals: [executable_interval; executable_interval_capacity],
    count: usize,
}

#[cfg(any(test, feature = "owned_multi_region_test"))]
impl executable_interval_set {
    pub(crate) fn new(
        intervals: &[executable_interval],
    ) -> Result<Self, executable_interval_error> {
        if intervals.is_empty() {
            return Err(executable_interval_error::empty);
        }
        if intervals.len() > executable_interval_capacity {
            return Err(executable_interval_error::capacity_exhausted);
        }
        let mut result = Self {
            intervals: [executable_interval { start: 0, end: 0 }; executable_interval_capacity],
            count: intervals.len(),
        };
        for (slot, interval) in result.intervals.iter_mut().zip(intervals) {
            if interval.start == 0
                || interval.start >= interval.end
                || interval.end > executable_address_limit
            {
                return Err(executable_interval_error::invalid_extent);
            }
            *slot = *interval;
        }
        result.intervals[..result.count].sort_unstable_by_key(|interval| interval.start);
        if result.intervals[..result.count]
            .windows(2)
            .any(|pair| pair[0].end > pair[1].start)
        {
            return Err(executable_interval_error::overlap);
        }
        Ok(result)
    }

    pub(crate) fn intervals(&self) -> &[executable_interval] {
        &self.intervals[..self.count]
    }

    pub(crate) const fn contains(self, address: u64) -> bool {
        let mut index = 0;
        while index < self.count {
            let interval = self.intervals[index];
            if address >= interval.start && address < interval.end {
                return true;
            }
            index += 1;
        }
        false
    }
}

/// Synthetic role metadata used to compare equivalent proof clients.
///
/// The value is assigned by the kernel test fixture. It does not authenticate a
/// person, machine, service, or input source and grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum synthetic_origin_metadata {
    human_role_fixture,
    machine_role_fixture,
    receiver_role_fixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum task_state {
    ready,
    running,
    suspended,
    blocked,
    faulted,
    exited,
}

impl task_state {
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::faulted | Self::exited)
    }

    pub(crate) const fn is_dispatchable(self) -> bool {
        matches!(self, Self::ready | Self::suspended)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum task_error {
    invalid_identity,
    stale_identity,
    capacity_exhausted,
    generation_exhausted,
    invalid_state,
    another_task_running,
    same_task,
    context_already_attached,
    context_missing,
    context_still_attached,
    children_live,
    circle_accounting_mismatch,
    record_clearing_failed,
    circle(circle_error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct task_bounds {
    code_start: u64,
    code_end: u64,
    entry: u64,
    stack_start: u64,
    stack_end: u64,
    initial_stack_pointer: u64,
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    executable_intervals: Option<executable_interval_set>,
}

impl task_bounds {
    pub(crate) const fn new(
        code_start: u64,
        code_end: u64,
        entry: u64,
        stack_start: u64,
        stack_end: u64,
        initial_stack_pointer: u64,
    ) -> Option<Self> {
        if code_start >= code_end
            || stack_start >= stack_end
            || entry < code_start
            || entry >= code_end
            || initial_stack_pointer <= stack_start
            || initial_stack_pointer > stack_end
            || (code_start < stack_end && stack_start < code_end)
        {
            return None;
        }
        Some(Self {
            code_start,
            code_end,
            entry,
            stack_start,
            stack_end,
            initial_stack_pointer,
            #[cfg(any(test, feature = "owned_multi_region_test"))]
            executable_intervals: None,
        })
    }

    /// Opt-in exact initialized executable membership. The legacy code getters
    /// describe only the entry-containing interval, never a hull across gaps.
    /// Mapping permissions and copied-byte provenance need separate admission.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(crate) const fn with_executable_intervals(
        intervals: executable_interval_set,
        entry: u64,
        stack_start: u64,
        stack_end: u64,
        initial_stack_pointer: u64,
    ) -> Option<Self> {
        if stack_start == 0
            || stack_start >= stack_end
            || stack_end > executable_address_limit
            || initial_stack_pointer <= stack_start
            || initial_stack_pointer > stack_end
            || initial_stack_pointer >= executable_address_limit
        {
            return None;
        }
        let mut selected = None;
        let mut index = 0;
        while index < intervals.count {
            let interval = intervals.intervals[index];
            if interval.start < stack_end && stack_start < interval.end {
                return None;
            }
            if entry >= interval.start && entry < interval.end {
                selected = Some(interval);
            }
            index += 1;
        }
        match selected {
            Some(code) => Some(Self {
                code_start: code.start,
                code_end: code.end,
                entry,
                stack_start,
                stack_end,
                initial_stack_pointer,
                executable_intervals: Some(intervals),
            }),
            None => None,
        }
    }

    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(crate) const fn executable_intervals(self) -> Option<executable_interval_set> {
        self.executable_intervals
    }

    pub(crate) const fn code_start(self) -> u64 {
        self.code_start
    }

    pub(crate) const fn code_end(self) -> u64 {
        self.code_end
    }

    pub(crate) const fn entry(self) -> u64 {
        self.entry
    }

    pub(crate) const fn stack_start(self) -> u64 {
        self.stack_start
    }

    pub(crate) const fn stack_end(self) -> u64 {
        self.stack_end
    }

    pub(crate) const fn initial_stack_pointer(self) -> u64 {
        self.initial_stack_pointer
    }

    pub(crate) const fn contains_code(self, address: u64) -> bool {
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if let Some(intervals) = self.executable_intervals {
            return intervals.contains(address);
        }
        address >= self.code_start && address < self.code_end
    }

    pub(crate) const fn contains_stack_pointer(self, address: u64) -> bool {
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        if self.executable_intervals.is_some() && address >= executable_address_limit {
            return false;
        }
        address > self.stack_start && address <= self.stack_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct task_binding {
    circle: crate::identity::circle_id,
    principal: fixture_principal_id,
    origin: synthetic_origin_metadata,
    bounds: task_bounds,
}

impl task_binding {
    pub(crate) const fn new(
        circle: crate::identity::circle_id,
        principal: fixture_principal_id,
        origin: synthetic_origin_metadata,
        bounds: task_bounds,
    ) -> Self {
        Self {
            circle,
            principal,
            origin,
            bounds,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct task_snapshot {
    identity: task_identity,
    principal: fixture_principal_id,
    origin: synthetic_origin_metadata,
    bounds: task_bounds,
    state: task_state,
    context_attached: bool,
}

impl task_snapshot {
    pub(crate) const fn identity(self) -> task_identity {
        self.identity
    }

    pub(crate) const fn task(self) -> task_id {
        self.identity.task()
    }

    pub(crate) const fn circle(self) -> crate::identity::circle_id {
        self.identity.circle()
    }

    pub(crate) const fn principal(self) -> fixture_principal_id {
        self.principal
    }

    pub(crate) const fn origin(self) -> synthetic_origin_metadata {
        self.origin
    }

    pub(crate) const fn bounds(self) -> task_bounds {
        self.bounds
    }

    pub(crate) const fn state(self) -> task_state {
        self.state
    }

    pub(crate) const fn context_attached(self) -> bool {
        self.context_attached
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct task_record {
    generation: u64,
    live: bool,
    binding: MaybeUninit<task_binding>,
    state: task_state,
    context_attached: bool,
}

impl task_record {
    const fn empty() -> Self {
        Self {
            generation: 0,
            live: false,
            binding: MaybeUninit::zeroed(),
            state: task_state::exited,
            context_attached: false,
        }
    }

    fn binding(&self) -> task_binding {
        debug_assert!(self.live);
        // SAFETY: Only `create` marks a record live, and it initializes the
        // binding before publishing that state. Teardown clears `live` before
        // the zeroed `MaybeUninit` storage can be observed through this path.
        unsafe { self.binding.assume_init() }
    }
}

pub(crate) struct task_table {
    records: [task_record; task_capacity],
}

/// Exact logical task-table state used only by failure-atomic unit tests.
#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct task_table_test_snapshot {
    records: [task_record_test_snapshot; task_capacity],
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
struct task_record_test_snapshot {
    generation: u64,
    live: bool,
    binding: Option<task_binding>,
    state: task_state,
    context_attached: bool,
}

#[must_use = "dropping a prepared task switch leaves every task unchanged"]
pub(crate) struct prepared_task_switch<'a> {
    tasks: &'a mut task_table,
    current_slot: usize,
    next_slot: usize,
    current_state: task_state,
}

impl prepared_task_switch<'_> {
    pub(crate) fn commit(self) {
        self.tasks.records[self.current_slot].state = self.current_state;
        self.tasks.records[self.next_slot].state = task_state::running;
    }
}

/// Scheduler-sealed proof that one exact task entered `blocked`.
///
/// Only committing [`prepared_task_block`] creates this move-only value. The
/// task table requires it again before preparing the corresponding nonterminal
/// wake, so a copied task identity is never sufficient authority.
#[must_use = "the exact scheduler wake must consume blocked-task authority"]
pub(crate) struct task_wake_authorization {
    identity: task_identity,
}

/// A native call exchanges one running context with either a ready peer or an
/// exactly authorized blocked peer. Dropping preparation changes neither task.
#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "commit only after all call effects and continuation checks are prepared"]
pub(crate) struct prepared_task_call_exchange<'a> {
    tasks: &'a mut task_table,
    current_slot: usize,
    next_slot: usize,
    current: task_identity,
    wake: Option<task_identity>,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_task_call_exchange<'_> {
    pub(crate) fn tasks(&self) -> &task_table {
        self.tasks
    }

    pub(crate) fn commit(self, wake: Option<task_wake_authorization>) -> task_wake_authorization {
        assert_eq!(wake.as_ref().map(|value| value.identity), self.wake);
        self.tasks.records[self.current_slot].state = task_state::blocked;
        self.tasks.records[self.next_slot].state = task_state::running;
        task_wake_authorization {
            identity: self.current,
        }
    }
}

#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "dropping fault preparation retains both contexts unchanged"]
pub(crate) struct prepared_task_call_fault<'a> {
    tasks: &'a mut task_table,
    current_slot: usize,
    caller_slot: usize,
    next_slot: usize,
    caller: task_identity,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_task_call_fault<'_> {
    pub(crate) fn commit(self, authority: task_wake_authorization) {
        assert_eq!(authority.identity, self.caller);
        self.tasks.records[self.current_slot].state = task_state::faulted;
        self.tasks.records[self.caller_slot].state = task_state::faulted;
        self.tasks.records[self.next_slot].state = task_state::running;
    }
}

/// Atomic containment of a failed service, its blocked caller and an optional
/// second dispatchable dependent. Unrelated runnable tasks remain unchanged.
#[cfg(any(test, feature = "veil_application_test"))]
#[must_use = "dropping preparation leaves every dependent task unchanged"]
pub(crate) struct prepared_task_service_fault<'a> {
    tasks: &'a mut task_table,
    current_slot: usize,
    caller_slot: usize,
    peer_slot: Option<usize>,
    next_slot: usize,
    caller: task_identity,
}

#[cfg(any(test, feature = "veil_application_test"))]
impl prepared_task_service_fault<'_> {
    pub(crate) fn commit(self, authority: task_wake_authorization) {
        assert_eq!(authority.identity, self.caller);
        self.tasks.records[self.current_slot].state = task_state::faulted;
        self.tasks.records[self.caller_slot].state = task_state::faulted;
        if let Some(peer_slot) = self.peer_slot {
            self.tasks.records[peer_slot].state = task_state::faulted;
        }
        self.tasks.records[self.next_slot].state = task_state::running;
    }
}

#[must_use = "dropping a prepared task block leaves every task unchanged"]
pub(crate) struct prepared_task_block<'a> {
    tasks: &'a mut task_table,
    current_slot: usize,
    next_slot: usize,
    current: task_identity,
}

impl prepared_task_block<'_> {
    pub(crate) fn commit(self) -> task_wake_authorization {
        self.tasks.records[self.current_slot].state = task_state::blocked;
        self.tasks.records[self.next_slot].state = task_state::running;
        task_wake_authorization {
            identity: self.current,
        }
    }
}

#[must_use = "dropping a prepared scheduled wake leaves the blocked task unchanged"]
pub(crate) struct prepared_task_wake<'a> {
    tasks: &'a mut task_table,
    slot: usize,
    identity: task_identity,
}

impl prepared_task_wake<'_> {
    pub(crate) fn commit(self, authorization: task_wake_authorization) {
        assert!(
            authorization.identity == self.identity,
            "prepared task wake authority changed after preflight"
        );
        self.tasks.records[self.slot].state = task_state::suspended;
    }
}

impl task_table {
    pub(crate) const fn new() -> Self {
        Self {
            records: [task_record::empty(); task_capacity],
        }
    }

    #[cfg(test)]
    pub(crate) fn test_snapshot(&self) -> task_table_test_snapshot {
        task_table_test_snapshot {
            records: core::array::from_fn(|index| {
                let record = &self.records[index];
                task_record_test_snapshot {
                    generation: record.generation,
                    live: record.live,
                    binding: if record.live {
                        Some(record.binding())
                    } else {
                        None
                    },
                    state: record.state,
                    context_attached: record.context_attached,
                }
            }),
        }
    }

    pub(crate) fn create(
        &mut self,
        circles: &mut circle_table,
        binding: task_binding,
    ) -> Result<task_identity, task_error> {
        circles
            .validate(binding.circle)
            .map_err(task_error::circle)?;

        let mut saw_inactive = false;
        let mut candidate = None;
        for (slot, record) in self.records.iter().enumerate() {
            if record.live {
                continue;
            }
            saw_inactive = true;
            if let Some(generation) = next_generation(record.generation) {
                candidate = Some((slot, generation));
                break;
            }
        }
        let Some((slot, generation)) = candidate else {
            return Err(if saw_inactive {
                task_error::generation_exhausted
            } else {
                task_error::capacity_exhausted
            });
        };

        circles
            .attach_child(binding.circle, circle_child_kind::task)
            .map_err(task_error::circle)?;
        self.records[slot] = task_record {
            generation: generation.get(),
            live: true,
            binding: MaybeUninit::new(binding),
            state: task_state::ready,
            context_attached: false,
        };
        Ok(task_identity::new(
            task_id::from_parts(slot, generation),
            binding.circle,
        ))
    }

    pub(crate) fn snapshot(&self, identity: task_identity) -> Result<task_snapshot, task_error> {
        let record = self.record(identity)?;
        let binding = record.binding();
        Ok(task_snapshot {
            identity,
            principal: binding.principal,
            origin: binding.origin,
            bounds: binding.bounds,
            state: record.state,
            context_attached: record.context_attached,
        })
    }

    pub(crate) fn validate(&self, identity: task_identity) -> Result<(), task_error> {
        self.record(identity).map(|_| ())
    }

    pub(crate) fn active_count(&self) -> usize {
        self.records.iter().filter(|record| record.live).count()
    }

    pub(crate) fn attached_context_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.live && record.context_attached)
            .count()
    }

    pub(crate) fn start(&mut self, identity: task_identity) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::ready || self.records[slot].context_attached {
            return Err(task_error::invalid_state);
        }
        if self.has_running_task_other_than(slot) {
            return Err(task_error::another_task_running);
        }
        self.records[slot].state = task_state::running;
        Ok(())
    }

    pub(crate) fn exit(&mut self, identity: task_identity) -> Result<(), task_error> {
        let record = self.record_mut(identity)?;
        if record.state != task_state::running || record.context_attached {
            return Err(task_error::invalid_state);
        }
        record.state = task_state::exited;
        Ok(())
    }

    /// Cancel an unstarted, unscheduled task without entering `running`.
    ///
    /// Only an exact ready task with no scheduler context may become terminal
    /// here. Identity or state rejection changes nothing. The terminal record
    /// retains its circle attachment and all binding metadata until ordinary
    /// destruction; the launch owner must first withdraw its other resources.
    pub(crate) fn cancel_unstarted(&mut self, identity: task_identity) -> Result<(), task_error> {
        let record = self.record_mut(identity)?;
        if record.state != task_state::ready || record.context_attached {
            return Err(task_error::invalid_state);
        }
        record.state = task_state::exited;
        Ok(())
    }

    /// Terminate the single, kernel-resumed native service without pretending
    /// that a peer scheduler context was selected.
    #[cfg(any(test, feature = "workspace_dev", feature = "memory_test"))]
    pub(crate) fn fault_unscheduled(&mut self, identity: task_identity) -> Result<(), task_error> {
        let record = self.record_mut(identity)?;
        if record.state != task_state::running || record.context_attached {
            return Err(task_error::invalid_state);
        }
        record.state = task_state::faulted;
        Ok(())
    }

    /// Attach the one kernel-owned saved-context lifetime used by a scheduler.
    pub(crate) fn attach_context(
        &mut self,
        circles: &mut circle_table,
        identity: task_identity,
    ) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::ready {
            return Err(task_error::invalid_state);
        }
        if self.records[slot].context_attached {
            return Err(task_error::context_already_attached);
        }
        let circle = self.records[slot].binding().circle;
        circles
            .attach_child(circle, circle_child_kind::task_context)
            .map_err(task_error::circle)?;
        self.records[slot].context_attached = true;
        Ok(())
    }

    /// Dispatch either a fresh or previously suspended scheduled task.
    pub(crate) fn dispatch(&mut self, identity: task_identity) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if !self.records[slot].state.is_dispatchable() {
            return Err(task_error::invalid_state);
        }
        if !self.records[slot].context_attached {
            return Err(task_error::context_missing);
        }
        if self.has_running_task_other_than(slot) {
            return Err(task_error::another_task_running);
        }
        self.records[slot].state = task_state::running;
        Ok(())
    }

    /// Atomically suspend the current task and dispatch a runnable peer.
    pub(crate) fn suspend_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<(), task_error> {
        self.prepare_suspend_and_dispatch(current, next)?.commit();
        Ok(())
    }

    /// Atomically block the current task and dispatch a runnable peer.
    pub(crate) fn block_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<(), task_error> {
        let _authorization = self.prepare_block_and_dispatch(current, next)?.commit();
        Ok(())
    }

    /// Block a scheduled task when no dispatch is committed with it.
    pub(crate) fn park_scheduled(&mut self, identity: task_identity) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::running || !self.records[slot].context_attached {
            return Err(task_error::invalid_state);
        }
        if self.has_running_task_other_than(slot) {
            return Err(task_error::another_task_running);
        }
        self.records[slot].state = task_state::blocked;
        Ok(())
    }

    /// Atomically contain a current-task fault and dispatch a runnable peer.
    pub(crate) fn fault_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<(), task_error> {
        self.prepare_fault_and_dispatch(current, next)?.commit();
        Ok(())
    }

    /// Complete a current task and enter its already runnable peer atomically.
    #[cfg(any(
        test,
        feature = "native_application_test",
        feature = "mixed_workspace_test"
    ))]
    pub(crate) fn complete_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<(), task_error> {
        self.prepare_switch(current, next, task_state::exited)?
            .commit();
        Ok(())
    }

    pub(crate) fn prepare_suspend_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<prepared_task_switch<'_>, task_error> {
        self.prepare_switch(current, next, task_state::suspended)
    }

    pub(crate) fn prepare_block_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<prepared_task_block<'_>, task_error> {
        let (current_slot, next_slot) = self.preflight_switch(current, next)?;
        Ok(prepared_task_block {
            tasks: self,
            current_slot,
            next_slot,
            current,
        })
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_call_exchange(
        &mut self,
        current: task_identity,
        next: task_identity,
        wake: Option<&task_wake_authorization>,
    ) -> Result<prepared_task_call_exchange<'_>, task_error> {
        let (current_slot, next_slot) = if let Some(authority) = wake {
            if self.validate_scheduled_wake_authorization(authority)? != next {
                return Err(task_error::invalid_identity);
            }
            self.preflight_wake(current, next)?
        } else {
            self.preflight_switch(current, next)?
        };
        Ok(prepared_task_call_exchange {
            tasks: self,
            current_slot,
            next_slot,
            current,
            wake: wake.map(|value| value.identity),
        })
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_call_fault(
        &mut self,
        current: task_identity,
        caller: &task_wake_authorization,
        next: task_identity,
    ) -> Result<prepared_task_call_fault<'_>, task_error> {
        let (current_slot, next_slot) = self.preflight_switch(current, next)?;
        let (caller, caller_slot) = self.scheduled_wake_slot(caller)?;
        if caller_slot == current_slot || caller_slot == next_slot {
            return Err(task_error::same_task);
        }
        Ok(prepared_task_call_fault {
            tasks: self,
            current_slot,
            caller_slot,
            next_slot,
            caller,
        })
    }

    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn prepare_service_fault(
        &mut self,
        current: task_identity,
        caller: &task_wake_authorization,
        peer: Option<task_identity>,
        next: task_identity,
    ) -> Result<prepared_task_service_fault<'_>, task_error> {
        let (current_slot, next_slot) = self.preflight_switch(current, next)?;
        let (caller, caller_slot) = self.scheduled_wake_slot(caller)?;
        if caller_slot == current_slot || caller_slot == next_slot {
            return Err(task_error::same_task);
        }
        let peer_slot = if let Some(peer) = peer {
            let slot = self.record_index(peer)?;
            if slot == current_slot || slot == caller_slot || slot == next_slot {
                return Err(task_error::same_task);
            }
            if !self.records[slot].state.is_dispatchable() || !self.records[slot].context_attached {
                return Err(task_error::invalid_state);
            }
            Some(slot)
        } else {
            None
        };
        Ok(prepared_task_service_fault {
            tasks: self,
            current_slot,
            caller_slot,
            peer_slot,
            next_slot,
            caller,
        })
    }

    /// Controller retirement of a blocked dependency never makes it runnable.
    /// The enclosing scheduler consumes the same sealed authority on success.
    #[cfg(any(test, feature = "veil_application_test"))]
    pub(crate) fn retire_authorized_wait(
        &mut self,
        authority: &task_wake_authorization,
    ) -> Result<(), task_error> {
        let (_, slot) = self.scheduled_wake_slot(authority)?;
        self.records[slot].state = task_state::exited;
        Ok(())
    }

    pub(crate) fn prepare_fault_and_dispatch(
        &mut self,
        current: task_identity,
        next: task_identity,
    ) -> Result<prepared_task_switch<'_>, task_error> {
        self.prepare_switch(current, next, task_state::faulted)
    }

    /// Prepare the scheduler half of one exact nonterminal wake.
    ///
    /// Committing this transition alone does not make the task runnable. The
    /// scheduler must append the same task to its runnable FIFO in the same
    /// exclusion interval.
    pub(crate) fn prepare_scheduled_wake(
        &mut self,
        authorization: &task_wake_authorization,
    ) -> Result<prepared_task_wake<'_>, task_error> {
        let (identity, slot) = self.scheduled_wake_slot(authorization)?;
        Ok(prepared_task_wake {
            tasks: self,
            slot,
            identity,
        })
    }

    /// Validate scheduler-sealed blocked-task authority without consuming it.
    pub(crate) fn validate_scheduled_wake_authorization(
        &self,
        authorization: &task_wake_authorization,
    ) -> Result<task_identity, task_error> {
        self.scheduled_wake_slot(authorization)
            .map(|(identity, _)| identity)
    }

    fn scheduled_wake_slot(
        &self,
        authorization: &task_wake_authorization,
    ) -> Result<(task_identity, usize), task_error> {
        let identity = authorization.identity;
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::blocked || !self.records[slot].context_attached {
            return Err(task_error::invalid_state);
        }
        Ok((identity, slot))
    }

    /// Atomically contain a current-task fault and wake a blocked task.
    pub(crate) fn fault_and_wake(
        &mut self,
        current: task_identity,
        wake: task_identity,
    ) -> Result<(), task_error> {
        let (current_slot, wake_slot) = self.preflight_wake(current, wake)?;
        self.records[current_slot].state = task_state::faulted;
        self.records[wake_slot].state = task_state::running;
        Ok(())
    }

    /// Atomically complete the current task and wake a blocked task.
    pub(crate) fn complete_and_wake(
        &mut self,
        current: task_identity,
        wake: task_identity,
    ) -> Result<(), task_error> {
        let (current_slot, wake_slot) = self.preflight_wake(current, wake)?;
        self.records[current_slot].state = task_state::exited;
        self.records[wake_slot].state = task_state::running;
        Ok(())
    }

    /// Finish a scheduled task while retaining its terminal context for wipe.
    pub(crate) fn complete_scheduled(&mut self, identity: task_identity) -> Result<(), task_error> {
        let record = self.record_mut(identity)?;
        if record.state != task_state::running || !record.context_attached {
            return Err(task_error::invalid_state);
        }
        record.state = task_state::exited;
        Ok(())
    }

    /// Fault a scheduled task while retaining its context for later withdrawal.
    #[cfg(any(test, feature = "application_profile_test"))]
    pub(crate) fn fault_scheduled(&mut self, identity: task_identity) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::running || !self.records[slot].context_attached {
            return Err(task_error::invalid_state);
        }
        if self.has_running_task_other_than(slot) {
            return Err(task_error::another_task_running);
        }
        self.records[slot].state = task_state::faulted;
        Ok(())
    }

    /// Roll back context admission for a task that has never been dispatched.
    ///
    /// The scheduler must first preflight withdrawal from its initial runnable
    /// queue and commit that removal in the same exclusion interval. This hook
    /// changes only context ownership: task identity, binding, ready state, and
    /// other circle children survive for the retained launch owner's cancellation.
    pub(crate) fn withdraw_unstarted_context(
        &mut self,
        circles: &mut circle_table,
        identity: task_identity,
    ) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if self.records[slot].state != task_state::ready {
            return Err(task_error::invalid_state);
        }
        if !self.records[slot].context_attached {
            return Err(task_error::context_missing);
        }
        let circle = self.records[slot].binding().circle;
        let children = circles
            .snapshot(circle)
            .map_err(task_error::circle)?
            .children();
        if children.tasks() != 1 || children.task_contexts() != 1 {
            return Err(task_error::circle_accounting_mismatch);
        }
        circles
            .detach_child(circle, circle_child_kind::task_context)
            .map_err(task_error::circle)?;
        self.records[slot].context_attached = false;
        Ok(())
    }

    /// Detach context ownership only after a task has become terminal.
    pub(crate) fn withdraw_context(
        &mut self,
        circles: &mut circle_table,
        identity: task_identity,
    ) -> Result<(), task_error> {
        let slot = self.record_index(identity)?;
        if !self.records[slot].state.is_terminal() {
            return Err(task_error::invalid_state);
        }
        if !self.records[slot].context_attached {
            return Err(task_error::context_missing);
        }
        let circle = self.records[slot].binding().circle;
        circles
            .detach_child(circle, circle_child_kind::task_context)
            .map_err(task_error::circle)?;
        self.records[slot].context_attached = false;
        Ok(())
    }

    pub(crate) fn destroy(
        &mut self,
        circles: &mut circle_table,
        identity: task_identity,
    ) -> Result<(), task_error> {
        // Privacy verification failure is terminal for this logical owner:
        // authority has already been detached and the slot is a tombstone, so
        // callers must contain the proof instead of attempting rollback.
        let slot = self.record_index(identity)?;
        if !self.records[slot].state.is_terminal() {
            return Err(task_error::invalid_state);
        }
        if self.records[slot].context_attached {
            return Err(task_error::context_still_attached);
        }
        let binding = self.records[slot].binding();
        let circle = circles
            .snapshot(binding.circle)
            .map_err(task_error::circle)?;
        if circle.children().tasks() != 1 {
            return Err(task_error::circle_accounting_mismatch);
        }
        if circle.children().total() != 1 {
            return Err(task_error::children_live);
        }
        circles
            .detach_child(binding.circle, circle_child_kind::task)
            .map_err(task_error::circle)?;
        if !clear_record_and_preserve_generation(&mut self.records[slot]) {
            return Err(task_error::record_clearing_failed);
        }
        Ok(())
    }

    fn record(&self, identity: task_identity) -> Result<&task_record, task_error> {
        let slot = self.record_index(identity)?;
        Ok(&self.records[slot])
    }

    fn record_mut(&mut self, identity: task_identity) -> Result<&mut task_record, task_error> {
        let slot = self.record_index(identity)?;
        Ok(&mut self.records[slot])
    }

    fn record_index(&self, identity: task_identity) -> Result<usize, task_error> {
        let task = identity.task();
        let Some(record) = self.records.get(task.slot()) else {
            return Err(task_error::invalid_identity);
        };
        if !record.live || record.generation != task.generation_value() {
            return Err(task_error::stale_identity);
        }
        let binding = record.binding();
        if binding.circle != identity.circle() {
            return Err(task_error::stale_identity);
        }
        Ok(task.slot())
    }

    fn preflight_switch(
        &self,
        current: task_identity,
        next: task_identity,
    ) -> Result<(usize, usize), task_error> {
        if current == next {
            return Err(task_error::same_task);
        }
        let current_slot = self.record_index(current)?;
        let next_slot = self.record_index(next)?;
        if self.records[current_slot].state != task_state::running
            || !self.records[current_slot].context_attached
            || !self.records[next_slot].state.is_dispatchable()
            || !self.records[next_slot].context_attached
        {
            return Err(task_error::invalid_state);
        }
        if self.has_running_task_other_than(current_slot) {
            return Err(task_error::another_task_running);
        }
        Ok((current_slot, next_slot))
    }

    fn prepare_switch(
        &mut self,
        current: task_identity,
        next: task_identity,
        current_state: task_state,
    ) -> Result<prepared_task_switch<'_>, task_error> {
        let (current_slot, next_slot) = self.preflight_switch(current, next)?;
        Ok(prepared_task_switch {
            tasks: self,
            current_slot,
            next_slot,
            current_state,
        })
    }

    fn preflight_wake(
        &self,
        current: task_identity,
        wake: task_identity,
    ) -> Result<(usize, usize), task_error> {
        if current == wake {
            return Err(task_error::same_task);
        }
        let current_slot = self.record_index(current)?;
        let wake_slot = self.record_index(wake)?;
        if self.records[current_slot].state != task_state::running
            || !self.records[current_slot].context_attached
            || self.records[wake_slot].state != task_state::blocked
            || !self.records[wake_slot].context_attached
        {
            return Err(task_error::invalid_state);
        }
        if self.has_running_task_other_than(current_slot) {
            return Err(task_error::another_task_running);
        }
        Ok((current_slot, wake_slot))
    }

    fn has_running_task_other_than(&self, selected_slot: usize) -> bool {
        self.records.iter().enumerate().any(|(slot, record)| {
            record.live && record.state == task_state::running && slot != selected_slot
        })
    }
}

fn clear_record_and_preserve_generation(record: &mut task_record) -> bool {
    let generation = record.generation;
    let generation_offset =
        ptr::addr_of!(record.generation) as usize - ptr::from_ref(record) as usize;
    let state_offset = ptr::addr_of!(record.state) as usize - ptr::from_ref(record) as usize;
    let record_pointer = ptr::from_mut(record);
    let byte_pointer = record_pointer.cast::<u8>();

    // SAFETY: `record_pointer` names one uniquely borrowed, initialized,
    // `Copy` record. No byte is observed while the record is being cleared,
    // and the field-wise volatile stores restore a valid tombstone before the
    // function returns.
    unsafe {
        for offset in 0..core::mem::size_of::<task_record>() {
            ptr::write_volatile(byte_pointer.add(offset), 0);
        }
        ptr::write_volatile(ptr::addr_of_mut!((*record_pointer).generation), generation);
        ptr::write_volatile(
            ptr::addr_of_mut!((*record_pointer).state),
            task_state::exited,
        );
    }
    compiler_fence(Ordering::SeqCst);

    let generation_bytes = generation.to_ne_bytes();
    for offset in 0..core::mem::size_of::<task_record>() {
        let expected =
            if (generation_offset..generation_offset + generation_bytes.len()).contains(&offset) {
                generation_bytes[offset - generation_offset]
            } else if offset == state_offset {
                task_state::exited as u8
            } else {
                0
            };
        // SAFETY: Every byte in the record was initialized by the volatile
        // clear or one of the two field stores above. `binding` is
        // `MaybeUninit`, so its cleared representation is always valid.
        if unsafe { ptr::read_volatile(byte_pointer.add(offset)) } != expected {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> task_bounds {
        task_bounds::new(0x1000, 0x2000, 0x1000, 0x3000, 0x4000, 0x4000).expect("valid bounds")
    }

    fn principal(value: u64) -> fixture_principal_id {
        fixture_principal_id::new(value).expect("fixture principal")
    }

    fn binding(circle: crate::identity::circle_id) -> task_binding {
        task_binding::new(
            circle,
            principal(1),
            synthetic_origin_metadata::human_role_fixture,
            bounds(),
        )
    }

    #[test]
    fn bounds_require_distinct_nonempty_code_and_stack_ranges() {
        assert!(task_bounds::new(1, 1, 1, 3, 4, 4).is_none());
        assert!(task_bounds::new(1, 2, 2, 3, 4, 4).is_none());
        assert!(task_bounds::new(1, 3, 1, 2, 4, 4).is_none());
        assert!(task_bounds::new(1, 2, 1, 3, 3, 3).is_none());
        assert!(task_bounds::new(1, 2, 1, 3, 4, 3).is_none());

        let bounds = bounds();
        assert!(bounds.contains_code(bounds.entry()));
        assert!(bounds.contains_stack_pointer(bounds.initial_stack_pointer()));
        assert!(!bounds.contains_code(bounds.code_end()));
        assert!(!bounds.contains_stack_pointer(bounds.stack_start()));
        assert_eq!(bounds.code_start(), 0x1000);
        assert_eq!(bounds.stack_end(), 0x4000);
        assert_eq!(bounds.executable_intervals(), None);
    }

    #[test]
    fn executable_intervals_are_canonical_exact_and_bounded() {
        let first = executable_interval {
            start: 0x1003,
            end: 0x1105,
        };
        let adjacent = executable_interval {
            start: 0x1105,
            end: 0x1120,
        };
        let last = executable_interval {
            start: 0x9000,
            end: 0x9087,
        };
        let set = executable_interval_set::new(&[last, adjacent, first]).expect("disjoint set");
        assert_eq!(set.intervals(), &[first, adjacent, last]);
        for address in [
            first.start,
            first.end - 1,
            adjacent.start,
            adjacent.end - 1,
            last.start,
            last.end - 1,
        ] {
            assert!(set.contains(address));
        }
        for address in [
            0,
            first.start - 1,
            adjacent.end,
            0x2000,
            last.start - 1,
            last.end,
            executable_address_limit,
            u64::MAX,
        ] {
            assert!(!set.contains(address));
        }
        assert_eq!(
            executable_interval_set::new(&[]),
            Err(executable_interval_error::empty)
        );
        let many: [_; executable_interval_capacity + 1] =
            core::array::from_fn(|index| executable_interval {
                start: 0x1000 + index as u64 * 4,
                end: 0x1002 + index as u64 * 4,
            });
        assert_eq!(
            executable_interval_set::new(&many),
            Err(executable_interval_error::capacity_exhausted)
        );
        assert_eq!(
            executable_interval_set::new(&many[..executable_interval_capacity])
                .unwrap()
                .intervals()
                .len(),
            executable_interval_capacity
        );
    }

    #[test]
    fn executable_intervals_reject_invalid_extents_and_every_overlap_shape() {
        for (start, end) in [
            (0, 1),
            (1, 1),
            (2, 1),
            (executable_address_limit - 1, executable_address_limit + 1),
            (executable_address_limit, executable_address_limit + 1),
            (u64::MAX - 1, u64::MAX),
        ] {
            assert_eq!(
                executable_interval_set::new(&[executable_interval { start, end }]),
                Err(executable_interval_error::invalid_extent)
            );
        }
        let valid = executable_interval {
            start: 0x1000,
            end: 0x2000,
        };
        for other in [
            valid,
            executable_interval {
                start: 0x1001,
                end: 0x1fff,
            },
            executable_interval {
                start: 0x1001,
                end: 0x2001,
            },
            executable_interval {
                start: 0xfff,
                end: 0x2001,
            },
        ] {
            for intervals in [[valid, other], [other, valid]] {
                assert_eq!(
                    executable_interval_set::new(&intervals),
                    Err(executable_interval_error::overlap)
                );
            }
        }
        let ceiling = executable_interval_set::new(&[executable_interval {
            start: executable_address_limit - 1,
            end: executable_address_limit,
        }])
        .unwrap();
        assert!(ceiling.contains(executable_address_limit - 1));
        assert!(!ceiling.contains(executable_address_limit));
    }

    #[test]
    fn exact_task_bounds_retain_all_intervals_without_widening_the_code_getters() {
        let set = executable_interval_set::new(&[
            executable_interval {
                start: 0x1010,
                end: 0x1070,
            },
            executable_interval {
                start: 0x8010,
                end: 0x8090,
            },
        ])
        .unwrap();
        for interval in set.intervals() {
            let bounds =
                task_bounds::with_executable_intervals(set, interval.start, 0x4000, 0x5000, 0x5000)
                    .unwrap();
            assert_eq!(bounds.code_start(), interval.start);
            assert_eq!(bounds.code_end(), interval.end);
            assert_eq!(bounds.executable_intervals(), Some(set));
            for expected in set.intervals() {
                assert!(bounds.contains_code(expected.start));
                assert!(bounds.contains_code(expected.end - 1));
                assert!(!bounds.contains_code(expected.end));
            }
            for gap in [0x1000, 0x2000, 0x4000, 0x5000, 0x8000, 0x9000] {
                assert!(!bounds.contains_code(gap));
            }
        }
        for entry in [
            0,
            0x100f,
            0x1070,
            0x4000,
            0x8090,
            executable_address_limit,
            u64::MAX,
        ] {
            assert!(
                task_bounds::with_executable_intervals(set, entry, 0x4000, 0x5000, 0x5000)
                    .is_none()
            );
        }
    }

    #[test]
    fn exact_task_stack_cannot_overlap_any_interval_or_use_noncanonical_pointers() {
        let set = executable_interval_set::new(&[
            executable_interval {
                start: 0x1000,
                end: 0x1100,
            },
            executable_interval {
                start: 0x8000,
                end: 0x8100,
            },
        ])
        .unwrap();
        for (start, end, rsp) in [
            (0, 0x4000, 0x4000),
            (0x3000, 0x3000, 0x3000),
            (0x4000, 0x3000, 0x3000),
            (0x3000, 0x4000, 0x3000),
            (0x3000, 0x4000, 0x4001),
            (0x1000, 0x1100, 0x1100),
            (0x7fff, 0x8001, 0x8001),
            (0x8000, 0x9000, 0x9000),
            (
                executable_address_limit - 0x1000,
                executable_address_limit,
                executable_address_limit,
            ),
            (
                executable_address_limit - 0x1000,
                executable_address_limit + 1,
                executable_address_limit - 16,
            ),
        ] {
            assert!(task_bounds::with_executable_intervals(set, 0x1000, start, end, rsp).is_none());
        }
        let bounds = task_bounds::with_executable_intervals(
            set,
            0x1000,
            executable_address_limit - 0x1000,
            executable_address_limit,
            executable_address_limit - 16,
        )
        .unwrap();
        assert!(bounds.contains_stack_pointer(executable_address_limit - 8));
        assert!(!bounds.contains_stack_pointer(executable_address_limit));
    }

    #[test]
    fn lifecycle_is_exactly_ready_running_exited() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");

        assert_eq!(
            tasks.snapshot(identity).expect("task").state(),
            task_state::ready
        );
        assert_eq!(tasks.exit(identity), Err(task_error::invalid_state));
        tasks.start(identity).expect("start task");
        assert_eq!(tasks.start(identity), Err(task_error::invalid_state));
        tasks.exit(identity).expect("exit task");
        assert_eq!(tasks.exit(identity), Err(task_error::invalid_state));
        assert_eq!(tasks.start(identity), Err(task_error::invalid_state));
        tasks
            .destroy(&mut circles, identity)
            .expect("destroy exited task");
        circles.destroy(circle).expect("task child removed");
    }

    #[test]
    fn unstarted_cancellation_retains_binding_and_children_until_destruction() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let peer_circle = circles.create().expect("peer circle");
        circles
            .attach_child(circle, circle_child_kind::address_space)
            .expect("address space");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        let peer = tasks
            .create(&mut circles, binding(peer_circle))
            .expect("peer");
        tasks.start(peer).expect("running peer");
        let before = tasks.snapshot(identity).expect("ready task");
        let peer_before = tasks.snapshot(peer).expect("peer state");
        let accounting = circles.accounting();

        tasks.cancel_unstarted(identity).expect("cancel ready task");
        assert_eq!(
            tasks.snapshot(identity),
            Ok(task_snapshot {
                state: task_state::exited,
                ..before
            })
        );
        assert_eq!(tasks.snapshot(peer), Ok(peer_before));
        assert_eq!(circles.accounting(), accounting);
        assert_eq!(tasks.start(identity), Err(task_error::invalid_state));
        assert_eq!(
            tasks.destroy(&mut circles, identity),
            Err(task_error::children_live)
        );
        let terminal = tasks.test_snapshot();
        assert_eq!(
            tasks.cancel_unstarted(identity),
            Err(task_error::invalid_state)
        );
        assert!(tasks.test_snapshot() == terminal);
        assert_eq!(circles.accounting(), accounting);
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .expect("withdraw address space");
        tasks.destroy(&mut circles, identity).expect("destroy task");
        circles.destroy(circle).expect("destroy circle");
        tasks.exit(peer).expect("exit peer");
        tasks.destroy(&mut circles, peer).expect("destroy peer");
        circles.destroy(peer_circle).expect("destroy peer circle");
    }

    #[test]
    fn unstarted_cancellation_rejects_every_started_state_atomically() {
        for state in [
            task_state::running,
            task_state::suspended,
            task_state::blocked,
            task_state::faulted,
            task_state::exited,
        ] {
            let mut circles = circle_table::new();
            let circle = circles.create().expect("circle");
            let peer_circle = circles.create().expect("peer circle");
            let mut tasks = task_table::new();
            let identity = tasks.create(&mut circles, binding(circle)).expect("task");
            let peer = tasks
                .create(&mut circles, binding(peer_circle))
                .expect("peer");
            match state {
                task_state::suspended | task_state::blocked => {
                    for task in [identity, peer] {
                        tasks.attach_context(&mut circles, task).expect("context");
                    }
                    tasks.dispatch(identity).expect("dispatch");
                    if state == task_state::suspended {
                        tasks.suspend_and_dispatch(identity, peer).expect("suspend");
                    } else {
                        tasks.block_and_dispatch(identity, peer).expect("block");
                    }
                }
                _ => {
                    tasks.start(identity).expect("start");
                    if state == task_state::faulted {
                        tasks.fault_unscheduled(identity).expect("fault");
                    } else if state == task_state::exited {
                        tasks.exit(identity).expect("exit");
                    }
                }
            }
            assert_eq!(tasks.snapshot(identity).expect("task").state(), state);
            let before = tasks.test_snapshot();
            let accounting = circles.accounting();
            assert_eq!(
                tasks.cancel_unstarted(identity),
                Err(task_error::invalid_state)
            );
            assert!(tasks.test_snapshot() == before);
            assert_eq!(circles.accounting(), accounting);
        }
    }

    #[test]
    fn unstarted_cancellation_rejects_attached_ready_context_atomically() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        tasks
            .attach_context(&mut circles, identity)
            .expect("context");
        let before = tasks.test_snapshot();
        let accounting = circles.accounting();
        assert_eq!(
            tasks.cancel_unstarted(identity),
            Err(task_error::invalid_state)
        );
        assert!(tasks.test_snapshot() == before);
        assert_eq!(circles.accounting(), accounting);
        tasks
            .dispatch(identity)
            .expect("ready context still dispatches");
    }

    #[test]
    fn unstarted_cancellation_rejects_invalid_stale_and_wrong_circle_identities() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let other_circle = circles.create().expect("other circle");
        let mut tasks = task_table::new();
        let stale = tasks.create(&mut circles, binding(circle)).expect("task");
        tasks
            .cancel_unstarted(stale)
            .expect("cancel first generation");
        tasks
            .destroy(&mut circles, stale)
            .expect("destroy first generation");
        let identity = tasks
            .create(&mut circles, binding(circle))
            .expect("replacement");
        assert_ne!(identity, stale);
        let invalid = task_identity::new(
            task_id::try_from_parts(task_capacity, 1).expect("invalid slot identity"),
            circle,
        );
        let wrong_circle = task_identity::new(identity.task(), other_circle);
        let before = tasks.test_snapshot();
        let accounting = circles.accounting();
        for (rejected, error) in [
            (invalid, task_error::invalid_identity),
            (stale, task_error::stale_identity),
            (wrong_circle, task_error::stale_identity),
        ] {
            assert_eq!(tasks.cancel_unstarted(rejected), Err(error));
            assert!(tasks.test_snapshot() == before);
            assert_eq!(circles.accounting(), accounting);
        }
        tasks
            .start(identity)
            .expect("replacement remains startable");
    }

    #[test]
    fn binding_and_synthetic_metadata_never_change() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let binding = task_binding::new(
            circle,
            principal(77),
            synthetic_origin_metadata::machine_role_fixture,
            bounds(),
        );
        let identity = tasks.create(&mut circles, binding).expect("task");
        let before = tasks.snapshot(identity).expect("snapshot");
        tasks.start(identity).expect("start");
        let running = tasks.snapshot(identity).expect("snapshot");
        tasks.exit(identity).expect("exit");
        let exited = tasks.snapshot(identity).expect("snapshot");

        assert_eq!(before.identity(), identity);
        assert_eq!(before.task(), identity.task());
        assert_eq!(before.circle(), circle);
        assert_eq!(before.principal(), principal(77));
        assert_eq!(
            before.origin(),
            synthetic_origin_metadata::machine_role_fixture
        );
        assert_eq!(before.bounds(), running.bounds());
        assert_eq!(running.bounds(), exited.bounds());
        assert_eq!(running.principal(), exited.principal());
        assert_eq!(running.origin(), exited.origin());
    }

    #[test]
    fn live_task_prevents_circle_destruction_and_early_task_destruction() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");

        assert_eq!(circles.destroy(circle), Err(circle_error::children_live));
        assert_eq!(
            tasks.destroy(&mut circles, identity),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            circles.snapshot(circle).expect("circle").children().tasks(),
            1
        );
        assert_eq!(
            tasks.create(&mut circles, binding(circle)),
            Err(task_error::circle(circle_error::child_limit_exceeded))
        );
        assert_eq!(
            circles.snapshot(circle).expect("circle").children().tasks(),
            1
        );
    }

    #[test]
    fn replacement_invalidates_old_task_and_advances_generation() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(circle))
            .expect("first task");
        tasks.start(first).expect("start");
        tasks.exit(first).expect("exit");
        tasks.destroy(&mut circles, first).expect("destroy first");
        let second = tasks
            .create(&mut circles, binding(circle))
            .expect("replacement task");

        assert_eq!(first.task().slot(), second.task().slot());
        assert_eq!(first.task().generation_value(), 1);
        assert_eq!(second.task().generation_value(), 2);
        assert_eq!(tasks.snapshot(first), Err(task_error::stale_identity));
        assert!(tasks.snapshot(second).is_ok());
    }

    #[test]
    fn forged_circle_component_makes_composite_identity_stale() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let identity = tasks
            .create(&mut circles, binding(first_circle))
            .expect("task");
        let forged = task_identity::new(identity.task(), second_circle);

        assert_eq!(tasks.snapshot(forged), Err(task_error::stale_identity));
        assert_eq!(
            tasks.snapshot(identity).expect("task").circle(),
            first_circle
        );
    }

    #[test]
    fn invalid_circle_and_task_capacity_fail_without_child_mutation() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let invalid_circle =
            crate::identity::circle_id::try_from_parts(crate::circle::circle_capacity, 1)
                .expect("syntactically valid identity");
        let mut tasks = task_table::new();
        assert_eq!(
            tasks.create(&mut circles, binding(invalid_circle)),
            Err(task_error::circle(circle_error::invalid_identity))
        );
        assert_eq!(
            circles.snapshot(circle).expect("circle").children().tasks(),
            0
        );

        let mut task_circles = [circle; task_capacity];
        for task_circle in task_circles.iter_mut().skip(1) {
            *task_circle = circles.create().expect("circle within capacity");
        }
        for (index, task_circle) in task_circles.into_iter().enumerate() {
            let origin = match index % 3 {
                0 => synthetic_origin_metadata::human_role_fixture,
                1 => synthetic_origin_metadata::machine_role_fixture,
                _ => synthetic_origin_metadata::receiver_role_fixture,
            };
            tasks
                .create(
                    &mut circles,
                    task_binding::new(task_circle, principal(index as u64 + 1), origin, bounds()),
                )
                .expect("task within capacity");
        }
        let before = circles.snapshot(circle).expect("snapshot");
        assert_eq!(
            tasks.create(&mut circles, binding(circle)),
            Err(task_error::capacity_exhausted)
        );
        assert_eq!(circles.snapshot(circle), Ok(before));
    }

    #[test]
    fn task_generation_exhaustion_is_failure_atomic() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        for record in &mut tasks.records {
            record.generation = u64::MAX;
        }
        let before = circles.snapshot(circle).expect("snapshot");

        assert_eq!(
            tasks.create(&mut circles, binding(circle)),
            Err(task_error::generation_exhausted)
        );
        assert_eq!(circles.snapshot(circle), Ok(before));
    }

    #[test]
    fn exhausted_task_slot_is_skipped_when_another_slot_is_reusable() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        tasks.records[0].generation = u64::MAX;

        let identity = tasks
            .create(&mut circles, binding(circle))
            .expect("second slot remains reusable");

        assert_eq!(identity.task().slot(), 1);
        assert!(tasks.validate(identity).is_ok());
        assert_eq!(
            circles.snapshot(circle).expect("circle").children().tasks(),
            1
        );
    }

    #[test]
    fn stale_and_invalid_task_ids_do_not_mutate_state() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        let invalid_task = task_id::try_from_parts(task_capacity, 1).expect("identity");
        let invalid = task_identity::new(invalid_task, circle);
        let stale_task = task_id::try_from_parts(identity.task().slot(), 2).expect("identity");
        let stale = task_identity::new(stale_task, circle);

        assert_eq!(tasks.start(invalid), Err(task_error::invalid_identity));
        assert_eq!(tasks.start(stale), Err(task_error::stale_identity));
        assert_eq!(
            tasks.snapshot(identity).expect("task").state(),
            task_state::ready
        );
    }

    #[test]
    fn only_one_task_can_run_without_a_scheduler() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");

        tasks.start(first).expect("start first");
        assert_eq!(tasks.start(second), Err(task_error::another_task_running));
        assert_eq!(
            tasks.snapshot(second).expect("second task").state(),
            task_state::ready
        );
        tasks.exit(first).expect("exit first");
        tasks.start(second).expect("start second after first exits");
    }

    #[test]
    fn active_count_tracks_task_creation_and_destruction() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let _second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        assert_eq!(tasks.active_count(), 2);

        tasks.start(first).expect("start first");
        tasks.exit(first).expect("exit first");
        tasks.destroy(&mut circles, first).expect("destroy first");
        assert_eq!(tasks.active_count(), 1);
    }

    #[test]
    fn child_authority_must_withdraw_before_task_storage() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        tasks.start(identity).expect("start");
        tasks.exit(identity).expect("exit");
        circles
            .attach_child(circle, circle_child_kind::sigil)
            .expect("attach authority");

        assert_eq!(
            tasks.destroy(&mut circles, identity),
            Err(task_error::children_live)
        );
        assert!(tasks.validate(identity).is_ok());
        circles
            .detach_child(circle, circle_child_kind::sigil)
            .expect("withdraw authority");
        tasks
            .destroy(&mut circles, identity)
            .expect("destroy after authority");
    }

    #[test]
    fn duplicate_and_stale_destroy_do_not_change_circle_accounting() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        tasks.start(identity).expect("start");
        tasks.exit(identity).expect("exit");
        tasks.destroy(&mut circles, identity).expect("destroy");
        let before = circles.snapshot(circle).expect("circle snapshot");
        let invalid_task = task_id::try_from_parts(task_capacity, 1).expect("identity");
        let invalid = task_identity::new(invalid_task, circle);

        assert_eq!(
            tasks.destroy(&mut circles, invalid),
            Err(task_error::invalid_identity)
        );
        assert_eq!(circles.snapshot(circle), Ok(before));

        assert_eq!(
            tasks.destroy(&mut circles, identity),
            Err(task_error::stale_identity)
        );
        assert_eq!(circles.snapshot(circle), Ok(before));
    }

    #[test]
    fn destroyed_task_binding_storage_is_volatile_zeroed() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks
            .create(
                &mut circles,
                task_binding::new(
                    circle,
                    principal(0xfeed_face),
                    synthetic_origin_metadata::machine_role_fixture,
                    bounds(),
                ),
            )
            .expect("task");
        tasks.start(identity).expect("start");
        tasks.exit(identity).expect("exit");
        tasks.destroy(&mut circles, identity).expect("destroy");

        let record = &tasks.records[identity.task().slot()];
        let binding_pointer = ptr::addr_of!(record.binding).cast::<u8>();
        for offset in 0..core::mem::size_of::<MaybeUninit<task_binding>>() {
            // SAFETY: Teardown initialized every byte in the `MaybeUninit`
            // storage through volatile zero stores.
            assert_eq!(
                unsafe { ptr::read_volatile(binding_pointer.add(offset)) },
                0
            );
        }
    }

    #[test]
    fn scheduled_lifecycle_is_ready_running_suspended_running_faulted() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");

        tasks
            .attach_context(&mut circles, first)
            .expect("first context");
        tasks
            .attach_context(&mut circles, second)
            .expect("second context");
        assert_eq!(tasks.attached_context_count(), 2);
        tasks.dispatch(first).expect("first dispatch");
        tasks
            .suspend_and_dispatch(first, second)
            .expect("dispatch second");
        assert_eq!(
            tasks.snapshot(first).expect("first").state(),
            task_state::suspended
        );
        assert_eq!(
            tasks.snapshot(second).expect("second").state(),
            task_state::running
        );
        tasks
            .suspend_and_dispatch(second, first)
            .expect("resume first");
        tasks
            .fault_and_dispatch(first, second)
            .expect("contain first fault");
        assert_eq!(
            tasks.snapshot(first).expect("first").state(),
            task_state::faulted
        );
        tasks.complete_scheduled(second).expect("complete second");
        assert_eq!(
            tasks.snapshot(second).expect("second").state(),
            task_state::exited
        );

        tasks
            .withdraw_context(&mut circles, first)
            .expect("withdraw first");
        tasks
            .withdraw_context(&mut circles, second)
            .expect("withdraw second");
        assert_eq!(tasks.attached_context_count(), 0);
        tasks
            .destroy(&mut circles, first)
            .expect("destroy faulted task");
        tasks
            .destroy(&mut circles, second)
            .expect("destroy exited task");
    }

    #[test]
    fn unstarted_context_withdrawal_preserves_ready_binding_and_other_children() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        circles
            .attach_child(circle, circle_child_kind::address_space)
            .unwrap();
        circles
            .attach_children(circle, circle_child_kind::mapping, 3)
            .unwrap();
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");
        let ready = tasks.snapshot(identity).unwrap();
        let original_children = circles.accounting();
        tasks.attach_context(&mut circles, identity).unwrap();
        assert_eq!(tasks.attached_context_count(), 1);

        tasks
            .withdraw_unstarted_context(&mut circles, identity)
            .unwrap();
        assert_eq!(tasks.snapshot(identity), Ok(ready));
        assert_eq!(circles.accounting(), original_children);
        assert_eq!(tasks.attached_context_count(), 0);
        // Withdrawal alone does not cancel the launch or manufacture execution.
        tasks.cancel_unstarted(identity).unwrap();
        assert_eq!(
            tasks.snapshot(identity).unwrap().state(),
            task_state::exited
        );
        circles
            .detach_children(circle, circle_child_kind::mapping, 3)
            .unwrap();
        circles
            .detach_child(circle, circle_child_kind::address_space)
            .unwrap();
        tasks.destroy(&mut circles, identity).unwrap();
        circles.destroy(circle).unwrap();
    }

    #[test]
    fn unstarted_context_withdrawal_rejects_every_started_state_atomically() {
        for state in [
            task_state::running,
            task_state::suspended,
            task_state::blocked,
            task_state::faulted,
            task_state::exited,
        ] {
            let mut circles = circle_table::new();
            let circle = circles.create().unwrap();
            let peer_circle = circles.create().unwrap();
            let mut tasks = task_table::new();
            let identity = tasks.create(&mut circles, binding(circle)).unwrap();
            let peer = tasks.create(&mut circles, binding(peer_circle)).unwrap();
            for task in [identity, peer] {
                tasks.attach_context(&mut circles, task).unwrap();
            }
            tasks.dispatch(identity).unwrap();
            match state {
                task_state::suspended => tasks.suspend_and_dispatch(identity, peer).unwrap(),
                task_state::blocked => tasks.block_and_dispatch(identity, peer).unwrap(),
                task_state::faulted => tasks.fault_and_dispatch(identity, peer).unwrap(),
                task_state::exited => tasks.complete_scheduled(identity).unwrap(),
                task_state::running => {}
                task_state::ready => unreachable!(),
            }
            let before = tasks.test_snapshot();
            let children = circles.accounting();
            assert_eq!(
                tasks.withdraw_unstarted_context(&mut circles, identity),
                Err(task_error::invalid_state)
            );
            assert!(tasks.test_snapshot() == before);
            assert_eq!(circles.accounting(), children);
        }
    }

    #[test]
    fn unstarted_context_withdrawal_rejects_missing_repeated_and_wrong_identities() {
        let mut circles = circle_table::new();
        let circle = circles.create().unwrap();
        let other_circle = circles.create().unwrap();
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).unwrap();
        let unattached = tasks.test_snapshot();
        let children = circles.accounting();
        assert_eq!(
            tasks.withdraw_unstarted_context(&mut circles, identity),
            Err(task_error::context_missing)
        );
        assert!(tasks.test_snapshot() == unattached);
        assert_eq!(circles.accounting(), children);
        tasks.attach_context(&mut circles, identity).unwrap();
        let attached = tasks.test_snapshot();
        let children = circles.accounting();
        let invalid =
            task_identity::new(task_id::try_from_parts(task_capacity, 1).unwrap(), circle);
        let stale = task_identity::new(
            task_id::try_from_parts(identity.task().slot(), 2).unwrap(),
            circle,
        );
        let wrong_circle = task_identity::new(identity.task(), other_circle);
        for (candidate, error) in [
            (invalid, task_error::invalid_identity),
            (stale, task_error::stale_identity),
            (wrong_circle, task_error::stale_identity),
        ] {
            assert_eq!(
                tasks.withdraw_unstarted_context(&mut circles, candidate),
                Err(error)
            );
            assert!(tasks.test_snapshot() == attached);
            assert_eq!(circles.accounting(), children);
        }
        tasks
            .withdraw_unstarted_context(&mut circles, identity)
            .unwrap();
        let withdrawn = tasks.test_snapshot();
        let children = circles.accounting();
        assert_eq!(
            tasks.withdraw_unstarted_context(&mut circles, identity),
            Err(task_error::context_missing)
        );
        assert!(tasks.test_snapshot() == withdrawn);
        assert_eq!(circles.accounting(), children);
    }

    #[test]
    fn unstarted_context_withdrawal_rejects_corrupt_circle_accounting_atomically() {
        for kind in [circle_child_kind::task, circle_child_kind::task_context] {
            let mut circles = circle_table::new();
            let circle = circles.create().unwrap();
            let mut tasks = task_table::new();
            let identity = tasks.create(&mut circles, binding(circle)).unwrap();
            tasks.attach_context(&mut circles, identity).unwrap();
            circles.detach_child(circle, kind).unwrap();
            let before = tasks.test_snapshot();
            let children = circles.accounting();
            assert_eq!(
                tasks.withdraw_unstarted_context(&mut circles, identity),
                Err(task_error::circle_accounting_mismatch)
            );
            assert!(tasks.test_snapshot() == before);
            assert_eq!(circles.accounting(), children);
            circles.attach_child(circle, kind).unwrap();
            tasks
                .withdraw_unstarted_context(&mut circles, identity)
                .unwrap();
            assert!(!tasks.snapshot(identity).unwrap().context_attached());
        }
    }

    #[test]
    fn context_attachment_and_withdrawal_require_exact_states() {
        let mut circles = circle_table::new();
        let circle = circles.create().expect("circle");
        let mut tasks = task_table::new();
        let identity = tasks.create(&mut circles, binding(circle)).expect("task");

        assert_eq!(
            tasks.withdraw_context(&mut circles, identity),
            Err(task_error::invalid_state)
        );
        tasks
            .attach_context(&mut circles, identity)
            .expect("attach");
        assert_eq!(
            tasks.attach_context(&mut circles, identity),
            Err(task_error::context_already_attached)
        );
        assert_eq!(tasks.start(identity), Err(task_error::invalid_state));
        tasks.dispatch(identity).expect("dispatch");
        assert_eq!(
            tasks.withdraw_context(&mut circles, identity),
            Err(task_error::invalid_state)
        );
        assert_eq!(tasks.exit(identity), Err(task_error::invalid_state));
        tasks
            .complete_scheduled(identity)
            .expect("scheduled completion");
        let terminal = tasks.snapshot(identity).expect("terminal task");
        assert_eq!(tasks.dispatch(identity), Err(task_error::invalid_state));
        assert_eq!(tasks.snapshot(identity), Ok(terminal));
        assert_eq!(
            tasks.destroy(&mut circles, identity),
            Err(task_error::context_still_attached)
        );
        tasks
            .withdraw_context(&mut circles, identity)
            .expect("withdraw");
        assert_eq!(
            tasks.withdraw_context(&mut circles, identity),
            Err(task_error::context_missing)
        );
        tasks.destroy(&mut circles, identity).expect("destroy");
    }

    #[test]
    fn composite_switch_failures_leave_both_tasks_unchanged() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        tasks
            .attach_context(&mut circles, first)
            .expect("first context");
        tasks.dispatch(first).expect("dispatch first");
        let first_before = tasks.snapshot(first).expect("first");
        let second_before = tasks.snapshot(second).expect("second");

        assert_eq!(
            tasks.suspend_and_dispatch(first, first),
            Err(task_error::same_task)
        );
        assert_eq!(
            tasks.suspend_and_dispatch(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            tasks.fault_and_dispatch(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        tasks
            .attach_context(&mut circles, second)
            .expect("second context");
        tasks
            .suspend_and_dispatch(first, second)
            .expect("valid switch");
        let first_suspended = tasks.snapshot(first).expect("first suspended");
        let second_running = tasks.snapshot(second).expect("second running");
        assert_eq!(
            tasks.fault_and_dispatch(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(tasks.snapshot(first), Ok(first_suspended));
        assert_eq!(tasks.snapshot(second), Ok(second_running));
    }

    #[test]
    fn blocked_task_is_live_saved_and_not_dispatchable() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        tasks
            .attach_context(&mut circles, first)
            .expect("first context");
        tasks
            .attach_context(&mut circles, second)
            .expect("second context");
        tasks.dispatch(first).expect("dispatch first");

        tasks
            .block_and_dispatch(first, second)
            .expect("block first");
        let blocked = tasks.snapshot(first).expect("blocked task");
        assert_eq!(blocked.state(), task_state::blocked);
        assert!(!blocked.state().is_terminal());
        assert!(!blocked.state().is_dispatchable());
        assert!(blocked.context_attached());
        assert_eq!(tasks.dispatch(first), Err(task_error::invalid_state));
        assert_eq!(
            tasks.withdraw_context(&mut circles, first),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            tasks.destroy(&mut circles, first),
            Err(task_error::invalid_state)
        );

        tasks
            .fault_and_wake(second, first)
            .expect("fault second and wake first");
        assert_eq!(
            tasks.snapshot(second).expect("faulted peer").state(),
            task_state::faulted
        );
        assert_eq!(
            tasks.snapshot(first).expect("woken task").state(),
            task_state::running
        );
        tasks.complete_scheduled(first).expect("complete first");
        for identity in [first, second] {
            tasks
                .withdraw_context(&mut circles, identity)
                .expect("withdraw context");
            tasks.destroy(&mut circles, identity).expect("destroy task");
        }
    }

    #[test]
    fn park_and_terminal_wake_composites_are_exact() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        for identity in [first, second] {
            tasks
                .attach_context(&mut circles, identity)
                .expect("attach context");
        }

        tasks.dispatch(first).expect("dispatch first");
        tasks.park_scheduled(first).expect("park first");
        assert_eq!(
            tasks.snapshot(first).expect("parked first").state(),
            task_state::blocked
        );
        tasks.dispatch(second).expect("dispatch second");
        tasks
            .complete_and_wake(second, first)
            .expect("complete second and wake first");
        assert_eq!(
            tasks.snapshot(second).expect("completed second").state(),
            task_state::exited
        );
        assert_eq!(
            tasks.snapshot(first).expect("woken first").state(),
            task_state::running
        );
    }

    #[test]
    fn blocked_composite_failures_leave_every_task_unchanged() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        tasks
            .attach_context(&mut circles, first)
            .expect("first context");
        tasks.dispatch(first).expect("dispatch first");
        let first_before = tasks.snapshot(first).expect("first");
        let second_before = tasks.snapshot(second).expect("second");

        assert_eq!(
            tasks.block_and_dispatch(first, first),
            Err(task_error::same_task)
        );
        assert_eq!(
            tasks.block_and_dispatch(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            tasks.fault_and_wake(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            tasks.complete_and_wake(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(tasks.snapshot(first), Ok(first_before));
        assert_eq!(tasks.snapshot(second), Ok(second_before));

        tasks
            .attach_context(&mut circles, second)
            .expect("second context");
        tasks
            .block_and_dispatch(first, second)
            .expect("block first");
        let first_blocked = tasks.snapshot(first).expect("first blocked");
        let second_running = tasks.snapshot(second).expect("second running");
        assert_eq!(tasks.park_scheduled(first), Err(task_error::invalid_state));
        assert_eq!(
            tasks.fault_and_wake(first, second),
            Err(task_error::invalid_state)
        );
        assert_eq!(
            tasks.complete_and_wake(second, second),
            Err(task_error::same_task)
        );
        assert_eq!(tasks.snapshot(first), Ok(first_blocked));
        assert_eq!(tasks.snapshot(second), Ok(second_running));
    }

    #[test]
    fn prepared_scheduled_wake_is_exact_and_failure_atomic() {
        let mut circles = circle_table::new();
        let first_circle = circles.create().expect("first circle");
        let second_circle = circles.create().expect("second circle");
        let mut tasks = task_table::new();
        let first = tasks
            .create(&mut circles, binding(first_circle))
            .expect("first task");
        let second = tasks
            .create(&mut circles, binding(second_circle))
            .expect("second task");
        for identity in [first, second] {
            tasks
                .attach_context(&mut circles, identity)
                .expect("attach context");
        }
        tasks.dispatch(first).expect("dispatch first");
        let authorization = tasks
            .prepare_block_and_dispatch(first, second)
            .expect("prepare block first")
            .commit();

        let first_blocked = tasks.snapshot(first).expect("blocked first");
        let second_running = tasks.snapshot(second).expect("running second");
        drop(
            tasks
                .prepare_scheduled_wake(&authorization)
                .expect("prepare exact wake"),
        );
        assert_eq!(tasks.snapshot(first), Ok(first_blocked));
        assert_eq!(tasks.snapshot(second), Ok(second_running));

        let prepared = tasks
            .prepare_scheduled_wake(&authorization)
            .expect("prepare exact wake");
        prepared.commit(authorization);
        assert_eq!(
            tasks.snapshot(first).expect("woken first").state(),
            task_state::suspended
        );
        assert_eq!(tasks.snapshot(second), Ok(second_running));

        let first_suspended = tasks.snapshot(first).expect("suspended first");
        let replayed = task_wake_authorization { identity: first };
        let wrong = task_wake_authorization { identity: second };
        assert_eq!(
            tasks.prepare_scheduled_wake(&replayed).err(),
            Some(task_error::invalid_state)
        );
        assert_eq!(
            tasks.prepare_scheduled_wake(&wrong).err(),
            Some(task_error::invalid_state)
        );
        assert_eq!(tasks.snapshot(first), Ok(first_suspended));
        assert_eq!(tasks.snapshot(second), Ok(second_running));
    }
}
