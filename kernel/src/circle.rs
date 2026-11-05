#![cfg_attr(not(test), allow(dead_code))]

//! Fixed-capacity lifecycle ownership for bounded protection domains.

use core::ptr;
use core::sync::atomic::{Ordering, compiler_fence};

use crate::identity::{circle_id, next_generation};

pub(crate) const circle_capacity: usize = 16;
pub(crate) const circle_mapping_capacity: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum circle_error {
    invalid_identity,
    stale_identity,
    capacity_exhausted,
    generation_exhausted,
    child_count_exhausted,
    child_count_underflow,
    child_limit_exceeded,
    children_live,
    record_clearing_failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum circle_child_kind {
    task,
    task_context,
    endpoint,
    sigil,
    supervision,
    address_space,
    mapping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(crate) struct circle_child_counts {
    tasks: u16,
    task_contexts: u16,
    endpoints: u16,
    sigils: u16,
    supervisions: u16,
    address_spaces: u16,
    mappings: u16,
}

impl circle_child_counts {
    const fn empty() -> Self {
        Self {
            tasks: 0,
            task_contexts: 0,
            endpoints: 0,
            sigils: 0,
            supervisions: 0,
            address_spaces: 0,
            mappings: 0,
        }
    }

    pub(crate) const fn tasks(self) -> u16 {
        self.tasks
    }

    pub(crate) const fn task_contexts(self) -> u16 {
        self.task_contexts
    }

    pub(crate) const fn endpoints(self) -> u16 {
        self.endpoints
    }

    pub(crate) const fn sigils(self) -> u16 {
        self.sigils
    }

    pub(crate) const fn supervisions(self) -> u16 {
        self.supervisions
    }

    pub(crate) const fn address_spaces(self) -> u16 {
        self.address_spaces
    }

    pub(crate) const fn mappings(self) -> u16 {
        self.mappings
    }

    pub(crate) const fn total(self) -> u32 {
        self.tasks as u32
            + self.task_contexts as u32
            + self.endpoints as u32
            + self.sigils as u32
            + self.supervisions as u32
            + self.address_spaces as u32
            + self.mappings as u32
    }

    fn count_mut(&mut self, kind: circle_child_kind) -> &mut u16 {
        match kind {
            circle_child_kind::task => &mut self.tasks,
            circle_child_kind::task_context => &mut self.task_contexts,
            circle_child_kind::endpoint => &mut self.endpoints,
            circle_child_kind::sigil => &mut self.sigils,
            circle_child_kind::supervision => &mut self.supervisions,
            circle_child_kind::address_space => &mut self.address_spaces,
            circle_child_kind::mapping => &mut self.mappings,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct circle_snapshot {
    identity: circle_id,
    children: circle_child_counts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct circle_accounting {
    pub circles: usize,
    pub tasks: usize,
    pub task_contexts: usize,
    pub endpoints: usize,
    pub sigils: usize,
    pub supervisions: usize,
    pub address_spaces: usize,
    pub mappings: usize,
}

impl circle_snapshot {
    pub(crate) const fn identity(self) -> circle_id {
        self.identity
    }

    pub(crate) const fn children(self) -> circle_child_counts {
        self.children
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
struct circle_record {
    generation: u64,
    live: bool,
    children: circle_child_counts,
}

impl circle_record {
    const fn empty() -> Self {
        Self {
            generation: 0,
            live: false,
            children: circle_child_counts::empty(),
        }
    }
}

pub(crate) struct circle_table {
    records: [circle_record; circle_capacity],
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test",
        feature = "owned_multi_region_test",
        feature = "linux_loader_entry_test"
    ))]
    mapping_capacity: u16,
}

impl circle_table {
    pub(crate) const fn new() -> Self {
        Self {
            records: [circle_record::empty(); circle_capacity],
            #[cfg(any(
                test,
                feature = "native_runtime_test",
                feature = "mixed_workspace_test",
                feature = "owned_multi_region_test",
                feature = "linux_loader_entry_test"
            ))]
            mapping_capacity: circle_mapping_capacity,
        }
    }

    /// Explicit trusted launch policy for the bounded native arena proof.
    /// It is immutable for this table's lifetime; ordinary tables retain the
    /// three-mapping limit even when both profiles are compiled together.
    #[cfg(any(
        test,
        feature = "native_runtime_test",
        feature = "mixed_workspace_test"
    ))]
    pub(crate) const fn new_native_arenas() -> Self {
        Self {
            records: [circle_record::empty(); circle_capacity],
            mapping_capacity: 4,
        }
    }

    /// Explicit controller policy for a nine-region owned launch. Ordinary
    /// tables retain their three-mapping limit even when this code is compiled.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(crate) const fn new_multi_region() -> Self {
        Self {
            records: [circle_record::empty(); circle_capacity],
            mapping_capacity: 9,
        }
    }

    /// Explicit trusted loader policy: nine launch mappings plus one retained
    /// runtime mapping. This capacity is not permission to attach or publish;
    /// the genuine owner must account for the extra mapping's whole lifecycle.
    #[cfg(any(test, feature = "linux_loader_entry_test"))]
    pub(crate) const fn new_loader_runtime() -> Self {
        Self {
            records: [circle_record::empty(); circle_capacity],
            mapping_capacity: 10,
        }
    }

    pub(crate) fn create(&mut self) -> Result<circle_id, circle_error> {
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
                circle_error::generation_exhausted
            } else {
                circle_error::capacity_exhausted
            });
        };

        self.records[slot] = circle_record {
            generation: generation.get(),
            live: true,
            children: circle_child_counts::empty(),
        };
        Ok(circle_id::from_parts(slot, generation))
    }

    pub(crate) fn validate(&self, identity: circle_id) -> Result<(), circle_error> {
        self.record(identity).map(|_| ())
    }

    pub(crate) fn snapshot(&self, identity: circle_id) -> Result<circle_snapshot, circle_error> {
        let record = self.record(identity)?;
        Ok(circle_snapshot {
            identity,
            children: record.children,
        })
    }

    pub(crate) fn accounting(&self) -> circle_accounting {
        let mut accounting = circle_accounting {
            circles: 0,
            tasks: 0,
            task_contexts: 0,
            endpoints: 0,
            sigils: 0,
            supervisions: 0,
            address_spaces: 0,
            mappings: 0,
        };
        for record in self.records.iter().filter(|record| record.live) {
            accounting.circles += 1;
            accounting.tasks += usize::from(record.children.tasks);
            accounting.task_contexts += usize::from(record.children.task_contexts);
            accounting.endpoints += usize::from(record.children.endpoints);
            accounting.sigils += usize::from(record.children.sigils);
            accounting.supervisions += usize::from(record.children.supervisions);
            accounting.address_spaces += usize::from(record.children.address_spaces);
            accounting.mappings += usize::from(record.children.mappings);
        }
        accounting
    }

    pub(crate) fn attach_child(
        &mut self,
        identity: circle_id,
        kind: circle_child_kind,
    ) -> Result<(), circle_error> {
        self.attach_children(identity, kind, 1)
    }

    pub(crate) fn attach_children(
        &mut self,
        identity: circle_id,
        kind: circle_child_kind,
        amount: u16,
    ) -> Result<(), circle_error> {
        #[cfg(not(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test",
            feature = "owned_multi_region_test",
            feature = "linux_loader_entry_test"
        )))]
        let mapping_limit = circle_mapping_capacity;
        #[cfg(any(
            test,
            feature = "native_runtime_test",
            feature = "mixed_workspace_test",
            feature = "owned_multi_region_test",
            feature = "linux_loader_entry_test"
        ))]
        let mapping_limit = self.mapping_capacity;
        let record = self.record_mut(identity)?;
        let count = record.children.count_mut(kind);
        let Some(next) = count.checked_add(amount) else {
            return Err(circle_error::child_count_exhausted);
        };
        let limit = match kind {
            circle_child_kind::task
            | circle_child_kind::task_context
            | circle_child_kind::supervision
            | circle_child_kind::address_space => 1,
            circle_child_kind::mapping => mapping_limit,
            circle_child_kind::endpoint | circle_child_kind::sigil => u16::MAX,
        };
        if next > limit {
            return Err(circle_error::child_limit_exceeded);
        }
        *count = next;
        Ok(())
    }

    pub(crate) fn detach_child(
        &mut self,
        identity: circle_id,
        kind: circle_child_kind,
    ) -> Result<(), circle_error> {
        self.detach_children(identity, kind, 1)
    }

    pub(crate) fn detach_children(
        &mut self,
        identity: circle_id,
        kind: circle_child_kind,
        amount: u16,
    ) -> Result<(), circle_error> {
        let record = self.record_mut(identity)?;
        let count = record.children.count_mut(kind);
        let Some(next) = count.checked_sub(amount) else {
            return Err(circle_error::child_count_underflow);
        };
        *count = next;
        Ok(())
    }

    pub(crate) fn destroy(&mut self, identity: circle_id) -> Result<(), circle_error> {
        // A clearing failure leaves a detached tombstone and is terminal for
        // the logical owner; callers must contain it rather than retrying.
        let slot = self.record_index(identity)?;
        if self.records[slot].children.total() != 0 {
            return Err(circle_error::children_live);
        }
        if !clear_record_and_preserve_generation(&mut self.records[slot]) {
            return Err(circle_error::record_clearing_failed);
        }
        Ok(())
    }

    fn record(&self, identity: circle_id) -> Result<&circle_record, circle_error> {
        let slot = self.record_index(identity)?;
        Ok(&self.records[slot])
    }

    fn record_mut(&mut self, identity: circle_id) -> Result<&mut circle_record, circle_error> {
        let slot = self.record_index(identity)?;
        Ok(&mut self.records[slot])
    }

    fn record_index(&self, identity: circle_id) -> Result<usize, circle_error> {
        let slot = identity.slot();
        let Some(record) = self.records.get(slot) else {
            return Err(circle_error::invalid_identity);
        };
        if !record.live || record.generation != identity.generation_value() {
            return Err(circle_error::stale_identity);
        }
        Ok(slot)
    }
}

fn clear_record_and_preserve_generation(record: &mut circle_record) -> bool {
    let generation = record.generation;
    let generation_offset =
        ptr::addr_of!(record.generation) as usize - ptr::from_ref(record) as usize;
    let record_pointer = ptr::from_mut(record);
    let byte_pointer = record_pointer.cast::<u8>();

    // SAFETY: `record_pointer` names one uniquely borrowed, initialized,
    // `Copy` record. Every byte is restored to a valid tombstone before the
    // record is observed again. Volatile stores make privacy clearing part of
    // the externally observable teardown contract.
    unsafe {
        for offset in 0..core::mem::size_of::<circle_record>() {
            ptr::write_volatile(byte_pointer.add(offset), 0);
        }
        ptr::write_volatile(ptr::addr_of_mut!((*record_pointer).generation), generation);
    }
    compiler_fence(Ordering::SeqCst);

    let generation_bytes = generation.to_ne_bytes();
    for offset in 0..core::mem::size_of::<circle_record>() {
        let expected =
            if (generation_offset..generation_offset + generation_bytes.len()).contains(&offset) {
                generation_bytes[offset - generation_offset]
            } else {
                0
            };
        // SAFETY: Every byte in the live record was initialized by the
        // volatile clear or preserved-generation store above.
        if unsafe { ptr::read_volatile(byte_pointer.add(offset)) } != expected {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_destroy_and_recreate_advance_generation() {
        let mut circles = circle_table::new();
        let first = circles.create().expect("first circle");
        circles.destroy(first).expect("destroy empty circle");
        let second = circles.create().expect("replacement circle");

        assert_eq!(first.slot(), second.slot());
        assert_eq!(first.generation_value(), 1);
        assert_eq!(second.generation_value(), 2);
        assert_eq!(circles.validate(first), Err(circle_error::stale_identity));
        assert!(circles.validate(second).is_ok());
    }

    #[test]
    fn capacity_failure_does_not_mutate_live_circles() {
        let mut circles = circle_table::new();
        let mut identities = [None; circle_capacity];
        for identity in &mut identities {
            *identity = Some(circles.create().expect("circle within capacity"));
        }

        assert_eq!(circles.create(), Err(circle_error::capacity_exhausted));
        for identity in identities.into_iter().flatten() {
            assert!(circles.validate(identity).is_ok());
        }
    }

    #[test]
    fn invalid_and_stale_identities_cannot_mutate_children() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        let invalid = circle_id::try_from_parts(circle_capacity, 1).expect("identity");
        let stale = circle_id::try_from_parts(identity.slot(), 2).expect("identity");

        assert_eq!(
            circles.attach_child(invalid, circle_child_kind::task),
            Err(circle_error::invalid_identity)
        );
        assert_eq!(
            circles.attach_child(stale, circle_child_kind::task),
            Err(circle_error::stale_identity)
        );
        assert_eq!(
            circles.snapshot(identity).expect("live circle").children(),
            circle_child_counts::empty()
        );
    }

    #[test]
    fn every_child_kind_blocks_destruction_until_detached() {
        for kind in [
            circle_child_kind::task,
            circle_child_kind::task_context,
            circle_child_kind::endpoint,
            circle_child_kind::sigil,
            circle_child_kind::supervision,
            circle_child_kind::address_space,
            circle_child_kind::mapping,
        ] {
            let mut circles = circle_table::new();
            let identity = circles.create().expect("circle");
            circles.attach_child(identity, kind).expect("attach child");
            let before = circles.snapshot(identity).expect("snapshot");

            assert_eq!(circles.destroy(identity), Err(circle_error::children_live));
            assert_eq!(circles.snapshot(identity), Ok(before));
            circles.detach_child(identity, kind).expect("detach child");
            circles.destroy(identity).expect("destroy empty circle");
        }
    }

    #[test]
    fn child_underflow_and_overflow_are_failure_atomic() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        assert_eq!(
            circles.detach_child(identity, circle_child_kind::task),
            Err(circle_error::child_count_underflow)
        );

        circles.records[identity.slot()].children.tasks = u16::MAX;
        let before = circles.snapshot(identity).expect("snapshot");
        assert_eq!(
            circles.attach_child(identity, circle_child_kind::task),
            Err(circle_error::child_count_exhausted)
        );
        assert_eq!(circles.snapshot(identity), Ok(before));
    }

    #[test]
    fn batch_child_updates_validate_before_one_mutation() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        circles
            .attach_children(identity, circle_child_kind::sigil, 7)
            .expect("batch attach");
        assert_eq!(
            circles
                .snapshot(identity)
                .expect("snapshot")
                .children()
                .sigils(),
            7
        );
        circles
            .detach_children(identity, circle_child_kind::sigil, 3)
            .expect("batch detach");
        assert_eq!(
            circles
                .snapshot(identity)
                .expect("snapshot")
                .children()
                .sigils(),
            4
        );

        let before = circles.snapshot(identity).expect("snapshot");
        assert_eq!(
            circles.detach_children(identity, circle_child_kind::sigil, 5),
            Err(circle_error::child_count_underflow)
        );
        assert_eq!(circles.snapshot(identity), Ok(before));

        circles
            .attach_children(identity, circle_child_kind::sigil, 0)
            .expect("zero attach");
        circles
            .detach_children(identity, circle_child_kind::sigil, 0)
            .expect("zero detach");
        assert_eq!(circles.snapshot(identity), Ok(before));
    }

    #[test]
    fn one_task_one_root_and_three_mapping_limits_are_failure_atomic() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        for kind in [
            circle_child_kind::task,
            circle_child_kind::task_context,
            circle_child_kind::supervision,
            circle_child_kind::address_space,
        ] {
            circles.attach_child(identity, kind).expect("first child");
            let before = circles.snapshot(identity).expect("snapshot");
            assert_eq!(
                circles.attach_child(identity, kind),
                Err(circle_error::child_limit_exceeded)
            );
            assert_eq!(circles.snapshot(identity), Ok(before));
            circles.detach_child(identity, kind).expect("detach child");
        }

        circles
            .attach_children(
                identity,
                circle_child_kind::mapping,
                circle_mapping_capacity,
            )
            .expect("fixed mapping set");
        let before = circles.snapshot(identity).expect("snapshot");
        assert_eq!(
            circles.attach_child(identity, circle_child_kind::mapping),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(circles.snapshot(identity), Ok(before));
    }

    #[test]
    fn native_arena_mapping_policy_is_explicit_bounded_and_failure_atomic() {
        let mut ordinary = circle_table::new();
        let identity = ordinary.create().unwrap();
        let before = ordinary.snapshot(identity).unwrap();
        assert_eq!(
            ordinary.attach_children(identity, circle_child_kind::mapping, 4),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(ordinary.snapshot(identity), Ok(before));

        let mut arenas = circle_table::new_native_arenas();
        let identity = arenas.create().unwrap();
        let before = arenas.snapshot(identity).unwrap();
        assert_eq!(
            arenas.attach_children(identity, circle_child_kind::mapping, 5),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(arenas.snapshot(identity), Ok(before));
        arenas
            .attach_children(identity, circle_child_kind::mapping, 4)
            .unwrap();
        let before = arenas.snapshot(identity).unwrap();
        assert_eq!(before.children().mappings(), 4);
        assert_eq!(before.children().total(), 4);
        assert_eq!(arenas.accounting().mappings, 4);
        assert_eq!(
            arenas.attach_child(identity, circle_child_kind::mapping),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(
            arenas.detach_children(identity, circle_child_kind::mapping, 5),
            Err(circle_error::child_count_underflow)
        );
        assert_eq!(arenas.snapshot(identity), Ok(before));
        arenas
            .detach_children(identity, circle_child_kind::mapping, 4)
            .unwrap();
        arenas.destroy(identity).unwrap();
        assert_eq!(arenas.accounting(), circle_table::new().accounting());
        let next = arenas.create().unwrap();
        assert_ne!(next, identity);
        arenas
            .attach_children(next, circle_child_kind::mapping, 4)
            .unwrap();
        assert_eq!(
            arenas.attach_child(next, circle_child_kind::mapping),
            Err(circle_error::child_limit_exceeded)
        );
    }

    #[test]
    fn multi_region_mapping_policy_preserves_old_limits_and_exact_lifecycle() {
        for (mut table, limit) in [
            (circle_table::new(), 3),
            (circle_table::new_native_arenas(), 4),
        ] {
            let identity = table.create().unwrap();
            let before = table.snapshot(identity).unwrap();
            assert_eq!(
                table.attach_children(identity, circle_child_kind::mapping, 9),
                Err(circle_error::child_limit_exceeded)
            );
            assert_eq!(table.snapshot(identity), Ok(before));
            table
                .attach_children(identity, circle_child_kind::mapping, limit)
                .unwrap();
            assert_eq!(
                table.attach_child(identity, circle_child_kind::mapping),
                Err(circle_error::child_limit_exceeded)
            );
        }
        let mut table = circle_table::new_multi_region();
        let identity = table.create().unwrap();
        table
            .attach_children(identity, circle_child_kind::mapping, 9)
            .unwrap();
        let before = table.snapshot(identity).unwrap();
        assert_eq!(before.children().mappings(), 9);
        assert_eq!(table.accounting().mappings, 9);
        assert_eq!(
            table.attach_child(identity, circle_child_kind::mapping),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(
            table.detach_children(identity, circle_child_kind::mapping, 10),
            Err(circle_error::child_count_underflow)
        );
        assert_eq!(table.snapshot(identity), Ok(before));
        table
            .detach_children(identity, circle_child_kind::mapping, 9)
            .unwrap();
        table.destroy(identity).unwrap();
        assert_eq!(table.accounting(), circle_table::new().accounting());
        let replacement = table.create().unwrap();
        assert_ne!(replacement, identity);
        assert_eq!(
            table.attach_children(identity, circle_child_kind::mapping, 9),
            Err(circle_error::stale_identity)
        );
        table
            .attach_children(replacement, circle_child_kind::mapping, 9)
            .unwrap();
    }

    #[test]
    fn loader_runtime_capacity_keeps_existing_limits_and_retains_extra_child_until_detached() {
        for (mut table, limit) in [
            (circle_table::new(), 3),
            (circle_table::new_native_arenas(), 4),
            (circle_table::new_multi_region(), 9),
        ] {
            let identity = table.create().unwrap();
            table
                .attach_children(identity, circle_child_kind::mapping, limit)
                .unwrap();
            let before = (table.snapshot(identity).unwrap(), table.accounting());
            assert_eq!(
                table.attach_child(identity, circle_child_kind::mapping),
                Err(circle_error::child_limit_exceeded)
            );
            assert_eq!(
                (table.snapshot(identity).unwrap(), table.accounting()),
                before
            );
        }
        let mut table = circle_table::new_loader_runtime();
        let identity = table.create().unwrap();
        table
            .attach_children(identity, circle_child_kind::mapping, 9)
            .unwrap();
        table
            .attach_child(identity, circle_child_kind::mapping)
            .unwrap();
        let before = (table.snapshot(identity).unwrap(), table.accounting());
        assert_eq!(before.0.children().mappings(), 10);
        assert_eq!(before.1.mappings, 10);
        assert_eq!(
            table.attach_child(identity, circle_child_kind::mapping),
            Err(circle_error::child_limit_exceeded)
        );
        assert_eq!(
            table.detach_children(identity, circle_child_kind::mapping, 11),
            Err(circle_error::child_count_underflow)
        );
        assert_eq!(
            (table.snapshot(identity).unwrap(), table.accounting()),
            before
        );
        table
            .detach_children(identity, circle_child_kind::mapping, 9)
            .unwrap();
        assert_eq!(table.destroy(identity), Err(circle_error::children_live));
        assert_eq!(table.snapshot(identity).unwrap().children().mappings(), 1);
        table
            .detach_child(identity, circle_child_kind::mapping)
            .unwrap();
        table.destroy(identity).unwrap();
        assert_eq!(table.accounting(), circle_table::new().accounting());
        let replacement = table.create().unwrap();
        assert_ne!(replacement, identity);
        assert_eq!(
            table.attach_child(identity, circle_child_kind::mapping),
            Err(circle_error::stale_identity)
        );
        table
            .attach_children(replacement, circle_child_kind::mapping, 10)
            .unwrap();
    }

    #[test]
    fn exhausted_slot_is_skipped_and_total_exhaustion_is_atomic() {
        let mut circles = circle_table::new();
        circles.records[0].generation = u64::MAX;
        let identity = circles.create().expect("second slot remains reusable");
        assert_eq!(identity.slot(), 1);

        circles.destroy(identity).expect("destroy circle");
        for record in &mut circles.records {
            record.generation = u64::MAX;
            record.live = false;
        }
        let before = circles.records.map(|record| record.generation);
        assert_eq!(circles.create(), Err(circle_error::generation_exhausted));
        assert_eq!(circles.records.map(|record| record.generation), before);
    }

    #[test]
    fn snapshot_reports_distinct_child_counts() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        circles
            .attach_child(identity, circle_child_kind::task)
            .expect("task child");
        circles
            .attach_child(identity, circle_child_kind::task_context)
            .expect("task-context child");
        circles
            .attach_child(identity, circle_child_kind::endpoint)
            .expect("endpoint child");
        circles
            .attach_child(identity, circle_child_kind::sigil)
            .expect("sigil child");
        circles
            .attach_child(identity, circle_child_kind::supervision)
            .expect("supervision child");
        circles
            .attach_child(identity, circle_child_kind::address_space)
            .expect("address-space child");
        circles
            .attach_children(identity, circle_child_kind::mapping, 3)
            .expect("mapping children");
        let children = circles.snapshot(identity).expect("snapshot").children();

        assert_eq!(children.tasks(), 1);
        assert_eq!(children.task_contexts(), 1);
        assert_eq!(children.endpoints(), 1);
        assert_eq!(children.sigils(), 1);
        assert_eq!(children.supervisions(), 1);
        assert_eq!(children.address_spaces(), 1);
        assert_eq!(children.mappings(), 3);
        assert_eq!(children.total(), 9);
    }

    #[test]
    fn accounting_tracks_live_circles_and_every_child_kind() {
        let mut circles = circle_table::new();
        let first = circles.create().expect("first circle");
        let second = circles.create().expect("second circle");
        circles
            .attach_child(first, circle_child_kind::task)
            .expect("task child");
        circles
            .attach_child(first, circle_child_kind::task_context)
            .expect("task-context child");
        circles
            .attach_child(first, circle_child_kind::endpoint)
            .expect("endpoint child");
        circles
            .attach_child(first, circle_child_kind::sigil)
            .expect("sigil child");
        circles
            .attach_child(first, circle_child_kind::supervision)
            .expect("supervision child");
        circles
            .attach_child(first, circle_child_kind::address_space)
            .expect("address-space child");
        circles
            .attach_children(first, circle_child_kind::mapping, 3)
            .expect("mapping children");

        assert_eq!(
            circles.accounting(),
            circle_accounting {
                circles: 2,
                tasks: 1,
                task_contexts: 1,
                endpoints: 1,
                sigils: 1,
                supervisions: 1,
                address_spaces: 1,
                mappings: 3,
            }
        );
        circles
            .destroy(second)
            .expect("destroy empty second circle");
        assert_eq!(circles.accounting().circles, 1);
    }

    #[test]
    fn duplicate_destroy_is_stale_and_tombstone_bytes_are_clear() {
        let mut circles = circle_table::new();
        let identity = circles.create().expect("circle");
        let live = circles.create().expect("live circle");
        let invalid = circle_id::try_from_parts(circle_capacity, 1).expect("identity");
        let before = circles.snapshot(live).expect("snapshot");
        assert_eq!(
            circles.destroy(invalid),
            Err(circle_error::invalid_identity)
        );
        assert_eq!(circles.snapshot(live), Ok(before));

        circles.destroy(identity).expect("destroy");
        assert_eq!(circles.destroy(identity), Err(circle_error::stale_identity));

        let record = &circles.records[identity.slot()];
        let record_pointer = ptr::from_ref(record).cast::<u8>();
        let generation_offset = ptr::addr_of!(record.generation) as usize - record_pointer as usize;
        let generation_bytes = identity.generation_value().to_ne_bytes();
        for offset in 0..core::mem::size_of::<circle_record>() {
            let expected = if (generation_offset..generation_offset + generation_bytes.len())
                .contains(&offset)
            {
                generation_bytes[offset - generation_offset]
            } else {
                0
            };
            // SAFETY: Circle teardown initialized the complete record extent.
            assert_eq!(
                unsafe { ptr::read_volatile(record_pointer.add(offset)) },
                expected
            );
        }
    }
}
