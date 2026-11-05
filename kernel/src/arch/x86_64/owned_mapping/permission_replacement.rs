//! Host-only model of a retained RO/NX to RX permission replacement.
//!
//! This model borrows real immutable mapping owners to derive expected table
//! words, but reads only supplied observations and emits inert store descriptions.
//! It never accesses physical memory, changes a plan, calls a graft, authenticates
//! a task/source, invalidates translations, releases leases or resumes a caller.
//! Numeric bindings and asserted observations are model inputs, not authority.
//! A future adapter must establish every observation from the retained owners,
//! current scheduler authorization and architectural exclusion/invalidation.
//! The saved caller frame and globally fresh mutation epoch remain external
//! obligations: constructing another model with repeated metadata is possible.
//! Equality checks only bind subsequent steps within this model instance.

use super::*;
use crate::arch::x86_64::owned_source::source_origin;
use crate::identity::task_identity;
use anomalous_veil_protocol::source_mapping_execute::{
    source_mapping_execute_request, source_mapping_retained_context,
};

const window_base: u64 = 1 << 32;
const parent_slot: usize = 4;
const accessed: u64 = 1 << 5;

/// Complete native correlation metadata. Equality is not authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct model_binding {
    caller: task_identity,
    veil: task_identity,
    source_origin: source_origin,
    retained: source_mapping_retained_context,
    request: source_mapping_execute_request,
    caller_root: u64,
    candidate_root: u64,
    /// Asserted prior admission of this exact graft, not inferred from caller use.
    graft_hardware_admitted: bool,
    correlation: [u64; 4],
    mutation_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum model_phase {
    prepared,
    revoked,
    detaching,
    detached,
    changing,
    storing,
    changed,
    verified,
    publishing,
    published,
    ready,
    completed,
    quarantined,
    terminal_pending,
    terminal_quiescent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum model_error {
    binding,
    geometry,
    active_root,
    phase,
    observation,
    uncertainty,
    result,
    mapping(mapping_error),
}

/// All entries of both owners, in retained table order, including absent slots.
/// These slices are neither physical aliases nor hardware verification receipts.
#[derive(Clone, Copy)]
struct tree_observation<'a> {
    caller: &'a [u64],
    candidate: &'a [u64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum model_tree {
    caller,
    candidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct model_store {
    tree: model_tree,
    word: usize,
    before: u64,
    after: u64,
}

/// Explicitly modeled facts; this type cannot certify actual consumer or TLB state.
#[derive(Clone, Copy)]
struct terminal_observation {
    selection_revoked: bool,
    all_consumers_quiescent: bool,
    edge_detached: bool,
    translations_invalidated: bool,
}

/// No Drop cleanup, automatic rollback, clone or implicit completion exists.
/// The immutable borrows preserve both original plans and lease geometry while
/// the model exists. They do not establish that a real source is retained.
struct replacement_model<'a> {
    caller: &'a staged_mapping,
    candidate: &'a staged_mapping,
    binding: model_binding,
    parent_word: usize,
    installed_entry: u64,
    first_leaf_word: usize,
    phase: model_phase,
    confirmed_pages: u64,
    pending: Option<model_store>,
}

impl<'a> replacement_model<'a> {
    fn new(
        caller: &'a staged_mapping,
        candidate: &'a staged_mapping,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
    ) -> Result<Self, model_error> {
        let creation = binding.retained.creation();
        let request = binding.request;
        if binding.mutation_epoch == 0
            || binding.correlation == [0; 4]
            || binding.caller == binding.veil
            || caller.root_address() != Some(binding.caller_root)
            || candidate.root_address() != Some(binding.candidate_root)
            || binding.caller_root == binding.candidate_root
            || !binding.graft_hardware_admitted
            || !matches!(binding.source_origin, source_origin::provisioned(grant)
                if grant.get() == binding.retained.binding().grant())
            || source_mapping_execute_request::decode(
                request.sequence(),
                binding.retained.binding(),
                binding.retained,
                request.words(),
            ) != Ok(request)
        {
            return Err(model_error::binding);
        }
        if !caller.is_ready()
            || !caller.hardware_used
            || caller.tables_cleared
            || !candidate.is_ready()
            || candidate.hardware_used
            || candidate.tables_cleared
            || candidate.residency.is_some()
            || creation.base() != window_base
            || creation.rounded_byte_length() > 1 << 30
            || request.page_count() == 0
            || request.page_count() > entries_per_table as u64
            || request.base() / (page_size * entries_per_table as u64)
                != (request.base() + request.byte_length() - 1)
                    / (page_size * entries_per_table as u64)
        {
            return Err(model_error::geometry);
        }
        caller
            .verify_lease_geometry()
            .map_err(model_error::mapping)?;
        candidate
            .verify_lease_geometry()
            .map_err(model_error::mapping)?;
        // Different root addresses do not establish disjoint ownership. Owners
        // from separate allocator lineages can describe overlapping physical
        // storage; compare every retained table/data extent, not virtual aliases.
        for left in caller.tables.iter().chain(caller.data.iter().flatten()) {
            let left = left.lease.range().ok_or(model_error::geometry)?;
            let left_end = left.end().ok_or(model_error::geometry)?;
            for right in candidate
                .tables
                .iter()
                .chain(candidate.data.iter().flatten())
            {
                let right = right.lease.range().ok_or(model_error::geometry)?;
                let right_end = right.end().ok_or(model_error::geometry)?;
                if left.base < right_end && right.base < left_end {
                    return Err(model_error::geometry);
                }
            }
        }
        let region = candidate
            .plan
            .regions()
            .next()
            .ok_or(model_error::geometry)?;
        let populated_end = creation.base() + creation.data_pages() * page_size;
        let virtual_end = creation.base() + creation.rounded_byte_length();
        if candidate.plan.regions().count() != 1
            || region.extent().start() != creation.base()
            || region.extent().end() != populated_end
            || !region.permissions().user
            || region.permissions().writable
            || region.permissions().executable
            || candidate.plan.reservations().count() != usize::from(populated_end < virtual_end)
            || candidate
                .plan
                .reservations()
                .any(|extent| extent.start() != populated_end || extent.end() != virtual_end)
            || caller.plan.regions().any(|region| {
                region.extent().start() < virtual_end && window_base < region.extent().end()
            })
            || caller
                .plan
                .reservations()
                .any(|extent| extent.start() < virtual_end && window_base < extent.end())
        {
            return Err(model_error::geometry);
        }
        let parent_table = table(caller, table_level::directory_pointer, 0)?;
        if caller
            .expected_entry(parent_table, parent_slot)
            .map_err(model_error::mapping)?
            != 0
        {
            return Err(model_error::geometry);
        }
        let directory = table(candidate, table_level::directory, 4)?;
        let candidate_parent = table(candidate, table_level::directory_pointer, 0)?;
        let installed_entry = candidate
            .table_address(directory)
            .map_err(model_error::mapping)?
            | present
            | writable
            | user_accessible
            | ownership_marker;
        if candidate
            .expected_entry(candidate_parent, parent_slot)
            .map_err(model_error::mapping)?
            != installed_entry
        {
            return Err(model_error::geometry);
        }
        let leaf_table = table(
            candidate,
            table_level::leaf,
            request.base() / (page_size * entries_per_table as u64),
        )?;
        let model = Self {
            caller,
            candidate,
            binding,
            parent_word: parent_table * entries_per_table + parent_slot,
            installed_entry,
            first_leaf_word: leaf_table * entries_per_table + paging::indexes(request.base()).table,
            phase: model_phase::prepared,
            confirmed_pages: 0,
            pending: None,
        };
        model.check_binding(binding, active_root)?;
        model.verify(trees, true, 0, false)?;
        Ok(model)
    }

    fn check_binding(&self, binding: model_binding, active_root: u64) -> Result<(), model_error> {
        if binding != self.binding {
            return Err(model_error::binding);
        }
        let current = paging::entry_address(active_root);
        if current == 0 || current == binding.caller_root || current == binding.candidate_root {
            return Err(model_error::active_root);
        }
        Ok(())
    }

    fn require(&self, phase: model_phase) -> Result<(), model_error> {
        if self.phase != phase {
            return Err(model_error::phase);
        }
        Ok(())
    }

    fn quarantine(&mut self) -> Result<(), model_error> {
        self.phase = model_phase::quarantined;
        Err(model_error::uncertainty)
    }

    fn revoke_selection(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        observed_revoked: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::prepared)?;
        self.verify(trees, true, 0, false)?;
        if !observed_revoked {
            return Err(model_error::observation);
        }
        self.phase = model_phase::revoked;
        Ok(())
    }

    fn begin_detach(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
    ) -> Result<model_store, model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::revoked)?;
        self.verify(trees, true, 0, false)?;
        self.phase = model_phase::detaching;
        Ok(model_store {
            tree: model_tree::caller,
            word: self.parent_word,
            before: trees.caller[self.parent_word],
            after: 0,
        })
    }

    fn finish_detach(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        store_reported_success: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::detaching)?;
        if !store_reported_success || self.verify(trees, false, 0, false).is_err() {
            return self.quarantine();
        }
        self.phase = model_phase::detached;
        Ok(())
    }

    fn confirm_detach_invalidation(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        observed_invalidated: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::detached)?;
        if !observed_invalidated || self.verify(trees, false, 0, false).is_err() {
            return self.quarantine();
        }
        self.phase = model_phase::changing;
        Ok(())
    }

    fn begin_leaf_store(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
    ) -> Result<model_store, model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::changing)?;
        self.verify(trees, false, self.confirmed_pages, false)?;
        let word = self.first_leaf_word + self.confirmed_pages as usize;
        let store = model_store {
            tree: model_tree::candidate,
            word,
            before: trees.candidate[word],
            after: trees.candidate[word] & !paging::no_execute,
        };
        self.pending = Some(store);
        self.phase = model_phase::storing;
        Ok(store)
    }

    fn finish_leaf_store(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        store_reported_success: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::storing)?;
        let store = self.pending.ok_or(model_error::phase)?;
        if !store_reported_success
            || trees.candidate.get(store.word) != Some(&store.after)
            || self
                .verify(trees, false, self.confirmed_pages + 1, false)
                .is_err()
        {
            return self.quarantine();
        }
        self.confirmed_pages += 1;
        self.pending = None;
        self.phase = if self.confirmed_pages == self.binding.request.page_count() {
            model_phase::changed
        } else {
            model_phase::changing
        };
        Ok(())
    }

    fn verify_replacement(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::changed)?;
        if self
            .verify(trees, false, self.confirmed_pages, false)
            .is_err()
        {
            return self.quarantine();
        }
        self.phase = model_phase::verified;
        Ok(())
    }

    fn begin_publish(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
    ) -> Result<model_store, model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::verified)?;
        if self
            .verify(trees, false, self.confirmed_pages, false)
            .is_err()
        {
            self.phase = model_phase::quarantined;
            return Err(model_error::uncertainty);
        }
        self.phase = model_phase::publishing;
        Ok(model_store {
            tree: model_tree::caller,
            word: self.parent_word,
            before: 0,
            after: self.installed_entry,
        })
    }

    fn finish_publish(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        store_reported_success: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::publishing)?;
        if !store_reported_success
            || trees.caller.get(self.parent_word) != Some(&self.installed_entry)
            || self
                .verify(trees, true, self.confirmed_pages, false)
                .is_err()
        {
            return self.quarantine();
        }
        self.phase = model_phase::published;
        Ok(())
    }

    fn confirm_publish_invalidation(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        observed_invalidated: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::published)?;
        if !observed_invalidated
            || self
                .verify(trees, true, self.confirmed_pages, false)
                .is_err()
        {
            return self.quarantine();
        }
        self.phase = model_phase::ready;
        Ok(())
    }

    /// Models one exact completion, without producing an executable ticket.
    fn complete_model(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        result: u64,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::ready)?;
        if result != self.binding.request.base() {
            return Err(model_error::result);
        }
        if self
            .verify(trees, true, self.confirmed_pages, false)
            .is_err()
        {
            return self.quarantine();
        }
        self.phase = model_phase::completed;
        Ok(())
    }

    fn begin_terminal(
        &mut self,
        binding: model_binding,
        active_root: u64,
        observed_revoked: bool,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        if matches!(
            self.phase,
            model_phase::terminal_pending | model_phase::terminal_quiescent
        ) {
            return Err(model_error::phase);
        }
        if !observed_revoked {
            return Err(model_error::observation);
        }
        self.phase = model_phase::terminal_pending;
        Ok(())
    }

    fn confirm_terminal(
        &mut self,
        binding: model_binding,
        active_root: u64,
        trees: tree_observation<'_>,
        facts: terminal_observation,
    ) -> Result<(), model_error> {
        self.check_binding(binding, active_root)?;
        self.require(model_phase::terminal_pending)?;
        if !facts.selection_revoked
            || !facts.all_consumers_quiescent
            || !facts.edge_detached
            || !facts.translations_invalidated
        {
            return Err(model_error::observation);
        }
        // The only indeterminate leaf is the recorded attempted store. Either
        // exact old or exact new permissions are acceptable for terminal clearing;
        // all confirmed leaves, untouched neighbors and ancestry stay exact.
        self.verify(trees, false, self.confirmed_pages, self.pending.is_some())?;
        self.phase = model_phase::terminal_quiescent;
        Ok(())
    }

    fn model_cleanup_permitted(&self) -> bool {
        self.phase == model_phase::terminal_quiescent
    }

    fn verify(
        &self,
        trees: tree_observation<'_>,
        published: bool,
        replaced_pages: u64,
        allow_pending: bool,
    ) -> Result<(), model_error> {
        if trees.caller.len() != self.caller.table_count() * entries_per_table
            || trees.candidate.len() != self.candidate.table_count() * entries_per_table
            || replaced_pages > self.binding.request.page_count()
        {
            return Err(model_error::observation);
        }
        for (word, actual) in trees.caller.iter().copied().enumerate() {
            let table = word / entries_per_table;
            let expected = if word == self.parent_word && published {
                self.installed_entry
            } else {
                self.caller
                    .expected_entry(table, word % entries_per_table)
                    .map_err(model_error::mapping)?
            };
            let allowed = if word == self.parent_word {
                if published { accessed } else { 0 }
            } else {
                self.caller.permitted_runtime_bits(table, expected)
            };
            if actual & !allowed != expected {
                return Err(model_error::observation);
            }
        }
        for (word, actual) in trees.candidate.iter().copied().enumerate() {
            let table = word / entries_per_table;
            let original = self
                .candidate
                .expected_entry(table, word % entries_per_table)
                .map_err(model_error::mapping)?;
            let replaced = word >= self.first_leaf_word
                && word < self.first_leaf_word + replaced_pages as usize;
            let expected = if replaced {
                original & !paging::no_execute
            } else {
                original
            };
            let allowed = if expected & present != 0
                && self.candidate.plan.tables().get(table).is_some_and(|node| {
                    matches!(node.level(), table_level::directory | table_level::leaf)
                }) {
                accessed
            } else {
                0
            };
            let canonical = actual & !allowed;
            let pending_new = allow_pending
                && self.pending.is_some_and(|pending| pending.word == word)
                && canonical == original & !paging::no_execute;
            if canonical != expected && !pending_new {
                return Err(model_error::observation);
            }
        }
        Ok(())
    }
}

fn table(owner: &staged_mapping, level: table_level, prefix: u64) -> Result<usize, model_error> {
    owner
        .plan
        .tables()
        .iter()
        .position(|node| node.level() == level && node.prefix() == prefix)
        .ok_or(model_error::geometry)
}

#[cfg(test)]
mod tests;
