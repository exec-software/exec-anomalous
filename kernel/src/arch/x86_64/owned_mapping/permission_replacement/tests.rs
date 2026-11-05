extern crate std;

use super::*;
use crate::arch::x86_64::mapping_plan::{mapping_budget, mapping_request, reserved_request};
use crate::arch::x86_64::paging::{no_execute, page_permissions};
use crate::boot_omen::memory_region;
use crate::identity::{circle_id, task_id};
use anomalous_veil_protocol::source_mapping::source_mapping_request;
use anomalous_veil_protocol::source_mapping_lifetime::source_mapping_handle;
use anomalous_veil_protocol::source_read::{source_binding, source_grant};
use core::num::NonZeroU64;
use std::{vec, vec::Vec};

const frame_count: usize = 1024;
const independent_root: u64 = frame_count as u64 * page_size;
const source_length: u64 = 2_190_608;
const read_only: page_permissions = page_permissions {
    user: true,
    writable: false,
    executable: false,
};

struct model_memory(Vec<u64>);

// SAFETY: Only allocation/construction/cleanup use this bounded private vector.
// The replacement model never receives this physical-memory implementation.
unsafe impl mapping_memory for model_memory {
    unsafe fn clear_and_verify(&mut self, range: physical_range) -> bool {
        let Some(end) = range.end() else {
            return false;
        };
        if !range.base.is_multiple_of(8) || !end.is_multiple_of(8) || end > self.0.len() as u64 * 8
        {
            return false;
        }
        self.0[(range.base / 8) as usize..(end / 8) as usize].fill(0);
        true
    }

    unsafe fn read_entry(&mut self, table: u64, index: usize) -> Option<u64> {
        if !table.is_multiple_of(page_size) || index >= entries_per_table {
            return None;
        }
        self.0.get((table / 8) as usize + index).copied()
    }

    unsafe fn write_entry(&mut self, table: u64, index: usize, value: u64) -> bool {
        if !table.is_multiple_of(page_size) || index >= entries_per_table {
            return false;
        }
        let Some(word) = self.0.get_mut((table / 8) as usize + index) else {
            return false;
        };
        *word = value;
        true
    }

    fn active_root(&self) -> u64 {
        independent_root
    }
}

#[derive(Clone)]
struct snapshots {
    caller: Vec<u64>,
    candidate: Vec<u64>,
}

impl snapshots {
    fn observation(&self) -> tree_observation<'_> {
        tree_observation {
            caller: &self.caller,
            candidate: &self.candidate,
        }
    }

    fn apply(&mut self, store: model_store) {
        let words = match store.tree {
            model_tree::caller => &mut self.caller,
            model_tree::candidate => &mut self.candidate,
        };
        assert_eq!(words[store.word], store.before);
        words[store.word] = store.after;
    }
}

fn identity(slot: usize) -> task_identity {
    task_identity::new(
        task_id::try_from_parts(slot, 7).unwrap(),
        circle_id::try_from_parts(slot, 9).unwrap(),
    )
}

fn retained() -> source_mapping_retained_context {
    let binding = source_binding::new(source_grant, source_length, 0x1234).unwrap();
    let creation = source_mapping_request::decode(
        11,
        binding,
        [11, source_grant, window_base, 2_231_696, source_length, 1],
    )
    .unwrap();
    source_mapping_retained_context::new(
        creation,
        binding,
        source_mapping_handle::new(1, 3).unwrap(),
    )
    .unwrap()
}

fn request(
    context: source_mapping_retained_context,
    first: u64,
    pages: u64,
) -> source_mapping_execute_request {
    source_mapping_execute_request::decode(
        12,
        context.binding(),
        context,
        [
            12,
            1,
            3,
            source_grant,
            source_length,
            0x1234,
            window_base + first * page_size,
            pages * page_size,
            first * page_size,
            5,
            0,
            0,
        ],
    )
    .unwrap()
}

fn owner(base: u64, pages: u64, tail: u64) -> staged_mapping {
    let reservations = [reserved_request {
        virtual_address: base + pages * page_size,
        page_count: tail,
    }];
    staged_mapping::new(
        mapping_plan::new(
            &[mapping_request {
                virtual_address: base,
                page_count: pages,
                permissions: read_only,
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

fn observe(owner: &staged_mapping, memory: &model_memory) -> Vec<u64> {
    let mut words = Vec::new();
    for table in 0..owner.table_count() {
        let start = (owner.table_address(table).unwrap() / 8) as usize;
        words.extend_from_slice(&memory.0[start..start + entries_per_table]);
    }
    words
}

fn exercise(run: impl FnOnce(&mut replacement_model<'_>, &mut snapshots)) {
    let mut ledger = vec![0; frame_count];
    let mut allocator = frame_allocator::initialize(
        &mut ledger,
        &[memory_region {
            base: 0,
            length: frame_count as u64 * page_size,
            kind: 1,
            attributes: 1,
        }],
        &[physical_range::new(0, page_size)],
    )
    .unwrap();
    let initial = allocator.accounting();
    let mut memory = model_memory(vec![0xa5; frame_count * entries_per_table]);
    let mut caller = owner(0x400000, 1, 0);
    let mut candidate = owner(window_base, 535, 10);
    // SAFETY: Exclusive construction in private model RAM, no selected owner.
    unsafe {
        caller.build(&mut allocator, &mut memory).unwrap();
        candidate.build(&mut allocator, &mut memory).unwrap();
    }
    caller.hardware_used = true;
    let binding = model_binding {
        caller: identity(1),
        veil: identity(2),
        source_origin: source_origin::provisioned(NonZeroU64::new(source_grant).unwrap()),
        retained: retained(),
        request: request(retained(), 40, 408),
        caller_root: caller.root_address().unwrap(),
        candidate_root: candidate.root_address().unwrap(),
        graft_hardware_admitted: true,
        correlation: [2, 3, 4, 5],
        mutation_epoch: 1,
    };
    let mut trees = snapshots {
        caller: observe(&caller, &memory),
        candidate: observe(&candidate, &memory),
    };
    let parent = table(&caller, table_level::directory_pointer, 0).unwrap() * entries_per_table
        + parent_slot;
    let directory = table(&candidate, table_level::directory, 4).unwrap();
    trees.caller[parent] = candidate.table_address(directory).unwrap()
        | present
        | writable
        | user_accessible
        | ownership_marker
        | accessed;
    // Existing hardware A bits are admitted only on graft-visible candidate
    // directories/present leaves; its unused private PML4/PDPT remain exact.
    for (index, node) in candidate.plan.tables().iter().enumerate() {
        if matches!(node.level(), table_level::directory | table_level::leaf) {
            for word in
                &mut trees.candidate[index * entries_per_table..(index + 1) * entries_per_table]
            {
                if *word & present != 0 {
                    *word |= accessed;
                }
            }
        }
    }
    let physical_before = memory.0.clone();
    let accounting = allocator.accounting();
    {
        let mut model = replacement_model::new(
            &caller,
            &candidate,
            binding,
            independent_root,
            trees.observation(),
        )
        .unwrap();
        run(&mut model, &mut trees);
    }
    assert_eq!(
        memory.0, physical_before,
        "the model must never write physical RAM"
    );
    assert_eq!(
        allocator.accounting(),
        accounting,
        "the model must never release or allocate a lease"
    );
    assert_eq!(candidate.plan.regions().count(), 1);
    assert_eq!(
        candidate.plan.regions().next().unwrap().permissions(),
        read_only
    );
    assert_eq!(
        candidate
            .plan
            .regions()
            .next()
            .unwrap()
            .extent()
            .page_count(),
        535
    );
    assert_eq!(
        candidate.plan.reservations().next().unwrap().page_count(),
        10
    );
    // SAFETY: Only copied observations were changed; actual private model RAM
    // never acquired an external edge or execution. Original owners stay intact.
    unsafe {
        candidate.cleanup(&mut allocator, &mut memory).unwrap();
        caller.cleanup(&mut allocator, &mut memory).unwrap();
    }
    assert_eq!(allocator.accounting(), initial);
}

fn detach(model: &mut replacement_model<'_>, trees: &mut snapshots) {
    let binding = model.binding;
    model
        .revoke_selection(binding, independent_root, trees.observation(), true)
        .unwrap();
    let store = model
        .begin_detach(binding, independent_root, trees.observation())
        .unwrap();
    trees.apply(store);
    model
        .finish_detach(binding, independent_root, trees.observation(), true)
        .unwrap();
    model
        .confirm_detach_invalidation(binding, independent_root, trees.observation(), true)
        .unwrap();
}

fn one_page(model: &mut replacement_model<'_>, trees: &mut snapshots) {
    let binding = model.binding;
    let store = model
        .begin_leaf_store(binding, independent_root, trees.observation())
        .unwrap();
    assert_eq!(store.before ^ store.after, no_execute);
    trees.apply(store);
    model
        .finish_leaf_store(binding, independent_root, trees.observation(), true)
        .unwrap();
}

fn replace(model: &mut replacement_model<'_>, trees: &mut snapshots) {
    detach(model, trees);
    for _ in 0..408 {
        one_page(model, trees);
    }
    assert_eq!(model.phase, model_phase::changed);
    model
        .verify_replacement(model.binding, independent_root, trees.observation())
        .unwrap();
}

fn publish(model: &mut replacement_model<'_>, trees: &mut snapshots) {
    let binding = model.binding;
    let store = model
        .begin_publish(binding, independent_root, trees.observation())
        .unwrap();
    trees.apply(store);
    model
        .finish_publish(binding, independent_root, trees.observation(), true)
        .unwrap();
}

fn terminal_facts() -> terminal_observation {
    terminal_observation {
        selection_revoked: true,
        all_consumers_quiescent: true,
        edge_detached: true,
        translations_invalidated: true,
    }
}

fn terminal(model: &mut replacement_model<'_>, trees: &mut snapshots) {
    model
        .begin_terminal(model.binding, independent_root, true)
        .unwrap();
    trees.caller[model.parent_word] = 0;
    model
        .confirm_terminal(
            model.binding,
            independent_root,
            trees.observation(),
            terminal_facts(),
        )
        .unwrap();
    assert!(model.model_cleanup_permitted());
}

/// Fork an inert model observation at a reachable transition for exhaustive
/// failure injection. This is test-only state exploration, not ticket cloning.
fn fork<'a>(model: &replacement_model<'a>) -> replacement_model<'a> {
    replacement_model {
        caller: model.caller,
        candidate: model.candidate,
        binding: model.binding,
        parent_word: model.parent_word,
        installed_entry: model.installed_entry,
        first_leaf_word: model.first_leaf_word,
        phase: model.phase,
        confirmed_pages: model.confirmed_pages,
        pending: model.pending,
    }
}

#[test]
fn exact_408_page_overlay_preserves_ancestry_neighbors_source_and_original_plan() {
    exercise(|model, trees| {
        let original = trees.clone();
        replace(model, trees);
        assert_eq!(model.confirmed_pages, 408);
        assert_eq!(model.binding.request.first_page(), 40);
        assert_eq!(model.binding.request.source_offset(), 0x28000);
        for (word, (old, actual)) in original.candidate.iter().zip(&trees.candidate).enumerate() {
            let changed = (model.first_leaf_word..model.first_leaf_word + 408).contains(&word);
            assert_eq!(*old ^ *actual, if changed { no_execute } else { 0 });
            if changed {
                assert_eq!(*actual & writable, 0);
            }
        }
        for (word, actual) in trees.caller.iter().enumerate() {
            assert_eq!(
                *actual,
                if word == model.parent_word {
                    0
                } else {
                    original.caller[word]
                }
            );
        }
        publish(model, trees);
        assert_eq!(model.phase, model_phase::published);
        assert_eq!(
            model.complete_model(
                model.binding,
                independent_root,
                trees.observation(),
                model.binding.request.base()
            ),
            Err(model_error::phase)
        );
        model
            .confirm_publish_invalidation(
                model.binding,
                independent_root,
                trees.observation(),
                true,
            )
            .unwrap();
        assert_eq!(
            model.complete_model(
                model.binding,
                independent_root,
                trees.observation(),
                window_base
            ),
            Err(model_error::result)
        );
        assert_eq!(model.phase, model_phase::ready);
        model
            .complete_model(
                model.binding,
                independent_root,
                trees.observation(),
                model.binding.request.base(),
            )
            .unwrap();
        assert_eq!(
            model.complete_model(
                model.binding,
                independent_root,
                trees.observation(),
                model.binding.request.base()
            ),
            Err(model_error::phase)
        );
        assert!(!model.model_cleanup_permitted());
        terminal(model, trees);
    });
}

#[test]
fn every_correlation_field_is_bound_independently_of_the_source_digest() {
    exercise(|model, trees| {
        let original = model.binding;
        let mut variants = Vec::new();
        let mut changed = original;
        changed.caller = identity(3);
        variants.push(changed);
        changed = original;
        changed.veil = identity(3);
        variants.push(changed);
        changed = original;
        changed.source_origin = source_origin::task(identity(3));
        variants.push(changed);
        changed = original;
        changed.graft_hardware_admitted = false;
        variants.push(changed);
        changed = original;
        changed.caller_root += page_size;
        variants.push(changed);
        changed = original;
        changed.candidate_root += page_size;
        variants.push(changed);
        changed = original;
        changed.mutation_epoch += 1;
        variants.push(changed);
        for index in 0..4 {
            changed = original;
            changed.correlation[index] ^= 1;
            variants.push(changed);
        }
        let retained = original.retained;
        changed = original;
        changed.retained = source_mapping_retained_context::new(
            retained.creation(),
            source_binding::new(source_grant, source_length, 0x5678).unwrap(),
            retained.handle(),
        )
        .unwrap();
        variants.push(changed);
        let longer_binding =
            source_binding::new(source_grant, source_length + page_size, 0x1234).unwrap();
        let longer_creation = source_mapping_request::decode(
            11,
            longer_binding,
            [
                11,
                source_grant,
                window_base,
                2_231_696,
                source_length + page_size,
                1,
            ],
        )
        .unwrap();
        changed = original;
        changed.retained = source_mapping_retained_context::new(
            longer_creation,
            longer_binding,
            retained.handle(),
        )
        .unwrap();
        variants.push(changed);
        let earlier_creation = source_mapping_request::decode(
            10,
            retained.binding(),
            [10, source_grant, window_base, 2_231_696, source_length, 1],
        )
        .unwrap();
        changed = original;
        changed.retained = source_mapping_retained_context::new(
            earlier_creation,
            retained.binding(),
            retained.handle(),
        )
        .unwrap();
        variants.push(changed);
        changed = original;
        changed.retained = source_mapping_retained_context::new(
            retained.creation(),
            retained.binding(),
            source_mapping_handle::new(2, 3).unwrap(),
        )
        .unwrap();
        variants.push(changed);
        changed = original;
        changed.retained = source_mapping_retained_context::new(
            retained.creation(),
            retained.binding(),
            source_mapping_handle::new(1, 4).unwrap(),
        )
        .unwrap();
        variants.push(changed);
        let mut words = original.request.words();
        words[0] = 13;
        changed = original;
        changed.request =
            source_mapping_execute_request::decode(13, retained.binding(), retained, words)
                .unwrap();
        variants.push(changed);
        for altered in variants {
            assert_eq!(
                model.revoke_selection(altered, independent_root, trees.observation(), true),
                Err(model_error::binding)
            );
            assert_eq!(model.phase, model_phase::prepared);
        }
        detach(model, trees);
        changed = original;
        changed.mutation_epoch += 1;
        assert_eq!(
            model.begin_leaf_store(changed, independent_root, trees.observation()),
            Err(model_error::binding)
        );
        terminal(model, trees);
    });
}

#[test]
fn active_roots_and_missing_revocation_refuse_before_any_store_description() {
    exercise(|model, trees| {
        let binding = model.binding;
        let active = [0, binding.caller_root, binding.candidate_root];
        for root in active {
            assert_eq!(
                model.revoke_selection(binding, root, trees.observation(), true),
                Err(model_error::active_root)
            );
        }
        assert_eq!(
            model.revoke_selection(binding, independent_root, trees.observation(), false),
            Err(model_error::observation)
        );
        assert_eq!(
            model.begin_detach(binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        model
            .revoke_selection(binding, independent_root, trees.observation(), true)
            .unwrap();
        for root in active {
            assert_eq!(
                model.begin_detach(binding, root, trees.observation()),
                Err(model_error::active_root)
            );
        }
        let store = model
            .begin_detach(binding, independent_root, trees.observation())
            .unwrap();
        assert_eq!(
            model.begin_leaf_store(binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        trees.apply(store);
        model
            .finish_detach(binding, independent_root, trees.observation(), true)
            .unwrap();
        assert_eq!(
            model.begin_leaf_store(binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        model
            .confirm_detach_invalidation(binding, independent_root, trees.observation(), true)
            .unwrap();
        for root in active {
            assert_eq!(
                model.begin_leaf_store(binding, root, trees.observation()),
                Err(model_error::active_root)
            );
        }
        for _ in 0..408 {
            one_page(model, trees);
        }
        assert_eq!(
            model.begin_leaf_store(binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        assert_eq!(
            model.begin_publish(binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        model
            .verify_replacement(binding, independent_root, trees.observation())
            .unwrap();
        for root in active {
            assert_eq!(
                model.begin_publish(binding, root, trees.observation()),
                Err(model_error::active_root)
            );
        }
        terminal(model, trees);
    });
}

#[test]
fn every_leaf_store_failure_retains_the_exact_prefix_and_uncertain_attempt() {
    exercise(|model, trees| {
        detach(model, trees);
        for page in 0..408 {
            for mode in 0..3 {
                let mut branch = fork(model);
                let mut observed = trees.clone();
                let store = branch
                    .begin_leaf_store(branch.binding, independent_root, observed.observation())
                    .unwrap();
                if mode != 0 {
                    observed.apply(store);
                }
                let success = mode == 2;
                if mode == 2 {
                    // A successful store followed by missing readback remains unknown.
                    observed.candidate.pop();
                }
                assert_eq!(
                    branch.finish_leaf_store(
                        branch.binding,
                        independent_root,
                        observed.observation(),
                        success
                    ),
                    Err(model_error::uncertainty)
                );
                assert_eq!(branch.phase, model_phase::quarantined);
                assert_eq!(branch.confirmed_pages, page);
                assert_eq!(branch.pending, Some(store));
                assert!(!branch.model_cleanup_permitted());
                assert_eq!(
                    branch.begin_leaf_store(
                        branch.binding,
                        independent_root,
                        observed.observation()
                    ),
                    Err(model_error::phase)
                );
                assert_eq!(
                    branch.begin_publish(branch.binding, independent_root, observed.observation()),
                    Err(model_error::phase)
                );
                if mode == 2 {
                    observed.candidate.push(*trees.candidate.last().unwrap());
                }
                terminal(&mut branch, &mut observed);
            }
            one_page(model, trees);
        }
        terminal(model, trees);
    });
}

#[test]
fn every_old_composition_word_is_checked_before_detachment() {
    exercise(|model, trees| {
        for candidate in [false, true] {
            let count = if candidate {
                trees.candidate.len()
            } else {
                trees.caller.len()
            };
            for word in 0..count {
                let mut branch = fork(model);
                let mut observed = trees.clone();
                let words = if candidate {
                    &mut observed.candidate
                } else {
                    &mut observed.caller
                };
                words[word] ^= present;
                assert_eq!(
                    branch.revoke_selection(
                        branch.binding,
                        independent_root,
                        observed.observation(),
                        true
                    ),
                    Err(model_error::observation)
                );
                assert_eq!(branch.phase, model_phase::prepared);
            }
        }
        terminal(model, trees);
    });
}

#[test]
fn altered_neighbors_private_ancestry_and_read_only_dirty_bits_never_become_expected() {
    exercise(|model, trees| {
        replace(model, trees);
        let first = model.first_leaf_word;
        let candidate_points = [
            0,
            entries_per_table,
            first - 1,
            first,
            first + 407,
            first + 408,
            trees.candidate.len() - 1,
        ];
        for word in candidate_points {
            for bit in [
                present,
                writable,
                user_accessible,
                1 << 6,
                page_size,
                no_execute,
            ] {
                let mut branch = fork(model);
                let mut observed = trees.clone();
                observed.candidate[word] ^= bit;
                assert_eq!(
                    branch.begin_publish(branch.binding, independent_root, observed.observation()),
                    Err(model_error::uncertainty)
                );
                assert!(!branch.model_cleanup_permitted());
                branch
                    .begin_terminal(branch.binding, independent_root, true)
                    .unwrap();
                assert_eq!(
                    branch.confirm_terminal(
                        branch.binding,
                        independent_root,
                        observed.observation(),
                        terminal_facts()
                    ),
                    Err(model_error::observation)
                );
            }
        }
        // Private PML4/PDPT Accessed is not masked; candidate directory and
        // present leaf A is allowed, but absent slots never gain even A.
        for (index, node) in model.candidate.plan.tables().iter().enumerate() {
            let word = index * entries_per_table;
            let mut observed = trees.clone();
            let existing = observed.candidate[word];
            observed.candidate[word] |= accessed;
            let permitted = matches!(node.level(), table_level::directory | table_level::leaf)
                && existing & present != 0;
            assert_eq!(
                model
                    .verify(observed.observation(), false, 408, false)
                    .is_ok(),
                permitted
            );
        }
        terminal(model, trees);
    });
}

#[test]
fn detach_publication_and_both_invalidation_failures_quarantine_without_retry() {
    exercise(|model, trees| {
        for after_store in [false, true] {
            let mut branch = fork(model);
            let mut observed = trees.clone();
            branch
                .revoke_selection(
                    branch.binding,
                    independent_root,
                    observed.observation(),
                    true,
                )
                .unwrap();
            let store = branch
                .begin_detach(branch.binding, independent_root, observed.observation())
                .unwrap();
            if after_store {
                observed.apply(store);
            }
            assert_eq!(
                branch.finish_detach(
                    branch.binding,
                    independent_root,
                    observed.observation(),
                    false
                ),
                Err(model_error::uncertainty)
            );
            assert_eq!(
                branch.begin_detach(branch.binding, independent_root, observed.observation()),
                Err(model_error::phase)
            );
            terminal(&mut branch, &mut observed);
        }
        let mut branch = fork(model);
        let mut observed = trees.clone();
        branch
            .revoke_selection(
                branch.binding,
                independent_root,
                observed.observation(),
                true,
            )
            .unwrap();
        let store = branch
            .begin_detach(branch.binding, independent_root, observed.observation())
            .unwrap();
        observed.apply(store);
        branch
            .finish_detach(
                branch.binding,
                independent_root,
                observed.observation(),
                true,
            )
            .unwrap();
        assert_eq!(
            branch.confirm_detach_invalidation(
                branch.binding,
                independent_root,
                observed.observation(),
                false
            ),
            Err(model_error::uncertainty)
        );
        terminal(&mut branch, &mut observed);
        replace(model, trees);
        for after_store in [false, true] {
            let mut branch = fork(model);
            let mut observed = trees.clone();
            let store = branch
                .begin_publish(branch.binding, independent_root, observed.observation())
                .unwrap();
            if after_store {
                observed.apply(store);
            }
            assert_eq!(
                branch.finish_publish(
                    branch.binding,
                    independent_root,
                    observed.observation(),
                    false
                ),
                Err(model_error::uncertainty)
            );
            assert_eq!(
                branch.begin_publish(branch.binding, independent_root, observed.observation()),
                Err(model_error::phase)
            );
            terminal(&mut branch, &mut observed);
        }
        publish(model, trees);
        assert_eq!(
            model.confirm_publish_invalidation(
                model.binding,
                independent_root,
                trees.observation(),
                false
            ),
            Err(model_error::uncertainty)
        );
        assert_eq!(
            model.complete_model(
                model.binding,
                independent_root,
                trees.observation(),
                model.binding.request.base()
            ),
            Err(model_error::phase)
        );
        terminal(model, trees);
    });
}

#[test]
fn every_terminal_fact_and_complete_mixed_tree_are_required_after_partial_store() {
    exercise(|model, trees| {
        detach(model, trees);
        for _ in 0..123 {
            one_page(model, trees);
        }
        let store = model
            .begin_leaf_store(model.binding, independent_root, trees.observation())
            .unwrap();
        trees.apply(store);
        assert_eq!(
            model.finish_leaf_store(model.binding, independent_root, trees.observation(), false),
            Err(model_error::uncertainty)
        );
        assert_eq!(
            model.begin_terminal(model.binding, independent_root, false),
            Err(model_error::observation)
        );
        model
            .begin_terminal(model.binding, independent_root, true)
            .unwrap();
        for index in 0..4 {
            let mut facts = terminal_facts();
            match index {
                0 => facts.selection_revoked = false,
                1 => facts.all_consumers_quiescent = false,
                2 => facts.edge_detached = false,
                _ => facts.translations_invalidated = false,
            }
            assert_eq!(
                model.confirm_terminal(model.binding, independent_root, trees.observation(), facts),
                Err(model_error::observation)
            );
            assert!(!model.model_cleanup_permitted());
        }
        for root in [0, model.binding.caller_root, model.binding.candidate_root] {
            assert_eq!(
                model.confirm_terminal(model.binding, root, trees.observation(), terminal_facts()),
                Err(model_error::active_root)
            );
        }
        let mut corrupt = trees.clone();
        corrupt.candidate[store.word] |= writable;
        assert_eq!(
            model.confirm_terminal(
                model.binding,
                independent_root,
                corrupt.observation(),
                terminal_facts()
            ),
            Err(model_error::observation)
        );
        corrupt = trees.clone();
        corrupt.caller[model.parent_word] = model.installed_entry;
        assert_eq!(
            model.confirm_terminal(
                model.binding,
                independent_root,
                corrupt.observation(),
                terminal_facts()
            ),
            Err(model_error::observation)
        );
        model
            .confirm_terminal(
                model.binding,
                independent_root,
                trees.observation(),
                terminal_facts(),
            )
            .unwrap();
        assert!(model.model_cleanup_permitted());
        assert_eq!(
            model.begin_leaf_store(model.binding, independent_root, trees.observation()),
            Err(model_error::phase)
        );
        assert_eq!(
            model.confirm_terminal(
                model.binding,
                independent_root,
                trees.observation(),
                terminal_facts()
            ),
            Err(model_error::phase)
        );
    });
}

#[test]
fn model_budget_and_original_geometry_are_checked_in_addition_to_protocol_decoding() {
    exercise(|model, trees| {
        let mut binding = model.binding;
        binding.request = request(binding.retained, 500, 30);
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::geometry)
        ));
        binding = model.binding;
        binding.mutation_epoch = 0;
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        binding = model.binding;
        binding.correlation = [0; 4];
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        binding = model.binding;
        binding.candidate_root = binding.caller_root;
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        binding = model.binding;
        binding.graft_hardware_admitted = false;
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        binding = model.binding;
        binding.source_origin =
            source_origin::provisioned(NonZeroU64::new(source_grant + 1).unwrap());
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        binding = model.binding;
        binding.source_origin = source_origin::task(identity(3));
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::binding)
        ));
        let retained = model.binding.retained;
        let creation = source_mapping_request::decode(
            11,
            retained.binding(),
            [
                11,
                source_grant,
                window_base,
                546 * page_size,
                source_length,
                1,
            ],
        )
        .unwrap();
        binding = model.binding;
        binding.retained =
            source_mapping_retained_context::new(creation, retained.binding(), retained.handle())
                .unwrap();
        assert!(matches!(
            replacement_model::new(
                model.caller,
                model.candidate,
                binding,
                independent_root,
                trees.observation()
            ),
            Err(model_error::geometry)
        ));
        terminal(model, trees);
    });
}

#[test]
fn distinct_allocator_lineages_and_different_roots_cannot_hide_overlapping_owned_frames() {
    exercise(|model, trees| {
        // Both owners have genuine allocator leases, but the independent ledger
        // starts its allocation inside the caller's retained table range. Their
        // different roots cannot establish physical ownership disjointness.
        let mut foreign_ledger = vec![0; frame_count];
        let mut foreign_allocator = frame_allocator::initialize(
            &mut foreign_ledger,
            &[memory_region {
                base: 0,
                length: frame_count as u64 * page_size,
                kind: 1,
                attributes: 1,
            }],
            &[physical_range::new(0, 2 * page_size)],
        )
        .unwrap();
        let initial = foreign_allocator.accounting();
        let mut foreign_memory = model_memory(vec![0; frame_count * entries_per_table]);
        let mut candidate = owner(window_base, 535, 10);
        // SAFETY: This independently owned vector and ledger model another
        // allocator lineage; they do not touch the first fixture's actual RAM.
        unsafe { candidate.build(&mut foreign_allocator, &mut foreign_memory) }.unwrap();
        let mut binding = model.binding;
        binding.candidate_root = candidate.root_address().unwrap();
        assert_ne!(binding.caller_root, binding.candidate_root);
        let original_tables = model.caller.table_range().unwrap();
        let foreign_tables = candidate.table_range().unwrap();
        assert!(
            original_tables.base < foreign_tables.end().unwrap()
                && foreign_tables.base < original_tables.end().unwrap()
        );
        let mut observed = snapshots {
            caller: trees.caller.clone(),
            candidate: observe(&candidate, &foreign_memory),
        };
        let directory = table(&candidate, table_level::directory, 4).unwrap();
        observed.caller[model.parent_word] = candidate.table_address(directory).unwrap()
            | present
            | writable
            | user_accessible
            | ownership_marker;
        // Both complete individual trees are canonical for their real owners;
        // rejection comes from cross-owner physical overlap, before any effect.
        assert_eq!(candidate.verify_lease_geometry(), Ok(()));
        assert_eq!(model.caller.verify_lease_geometry(), Ok(()));
        assert!(matches!(
            replacement_model::new(
                model.caller,
                &candidate,
                binding,
                independent_root,
                observed.observation()
            ),
            Err(model_error::geometry)
        ));
        // SAFETY: The rejected metadata model never published an actual edge.
        unsafe { candidate.cleanup(&mut foreign_allocator, &mut foreign_memory) }.unwrap();
        assert_eq!(foreign_allocator.accounting(), initial);
        terminal(model, trees);
    });
}
