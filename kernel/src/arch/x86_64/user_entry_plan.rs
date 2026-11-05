#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Checked code and stack geometry for a bounded user entry.
//!
//! This metadata owns no mappings and establishes neither initialized contents
//! nor authority to enter a task. Its caller must separately prove that the
//! declared payload was copied, the mappings are installed, and every kernel
//! return path and transition stack remains resident.

use super::mapping_plan::mapping_plan;
use super::paging::page_size;
#[cfg(any(test, feature = "owned_multi_region_test"))]
use crate::task::executable_interval_set;
use crate::task::task_bounds;

const initial_stack_headroom: u64 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum user_entry_error {
    missing_region,
    same_region,
    code_permissions,
    stack_permissions,
    initialized_code,
    entry_outside_payload,
    initial_stack,
    missing_stack_guard,
    invalid_bounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct user_entry_plan {
    bounds: task_bounds,
}

impl user_entry_plan {
    /// Region indexes refer to the mapping plan's canonical address order.
    /// `initialized_code_bytes` declares a prefix beginning at the code region
    /// base; validation does not read or initialize those bytes.
    pub(super) fn new(
        plan: &mapping_plan,
        code_region: usize,
        initialized_code_bytes: u64,
        entry: u64,
        stack_region: usize,
        initial_rsp: u64,
    ) -> Result<Self, user_entry_error> {
        if code_region == stack_region {
            return Err(user_entry_error::same_region);
        }
        let code = plan
            .regions()
            .nth(code_region)
            .ok_or(user_entry_error::missing_region)?;
        let stack = plan
            .regions()
            .nth(stack_region)
            .ok_or(user_entry_error::missing_region)?;
        let code_permissions = code.permissions();
        if !code_permissions.user || !code_permissions.executable || code_permissions.writable {
            return Err(user_entry_error::code_permissions);
        }
        let stack_permissions = stack.permissions();
        if !stack_permissions.user || !stack_permissions.writable || stack_permissions.executable {
            return Err(user_entry_error::stack_permissions);
        }
        let code = code.extent();
        let stack = stack.extent();
        let code_end = code
            .start()
            .checked_add(initialized_code_bytes)
            .filter(|end| initialized_code_bytes != 0 && *end <= code.end())
            .ok_or(user_entry_error::initialized_code)?;
        if entry < code.start() || entry >= code_end {
            return Err(user_entry_error::entry_outside_payload);
        }
        if !initial_rsp.is_multiple_of(16)
            || initial_rsp > stack.end()
            || initial_rsp
                .checked_sub(stack.start())
                .is_none_or(|headroom| headroom < initial_stack_headroom)
        {
            return Err(user_entry_error::initial_stack);
        }
        let lower_guard = stack
            .start()
            .checked_sub(page_size)
            .ok_or(user_entry_error::missing_stack_guard)?;
        let upper_guard_end = stack
            .end()
            .checked_add(page_size)
            .ok_or(user_entry_error::missing_stack_guard)?;
        if !reserved(plan, lower_guard, stack.start())
            || !reserved(plan, stack.end(), upper_guard_end)
        {
            return Err(user_entry_error::missing_stack_guard);
        }
        let bounds = task_bounds::new(
            code.start(),
            code_end,
            entry,
            stack.start(),
            stack.end(),
            initial_rsp,
        )
        .ok_or(user_entry_error::invalid_bounds)?;
        Ok(Self { bounds })
    }

    /// Admit an exact set of declared initialized executable intervals. Each
    /// interval must fit wholly inside one user RX mapping; neither gaps nor
    /// surrounding executable page slack becomes an admitted return address.
    /// The selected stack is the complete canonical region, with the same
    /// guard, headroom and initial alignment requirements as `new`.
    ///
    /// This metadata still proves no copying, mapping ownership or authority.
    #[cfg(any(test, feature = "owned_multi_region_test"))]
    pub(super) fn with_executable_intervals(
        plan: &mapping_plan,
        intervals: executable_interval_set,
        entry: u64,
        stack_region: usize,
        initial_rsp: u64,
    ) -> Result<Self, user_entry_error> {
        for interval in intervals.intervals() {
            let region = plan
                .regions()
                .find(|region| {
                    region.extent().start() <= interval.start
                        && interval.end <= region.extent().end()
                })
                .ok_or(user_entry_error::initialized_code)?;
            let permissions = region.permissions();
            if !permissions.user || !permissions.executable || permissions.writable {
                return Err(user_entry_error::code_permissions);
            }
        }
        if !intervals.contains(entry) {
            return Err(user_entry_error::entry_outside_payload);
        }
        let stack = plan
            .regions()
            .nth(stack_region)
            .ok_or(user_entry_error::missing_region)?;
        let permissions = stack.permissions();
        if !permissions.user || !permissions.writable || permissions.executable {
            return Err(user_entry_error::stack_permissions);
        }
        let stack = stack.extent();
        if !initial_rsp.is_multiple_of(16)
            || initial_rsp > stack.end()
            || initial_rsp
                .checked_sub(stack.start())
                .is_none_or(|headroom| headroom < initial_stack_headroom)
        {
            return Err(user_entry_error::initial_stack);
        }
        let lower_guard = stack
            .start()
            .checked_sub(page_size)
            .ok_or(user_entry_error::missing_stack_guard)?;
        let upper_guard_end = stack
            .end()
            .checked_add(page_size)
            .ok_or(user_entry_error::missing_stack_guard)?;
        if !reserved(plan, lower_guard, stack.start())
            || !reserved(plan, stack.end(), upper_guard_end)
        {
            return Err(user_entry_error::missing_stack_guard);
        }
        let bounds = task_bounds::with_executable_intervals(
            intervals,
            entry,
            stack.start(),
            stack.end(),
            initial_rsp,
        )
        .ok_or(user_entry_error::invalid_bounds)?;
        Ok(Self { bounds })
    }

    pub(super) const fn bounds(self) -> task_bounds {
        self.bounds
    }

    /// Check only the admitted initialized RX interval(s) and writable stack
    /// interval. A caller still validates the live root, identity, selectors,
    /// flags, complete saved frame, and any feature-specific exact return site.
    pub(super) const fn accepts_return(self, rip: u64, rsp: u64) -> bool {
        self.bounds.contains_code(rip)
            && self.bounds.contains_stack_pointer(rsp)
            && rsp.is_multiple_of(8)
    }
}

fn reserved(plan: &mapping_plan, start: u64, end: u64) -> bool {
    // Both guard boundaries and every reservation are page-aligned; covering
    // this single page therefore requires one reservation spanning it whole.
    plan.reservations()
        .any(|reservation| reservation.start() <= start && reservation.end() >= end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::mapping_plan::{
        mapping_budget, mapping_request, reserved_request, user_address_limit,
    };
    use crate::arch::x86_64::paging::page_permissions;
    use crate::task::executable_interval;

    const code_start: u64 = 0x400000;
    const other_code_start: u64 = 0x800000;
    const stack_start: u64 = 0x1000000;
    const stack_end: u64 = stack_start + 2 * page_size;
    const payload_bytes: u64 = 257;
    const rx: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: true,
    };
    const rw: page_permissions = page_permissions {
        user: true,
        writable: true,
        executable: false,
    };
    const ro: page_permissions = page_permissions {
        user: true,
        writable: false,
        executable: false,
    };

    fn guards() -> [reserved_request; 2] {
        [
            reserved_request {
                virtual_address: stack_start - page_size,
                page_count: 1,
            },
            reserved_request {
                virtual_address: stack_end,
                page_count: 1,
            },
        ]
    }

    fn fixture(
        code_permissions: page_permissions,
        stack_permissions: page_permissions,
        reservations: &[reserved_request],
    ) -> mapping_plan {
        // Deliberately unordered: callers select canonical region indexes.
        mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: stack_start,
                    page_count: 2,
                    permissions: stack_permissions,
                },
                mapping_request {
                    virtual_address: other_code_start,
                    page_count: 1,
                    permissions: rx,
                },
                mapping_request {
                    virtual_address: code_start,
                    page_count: 2,
                    permissions: code_permissions,
                },
            ],
            reservations,
            mapping_budget {
                data_pages: 5,
                table_frames: 32,
            },
        )
        .expect("valid mapping geometry")
    }

    fn admit(
        plan: &mapping_plan,
        entry: u64,
        rsp: u64,
    ) -> Result<user_entry_plan, user_entry_error> {
        user_entry_plan::new(plan, 0, payload_bytes, entry, 2, rsp)
    }

    #[test]
    fn initialized_interval_excludes_rx_tail_gaps_and_other_executable_regions() {
        let plan = fixture(rx, rw, &guards());
        let admitted = admit(&plan, code_start + 3, stack_end).expect("admit payload");
        let bounds = admitted.bounds();
        assert_eq!(bounds.code_start(), code_start);
        assert_eq!(bounds.code_end(), code_start + payload_bytes);
        assert_eq!(bounds.entry(), code_start + 3);
        assert!(admitted.accepts_return(code_start + payload_bytes - 1, stack_end));
        for rip in [
            code_start - 1,
            code_start + payload_bytes,
            code_start + page_size,
            code_start + 2 * page_size,
            other_code_start,
        ] {
            assert!(!admitted.accepts_return(rip, stack_end));
            assert_eq!(
                admit(&plan, rip, stack_end),
                Err(user_entry_error::entry_outside_payload)
            );
        }
        assert!(plan.contains_executable_address(other_code_start));
        assert!(plan.contains_executable_address(code_start + page_size));
    }

    #[test]
    fn declared_payload_must_be_nonempty_bounded_and_without_overflow() {
        let plan = fixture(rx, rw, &guards());
        for length in [0, 2 * page_size + 1, u64::MAX] {
            assert_eq!(
                user_entry_plan::new(&plan, 0, length, code_start, 2, stack_end),
                Err(user_entry_error::initialized_code)
            );
        }
        assert!(user_entry_plan::new(&plan, 0, 2 * page_size, code_start, 2, stack_end).is_ok());
    }

    #[test]
    fn admission_rejects_missing_same_and_wrong_permission_regions() {
        let plan = fixture(rx, rw, &guards());
        assert_eq!(
            user_entry_plan::new(&plan, 0, payload_bytes, code_start, 0, stack_end),
            Err(user_entry_error::same_region)
        );
        for (code, stack) in [(3, 2), (0, 3), (usize::MAX, 2), (0, usize::MAX)] {
            assert_eq!(
                user_entry_plan::new(&plan, code, payload_bytes, code_start, stack, stack_end),
                Err(user_entry_error::missing_region)
            );
        }
        for permissions in [ro, rw] {
            assert_eq!(
                admit(&fixture(permissions, rw, &guards()), code_start, stack_end),
                Err(user_entry_error::code_permissions)
            );
        }
        for permissions in [ro, rx] {
            assert_eq!(
                admit(&fixture(rx, permissions, &guards()), code_start, stack_end),
                Err(user_entry_error::stack_permissions)
            );
        }
    }

    #[test]
    fn initial_stack_requires_alignment_extent_and_headroom() {
        let plan = fixture(rx, rw, &guards());
        for rsp in [
            0,
            stack_start - 16,
            stack_start,
            stack_start + initial_stack_headroom - 16,
            stack_start + initial_stack_headroom + 8,
            stack_end + 16,
            u64::MAX,
        ] {
            assert_eq!(
                admit(&plan, code_start, rsp),
                Err(user_entry_error::initial_stack)
            );
        }
        let rsp = stack_start + initial_stack_headroom;
        assert_eq!(
            admit(&plan, code_start, rsp)
                .expect("minimum headroom")
                .bounds()
                .initial_stack_pointer(),
            rsp
        );
    }

    #[test]
    fn resumed_stack_accepts_nested_calls_without_loosening_its_extent() {
        let plan = fixture(rx, rw, &guards());
        let admitted = admit(&plan, code_start, stack_end).expect("admit entry");
        for rsp in [stack_end, stack_end - 8, stack_end - 24, stack_start + 8] {
            assert!(admitted.accepts_return(code_start + 16, rsp));
        }
        for rsp in [
            stack_start - 8,
            stack_start,
            stack_start + 1,
            stack_end - 1,
            stack_end + 8,
            u64::MAX,
        ] {
            assert!(!admitted.accepts_return(code_start + 16, rsp));
        }
    }

    #[test]
    fn guards_must_reserve_the_immediately_adjacent_complete_pages() {
        let valid = guards();
        for reservations in [&valid[..0], &valid[..1], &valid[1..]] {
            assert_eq!(
                admit(&fixture(rx, rw, reservations), code_start, stack_end),
                Err(user_entry_error::missing_stack_guard)
            );
        }
        for index in 0..2 {
            let mut displaced = valid;
            if index == 0 {
                displaced[index].virtual_address -= page_size;
            } else {
                displaced[index].virtual_address += page_size;
            }
            assert_eq!(
                admit(&fixture(rx, rw, &displaced), code_start, stack_end),
                Err(user_entry_error::missing_stack_guard)
            );
        }
        let encompassing = [
            reserved_request {
                virtual_address: stack_start - 3 * page_size,
                page_count: 3,
            },
            reserved_request {
                virtual_address: stack_end,
                page_count: 2,
            },
        ];
        assert!(admit(&fixture(rx, rw, &encompassing), code_start, stack_end).is_ok());
    }

    #[test]
    fn stack_at_user_ceiling_cannot_claim_an_unrepresentable_upper_guard() {
        let start = user_address_limit - page_size;
        let plan = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: code_start,
                    page_count: 1,
                    permissions: rx,
                },
                mapping_request {
                    virtual_address: start,
                    page_count: 1,
                    permissions: rw,
                },
            ],
            &[reserved_request {
                virtual_address: start - page_size,
                page_count: 1,
            }],
            mapping_budget {
                data_pages: 2,
                table_frames: 32,
            },
        )
        .expect("mapping may end at user ceiling");
        assert_eq!(
            user_entry_plan::new(&plan, 0, 1, code_start, 1, user_address_limit - 16),
            Err(user_entry_error::missing_stack_guard)
        );
        let intervals = executable_interval_set::new(&[executable_interval {
            start: code_start,
            end: code_start + 1,
        }])
        .unwrap();
        assert_eq!(
            user_entry_plan::with_executable_intervals(
                &plan,
                intervals,
                code_start,
                1,
                user_address_limit - 16
            ),
            Err(user_entry_error::missing_stack_guard)
        );
    }

    fn multiple_intervals() -> executable_interval_set {
        executable_interval_set::new(&[
            executable_interval {
                start: other_code_start + 19,
                end: other_code_start + 113,
            },
            executable_interval {
                start: code_start + 31,
                end: code_start + payload_bytes,
            },
            executable_interval {
                start: code_start + 512,
                end: code_start + 544,
            },
        ])
        .expect("three exact initialized ranges in two RX mappings")
    }

    #[test]
    fn multi_entry_and_returns_use_exact_bytes_in_both_rx_regions() {
        let plan = fixture(rx, rw, &guards());
        let intervals = multiple_intervals();
        let admitted = user_entry_plan::with_executable_intervals(
            &plan,
            intervals,
            other_code_start + 23,
            2,
            stack_end,
        )
        .unwrap();
        assert_eq!(admitted.bounds().code_start(), other_code_start + 19);
        assert_eq!(admitted.bounds().code_end(), other_code_start + 113);
        assert_eq!(admitted.bounds().executable_intervals(), Some(intervals));
        assert_eq!(admitted.bounds().stack_start(), stack_start);
        assert_eq!(admitted.bounds().stack_end(), stack_end);
        for interval in intervals.intervals() {
            for rip in [interval.start, interval.end - 1] {
                for rsp in [stack_end, stack_end - 8, stack_start + 8] {
                    assert!(admitted.accepts_return(rip, rsp));
                }
                assert!(
                    user_entry_plan::with_executable_intervals(&plan, intervals, rip, 2, stack_end)
                        .is_ok()
                );
            }
        }
        for rip in [
            code_start,
            code_start + 30,
            code_start + payload_bytes,
            code_start + 511,
            code_start + 544,
            code_start + page_size,
            code_start + 2 * page_size,
            other_code_start - 1,
            other_code_start,
            other_code_start + 113,
            stack_start,
            user_address_limit,
            u64::MAX,
        ] {
            assert!(!admitted.accepts_return(rip, stack_end));
            assert_eq!(
                user_entry_plan::with_executable_intervals(&plan, intervals, rip, 2, stack_end),
                Err(user_entry_error::entry_outside_payload)
            );
        }
        for rsp in [
            stack_start,
            stack_start - 8,
            stack_start + 1,
            stack_end - 1,
            stack_end + 8,
            user_address_limit,
            u64::MAX,
        ] {
            assert!(!admitted.accepts_return(code_start + 31, rsp));
        }
        assert!(plan.contains_executable_address(code_start + payload_bytes));
        assert!(plan.contains_executable_address(other_code_start));
    }

    #[test]
    fn multi_admission_validates_every_interval_and_its_complete_rx_coverage() {
        let plan = fixture(rx, rw, &guards());
        let entry = executable_interval {
            start: code_start + 31,
            end: code_start + 47,
        };
        for interval in [
            executable_interval {
                start: other_code_start - 1,
                end: other_code_start + 1,
            },
            executable_interval {
                start: other_code_start,
                end: other_code_start + page_size + 1,
            },
            executable_interval {
                start: code_start + 2 * page_size - 1,
                end: other_code_start + 1,
            },
            executable_interval {
                start: 0x2000000,
                end: 0x2000001,
            },
        ] {
            let intervals = executable_interval_set::new(&[entry, interval]).unwrap();
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &plan,
                    intervals,
                    entry.start,
                    2,
                    stack_end
                ),
                Err(user_entry_error::initialized_code)
            );
        }
        for permissions in [ro, rw] {
            let wrong_code = fixture(permissions, rw, &guards());
            // The entry is in the other valid RX mapping; the complete set must
            // still reject the unrelated interval with incorrect permissions.
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &wrong_code,
                    multiple_intervals(),
                    other_code_start + 23,
                    2,
                    stack_end
                ),
                Err(user_entry_error::code_permissions)
            );
        }
        let stack_as_code = executable_interval_set::new(&[
            entry,
            executable_interval {
                start: stack_start,
                end: stack_start + 1,
            },
        ])
        .unwrap();
        assert_eq!(
            user_entry_plan::with_executable_intervals(
                &plan,
                stack_as_code,
                entry.start,
                2,
                stack_end
            ),
            Err(user_entry_error::code_permissions)
        );
    }

    #[test]
    fn multi_intervals_keep_adjacent_mapping_boundaries_explicit() {
        let plan = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: code_start,
                    page_count: 1,
                    permissions: rx,
                },
                mapping_request {
                    virtual_address: code_start + page_size,
                    page_count: 1,
                    permissions: rx,
                },
                mapping_request {
                    virtual_address: stack_start,
                    page_count: 2,
                    permissions: rw,
                },
            ],
            &guards(),
            mapping_budget {
                data_pages: 4,
                table_frames: 32,
            },
        )
        .unwrap();
        let cross_boundary = executable_interval_set::new(&[executable_interval {
            start: code_start + 17,
            end: code_start + page_size + 19,
        }])
        .unwrap();
        assert_eq!(
            user_entry_plan::with_executable_intervals(
                &plan,
                cross_boundary,
                code_start + 17,
                2,
                stack_end
            ),
            Err(user_entry_error::initialized_code)
        );
        let exact = executable_interval_set::new(&[
            executable_interval {
                start: code_start + 17,
                end: code_start + page_size,
            },
            executable_interval {
                start: code_start + page_size,
                end: code_start + page_size + 19,
            },
        ])
        .unwrap();
        let admitted =
            user_entry_plan::with_executable_intervals(&plan, exact, code_start + 17, 2, stack_end)
                .unwrap();
        assert!(admitted.accepts_return(code_start + page_size - 1, stack_end));
        assert!(admitted.accepts_return(code_start + page_size, stack_end));
        assert!(!admitted.accepts_return(code_start + page_size + 19, stack_end));
    }

    #[test]
    fn multi_stack_uses_complete_selected_region_permissions_and_initial_rsp_rules() {
        let plan = fixture(rx, rw, &guards());
        let intervals = multiple_intervals();
        let entry = code_start + 31;
        for index in [3, usize::MAX] {
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &plan, intervals, entry, index, stack_end
                ),
                Err(user_entry_error::missing_region)
            );
        }
        for index in [0, 1] {
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &plan, intervals, entry, index, stack_end
                ),
                Err(user_entry_error::stack_permissions)
            );
        }
        for permissions in [ro, rx] {
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &fixture(rx, permissions, &guards()),
                    intervals,
                    entry,
                    2,
                    stack_end
                ),
                Err(user_entry_error::stack_permissions)
            );
        }
        for rsp in [
            0,
            stack_start - 16,
            stack_start,
            stack_start + initial_stack_headroom - 16,
            stack_start + initial_stack_headroom + 8,
            stack_end + 16,
            user_address_limit,
            u64::MAX,
        ] {
            assert_eq!(
                user_entry_plan::with_executable_intervals(&plan, intervals, entry, 2, rsp),
                Err(user_entry_error::initial_stack)
            );
        }
        let minimum = user_entry_plan::with_executable_intervals(
            &plan,
            intervals,
            entry,
            2,
            stack_start + initial_stack_headroom,
        )
        .unwrap();
        assert_eq!(minimum.bounds().stack_start(), stack_start);
        assert_eq!(minimum.bounds().stack_end(), stack_end);
    }

    #[test]
    fn multi_stack_requires_both_complete_adjacent_guard_pages() {
        let valid = guards();
        for reservations in [&valid[..0], &valid[..1], &valid[1..]] {
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &fixture(rx, rw, reservations),
                    multiple_intervals(),
                    code_start + 31,
                    2,
                    stack_end
                ),
                Err(user_entry_error::missing_stack_guard)
            );
        }
        for index in 0..2 {
            let mut displaced = valid;
            if index == 0 {
                displaced[index].virtual_address -= page_size;
            } else {
                displaced[index].virtual_address += page_size;
            }
            assert_eq!(
                user_entry_plan::with_executable_intervals(
                    &fixture(rx, rw, &displaced),
                    multiple_intervals(),
                    code_start + 31,
                    2,
                    stack_end
                ),
                Err(user_entry_error::missing_stack_guard)
            );
        }
    }
}
