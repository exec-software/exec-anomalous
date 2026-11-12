//! The geometry an arena-bearing launch is measured against (ADR 0094).
//!
//! Deliberately pure: a profile kind, the user geometry constants, and the
//! mapping plan a launch is admitted by. It owns no mappings, borrows no
//! allocator and reaches no privileged state, so it compiles for the host
//! tests that `owned_execution` is excluded from -- which is the only way the
//! golden tests below can run at all.

// The geometry constants need only the address limit and the page size; the
// plan builder's types belong to the gated paths that construct one.
use super::mapping_plan::user_address_limit;
#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
use super::mapping_plan::{mapping_budget, mapping_plan, mapping_request, reserved_request};
#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
use super::paging::page_permissions;
use super::paging::page_size;

pub(super) const user_code_base: u64 = 0x400000;
pub(super) const user_data_base: u64 = 0x800000;
pub(super) const user_stack_top: u64 = user_address_limit - 2 * page_size;
pub(super) const user_stack_base: u64 = user_stack_top - 2 * page_size;

#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
/// The two addresses a native application arena may sit at.
///
/// ADR 0094 keeps these as fixed constants rather than validated values, so
/// this set is pinned by the golden tests and cannot widen with a caller.
pub(super) const native_arena_bases: [u64; 2] = [0x804000, 0x814000];

/// The two addresses a program arena may sit at.
///
/// Deliberately not the native application's pair: those are 64 KiB apart, so
/// a 64 KiB arena at the first would run into the second with no guard page
/// between them. These leave a guard either side of both.
#[cfg(any(test, feature = "native_program_test"))]
pub(super) const program_arena_bases: [u64; 2] = [0x804000, 0x818000];

#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
/// A kind, not a size.
///
/// The controller chooses it before preparation and it is retained with the
/// prepared owner, so a task's own writable launch bytes cannot select a
/// profile or widen an authority: launch data describes a launch, it never
/// chooses the terms of one.
// A program build selects only the program kind, but the native application's
// kind must still exist: it is the geometry the golden tests below pin, and
// deleting it under a feature would delete the comparison that keeps the two
// profiles from drifting into each other.
// Unconstructed in two cases, and dead only in the compiler's sense in both.
// A program build selects the program kind, so `prepare_native_arenas` is
// itself unused there and its body does not count as a use. And the
// controller is compiled at all only under the condition mirrored here from
// `owned_execution::prepare_native_arenas`; feature combinations outside it
// -- `native_durable_failure_test,elf_application_test`, for one -- have no
// controller that names this profile. Neither case means the kind may be
// deleted: its geometry is what the golden tests below pin, and dropping it
// under a feature would drop the comparison that keeps the two profiles from
// drifting into each other.
#[cfg_attr(
    any(
        feature = "native_program_test",
        not(all(
            feature = "native_runtime_test",
            any(
                not(feature = "elf_application_test"),
                all(
                    any(feature = "elf_document_test", feature = "native_relaunch_test"),
                    not(feature = "linux_application_test")
                )
            )
        ))
    ),
    allow(dead_code)
)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum arena_profile {
    /// A bounded native application: eight RX code pages and a fixed 32 KiB
    /// arena.
    native_application,
    /// A program-bearing task: two RX code pages and a 64 KiB arena.
    /// ADR 0101 admits the measured 5,605-byte edition-delivery client; this
    /// bounded increment does not select an interpreter's eventual geometry.
    ///
    /// Gated so a build without the feature cannot name this profile at all.
    /// Leaving it unconditional would let a `native_runtime_test` or
    /// `mixed_workspace_test` build construct a program `prepared_launch` and
    /// have its validator accept 64 KiB geometry, which is wider than ADR
    /// 0094 states even while no controller selects it.
    #[cfg(any(test, feature = "native_program_test"))]
    program,
}

#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
impl arena_profile {
    /// RX pages for the named task profile. Two pages require the retained
    /// program selection; the generic one/eight-page gate remains unchanged.
    pub(super) const fn code_pages(self) -> u64 {
        match self {
            Self::native_application => 8,
            #[cfg(any(test, feature = "native_program_test"))]
            Self::program => 2,
        }
    }

    /// RW pages for the arena itself.
    pub(super) const fn arena_pages(self) -> u64 {
        match self {
            Self::native_application => 8,
            #[cfg(any(test, feature = "native_program_test"))]
            Self::program => 16,
        }
    }

    pub(super) const fn arena_bytes(self) -> u64 {
        self.arena_pages() * page_size
    }

    /// Is this an address this profile will map an arena at?
    ///
    /// Each profile answers only for its own bases, so admitting a program
    /// base can never widen the native application's set.
    pub(super) fn admits_base(self, base: u64) -> bool {
        match self {
            Self::native_application => native_arena_bases.contains(&base),
            #[cfg(any(test, feature = "native_program_test"))]
            Self::program => program_arena_bases.contains(&base),
        }
    }

    /// Is the geometry itself coherent, before any task is measured against it?
    ///
    /// Checked first and separately so an incoherent profile is reported as
    /// the profile's fault rather than the task's, and so nothing is acquired
    /// on behalf of a geometry that does not hold together.
    pub(super) fn is_coherent(self, base: u64) -> bool {
        let (code, arena) = (self.code_pages(), self.arena_pages());
        if code == 0 || arena == 0 || base == 0 || !base.is_multiple_of(page_size) {
            return false;
        }
        let Some(span) = arena.checked_mul(page_size) else {
            return false;
        };
        let (Some(end), Some(guard_below)) = (base.checked_add(span), base.checked_sub(page_size))
        else {
            return false;
        };
        let Some(guard_above) = end.checked_add(page_size) else {
            return false;
        };
        let Some(code_end) = code
            .checked_mul(page_size)
            .and_then(|span| user_code_base.checked_add(span))
        else {
            return false;
        };
        guard_above <= user_stack_base - page_size
            && guard_below >= user_data_base + 2 * page_size
            && code_end <= user_data_base
            && end <= user_address_limit
    }
}

#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
/// The mapping plan an arena-bearing launch is admitted by.
///
/// Pure and allocation-free, and separated from staging for two reasons: it
/// runs before anything is acquired, so an incoherent geometry costs nothing;
/// and it is the thing the golden tests pin, so geometry can be asserted
/// without standing up an access group.
pub(super) fn arena_mapping_plan(
    arena_base: u64,
    profile: arena_profile,
) -> Result<mapping_plan, &'static str> {
    if !profile.admits_base(arena_base) || !profile.is_coherent(arena_base) {
        return Err("owned_arena_profile");
    }
    let code_pages = profile.code_pages();
    let arena_pages = profile.arena_pages();
    let requests = [
        (user_code_base, code_pages, false, true),
        (user_data_base, 2, true, false),
        (arena_base, arena_pages, true, false),
        (user_stack_base, 2, true, false),
    ]
    .map(
        |(virtual_address, page_count, writable, executable)| mapping_request {
            virtual_address,
            page_count,
            permissions: page_permissions {
                user: true,
                writable,
                executable,
            },
        },
    );
    let reserved = [
        (0, 512),
        (arena_base - page_size, 1),
        (arena_base + arena_pages * page_size, 1),
        (user_stack_base - page_size, 1),
        (user_stack_top, 1),
    ]
    .map(|(virtual_address, page_count)| reserved_request {
        virtual_address,
        page_count,
    });
    // The data budget is the sum of what is actually requested rather than a
    // literal, so a geometry change cannot quietly exceed a stale number. It
    // reproduces the native application's long-standing 20 exactly.
    let data_pages = code_pages + 2 + arena_pages + 2;
    mapping_plan::new(
        &requests,
        &reserved,
        mapping_budget {
            data_pages,
            table_frames: 8,
        },
    )
    .map_err(|_| "owned_arena_plan")
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::{vec, vec::Vec};

    /// Every value here is restated independently of `arena_profile`.
    ///
    /// Deliberately not a comparison against the profile the production path
    /// delegates to: that would prove only that the two agree, and they can
    /// drift together while it passes. The ELF boundary's
    /// `parse == parse_for(anel_v1)` shape is insufficient for the same
    /// reason. These are the numbers the native application has always been
    /// admitted by, written out so a profile that narrowed or widened fails.
    const golden_native_bases: [u64; 2] = [0x804000, 0x814000];
    const golden_native_arena_pages: u64 = 8;
    const golden_native_code_pages: u64 = 8;
    const golden_native_data_pages: u64 = 20;
    const golden_program_data_pages: u64 = 22;
    const golden_mappings: usize = 4;
    const golden_reservations: usize = 5;

    /// Leaf-table frames the planner actually requires, derived from its own
    /// algorithm rather than inferred from a page count.
    ///
    /// `mapping_plan::new` seeds the root, then for each 2 MiB branch a region
    /// spans adds a directory-pointer, a directory and a leaf requirement,
    /// deduplicated. Every admitted geometry here touches exactly three
    /// branches -- code at `0x400000 >> 21 == 2`, data and arena both at
    /// `0x800000 >> 21 == 4`, and the stack at `(1 << 47) - 4 * page >> 21`.
    /// Branches 2 and 4 share a directory-pointer and a directory, so the
    /// total is root + 3 + 1 + 3.
    ///
    /// This is exactly `table_frames`, with no headroom. That is deliberate
    /// and fail-closed: a future geometry crossing one more 2 MiB boundary is
    /// refused by `table_budget_exhausted` rather than quietly mapping.
    const golden_plan_tables: usize = 8;

    fn plan_of(base: u64, profile: arena_profile) -> mapping_plan {
        arena_mapping_plan(base, profile).expect("an admitted base plans")
    }

    fn geometry(
        base: u64,
        profile: arena_profile,
    ) -> (Vec<(u64, u64, bool, bool)>, Vec<(u64, u64)>) {
        let plan = plan_of(base, profile);
        let regions = plan
            .regions()
            .map(|region| {
                (
                    region.extent().start(),
                    (region.extent().end() - region.extent().start()) / page_size,
                    region.permissions().writable,
                    region.permissions().executable,
                )
            })
            .collect::<Vec<_>>();
        let reserved = plan
            .reservations()
            .map(|extent| (extent.start(), (extent.end() - extent.start()) / page_size))
            .collect::<Vec<_>>();
        (regions, reserved)
    }

    #[test]
    fn the_native_application_keeps_the_geometry_it_always_had() {
        for base in golden_native_bases {
            let (regions, reserved) = geometry(base, arena_profile::native_application);
            assert_eq!(regions.len(), golden_mappings);
            assert_eq!(reserved.len(), golden_reservations);
            assert_eq!(
                regions,
                vec![
                    (user_code_base, golden_native_code_pages, false, true),
                    (user_data_base, 2, true, false),
                    (base, golden_native_arena_pages, true, false),
                    (user_stack_base, 2, true, false),
                ]
            );
            assert_eq!(
                reserved,
                vec![
                    (0, 512),
                    (base - page_size, 1),
                    (base + golden_native_arena_pages * page_size, 1),
                    (user_stack_base - page_size, 1),
                    (user_stack_top, 1),
                ]
            );
            let pages: u64 = regions.iter().map(|(_, count, _, _)| *count).sum();
            assert_eq!(pages, golden_native_data_pages);
            // W^X, per region, is the property that must never slip.
            for (_, _, writable, executable) in &regions {
                assert!(!(*writable && *executable));
            }
            assert_eq!(
                plan_of(base, arena_profile::native_application)
                    .tables()
                    .len(),
                golden_plan_tables
            );
        }
    }

    #[test]
    fn no_other_base_is_admitted_for_a_native_application() {
        // Preserving an accepted set means rejecting everything outside it,
        // including every near miss and the program profile's second base.
        for base in [
            0,
            page_size,
            0x803000,
            0x805000,
            0x804001,
            0x813000,
            0x815000,
            0x818000,
            user_data_base,
            user_stack_base,
            user_stack_top,
            user_address_limit,
            u64::MAX,
        ] {
            assert!(
                arena_mapping_plan(base, arena_profile::native_application).is_err(),
                "{base:#x} must not be admitted"
            );
        }
    }

    #[test]
    fn a_profile_answers_only_for_its_own_bases() {
        for base in golden_native_bases {
            assert!(arena_profile::native_application.admits_base(base));
        }
        assert!(!arena_profile::native_application.admits_base(0x818000));
        assert!(!arena_profile::program.admits_base(0x814000));
        assert_eq!(arena_profile::native_application.arena_bytes(), 32 * 1024);
    }

    #[test]
    fn a_program_arena_is_sixty_four_kibibytes_and_guarded() {
        assert_eq!(arena_profile::program.arena_bytes(), 64 * 1024);
        assert_eq!(arena_profile::program.code_pages(), 2);
        for base in program_arena_bases {
            let (regions, reserved) = geometry(base, arena_profile::program);
            assert_eq!(
                regions,
                vec![
                    (user_code_base, 2, false, true),
                    (user_data_base, 2, true, false),
                    (base, 16, true, false),
                    (user_stack_base, 2, true, false),
                ]
            );
            let pages: u64 = regions.iter().map(|(_, count, _, _)| *count).sum();
            assert_eq!(pages, golden_program_data_pages);
            // The guard above the arena moves with its size.
            assert_eq!(reserved[2], (base + 64 * 1024, 1));
            assert_eq!(reserved[1], (base - page_size, 1));
            // A 64 KiB arena costs no extra leaf table, because it stays
            // inside the one 2 MiB branch the data region already needs. That
            // is why the budget still holds, and it is asserted rather than
            // assumed.
            assert_eq!(base >> 21, (base + 64 * 1024 - 1) >> 21);
            assert_eq!(base >> 21, user_data_base >> 21);
            assert_eq!(
                plan_of(base, arena_profile::program).tables().len(),
                golden_plan_tables
            );
        }
    }

    #[test]
    fn the_two_program_arenas_cannot_run_into_each_other() {
        // The native application's pair is 64 KiB apart, so a 64 KiB arena at
        // the first would reach the second with no guard between them. The
        // program bases leave room either side of both.
        let [first, second] = program_arena_bases;
        assert!(first + arena_profile::program.arena_bytes() + page_size <= second - page_size);
        for base in program_arena_bases {
            assert!(arena_profile::program.is_coherent(base));
        }
    }

    #[test]
    fn an_incoherent_geometry_is_the_profiles_fault_not_a_tasks() {
        // Refused before anything is acquired, which is why the check is
        // separate from staging.
        for base in [0, 0x804001, u64::MAX, user_address_limit] {
            assert!(!arena_profile::program.is_coherent(base));
        }
    }
}
