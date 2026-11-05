#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Checked geometry for one bounded low supervisor island and a borrowed alias.
//!
//! These numbers prove neither residency nor authority. The activation caller
//! must prove that every executing path, live stack, descriptor, and exception
//! stack is covered, and retain ownership/lifetimes of all borrowed storage.

use super::mapping_plan::{mapping_plan, table_frame_capacity, table_level};
use super::paging::{self, no_execute, page_permissions, page_size, present, writable};

pub(super) const resident_capacity: usize = 8;
pub(super) const resident_limit: u64 = 2 * 1024 * 1024;
pub(super) const resident_alias_slot: usize = 288;
pub(super) const resident_alias_base: u64 = 0xffff_9000_0000_0000;
const accessed: u64 = 1 << 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum resident_error {
    empty,
    capacity_exhausted,
    invalid_extent,
    invalid_permissions,
    overlap,
    user_collision,
    missing_low_branch,
    invalid_alias,
    invalid_budget,
    table_budget_exhausted,
    changed_table_count,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct resident_request {
    pub virtual_address: u64,
    pub page_count: u64,
    pub permissions: page_permissions,
}

/// One identity-mapped supervisor interval, with no ownership claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct resident_region {
    start: u64,
    end: u64,
    permissions: page_permissions,
}

impl resident_region {
    pub(super) const fn start(self) -> u64 {
        self.start
    }

    pub(super) const fn end(self) -> u64 {
        self.end
    }

    pub(super) const fn permissions(self) -> page_permissions {
        self.permissions
    }
}

/// An immutable bounded profile, not evidence that its borrowed pages are live.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct resident_plan {
    regions: [Option<resident_region>; resident_capacity],
    alias_entry: u64,
    table_count: usize,
    table_budget: usize,
    // None preserves the original shared-low-branch profile. The opt-in
    // profile appends only missing supervisor ancestors, in parent-first order.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    additional_tables: Option<[Option<table_level>; 3]>,
}

impl resident_plan {
    pub(super) fn new(
        user: &mapping_plan,
        requests: &[resident_request],
        borrowed_alias_entry: u64,
        table_budget: usize,
    ) -> Result<Self, resident_error> {
        Self::new_profile(
            user,
            requests,
            borrowed_alias_entry,
            table_budget,
            #[cfg(any(test, feature = "owned_multi_region_test"))]
            false,
        )
    }

    /// Explicitly admit a user tree which need not share the low resident
    /// ancestry. Existing user table indexes stay unchanged; missing prefix-zero
    /// PDPT, PD and PT nodes are appended in that order. This is still geometry,
    /// not ownership of the resident pages or the borrowed physical alias.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(super) fn new_with_ancestry(
        user: &mapping_plan,
        requests: &[resident_request],
        borrowed_alias_entry: u64,
        table_budget: usize,
    ) -> Result<Self, resident_error> {
        Self::new_profile(user, requests, borrowed_alias_entry, table_budget, true)
    }

    fn new_profile(
        user: &mapping_plan,
        requests: &[resident_request],
        borrowed_alias_entry: u64,
        table_budget: usize,
        #[cfg(any(test, feature = "owned_multi_region_test"))] complete_ancestry: bool,
    ) -> Result<Self, resident_error> {
        if requests.is_empty() {
            return Err(resident_error::empty);
        }
        if requests.len() > resident_capacity {
            return Err(resident_error::capacity_exhausted);
        }
        if !(1..=table_frame_capacity).contains(&table_budget) {
            return Err(resident_error::invalid_budget);
        }
        let child = paging::entry_address(borrowed_alias_entry);
        let alias_entry = child | present | writable | no_execute;
        if child == 0
            || child >= paging::identity_map_limit
            || borrowed_alias_entry & !accessed != alias_entry
            || paging::indexes(resident_alias_base).pml4 != resident_alias_slot
        {
            return Err(resident_error::invalid_alias);
        }
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        let additional_tables = complete_ancestry.then(|| missing_ancestry(user));
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        let extra_count = additional_tables
            .as_ref()
            .map_or(1, |tables| tables.iter().flatten().count());
        #[cfg(not(any(test, feature = "owned_multi_region_test")))]
        let extra_count = 1;
        let table_count = user
            .tables()
            .len()
            .checked_add(extra_count)
            .ok_or(resident_error::table_budget_exhausted)?;
        let mut plan = Self {
            regions: [None; resident_capacity],
            alias_entry,
            table_count,
            table_budget,
            #[cfg(any(test, feature = "owned_multi_region_test"))]
            additional_tables,
        };
        plan.validate_user_plan(user)?;
        for (index, request) in requests.iter().enumerate() {
            let start = request.virtual_address;
            let end = request
                .page_count
                .checked_mul(page_size)
                .and_then(|length| start.checked_add(length))
                .filter(|end| *end > start && *end <= resident_limit)
                .ok_or(resident_error::invalid_extent)?;
            if !start.is_multiple_of(page_size) {
                return Err(resident_error::invalid_extent);
            }
            if request.permissions.user || paging::leaf_flags(request.permissions).is_none() {
                return Err(resident_error::invalid_permissions);
            }
            if plan
                .regions()
                .any(|other| start < other.end && other.start < end)
            {
                return Err(resident_error::overlap);
            }
            plan.regions[index] = Some(resident_region {
                start,
                end,
                permissions: request.permissions,
            });
        }
        plan.regions[..requests.len()]
            .sort_unstable_by_key(|region| region.map(|range| range.start));
        Ok(plan)
    }

    /// Recheck compatibility when the resident profile is attached to an owner.
    pub(super) fn validate_user_plan(&self, user: &mapping_plan) -> Result<(), resident_error> {
        if user
            .regions()
            .any(|region| region.extent().start() < resident_limit)
        {
            return Err(resident_error::user_collision);
        }
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        let complete_ancestry = self.additional_tables.is_some();
        #[cfg(not(any(test, feature = "owned_multi_region_test")))]
        let complete_ancestry = false;
        if !complete_ancestry {
            for level in [table_level::directory_pointer, table_level::directory] {
                if !user
                    .tables()
                    .iter()
                    .any(|node| node.level() == level && node.prefix() == 0)
                {
                    return Err(resident_error::missing_low_branch);
                }
            }
        }
        #[cfg(any(test, feature = "owned_multi_region_test"))]
        let extra_count = if let Some(tables) = &self.additional_tables {
            // A same-sized but differently rooted user tree cannot silently
            // reuse a resident plan with different supplemental ancestors.
            if *tables != missing_ancestry(user) {
                return Err(resident_error::changed_table_count);
            }
            tables.iter().flatten().count()
        } else {
            1
        };
        #[cfg(not(any(test, feature = "owned_multi_region_test")))]
        let extra_count = 1;
        let count = user
            .tables()
            .len()
            .checked_add(extra_count)
            .ok_or(resident_error::table_budget_exhausted)?;
        if count > self.table_budget || count > table_frame_capacity {
            return Err(resident_error::table_budget_exhausted);
        }
        if count != self.table_count {
            return Err(resident_error::changed_table_count);
        }
        Ok(())
    }

    pub(super) fn regions(&self) -> impl Iterator<Item = &resident_region> {
        self.regions.iter().flatten()
    }

    pub(super) const fn alias_entry(&self) -> u64 {
        self.alias_entry
    }

    pub(super) const fn alias_slot(&self) -> usize {
        resident_alias_slot
    }

    pub(super) const fn table_count(&self) -> usize {
        self.table_count
    }

    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(super) const fn completes_ancestry(&self) -> bool {
        self.additional_tables.is_some()
    }

    /// Index is relative to the end of the unchanged user table list. Every
    /// appended node describes prefix zero and is exclusively supervisor-owned.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(super) fn additional_table(&self, index: usize) -> Option<(table_level, u64)> {
        self.additional_tables
            .as_ref()?
            .get(index)?
            .map(|level| (level, 0))
    }
}

#[cfg(any(test, feature = "owned_multi_region_test"))]
fn missing_ancestry(user: &mapping_plan) -> [Option<table_level>; 3] {
    let mut result = [None; 3];
    let mut count = 0;
    for level in [
        table_level::directory_pointer,
        table_level::directory,
        table_level::leaf,
    ] {
        if !user
            .tables()
            .iter()
            .any(|node| node.level() == level && node.prefix() == 0)
        {
            result[count] = Some(level);
            count += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::mapping_plan::{mapping_budget, mapping_request};
    use super::super::paging::{huge_page, ownership_marker, user_accessible};
    use super::*;

    const alias: u64 = 0x13000 | present | writable | no_execute;
    const read_execute: page_permissions = page_permissions {
        user: false,
        writable: false,
        executable: true,
    };
    const read_write: page_permissions = page_permissions {
        user: false,
        writable: true,
        executable: false,
    };

    fn user(address: u64, pages: u64) -> mapping_plan {
        mapping_plan::new(
            &[mapping_request {
                virtual_address: address,
                page_count: pages,
                permissions: page_permissions {
                    user: true,
                    writable: false,
                    executable: true,
                },
            }],
            &[],
            mapping_budget {
                data_pages: u64::MAX,
                table_frames: table_frame_capacity,
            },
        )
        .unwrap()
    }

    fn request(address: u64, pages: u64, permissions: page_permissions) -> resident_request {
        resident_request {
            virtual_address: address,
            page_count: pages,
            permissions,
        }
    }

    #[test]
    fn disjoint_residency_is_sorted_and_leaves_gaps_undescribed() {
        let user = user(0x400000, 1);
        let profile = resident_plan::new(
            &user,
            &[
                request(0x18000, 16, read_write),
                request(0x110000, 2, read_write),
                request(0x100000, 8, read_execute),
            ],
            alias | accessed,
            5,
        )
        .unwrap();
        assert_eq!(profile.table_count(), 5);
        assert_eq!(profile.alias_entry(), alias);
        assert_eq!(profile.alias_slot(), 288);
        let mut regions = profile.regions();
        assert_eq!(regions.next().unwrap().start(), 0x18000);
        let text = regions.next().unwrap();
        assert_eq!(
            (text.start(), text.end(), text.permissions()),
            (0x100000, 0x108000, read_execute)
        );
        assert_eq!(regions.next().unwrap().start(), 0x110000);
        assert!(regions.next().is_none());
    }

    #[test]
    fn malformed_extents_overlap_and_user_permissions_are_rejected() {
        let user = user(0x400000, 1);
        for item in [
            request(0x100000, 0, read_execute),
            request(0x100001, 1, read_execute),
            request(resident_limit - page_size, 2, read_execute),
            request(u64::MAX - page_size + 1, 1, read_execute),
            request(0, u64::MAX, read_execute),
        ] {
            assert_eq!(
                resident_plan::new(&user, &[item], alias, 5),
                Err(resident_error::invalid_extent)
            );
        }
        for permissions in [
            page_permissions {
                user: true,
                ..read_execute
            },
            page_permissions {
                writable: true,
                ..read_execute
            },
        ] {
            assert_eq!(
                resident_plan::new(&user, &[request(0x100000, 1, permissions)], alias, 5),
                Err(resident_error::invalid_permissions)
            );
        }
        assert_eq!(
            resident_plan::new(
                &user,
                &[
                    request(0x100000, 3, read_execute),
                    request(0x102000, 2, read_write),
                ],
                alias,
                5
            ),
            Err(resident_error::overlap)
        );
        assert!(
            resident_plan::new(
                &user,
                &[
                    request(0x100000, 2, read_execute),
                    request(0x102000, 2, read_write),
                ],
                alias,
                5
            )
            .is_ok()
        );
    }

    #[test]
    fn alias_requires_exact_supervisor_writable_nx_parent_and_valid_child() {
        let user = user(0x400000, 1);
        let regions = [request(0x100000, 1, read_execute)];
        for entry in [
            present | writable | no_execute,
            (1 << 32) | present | writable | no_execute,
            alias & !present,
            alias & !writable,
            alias & !no_execute,
            alias | user_accessible,
            alias | huge_page,
            alias | ownership_marker,
            alias | (1 << 8),
            alias | (1 << 6),
            alias | (1 << 4),
            alias | (1 << 59),
        ] {
            assert_eq!(
                resident_plan::new(&user, &regions, entry, 5),
                Err(resident_error::invalid_alias)
            );
        }
    }

    #[test]
    fn low_user_collision_and_missing_shared_branch_are_rejected() {
        let regions = [request(0x100000, 1, read_execute)];
        assert_eq!(
            resident_plan::new(&user(0x1000, 1), &regions, alias, 5),
            Err(resident_error::user_collision)
        );
        for address in [1 << 30, 1 << 39] {
            assert_eq!(
                resident_plan::new(&user(address, 1), &regions, alias, 5),
                Err(resident_error::missing_low_branch)
            );
        }
        let profile = resident_plan::new(&user(0x400000, 1), &regions, alias, 32).unwrap();
        assert_eq!(
            profile.validate_user_plan(&user(0x400000, 513)),
            Err(resident_error::changed_table_count)
        );
    }

    #[test]
    fn resident_and_combined_table_capacities_are_exact() {
        let user_plan = user(0x400000, 1);
        let eight = core::array::from_fn::<_, resident_capacity, _>(|index| {
            request(0x100000 + index as u64 * page_size, 1, read_execute)
        });
        let nine = [request(0x100000, 1, read_execute); resident_capacity + 1];
        assert!(resident_plan::new(&user_plan, &eight, alias, 5).is_ok());
        assert_eq!(
            resident_plan::new(&user_plan, &nine, alias, 5),
            Err(resident_error::capacity_exhausted)
        );
        assert_eq!(
            resident_plan::new(&user_plan, &[], alias, 5),
            Err(resident_error::empty)
        );
        for budget in [0, table_frame_capacity + 1] {
            assert_eq!(
                resident_plan::new(&user_plan, &eight, alias, budget),
                Err(resident_error::invalid_budget)
            );
        }
        assert_eq!(
            resident_plan::new(&user_plan, &eight, alias, 4),
            Err(resident_error::table_budget_exhausted)
        );
        let exact = user(resident_limit, 28 * 512);
        assert_eq!(exact.tables().len(), 31);
        assert_eq!(
            resident_plan::new(&exact, &eight, alias, 32)
                .unwrap()
                .table_count(),
            32
        );
        let exceeded = user(resident_limit, 29 * 512);
        assert_eq!(exceeded.tables().len(), 32);
        assert_eq!(
            resident_plan::new(&exceeded, &eight, alias, 32),
            Err(resident_error::table_budget_exhausted)
        );
    }

    #[test]
    fn separate_image_and_stack_branches_append_only_missing_ancestry() {
        // Rounded geometry of the reviewed true/interpreter images plus 64KiB
        // stack. These are native mapping inputs, not ELF fields or authority.
        let requests = [
            (0x40000000, 2, false, false),
            (0x40002000, 4, false, true),
            (0x40006000, 1, false, false),
            (0x40007000, 2, true, false),
            (0x60000000, 1, false, false),
            (0x60001000, 47, false, true),
            (0x60030000, 11, false, false),
            (0x6003b000, 4, true, false),
            (0x7fff00000000, 16, true, false),
        ]
        .map(
            |(virtual_address, page_count, is_writable, executable)| mapping_request {
                virtual_address,
                page_count,
                permissions: page_permissions {
                    user: true,
                    writable: is_writable,
                    executable,
                },
            },
        );
        let user = mapping_plan::new(
            &requests,
            &[],
            mapping_budget {
                data_pages: 88,
                table_frames: 8,
            },
        )
        .unwrap();
        assert_eq!(user.tables().len(), 8);
        assert_eq!(user.regions().count(), 9);
        let regions = [request(0x100000, 2, read_execute)];
        assert_eq!(
            resident_plan::new(&user, &regions, alias, 10),
            Err(resident_error::missing_low_branch),
        );
        let resident = resident_plan::new_with_ancestry(&user, &regions, alias, 10).unwrap();
        assert_eq!(resident.table_count(), 10);
        assert_eq!(
            resident.additional_table(0),
            Some((table_level::directory, 0))
        );
        assert_eq!(resident.additional_table(1), Some((table_level::leaf, 0)));
        assert_eq!(resident.additional_table(2), None);
        assert_eq!(resident.additional_table(usize::MAX), None);
        assert_eq!(resident.validate_user_plan(&user), Ok(()));
        assert_eq!(
            resident_plan::new_with_ancestry(&user, &regions, alias, 9),
            Err(resident_error::table_budget_exhausted),
        );
    }

    #[test]
    fn optional_ancestry_has_exact_budget_and_cannot_be_rebound_to_different_branches() {
        let regions = [request(0x100000, 1, read_execute)];
        for (address, extra) in [(0x400000, 1), (1 << 30, 2), (1 << 39, 3)] {
            let user = user(address, 1);
            let count = user.tables().len() + extra;
            let resident = resident_plan::new_with_ancestry(&user, &regions, alias, count).unwrap();
            assert_eq!(resident.table_count(), count);
            assert_eq!(
                resident.additional_table(extra - 1),
                Some((table_level::leaf, 0))
            );
            assert_eq!(resident.additional_table(extra), None);
            assert_eq!(
                resident_plan::new_with_ancestry(&user, &regions, alias, count - 1),
                Err(resident_error::table_budget_exhausted),
            );
        }
        let resident =
            resident_plan::new_with_ancestry(&user(1 << 30, 1), &regions, alias, 6).unwrap();
        // Both user plans have four nodes, but need different extra ancestors.
        assert_eq!(
            resident.validate_user_plan(&user(1 << 39, 1)),
            Err(resident_error::changed_table_count),
        );
        assert_eq!(
            resident_plan::new_with_ancestry(&user(0x1000, 1), &regions, alias, 7),
            Err(resident_error::user_collision),
        );
        assert_eq!(
            resident_plan::new_with_ancestry(
                &user(1 << 30, 1),
                &regions,
                alias | user_accessible,
                6
            ),
            Err(resident_error::invalid_alias),
        );
        assert_eq!(
            resident_plan::new_with_ancestry(
                &user(1 << 30, 1),
                &[request(
                    0x100000,
                    1,
                    page_permissions {
                        user: true,
                        ..read_execute
                    }
                )],
                alias,
                6
            ),
            Err(resident_error::invalid_permissions),
        );
        // The user plan already consumes all 32 table slots.
        let maximum_user = user(1 << 30, 29 * 512);
        assert_eq!(maximum_user.tables().len(), table_frame_capacity);
        assert_eq!(
            resident_plan::new_with_ancestry(&maximum_user, &regions, alias, table_frame_capacity),
            Err(resident_error::table_budget_exhausted),
        );
    }
}
