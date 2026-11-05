#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Inert geometry for a future four-level, 4-KiB user mapper.
//!
//! A successful plan owns no frames and grants no access. In particular, low
//! addresses accepted here remain forbidden by the live bootstrap mapper.
//! Counts cover a new root and its user-tree nodes only; retained supervisor
//! mappings and a physical window need a separately validated combined budget.

use super::paging::{page_permissions, page_size};

#[cfg(not(any(test, feature = "owned_multi_region_test")))]
pub(super) const region_capacity: usize = 8;
#[cfg(any(test, feature = "owned_multi_region_test"))]
pub(super) const region_capacity: usize = 16;
pub(super) const reservation_capacity: usize = 8;
pub(super) const table_frame_capacity: usize = 32;
pub(super) const user_address_limit: u64 = 1 << 47;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct mapping_request {
    pub virtual_address: u64,
    pub page_count: u64,
    pub permissions: page_permissions,
}

/// An exclusion in the lower canonical half, not a request to map that range.
/// Exclusions may overlap: their union is unavailable for user mappings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct reserved_request {
    pub virtual_address: u64,
    pub page_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct mapping_budget {
    pub data_pages: u64,
    pub table_frames: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum plan_error {
    empty,
    region_capacity_exhausted,
    reservation_capacity_exhausted,
    invalid_budget,
    invalid_user_extent,
    invalid_reservation,
    supervisor_permission,
    write_execute,
    overlap,
    reserved_collision,
    data_budget_exhausted,
    table_budget_exhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct planned_extent {
    start: u64,
    end: u64,
}

impl planned_extent {
    fn from_pages(start: u64, pages: u64) -> Option<Self> {
        if pages == 0 || !start.is_multiple_of(page_size) {
            return None;
        }
        let end = start.checked_add(pages.checked_mul(page_size)?)?;
        (end <= user_address_limit).then_some(Self { start, end })
    }

    pub(super) const fn start(self) -> u64 {
        self.start
    }

    pub(super) const fn end(self) -> u64 {
        self.end
    }

    pub(super) const fn page_count(self) -> u64 {
        (self.end - self.start) / page_size
    }

    fn overlaps(self, other: Self) -> bool {
        self.start < other.end && other.start < self.end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct planned_region {
    extent: planned_extent,
    permissions: page_permissions,
}

impl planned_region {
    pub(super) const fn extent(self) -> planned_extent {
        self.extent
    }

    pub(super) const fn permissions(self) -> page_permissions {
        self.permissions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum table_level {
    root,
    directory_pointer,
    directory,
    leaf,
}

/// One unique table node, identified by the complete virtual prefix above it.
/// This is not a physical address or an encoded hardware page-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct table_requirement {
    level: table_level,
    prefix: u64,
}

impl table_requirement {
    pub(super) const fn level(self) -> table_level {
        self.level
    }

    pub(super) const fn prefix(self) -> u64 {
        self.prefix
    }
}

/// Fully checked, immutable geometry; it has no allocation or activation API.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct mapping_plan {
    regions: [Option<planned_region>; region_capacity],
    reservations: [Option<planned_extent>; reservation_capacity],
    tables: [table_requirement; table_frame_capacity],
    table_count: usize,
    data_pages: u64,
}

impl mapping_plan {
    /// Validate the complete input before returning any usable plan. Inputs are
    /// borrowed immutably; failure cannot modify a previous plan or ownership.
    pub(super) fn new(
        requests: &[mapping_request],
        reservations: &[reserved_request],
        budget: mapping_budget,
    ) -> Result<Self, plan_error> {
        if requests.is_empty() {
            return Err(plan_error::empty);
        }
        if requests.len() > region_capacity {
            return Err(plan_error::region_capacity_exhausted);
        }
        if reservations.len() > reservation_capacity {
            return Err(plan_error::reservation_capacity_exhausted);
        }
        if budget.data_pages == 0 || !(1..=table_frame_capacity).contains(&budget.table_frames) {
            return Err(plan_error::invalid_budget);
        }
        let root = table_requirement {
            level: table_level::root,
            prefix: 0,
        };
        let mut plan = Self {
            regions: [None; region_capacity],
            reservations: [None; reservation_capacity],
            tables: [root; table_frame_capacity],
            table_count: 1,
            data_pages: 0,
        };
        // Even exclusions which do not intersect a request must be valid.
        for (index, request) in reservations.iter().enumerate() {
            plan.reservations[index] = Some(
                planned_extent::from_pages(request.virtual_address, request.page_count)
                    .ok_or(plan_error::invalid_reservation)?,
            );
        }
        for (index, request) in requests.iter().enumerate() {
            let extent = planned_extent::from_pages(request.virtual_address, request.page_count)
                .filter(|range| range.start != 0)
                .ok_or(plan_error::invalid_user_extent)?;
            if !request.permissions.user {
                return Err(plan_error::supervisor_permission);
            }
            if request.permissions.writable && request.permissions.executable {
                return Err(plan_error::write_execute);
            }
            if plan
                .regions
                .iter()
                .flatten()
                .any(|other| extent.overlaps(other.extent))
            {
                return Err(plan_error::overlap);
            }
            if plan
                .reservations
                .iter()
                .flatten()
                .any(|other| extent.overlaps(*other))
            {
                return Err(plan_error::reserved_collision);
            }
            plan.data_pages = plan
                .data_pages
                .checked_add(request.page_count)
                .filter(|pages| *pages <= budget.data_pages)
                .ok_or(plan_error::data_budget_exhausted)?;
            plan.regions[index] = Some(planned_region {
                extent,
                permissions: request.permissions,
            });
        }
        // Canonical order makes the complete geometry independent of request
        // order, without coalescing adjacent regions or changing their rights.
        plan.regions[..requests.len()]
            .sort_unstable_by_key(|region| region.map(|r| r.extent.start));
        plan.reservations[..reservations.len()]
            .sort_unstable_by_key(|range| range.map(|r| (r.start, r.end)));

        // All data extents and the total data budget were checked above. Walk
        // leaf-table spans, never individual pages. Every new span consumes a
        // table slot; duplicate spans are bounded by the eight region inputs.
        for index in 0..requests.len() {
            let region = plan.regions[index].expect("validated region prefix");
            let first = region.extent.start >> 21;
            let last = (region.extent.end - 1) >> 21;
            for branch in first..=last {
                for requirement in [
                    table_requirement {
                        level: table_level::directory_pointer,
                        prefix: branch >> 18,
                    },
                    table_requirement {
                        level: table_level::directory,
                        prefix: branch >> 9,
                    },
                    table_requirement {
                        level: table_level::leaf,
                        prefix: branch,
                    },
                ] {
                    if plan.tables[..plan.table_count].contains(&requirement) {
                        continue;
                    }
                    if plan.table_count == budget.table_frames {
                        return Err(plan_error::table_budget_exhausted);
                    }
                    plan.tables[plan.table_count] = requirement;
                    plan.table_count += 1;
                }
            }
        }
        Ok(plan)
    }

    pub(super) fn regions(&self) -> impl Iterator<Item = &planned_region> {
        self.regions.iter().flatten()
    }

    pub(super) fn reservations(&self) -> impl Iterator<Item = &planned_extent> {
        self.reservations.iter().flatten()
    }

    pub(super) fn tables(&self) -> &[table_requirement] {
        &self.tables[..self.table_count]
    }

    pub(super) const fn data_pages(&self) -> u64 {
        self.data_pages
    }

    /// Exact RX mapping membership only, not proof of an initialized entry
    /// point, live task return frame, or installed executable translation.
    pub(super) fn contains_executable_address(&self, address: u64) -> bool {
        self.regions().any(|region| {
            region.permissions.executable
                && address >= region.extent.start
                && address < region.extent.end
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::collections::BTreeSet;

    const read_only: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: false,
    };
    const read_write: page_permissions = page_permissions {
        user: true,
        writable: true,
        executable: false,
    };
    const read_execute: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: true,
    };
    const generous: mapping_budget = mapping_budget {
        data_pages: u64::MAX,
        table_frames: table_frame_capacity,
    };

    fn request(address: u64, pages: u64, permissions: page_permissions) -> mapping_request {
        mapping_request {
            virtual_address: address,
            page_count: pages,
            permissions,
        }
    }

    fn reserve(address: u64, pages: u64) -> reserved_request {
        reserved_request {
            virtual_address: address,
            page_count: pages,
        }
    }

    // Independent oracle enumerates individual pages with divisions by their
    // table coverage sizes, unlike the production branch-span traversal.
    fn oracle(requests: &[mapping_request]) -> BTreeSet<(u8, u64)> {
        let mut nodes = BTreeSet::from([(4, 0)]);
        for request in requests {
            assert!(
                request.page_count <= 1024,
                "bound the host oracle's page walk"
            );
            for page in 0..request.page_count {
                let address = request.virtual_address + page * 4096;
                for (level, coverage) in [
                    (3, 512 * 1024 * 1024 * 1024),
                    (2, 1024 * 1024 * 1024),
                    (1, 2 * 1024 * 1024),
                ] {
                    nodes.insert((level, address / coverage));
                }
            }
        }
        nodes
    }

    fn assert_oracle(requests: &[mapping_request]) {
        let plan = mapping_plan::new(requests, &[], generous).unwrap();
        let actual: BTreeSet<_> = plan
            .tables()
            .iter()
            .map(|node| {
                (
                    match node.level() {
                        table_level::root => 4,
                        table_level::directory_pointer => 3,
                        table_level::directory => 2,
                        table_level::leaf => 1,
                    },
                    node.prefix(),
                )
            })
            .collect();
        assert_eq!(
            actual.len(),
            plan.tables().len(),
            "no duplicated table frame"
        );
        assert_eq!(actual, oracle(requests));
        assert_eq!(
            plan.data_pages(),
            requests.iter().map(|r| r.page_count).sum()
        );
    }

    #[test]
    fn low_segments_and_distant_stack_preserve_exact_geometry() {
        let requests = [
            request(0x400000, 1, read_execute),
            request(0x5ff000, 2, read_only),
            request(0x800000, 2, read_write),
            request(user_address_limit - 4 * page_size, 2, read_write),
        ];
        let reservations = [
            reserve(0, 1),
            reserve(0x100000, 128),
            reserve(user_address_limit - 5 * page_size, 1),
            reserve(user_address_limit - 2 * page_size, 1),
        ];
        let plan = mapping_plan::new(&requests, &reservations, generous).unwrap();
        assert_eq!(plan.data_pages(), 7);
        assert_eq!(plan.tables().len(), 9);
        assert_eq!(plan.reservations().count(), 4);
        assert_eq!(plan.regions().nth(1).unwrap().extent().start(), 0x5ff000);
        assert_eq!(plan.regions().nth(1).unwrap().extent().page_count(), 2);
        assert_eq!(plan.regions().nth(1).unwrap().extent().end(), 0x601000);
        assert_eq!(plan.regions().nth(2).unwrap().permissions(), read_write);
        assert_oracle(&requests);
    }

    #[test]
    fn all_table_boundaries_and_sparse_parent_prefixes_match_page_oracle() {
        for boundary in [1 << 21, 1 << 30, 1 << 39] {
            for before in [1, 2, 17] {
                for after in [1, 2, 17] {
                    assert_oracle(&[request(
                        boundary - before * page_size,
                        before + after,
                        read_only,
                    )]);
                }
            }
        }
        assert_oracle(&[
            request(0x400000, 1, read_only),
            request((1 << 30) + 0x400000, 1, read_only),
            request((1 << 39) + 0x400000, 1, read_only),
        ]);
    }

    #[test]
    fn input_order_has_no_effect_and_adjacent_permissions_stay_separate() {
        let requests = [
            request(0x400000, 1, read_execute),
            request(0x401000, 1, read_write),
            request(1 << 39, 1, read_only),
        ];
        let reservations = [reserve(0, 1), reserve(0x100000, 2)];
        let first = mapping_plan::new(&requests, &reservations, generous).unwrap();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let shuffled = order.map(|i| requests[i]);
            let reversed = [reservations[1], reservations[0]];
            assert_eq!(
                first,
                mapping_plan::new(&shuffled, &reversed, generous).unwrap()
            );
        }
        assert_eq!(first.regions().count(), 3);
        assert_eq!(first.tables().len(), 7);
    }

    #[test]
    fn executable_membership_does_not_include_gaps_or_other_permissions() {
        let requests = [
            request(0x400000, 1, read_execute),
            request(0x402000, 1, read_write),
            request(0x405000, 2, read_execute),
        ];
        let plan = mapping_plan::new(&requests, &[], generous).unwrap();
        for address in [0x400000, 0x400fff, 0x405000, 0x406fff] {
            assert!(plan.contains_executable_address(address));
        }
        for address in [
            0,
            0x3fffff,
            0x401000,
            0x402000,
            0x404fff,
            0x407000,
            user_address_limit,
            u64::MAX,
        ] {
            assert!(!plan.contains_executable_address(address));
        }
        let data = mapping_plan::new(&[request(0x400000, 1, read_write)], &[], generous).unwrap();
        assert!(!data.contains_executable_address(0x400000));
    }

    #[test]
    fn canonical_last_page_is_valid_but_zero_hole_wrap_and_misalignment_are_not() {
        let last = request(user_address_limit - page_size, 1, read_only);
        let plan = mapping_plan::new(&[last], &[], generous).unwrap();
        assert_eq!(
            plan.regions().next().unwrap().extent().end(),
            user_address_limit
        );
        for (address, pages) in [
            (0, 1),
            (1, 1),
            (page_size, 0),
            (page_size, u64::MAX),
            (user_address_limit - page_size, 2),
            (user_address_limit, 1),
            (0xffff800000000000, 1),
            (u64::MAX - page_size + 1, 1),
        ] {
            assert_eq!(
                mapping_plan::new(&[request(address, pages, read_only)], &[], generous),
                Err(plan_error::invalid_user_extent)
            );
        }
    }

    #[test]
    fn all_reservations_are_validated_and_their_union_excludes_user_pages() {
        let valid = [request(0x400000, 1, read_only)];
        for invalid in [
            reserve(0, 0),
            reserve(1, 1),
            reserve(0, u64::MAX),
            reserve(user_address_limit, 1),
        ] {
            assert_eq!(
                mapping_plan::new(&valid, &[invalid], generous),
                Err(plan_error::invalid_reservation)
            );
        }
        let overlapping = [reserve(0, 1), reserve(0x100000, 3), reserve(0x101000, 3)];
        assert!(mapping_plan::new(&valid, &overlapping, generous).is_ok());
        for base in [0x100000, 0x102000, 0x103000] {
            assert_eq!(
                mapping_plan::new(&[request(base, 1, read_only)], &overlapping, generous),
                Err(plan_error::reserved_collision)
            );
        }
        // Express the current bootstrap exclusion explicitly. A low geometry
        // plan never silently removes that live kernel restriction.
        assert_eq!(
            mapping_plan::new(&valid, &[reserve(0, (1 << 39) / page_size)], generous),
            Err(plan_error::reserved_collision)
        );
        assert!(
            mapping_plan::new(&[request(0x104000, 1, read_only)], &overlapping, generous).is_ok()
        );
    }

    #[test]
    fn rejects_user_overlap_and_privilege_or_write_execute_escalation() {
        let first = request(0x400000, 2, read_execute);
        for second in [
            request(0x400000, 1, read_only),
            request(0x401000, 2, read_write),
        ] {
            assert_eq!(
                mapping_plan::new(&[first, second], &[], generous),
                Err(plan_error::overlap)
            );
            assert_eq!(
                mapping_plan::new(&[second, first], &[], generous),
                Err(plan_error::overlap)
            );
        }
        assert_eq!(
            mapping_plan::new(
                &[request(
                    0x400000,
                    1,
                    page_permissions::supervisor_read_write
                )],
                &[],
                generous
            ),
            Err(plan_error::supervisor_permission)
        );
        let wx = page_permissions {
            executable: true,
            ..read_write
        };
        assert_eq!(
            mapping_plan::new(&[request(0x400000, 1, wx)], &[], generous),
            Err(plan_error::write_execute)
        );
    }

    #[test]
    fn descriptor_capacities_and_empty_input_are_explicit() {
        let requests = core::array::from_fn::<_, { region_capacity + 1 }, _>(|i| {
            request(0x400000 + i as u64 * page_size, 1, read_only)
        });
        let reservations = core::array::from_fn::<_, 9, _>(|i| reserve(i as u64 * page_size, 1));
        assert!(
            mapping_plan::new(&requests[..region_capacity], &reservations[..8], generous).is_ok()
        );
        assert_eq!(
            mapping_plan::new(&requests, &[], generous),
            Err(plan_error::region_capacity_exhausted)
        );
        assert_eq!(
            mapping_plan::new(&requests[..1], &reservations, generous),
            Err(plan_error::reservation_capacity_exhausted)
        );
        assert_eq!(
            mapping_plan::new(&[], &[], generous),
            Err(plan_error::empty)
        );
        assert_eq!(
            mapping_plan::new(&[], &reservations[..1], generous),
            Err(plan_error::empty)
        );
    }

    #[test]
    fn data_and_table_budgets_are_independent_and_exact() {
        let request = request(0x1ff000, 2, read_only);
        let exact = mapping_budget {
            data_pages: 2,
            table_frames: 5,
        };
        let plan = mapping_plan::new(&[request], &[], exact).unwrap();
        assert_eq!(plan.data_pages(), 2);
        assert_eq!(plan.tables().len(), 5);
        assert_eq!(
            mapping_plan::new(
                &[request],
                &[],
                mapping_budget {
                    data_pages: 1,
                    ..exact
                }
            ),
            Err(plan_error::data_budget_exhausted)
        );
        assert_eq!(
            mapping_plan::new(
                &[request],
                &[],
                mapping_budget {
                    table_frames: 4,
                    ..exact
                }
            ),
            Err(plan_error::table_budget_exhausted)
        );
        for invalid in [
            mapping_budget {
                data_pages: 0,
                ..exact
            },
            mapping_budget {
                table_frames: 0,
                ..exact
            },
            mapping_budget {
                table_frames: 33,
                ..exact
            },
        ] {
            assert_eq!(
                mapping_plan::new(&[request], &[], invalid),
                Err(plan_error::invalid_budget)
            );
        }
    }

    #[test]
    fn table_capacity_bounds_work_even_for_attacker_sized_ranges() {
        // A contiguous range uses one root, one PDPT, one PD, and 29 PTs.
        let exact = request(1 << 21, 29 * 512, read_only);
        let plan = mapping_plan::new(&[exact], &[], generous).unwrap();
        assert_eq!(plan.tables().len(), table_frame_capacity);
        assert_eq!(
            mapping_plan::new(
                &[mapping_request {
                    page_count: exact.page_count + 1,
                    ..exact
                }],
                &[],
                generous
            ),
            Err(plan_error::table_budget_exhausted)
        );
        let huge = request(page_size, (user_address_limit / page_size) - 1, read_only);
        assert_eq!(
            mapping_plan::new(
                &[huge],
                &[],
                mapping_budget {
                    data_pages: 1,
                    ..generous
                }
            ),
            Err(plan_error::data_budget_exhausted)
        );
        assert_eq!(
            mapping_plan::new(&[huge], &[], generous),
            Err(plan_error::table_budget_exhausted)
        );
    }

    #[test]
    fn failed_replanning_preserves_inputs_and_previous_plan() {
        let requests = [request(0x400000, 1, read_execute)];
        let reservations = [reserve(0, 1)];
        let original = mapping_plan::new(&requests, &reservations, generous).unwrap();
        let saved_requests = requests;
        let saved_reservations = reservations;
        assert!(
            mapping_plan::new(
                &requests,
                &reservations,
                mapping_budget {
                    table_frames: 1,
                    ..generous
                }
            )
            .is_err()
        );
        assert_eq!(requests, saved_requests);
        assert_eq!(reservations, saved_reservations);
        assert_eq!(
            original,
            mapping_plan::new(&requests, &reservations, generous).unwrap()
        );
    }
}
