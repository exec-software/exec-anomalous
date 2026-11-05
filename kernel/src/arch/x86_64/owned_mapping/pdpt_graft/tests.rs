extern crate std;

use super::*;
use crate::arch::x86_64::mapping_plan::{mapping_budget, mapping_request, reserved_request};
use crate::arch::x86_64::paging::page_permissions;
use crate::boot_omen::memory_region;
use std::{vec, vec::Vec};

const frames: usize = 128;
const other_root: u64 = frames as u64 * page_size;
const original_base: u64 = 0x400000;
const read_only: page_permissions = page_permissions {
    user: true,
    writable: false,
    executable: false,
};

struct model_memory {
    words: Vec<u64>,
    active: u64,
    reads: Vec<u64>,
    writes: usize,
    fail_write_after_store: bool,
    fail_read_after_store: bool,
    fail_next_read: bool,
}

impl model_memory {
    fn new() -> Self {
        Self::with_frame_count(frames)
    }

    fn with_frame_count(frame_count: usize) -> Self {
        Self {
            words: vec![0xa5; frame_count * entries_per_table],
            active: frame_count as u64 * page_size,
            reads: Vec::new(),
            writes: 0,
            fail_write_after_store: false,
            fail_read_after_store: false,
            fail_next_read: false,
        }
    }

    fn offset(&self, table: u64, slot: usize) -> Option<usize> {
        let offset = usize::try_from(table / 8).ok()?.checked_add(slot)?;
        (table.is_multiple_of(page_size) && slot < entries_per_table && offset < self.words.len())
            .then_some(offset)
    }

    fn get(&self, table: u64, slot: usize) -> u64 {
        self.words[self.offset(table, slot).unwrap()]
    }

    fn set(&mut self, table: u64, slot: usize, value: u64) {
        let offset = self.offset(table, slot).unwrap();
        self.words[offset] = value;
    }
}

// SAFETY: Every physical address is a bounded offset into private model RAM.
// Injected store failures deliberately retain their actual mutation. Read
// failures never fabricate evidence, and successful clears verify all words.
unsafe impl mapping_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        let Some(end) = range.end() else {
            return false;
        };
        if !range.base.is_multiple_of(8)
            || !end.is_multiple_of(8)
            || end > self.words.len() as u64 * 8
        {
            return false;
        }
        let bounds = (range.base / 8) as usize..(end / 8) as usize;
        self.words[bounds.clone()].fill(0);
        self.words[bounds].iter().all(|word| *word == 0)
    }

    unsafe fn read_entry(&mut self, table: u64, slot: usize) -> Option<u64> {
        self.reads.push(table);
        if core::mem::take(&mut self.fail_next_read) {
            return None;
        }
        Some(self.words[self.offset(table, slot)?])
    }

    unsafe fn write_entry(&mut self, table: u64, slot: usize, value: u64) -> bool {
        self.writes += 1;
        let Some(offset) = self.offset(table, slot) else {
            return false;
        };
        self.words[offset] = value;
        if core::mem::take(&mut self.fail_read_after_store) {
            self.fail_next_read = true;
        }
        !core::mem::take(&mut self.fail_write_after_store)
    }

    fn active_root(&self) -> u64 {
        self.active
    }
}

fn allocator(ledger: &mut [u8]) -> frame_allocator<'_> {
    let frame_count = ledger.len();
    frame_allocator::initialize(
        ledger,
        &[memory_region {
            base: 0,
            length: frame_count as u64 * page_size,
            kind: 1,
            attributes: 1,
        }],
        &[physical_range::new(0, page_size)],
    )
    .unwrap()
}

fn owner(base: u64, pages: u64, permissions: page_permissions, tail: u64) -> staged_mapping {
    let reservations = [reserved_request {
        virtual_address: base + pages * page_size,
        page_count: tail,
    }];
    staged_mapping::new(
        mapping_plan::new(
            &[mapping_request {
                virtual_address: base,
                page_count: pages,
                permissions,
            }],
            if tail == 0 { &[] } else { &reservations },
            mapping_budget {
                data_pages: pages,
                table_frames: 32,
            },
        )
        .unwrap(),
    )
}

unsafe fn build_pair(
    caller: &mut staged_mapping,
    candidate: &mut staged_mapping,
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
) {
    // SAFETY: Isolated model owners share one exclusive allocator and RAM.
    unsafe { caller.build(allocator, memory) }.unwrap();
    // Model the existing admitted caller's previous hardware use. This test
    // does not activate either root or claim any supervisor residency.
    caller.hardware_used = true;
    // SAFETY: The candidate is separate and never used by model hardware.
    unsafe { candidate.build(allocator, memory) }.unwrap();
}

unsafe fn release_pair(
    caller: &mut staged_mapping,
    candidate: &mut staged_mapping,
    allocator: &mut frame_allocator<'_>,
    memory: &mut model_memory,
) {
    // SAFETY: Tests call this only after no edge was published or after explicit
    // detach and modeled invalidation. Both roots are inactive.
    unsafe { candidate.cleanup(allocator, memory) }.unwrap();
    // SAFETY: The original owner has no remaining external child reference.
    unsafe { caller.cleanup(allocator, memory) }.unwrap();
}

#[test]
fn exact_edge_composes_copy_and_restores_tree_before_cleanup() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let initial = allocator.accounting();
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 2, read_only, 1);
    // SAFETY: This test owns all model state and admits no concurrent consumer.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let before = memory.words.clone();
        let accounting = allocator.accounting();
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + 3 * page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        assert_eq!(graft.phase(), graft_phase::prepared);
        assert_eq!(memory.words, before);
        assert_eq!(
            graft.confirm_invalidation(&caller, &candidate, &mut memory),
            Err(graft_error::invalid_state)
        );
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        assert_eq!(graft.phase(), graft_phase::published);
        assert_eq!(
            memory.get(graft.parent_table_address(), parent_slot),
            graft.installed_entry()
        );
        assert_eq!(
            memory
                .words
                .iter()
                .zip(&before)
                .filter(|(a, b)| a != b)
                .count(),
            1
        );
        assert_eq!(allocator.accounting(), accounting);
        let original = graft
            .preflight_inactive_user_copy(
                &caller,
                &candidate,
                &mut memory,
                other_root,
                copy_direction::from_user,
                original_base,
                8,
            )
            .unwrap();
        assert_eq!(
            original.ranges(),
            &[physical_range::new(caller.data_range(0).unwrap().base, 8)]
        );
        let source = graft
            .preflight_inactive_user_copy(
                &caller,
                &candidate,
                &mut memory,
                other_root,
                copy_direction::from_user,
                window_base + page_size - 4,
                8,
            )
            .unwrap();
        assert_eq!(
            source.ranges(),
            &[
                physical_range::new(candidate.data_range(0).unwrap().base + page_size - 4, 4),
                physical_range::new(candidate.data_range(0).unwrap().base + page_size, 4),
            ]
        );
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::to_user,
                    window_base,
                    8
                )
                .err(),
            Some(user_copy_error::permission_denied)
        );
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base + 2 * page_size - 4,
                    8
                )
                .err(),
            Some(user_copy_error::unmapped)
        );
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base + 2 * page_size,
                    8
                )
                .err(),
            Some(user_copy_error::unmapped)
        );
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        assert_eq!(graft.phase(), graft_phase::detached);
        assert_eq!(memory.words, before);
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base,
                    8
                )
                .err(),
            Some(user_copy_error::owner_unavailable)
        );
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        assert_eq!(graft.phase(), graft_phase::withdrawn);
        assert_eq!(
            graft.publish(&caller, &candidate, &allocator, &mut memory),
            Err(graft_error::invalid_state)
        );
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
    assert_eq!(allocator.accounting(), initial);
}

#[test]
fn composed_copy_can_cross_from_original_owner_to_candidate_owner() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(window_base - page_size, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: Separate retained model owners; caller execution is excluded.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let spans = graft
            .preflight_inactive_user_copy(
                &caller,
                &candidate,
                &mut memory,
                other_root,
                copy_direction::from_user,
                window_base - 4,
                8,
            )
            .unwrap();
        assert_eq!(
            spans.ranges(),
            &[
                physical_range::new(caller.data_range(0).unwrap().base + page_size - 4, 4),
                physical_range::new(candidate.data_range(0).unwrap().base, 4),
            ]
        );
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn complete_extent_and_empty_parent_are_independent_admission_requirements() {
    for case in 0..8 {
        let mut ledger = vec![0; frames];
        let mut allocator = allocator(&mut ledger);
        let mut memory = model_memory::new();
        let mut caller = match case {
            0 => owner(window_base + 2 * page_size, 1, read_only, 0),
            1 => owner(window_base - page_size, 1, read_only, 3),
            2 => owner(window_base + 64 * page_size, 1, read_only, 0),
            _ => owner(original_base, 1, read_only, 0),
        };
        let mut candidate = owner(
            if case == 3 {
                window_base + page_size
            } else {
                window_base
            },
            2,
            page_permissions {
                writable: case == 4,
                executable: case == 5,
                ..read_only
            },
            if case == 6 { 0 } else { 1 },
        );
        let end = if case == 7 {
            window_end + page_size
        } else {
            window_base + 3 * page_size
        };
        // SAFETY: All rejection probes use private inactive model trees.
        unsafe {
            build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
            let before = memory.words.clone();
            let writes = memory.writes;
            assert!(matches!(
                pdpt_graft::prepare(&caller, &candidate, end, &allocator, &mut memory),
                Err(graft_error::invalid_geometry)
            ));
            assert_eq!(memory.words, before);
            assert_eq!(memory.writes, writes);
            release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        }
    }
}

#[test]
fn changed_caller_or_candidate_tree_is_rejected_before_publication() {
    for damage_candidate in [false, true] {
        let mut ledger = vec![0; frames];
        let mut allocator = allocator(&mut ledger);
        let mut memory = model_memory::new();
        let mut caller = owner(original_base, 1, read_only, 0);
        let mut candidate = owner(window_base, 1, read_only, 0);
        // SAFETY: The deliberately corrupt entry remains in isolated model RAM.
        unsafe {
            build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
            let mut graft = pdpt_graft::prepare(
                &caller,
                &candidate,
                window_base + page_size,
                &allocator,
                &mut memory,
            )
            .unwrap();
            let root = if damage_candidate {
                candidate.root_address().unwrap()
            } else {
                caller.root_address().unwrap()
            };
            let expected = memory.get(root, 0);
            memory.set(root, 0, expected ^ writable);
            let before = memory.words.clone();
            let writes = memory.writes;
            assert_eq!(
                graft.publish(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::mapping(mapping_error::entry_mismatch))
            );
            assert_eq!(graft.phase(), graft_phase::prepared);
            assert_eq!(memory.words, before);
            assert_eq!(memory.writes, writes);
            memory.set(root, 0, expected);
            release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        }
    }
}

#[test]
fn publication_store_and_readback_failures_retain_uncertain_reference() {
    for readback_failure in [false, true] {
        let mut ledger = vec![0; frames];
        let mut allocator = allocator(&mut ledger);
        let mut memory = model_memory::new();
        let mut caller = owner(original_base, 1, read_only, 0);
        let mut candidate = owner(window_base, 1, read_only, 0);
        // SAFETY: Both owners remain retained through every injected failure.
        unsafe {
            build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
            let mut graft = pdpt_graft::prepare(
                &caller,
                &candidate,
                window_base + page_size,
                &allocator,
                &mut memory,
            )
            .unwrap();
            let accounting = allocator.accounting();
            memory.fail_read_after_store = readback_failure;
            memory.fail_write_after_store = !readback_failure;
            assert_eq!(
                graft.publish(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::publication_unknown)
            );
            assert_eq!(graft.phase(), graft_phase::uncertain);
            assert_eq!(
                memory.get(graft.parent_table, parent_slot),
                graft.directory_entry
            );
            assert_eq!(allocator.accounting(), accounting);
            assert_eq!(
                graft.publish(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::invalid_state)
            );
            assert_eq!(
                graft.confirm_invalidation(&caller, &candidate, &mut memory),
                Err(graft_error::invalid_state)
            );
            assert_eq!(
                graft
                    .preflight_inactive_user_copy(
                        &caller,
                        &candidate,
                        &mut memory,
                        other_root,
                        copy_direction::from_user,
                        window_base,
                        1
                    )
                    .err(),
                Some(user_copy_error::owner_unavailable)
            );
            graft.detach(&caller, &candidate, &mut memory).unwrap();
            graft
                .confirm_invalidation(&caller, &candidate, &mut memory)
                .unwrap();
            release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        }
    }
}

#[test]
fn withdrawal_failures_retry_the_same_reference_before_invalidation() {
    for failure in 0..3 {
        let mut ledger = vec![0; frames];
        let mut allocator = allocator(&mut ledger);
        let mut memory = model_memory::new();
        let mut caller = owner(original_base, 1, read_only, 0);
        let mut candidate = owner(window_base, 1, read_only, 0);
        // SAFETY: Model consumers are barred and all leases survive failed work.
        unsafe {
            build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
            let mut graft = pdpt_graft::prepare(
                &caller,
                &candidate,
                window_base + page_size,
                &allocator,
                &mut memory,
            )
            .unwrap();
            graft
                .publish(&caller, &candidate, &allocator, &mut memory)
                .unwrap();
            memory.fail_next_read = failure == 0;
            memory.fail_write_after_store = failure == 1;
            memory.fail_read_after_store = failure == 2;
            assert_eq!(
                graft.detach(&caller, &candidate, &mut memory),
                Err(graft_error::withdrawal_unknown)
            );
            assert_eq!(graft.phase(), graft_phase::uncertain);
            assert_eq!(
                graft.confirm_invalidation(&caller, &candidate, &mut memory),
                Err(graft_error::invalid_state)
            );
            graft.detach(&caller, &candidate, &mut memory).unwrap();
            assert_eq!(memory.get(graft.parent_table, parent_slot), 0);
            graft
                .confirm_invalidation(&caller, &candidate, &mut memory)
                .unwrap();
            release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        }
    }
}

#[test]
fn corrupt_external_edge_is_not_followed_or_silently_removed() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: Corrupt addresses cannot leave the private model backend.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let forged = (1 << 40) | present | writable | user_accessible | ownership_marker;
        memory.set(graft.parent_table, parent_slot, forged);
        memory.reads.clear();
        let writes = memory.writes;
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::translation_mismatch)
        );
        assert_eq!(
            graft.detach(&caller, &candidate, &mut memory),
            Err(graft_error::withdrawal_unknown)
        );
        assert_eq!(memory.get(graft.parent_table, parent_slot), forged);
        assert_eq!(memory.writes, writes);
        assert!(
            memory
                .reads
                .iter()
                .all(|table| *table == graft.parent_table)
        );
        memory.set(graft.parent_table, parent_slot, graft.directory_entry);
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn corrupted_candidate_translation_never_selects_an_unowned_table() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: Read traces prove the forged address is not used by the adapter.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let directory = candidate
            .table_address(table_index(&candidate, table_level::directory, 4).unwrap())
            .unwrap();
        let original = memory.get(directory, 0);
        memory.set(
            directory,
            0,
            (1 << 40) | present | writable | user_accessible | ownership_marker,
        );
        memory.reads.clear();
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::translation_mismatch)
        );
        assert!(
            memory
                .reads
                .iter()
                .all(|table| *table < frames as u64 * page_size)
        );
        assert_eq!(memory.reads.last(), Some(&directory));
        memory.set(directory, 0, original);
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn active_roots_and_wrong_owner_are_rejected_without_a_store() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    let mut other_candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: Only the model's reported active root changes, with no hardware.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        other_candidate.build(&mut allocator, &mut memory).unwrap();
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        let writes = memory.writes;
        assert_eq!(
            graft.publish(&caller, &other_candidate, &allocator, &mut memory),
            Err(graft_error::owner_mismatch)
        );
        for current in [
            0,
            caller.root_address().unwrap(),
            candidate.root_address().unwrap(),
        ] {
            memory.active = current;
            assert_eq!(
                graft.publish(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::active_root)
            );
        }
        assert_eq!(memory.writes, writes);
        memory.active = other_root;
        other_candidate
            .cleanup(&mut allocator, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn final_invalidation_read_failure_reopens_uncertainty() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: The modeled invalidation is asserted only after a verified detach.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        memory.fail_next_read = true;
        assert_eq!(
            graft.confirm_invalidation(&caller, &candidate, &mut memory),
            Err(graft_error::withdrawal_unknown)
        );
        assert_eq!(graft.phase(), graft_phase::uncertain);
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn composed_copy_crosses_leaf_tables_and_rejects_corrupt_later_translation() {
    const model_frames: usize = 560;
    let mut ledger = vec![0; model_frames];
    let mut allocator = allocator(&mut ledger);
    let initial = allocator.accounting();
    let mut memory = model_memory::with_frame_count(model_frames);
    let current_root = memory.active;
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 535, read_only, 10);
    // SAFETY: The full 535-page prefix and ten-page absent tail use private
    // model RAM. No hardware or concurrent consumer can reach either owner.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        assert_eq!(candidate.plan().tables().len(), 5);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + 545 * page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let boundary = window_base + 512 * page_size;
        graft
            .admit_hardware_use(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let address = boundary - 4;
        let expected_ranges = [
            physical_range::new(
                candidate.data_range(0).unwrap().base + 512 * page_size - 4,
                4,
            ),
            physical_range::new(candidate.data_range(0).unwrap().base + 512 * page_size, 4),
        ];
        let spans = graft
            .preflight_inactive_user_copy(
                &caller,
                &candidate,
                &mut memory,
                current_root,
                copy_direction::from_user,
                address,
                8,
            )
            .unwrap();
        assert_eq!(spans.ranges(), &expected_ranges);
        let first_leaf = candidate
            .table_address(table_index(&candidate, table_level::leaf, address >> 21).unwrap())
            .unwrap();
        let later_leaf = candidate
            .table_address(table_index(&candidate, table_level::leaf, boundary >> 21).unwrap())
            .unwrap();
        assert_ne!(first_leaf, later_leaf);
        let original = memory.get(later_leaf, 0);
        memory.set(later_leaf, 0, original ^ writable);
        memory.reads.clear();
        let result = graft.preflight_inactive_user_copy(
            &caller,
            &candidate,
            &mut memory,
            current_root,
            copy_direction::from_user,
            address,
            8,
        );
        // The first page was validated, but no copy proof escapes after the
        // second leaf is rejected. Consumers cannot commit a successful prefix.
        assert_eq!(result.err(), Some(user_copy_error::translation_mismatch));
        assert!(memory.reads.contains(&first_leaf));
        assert_eq!(memory.reads.last(), Some(&later_leaf));
        assert_eq!(
            graft.verify_selection(&caller, &candidate, &mut memory),
            Err(graft_error::mapping(mapping_error::entry_mismatch))
        );
        memory.active = caller.root_address().unwrap();
        memory.reads.clear();
        assert_eq!(
            graft
                .preflight_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    copy_direction::from_user,
                    address,
                    8
                )
                .err(),
            Some(user_copy_error::translation_mismatch)
        );
        assert!(memory.reads.contains(&first_leaf));
        assert_eq!(memory.reads.last(), Some(&later_leaf));
        memory.active = current_root;
        memory.set(later_leaf, 0, original);
        let spans = graft
            .preflight_inactive_user_copy(
                &caller,
                &candidate,
                &mut memory,
                current_root,
                copy_direction::from_user,
                address,
                8,
            )
            .unwrap();
        assert_eq!(spans.ranges(), &expected_ranges);
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    current_root,
                    copy_direction::from_user,
                    window_base + 535 * page_size - 4,
                    8,
                )
                .err(),
            Some(user_copy_error::unmapped)
        );
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
    assert_eq!(allocator.accounting(), initial);
}

#[test]
fn admitted_active_copy_preserves_both_owners_and_only_permitted_hardware_bits() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let initial = allocator.accounting();
    let mut memory = model_memory::new();
    let mut caller = owner(
        window_base - page_size,
        1,
        page_permissions {
            writable: true,
            ..read_only
        },
        0,
    );
    let mut candidate = owner(window_base, 2, read_only, 1);
    // SAFETY: Isolated model RAM and retained owners represent one excluded
    // caller. Root changes below model selection without real hardware access.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let before = caller
            .verified_inactive_fingerprint(&allocator, &mut memory)
            .unwrap();
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + 3 * page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let root = caller.root_address().unwrap();
        memory.active = root;
        assert_eq!(
            graft
                .preflight_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::owner_unavailable)
        );
        memory.active = other_root;
        assert_eq!(
            graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
            Ok(root)
        );
        assert!(graft.hardware_admitted());
        assert!(!candidate.hardware_used);
        assert_eq!(
            graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
            Err(graft_error::invalid_state)
        );
        assert_eq!(
            graft.verify_selection(&caller, &candidate, &mut memory),
            Ok(root)
        );
        // Model hardware touching every original writable caller path, the
        // installed external edge, and the reachable candidate subtree only.
        for index in 0..caller.table_count() {
            let table = caller.table_address(index).unwrap();
            for slot in 0..entries_per_table {
                let expected = caller.expected_entry(index, slot).unwrap();
                if expected & present != 0 {
                    memory.set(
                        table,
                        slot,
                        expected | caller.permitted_runtime_bits(index, expected),
                    );
                }
            }
        }
        memory.set(
            graft.parent_table,
            parent_slot,
            graft.directory_entry | accessed,
        );
        for index in 0..candidate.table_count() {
            let table = candidate.table_address(index).unwrap();
            for slot in 0..entries_per_table {
                let expected = candidate.expected_entry(index, slot).unwrap();
                memory.set(
                    table,
                    slot,
                    expected | graft.candidate_runtime_bits(&candidate, index, expected),
                );
            }
        }
        assert_eq!(
            graft.verify_hardware_use(&caller, &candidate, &allocator, &mut memory),
            Ok(root)
        );
        memory.active = root;
        let spans = graft
            .preflight_user_copy(
                &caller,
                &candidate,
                &mut memory,
                copy_direction::from_user,
                window_base - 4,
                8,
            )
            .unwrap();
        assert_eq!(
            spans.ranges(),
            &[
                physical_range::new(caller.data_range(0).unwrap().base + page_size - 4, 4),
                physical_range::new(candidate.data_range(0).unwrap().base, 4),
            ]
        );
        assert_eq!(
            graft
                .preflight_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    copy_direction::to_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::permission_denied)
        );
        assert_eq!(
            graft
                .preflight_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    copy_direction::from_user,
                    window_base + 2 * page_size - 4,
                    8
                )
                .err(),
            Some(user_copy_error::unmapped)
        );
        assert_eq!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    root,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::owner_unavailable)
        );
        assert_eq!(
            graft.verify_selection(&caller, &candidate, &mut memory),
            Err(graft_error::active_root)
        );
        memory.active = other_root;
        assert!(
            graft
                .preflight_inactive_user_copy(
                    &caller,
                    &candidate,
                    &mut memory,
                    other_root,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .is_ok()
        );
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        assert_eq!(
            caller.verified_inactive_fingerprint(&allocator, &mut memory),
            Ok(before)
        );
        let original_leaf = caller
            .table_address(
                table_index(&caller, table_level::leaf, (window_base - page_size) >> 21).unwrap(),
            )
            .unwrap();
        let original_slot = paging::indexes(window_base - page_size).table;
        let actual = memory.get(original_leaf, original_slot);
        memory.set(original_leaf, original_slot, actual ^ writable);
        assert_eq!(
            caller.verified_inactive_fingerprint(&allocator, &mut memory),
            Err(mapping_error::entry_mismatch)
        );
        memory.set(original_leaf, original_slot, actual);
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        assert_eq!(
            graft.verify_selection(&caller, &candidate, &mut memory),
            Err(graft_error::invalid_state)
        );
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
    assert_eq!(allocator.accounting(), initial);
}

#[test]
fn admission_rejects_early_accessed_and_never_admits_dirty_or_private_ancestry_changes() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 1);
    // SAFETY: Every mutation stays in isolated retained model table storage.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + 2 * page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        assert_eq!(
            graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
            Err(graft_error::invalid_state)
        );
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let directory = candidate
            .table_address(table_index(&candidate, table_level::directory, 4).unwrap())
            .unwrap();
        let leaf = candidate
            .table_address(table_index(&candidate, table_level::leaf, window_base >> 21).unwrap())
            .unwrap();
        for (table, slot) in [(graft.parent_table, parent_slot), (directory, 0), (leaf, 0)] {
            let expected = memory.get(table, slot);
            memory.set(table, slot, expected | accessed);
            assert_eq!(
                graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::mapping(mapping_error::entry_mismatch))
            );
            assert!(!graft.hardware_admitted());
            memory.set(table, slot, expected);
        }
        let root = graft
            .admit_hardware_use(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let private_root = candidate.root_address().unwrap();
        let private_parent = candidate
            .table_address(table_index(&candidate, table_level::directory_pointer, 0).unwrap())
            .unwrap();
        const dirty: u64 = 1 << 6;
        for (table, slot, extra) in [
            (graft.parent_table, parent_slot, dirty),
            (directory, 0, dirty),
            (leaf, 0, dirty),
            (private_root, 0, accessed),
            (private_parent, parent_slot, accessed),
            (leaf, 1, accessed),
            (leaf, 0, writable),
            (directory, 1, present),
            (directory, 0, 1 << 40),
        ] {
            let expected = memory.get(table, slot);
            memory.set(table, slot, expected | extra);
            assert_eq!(
                graft.verify_hardware_use(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::mapping(mapping_error::entry_mismatch))
            );
            assert_eq!(
                graft.verify_selection(&caller, &candidate, &mut memory),
                Err(graft_error::mapping(mapping_error::entry_mismatch))
            );
            assert_eq!(graft.phase(), graft_phase::published);
            assert!(candidate.retained_leases() > 0);
            assert!(
                memory
                    .reads
                    .iter()
                    .all(|table| *table < frames as u64 * page_size)
            );
            memory.set(table, slot, expected);
        }
        assert_eq!(
            graft.verify_selection(&caller, &candidate, &mut memory),
            Ok(root)
        );
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn hardware_admission_binds_both_owners_allocator_and_inactive_roots() {
    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let mut other_ledger = vec![0; frames];
    let foreign_allocator = self::allocator(&mut other_ledger);
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let mut candidate = owner(window_base, 1, read_only, 0);
    let mut other_caller = owner(original_base, 1, read_only, 0);
    let mut other_candidate = owner(window_base, 1, read_only, 0);
    // SAFETY: Alternate owners have independently acquired leases in model RAM.
    unsafe {
        build_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
        build_pair(
            &mut other_caller,
            &mut other_candidate,
            &mut allocator,
            &mut memory,
        );
        let mut graft = pdpt_graft::prepare(
            &caller,
            &candidate,
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        graft
            .publish(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        let writes = memory.writes;
        assert!(
            graft
                .admit_hardware_use(&caller, &candidate, &foreign_allocator, &mut memory)
                .is_err()
        );
        assert_eq!(
            graft.admit_hardware_use(&other_caller, &candidate, &allocator, &mut memory),
            Err(graft_error::owner_mismatch)
        );
        assert_eq!(
            graft.admit_hardware_use(&caller, &other_candidate, &allocator, &mut memory),
            Err(graft_error::owner_mismatch)
        );
        for current in [
            0,
            caller.root_address().unwrap(),
            candidate.root_address().unwrap(),
        ] {
            memory.active = current;
            assert_eq!(
                graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
                Err(graft_error::active_root)
            );
        }
        assert!(!graft.hardware_admitted());
        assert_eq!(memory.writes, writes);
        memory.active = other_root;
        // Full admission must reject an unrelated original leaf, even though
        // the requested runtime page's own translation path remains correct.
        let original_leaf = caller
            .table_address(table_index(&caller, table_level::leaf, original_base >> 21).unwrap())
            .unwrap();
        memory.set(original_leaf, 511, (1 << 40) | present);
        assert_eq!(
            graft.admit_hardware_use(&caller, &candidate, &allocator, &mut memory),
            Err(graft_error::mapping(mapping_error::entry_mismatch))
        );
        assert!(!graft.hardware_admitted());
        memory.set(original_leaf, 511, 0);
        let root = graft
            .admit_hardware_use(&caller, &candidate, &allocator, &mut memory)
            .unwrap();
        for current in [0, other_root, candidate.root_address().unwrap()] {
            memory.active = current;
            assert_eq!(
                graft
                    .preflight_user_copy(
                        &caller,
                        &candidate,
                        &mut memory,
                        copy_direction::from_user,
                        window_base,
                        1
                    )
                    .err(),
                Some(user_copy_error::owner_unavailable)
            );
        }
        memory.active = root;
        assert_eq!(
            graft
                .preflight_user_copy(
                    &other_caller,
                    &candidate,
                    &mut memory,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::owner_unavailable)
        );
        assert_eq!(
            graft
                .preflight_user_copy(
                    &caller,
                    &other_candidate,
                    &mut memory,
                    copy_direction::from_user,
                    window_base,
                    1
                )
                .err(),
            Some(user_copy_error::owner_unavailable)
        );
        memory.active = other_root;
        graft.detach(&caller, &candidate, &mut memory).unwrap();
        graft
            .confirm_invalidation(&caller, &candidate, &mut memory)
            .unwrap();
        release_pair(
            &mut other_caller,
            &mut other_candidate,
            &mut allocator,
            &mut memory,
        );
        release_pair(&mut caller, &mut candidate, &mut allocator, &mut memory);
    }
}

#[test]
fn admitted_runtime_owner_cannot_release_before_quiescence_detach_and_invalidation() {
    use crate::arch::x86_64::runtime_mapping::{
        publication_result, runtime_budget, runtime_error, runtime_mapping, runtime_phase,
        runtime_plan, runtime_request,
    };

    let mut ledger = vec![0; frames];
    let mut allocator = allocator(&mut ledger);
    let initial = allocator.accounting();
    let mut memory = model_memory::new();
    let mut caller = owner(original_base, 1, read_only, 0);
    let plan = runtime_plan::new(
        caller.plan(),
        runtime_request {
            virtual_address: window_base,
            byte_length: page_size,
            populated_byte_length: page_size,
            permissions: read_only,
        },
        runtime_budget {
            virtual_pages: 1,
            data_pages: 1,
            table_frames: 4,
        },
    )
    .unwrap();
    let mut runtime = runtime_mapping::new(plan, 42u64);
    // SAFETY: All data is zero-initialized isolated model RAM. This tests the
    // runtime's ownership fence with a backing witness, not a real source reader.
    unsafe {
        caller.build(&mut allocator, &mut memory).unwrap();
        caller.hardware_used = true;
        runtime
            .prepare(&mut allocator, &mut memory, |mapping, _, memory| {
                let range = mapping.data_range(0).unwrap();
                memory.words[(range.base / 8) as usize..(range.end().unwrap() / 8) as usize]
                    .iter()
                    .all(|word| *word == 0)
            })
            .unwrap();
        let mut graft = pdpt_graft::prepare(
            &caller,
            runtime.mapping(),
            window_base + page_size,
            &allocator,
            &mut memory,
        )
        .unwrap();
        runtime
            .publish(|mapping, _, _| {
                graft
                    .publish(&caller, mapping, &allocator, &mut memory)
                    .unwrap();
                publication_result::published
            })
            .unwrap();
        let root = graft
            .admit_hardware_use(&caller, runtime.mapping(), &allocator, &mut memory)
            .unwrap();
        let owned = allocator.accounting();
        assert_eq!(
            runtime.cleanup(&mut allocator, &mut memory),
            Err(runtime_error::invalid_phase)
        );
        runtime = match runtime.into_backing() {
            Err(retained) => retained,
            Ok(_) => panic!("published source escaped"),
        };
        assert_eq!(runtime.phase(), runtime_phase::published);
        memory.active = root;
        memory.set(
            graft.parent_table,
            parent_slot,
            graft.directory_entry | accessed,
        );
        assert_eq!(
            graft.detach(&caller, runtime.mapping(), &mut memory),
            Err(graft_error::active_root)
        );
        assert_eq!(graft.phase(), graft_phase::uncertain);
        assert_eq!(
            runtime.cleanup(&mut allocator, &mut memory),
            Err(runtime_error::invalid_phase)
        );
        assert_eq!(allocator.accounting(), owned);
        memory.active = other_root;
        memory.fail_write_after_store = true;
        assert_eq!(
            graft.detach(&caller, runtime.mapping(), &mut memory),
            Err(graft_error::withdrawal_unknown)
        );
        assert_eq!(memory.get(graft.parent_table, parent_slot), 0);
        assert_eq!(
            graft.confirm_invalidation(&caller, runtime.mapping(), &mut memory),
            Err(graft_error::invalid_state)
        );
        assert_eq!(
            runtime.cleanup(&mut allocator, &mut memory),
            Err(runtime_error::invalid_phase)
        );
        graft
            .detach(&caller, runtime.mapping(), &mut memory)
            .unwrap();
        caller.verify_inactive(&allocator, &mut memory).unwrap();
        assert_eq!(
            runtime.cleanup(&mut allocator, &mut memory),
            Err(runtime_error::invalid_phase)
        );
        memory.fail_next_read = true;
        assert_eq!(
            graft.confirm_invalidation(&caller, runtime.mapping(), &mut memory),
            Err(graft_error::withdrawal_unknown)
        );
        assert_eq!(
            runtime.cleanup(&mut allocator, &mut memory),
            Err(runtime_error::invalid_phase)
        );
        assert_eq!(allocator.accounting(), owned);
        graft
            .detach(&caller, runtime.mapping(), &mut memory)
            .unwrap();
        // This explicit model step represents invalidation while no saved
        // continuation can resume. The unsafe runtime gate cannot infer it.
        graft
            .confirm_invalidation(&caller, runtime.mapping(), &mut memory)
            .unwrap();
        runtime.confirm_withdrawal().unwrap();
        runtime.cleanup(&mut allocator, &mut memory).unwrap();
        assert_eq!(runtime.into_backing().ok(), Some(42));
        caller.cleanup(&mut allocator, &mut memory).unwrap();
    }
    assert_eq!(allocator.accounting(), initial);
}
