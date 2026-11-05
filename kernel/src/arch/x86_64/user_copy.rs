#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Inert bounded user-copy geometry and exact live task binding.
//! Plans grant no memory access. A retained mapping owner must check every
//! touched translation and keep its physical alias live for the whole copy.

use super::mapping_plan::{mapping_plan, user_address_limit};
use super::paging::page_size;
use crate::circle::circle_table;
use crate::identity::task_identity;
use crate::scheduler::scheduler_table;
#[cfg(any(test, feature = "veil_application_test"))]
use crate::scheduler::{scheduler_call_authorization, scheduler_context};
use crate::task::{task_bounds, task_state, task_table};

pub(super) const copy_byte_limit: usize = 4096;
pub(super) const copy_span_capacity: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum copy_direction {
    from_user,
    to_user,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum user_copy_error {
    empty,
    too_large,
    address_overflow,
    non_user_address,
    unmapped,
    permission_denied,
    owner_unavailable,
    task_binding,
    task_not_current,
    circle_binding,
    translation_mismatch,
    access_unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct copy_span {
    pub(super) region: usize,
    pub(super) region_offset: u64,
    pub(super) virtual_address: u64,
    pub(super) length: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct copy_plan {
    spans: [copy_span; copy_span_capacity],
    count: usize,
}

impl copy_plan {
    /// Empty requests are rejected, including when their pointer is otherwise
    /// valid. Reads may include RX pages; writes require user writable pages.
    pub(super) fn new(
        mapping: &mapping_plan,
        direction: copy_direction,
        address: u64,
        length: usize,
    ) -> Result<Self, user_copy_error> {
        if length == 0 {
            return Err(user_copy_error::empty);
        }
        if length > copy_byte_limit {
            return Err(user_copy_error::too_large);
        }
        let end = address
            .checked_add(length as u64)
            .ok_or(user_copy_error::address_overflow)?;
        if address >= user_address_limit || end > user_address_limit {
            return Err(user_copy_error::non_user_address);
        }
        let mut result = Self {
            spans: [copy_span {
                region: 0,
                region_offset: 0,
                virtual_address: 0,
                length: 0,
            }; copy_span_capacity],
            count: 0,
        };
        let mut cursor = address;
        while cursor < end {
            let (index, region) = mapping
                .regions()
                .enumerate()
                .find(|(_, region)| {
                    cursor >= region.extent().start() && cursor < region.extent().end()
                })
                .ok_or(user_copy_error::unmapped)?;
            let permissions = region.permissions();
            if !permissions.user || (direction == copy_direction::to_user && !permissions.writable)
            {
                return Err(user_copy_error::permission_denied);
            }
            let page_end = (cursor & !(page_size - 1)) + page_size;
            let next = end.min(region.extent().end()).min(page_end);
            let span = result
                .spans
                .get_mut(result.count)
                .ok_or(user_copy_error::too_large)?;
            *span = copy_span {
                region: index,
                region_offset: cursor - region.extent().start(),
                virtual_address: cursor,
                length: (next - cursor) as usize,
            };
            result.count += 1;
            cursor = next;
        }
        Ok(result)
    }

    pub(super) fn spans(&self) -> &[copy_span] {
        &self.spans[..self.count]
    }
}

/// Validate identities and lifecycle under one caller-owned exclusion interval.
/// No copied identity or inert bounds value is an independently usable grant.
pub(super) fn validate_copy_binding(
    retained: task_identity,
    bounds: task_bounds,
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    validate_profile_copy_binding::<3>(retained, bounds, tasks, circles, scheduler, identity)
}

/// Explicit controller-selected nine-region profile; an observed circle count
/// or user request cannot select this validator.
#[cfg(any(test, feature = "owned_source_test"))]
pub(super) fn validate_multi_region_copy_binding(
    retained: task_identity,
    bounds: task_bounds,
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    validate_profile_copy_binding::<9>(retained, bounds, tasks, circles, scheduler, identity)
}

/// Exact current loader admission after its retained runtime mapping has
/// committed a creation-bound reply. The activation owner selects this path
/// only for the genuine retained caller; an observed ten-child count grants
/// no access by itself.
#[cfg(any(test, feature = "linux_loader_entry_test"))]
pub(super) fn validate_loader_mapping_copy_binding(
    retained: task_identity,
    bounds: task_bounds,
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    validate_profile_copy_binding::<10>(retained, bounds, tasks, circles, scheduler, identity)
}

/// The retained native-arena owner selects this exact four-mapping profile.
/// A user descriptor or an observed circle count must never select it.
#[cfg(any(
    test,
    feature = "native_runtime_test",
    feature = "mixed_workspace_test"
))]
pub(super) fn validate_arena_copy_binding(
    retained: task_identity,
    bounds: task_bounds,
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    validate_profile_copy_binding::<4>(retained, bounds, tasks, circles, scheduler, identity)
}

fn validate_profile_copy_binding<const mapping_count: u16>(
    retained: task_identity,
    bounds: task_bounds,
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    if retained != identity {
        return Err(user_copy_error::task_binding);
    }
    let task = tasks
        .snapshot(identity)
        .map_err(|_| user_copy_error::task_binding)?;
    if task.state() != task_state::running || !task.context_attached() || task.bounds() != bounds {
        return Err(user_copy_error::task_binding);
    }
    let scheduled = scheduler
        .snapshot(tasks)
        .map_err(|_| user_copy_error::task_not_current)?;
    if scheduled.current() != Some(identity) || scheduled.reservation().is_some() {
        return Err(user_copy_error::task_not_current);
    }
    validate_profile_circle_binding::<mapping_count>(circles, identity)
}

fn validate_circle_binding(
    circles: &circle_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    validate_profile_circle_binding::<3>(circles, identity)
}

fn validate_profile_circle_binding<const mapping_count: u16>(
    circles: &circle_table,
    identity: task_identity,
) -> Result<(), user_copy_error> {
    let children = circles
        .snapshot(identity.circle())
        .map_err(|_| user_copy_error::circle_binding)?
        .children();
    if children.tasks() != 1
        || children.task_contexts() != 1
        || children.address_spaces() != 1
        || children.mappings() != mapping_count
        || children.total() != u32::from(mapping_count) + 3
    {
        return Err(user_copy_error::circle_binding);
    }
    Ok(())
}

/// Inert lifecycle check for the one caller bound to a sealed live call. The
/// activation owner must still verify both retained roots, the current veil's
/// privileged stack and the caller's saved frame before any memory access.
#[cfg(any(test, feature = "veil_application_test"))]
pub(super) fn validate_suspended_copy_binding(
    retained_caller: (task_identity, task_bounds),
    retained_veil: (task_identity, task_bounds),
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    authorization: &scheduler_call_authorization,
) -> Result<(), user_copy_error> {
    scheduler
        .validate_call_authorization(tasks, authorization)
        .map_err(|_| user_copy_error::task_binding)?;
    if retained_caller.0 != authorization.caller()
        || retained_veil.0 != authorization.veil()
        || retained_caller.0 == retained_veil.0
    {
        return Err(user_copy_error::task_binding);
    }
    validate_copy_binding(
        retained_veil.0,
        retained_veil.1,
        tasks,
        circles,
        scheduler,
        authorization.veil(),
    )?;
    let caller = tasks
        .snapshot(authorization.caller())
        .map_err(|_| user_copy_error::task_binding)?;
    if caller.state() != task_state::blocked
        || !caller.context_attached()
        || caller.bounds() != retained_caller.1
        || scheduler.context(tasks, authorization.caller()).ok() != Some(scheduler_context::saved)
    {
        return Err(user_copy_error::task_binding);
    }
    validate_circle_binding(circles, authorization.caller())
}

/// Explicit mixed loader admission: the blocked caller owns nine mappings and
/// the currently executing native veil owns three. Only a retained launch kind
/// selects this path; observed circle counts never choose a weaker validator.
#[cfg(any(test, feature = "linux_loader_entry_test"))]
pub(super) fn validate_loader_suspended_copy_binding(
    retained_caller: (task_identity, task_bounds),
    retained_veil: (task_identity, task_bounds),
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    authorization: &scheduler_call_authorization,
) -> Result<(), user_copy_error> {
    validate_loader_profile_suspended_copy_binding::<9>(
        retained_caller,
        retained_veil,
        tasks,
        circles,
        scheduler,
        authorization,
    )
}

/// Explicit loader admission after its genuine retained runtime mapping owner
/// has attached exactly one extra circle child. The blocked caller owns ten
/// mappings and the running veil owns three. Only the retained owner's state
/// selects this validator; counts and untrusted requests cannot select it.
#[cfg(any(test, feature = "linux_loader_entry_test"))]
pub(super) fn validate_loader_mapping_suspended_copy_binding(
    retained_caller: (task_identity, task_bounds),
    retained_veil: (task_identity, task_bounds),
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    authorization: &scheduler_call_authorization,
) -> Result<(), user_copy_error> {
    validate_loader_profile_suspended_copy_binding::<10>(
        retained_caller,
        retained_veil,
        tasks,
        circles,
        scheduler,
        authorization,
    )
}

#[cfg(any(test, feature = "linux_loader_entry_test"))]
fn validate_loader_profile_suspended_copy_binding<const caller_mappings: u16>(
    retained_caller: (task_identity, task_bounds),
    retained_veil: (task_identity, task_bounds),
    tasks: &task_table,
    circles: &circle_table,
    scheduler: &scheduler_table,
    authorization: &scheduler_call_authorization,
) -> Result<(), user_copy_error> {
    scheduler
        .validate_call_authorization(tasks, authorization)
        .map_err(|_| user_copy_error::task_binding)?;
    if retained_caller.0 != authorization.caller()
        || retained_veil.0 != authorization.veil()
        || retained_caller.0 == retained_veil.0
    {
        return Err(user_copy_error::task_binding);
    }
    validate_copy_binding(
        retained_veil.0,
        retained_veil.1,
        tasks,
        circles,
        scheduler,
        authorization.veil(),
    )?;
    let caller = tasks
        .snapshot(authorization.caller())
        .map_err(|_| user_copy_error::task_binding)?;
    if caller.state() != task_state::blocked
        || !caller.context_attached()
        || caller.bounds() != retained_caller.1
        || scheduler.context(tasks, authorization.caller()).ok() != Some(scheduler_context::saved)
    {
        return Err(user_copy_error::task_binding);
    }
    validate_profile_circle_binding::<caller_mappings>(circles, authorization.caller())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::mapping_plan::{mapping_budget, mapping_request, reserved_request};
    use crate::arch::x86_64::paging::page_permissions;
    use crate::circle::circle_child_kind;
    use crate::identity::{fixture_principal_id, task_id};
    use crate::task::{synthetic_origin_metadata, task_binding};

    const code: u64 = 0x400000;
    const data: u64 = 0x800000;
    const stack: u64 = user_address_limit - 4 * page_size;

    fn mapping() -> mapping_plan {
        mapping_plan::new(
            &[
                (code, 1, false, true),
                (data, 2, true, false),
                (stack, 2, true, false),
            ]
            .map(|(virtual_address, page_count, writable, executable)| {
                mapping_request {
                    virtual_address,
                    page_count,
                    permissions: page_permissions {
                        user: true,
                        writable,
                        executable,
                    },
                }
            }),
            &[
                reserved_request {
                    virtual_address: stack - page_size,
                    page_count: 1,
                },
                reserved_request {
                    virtual_address: stack + 2 * page_size,
                    page_count: 1,
                },
            ],
            mapping_budget {
                data_pages: 5,
                table_frames: 8,
            },
        )
        .unwrap()
    }

    #[test]
    fn copy_plan_covers_unaligned_cross_page_data_and_stack_exactly() {
        let mapping = mapping();
        for (region, base) in [(1, data), (2, stack)] {
            for direction in [copy_direction::from_user, copy_direction::to_user] {
                let plan = copy_plan::new(&mapping, direction, base + page_size - 7, 19).unwrap();
                assert_eq!(
                    plan.spans(),
                    &[
                        copy_span {
                            region,
                            region_offset: page_size - 7,
                            virtual_address: base + page_size - 7,
                            length: 7
                        },
                        copy_span {
                            region,
                            region_offset: page_size,
                            virtual_address: base + page_size,
                            length: 12
                        },
                    ]
                );
                assert_eq!(
                    copy_plan::new(&mapping, direction, base, copy_byte_limit)
                        .unwrap()
                        .spans()
                        .len(),
                    1
                );
                assert_eq!(
                    copy_plan::new(&mapping, direction, base + 1, copy_byte_limit)
                        .unwrap()
                        .spans()
                        .len(),
                    2
                );
                assert!(copy_plan::new(&mapping, direction, base + 2 * page_size - 1, 1).is_ok());
            }
        }
        assert!(copy_plan::new(&mapping, copy_direction::from_user, code + 3, 11).is_ok());
        assert_eq!(
            copy_plan::new(&mapping, copy_direction::to_user, code, 1),
            Err(user_copy_error::permission_denied)
        );
    }

    #[test]
    fn copy_plan_rejects_empty_overflow_ceiling_gaps_and_guards_including_invalid_tail() {
        let mapping = mapping();
        for (address, length, expected) in [
            (data, 0, user_copy_error::empty),
            (data, copy_byte_limit + 1, user_copy_error::too_large),
            (u64::MAX - 2, 4, user_copy_error::address_overflow),
            (user_address_limit - 1, 2, user_copy_error::non_user_address),
            (user_address_limit, 1, user_copy_error::non_user_address),
            (0xffff_9000_0000_0000, 1, user_copy_error::non_user_address),
            (0x100000, 1, user_copy_error::unmapped),
            (0, 1, user_copy_error::unmapped),
            (data - 1, 2, user_copy_error::unmapped),
            (data + 2 * page_size - 1, 2, user_copy_error::unmapped),
            (stack - 1, 2, user_copy_error::unmapped),
            (stack + 2 * page_size - 1, 2, user_copy_error::unmapped),
        ] {
            for direction in [copy_direction::from_user, copy_direction::to_user] {
                assert_eq!(
                    copy_plan::new(&mapping, direction, address, length),
                    Err(expected)
                );
            }
        }
    }

    #[test]
    fn copy_plan_checks_permissions_after_crossing_into_a_second_region() {
        let mapping = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: data,
                    page_count: 1,
                    permissions: page_permissions {
                        user: true,
                        writable: true,
                        executable: false,
                    },
                },
                mapping_request {
                    virtual_address: data + page_size,
                    page_count: 1,
                    permissions: page_permissions {
                        user: true,
                        writable: false,
                        executable: true,
                    },
                },
            ],
            &[],
            mapping_budget {
                data_pages: 2,
                table_frames: 4,
            },
        )
        .unwrap();
        let address = data + page_size - 1;
        assert_eq!(
            copy_plan::new(&mapping, copy_direction::to_user, address, 2),
            Err(user_copy_error::permission_denied)
        );
        let plan = copy_plan::new(&mapping, copy_direction::from_user, address, 2).unwrap();
        assert_eq!(plan.spans()[0].region, 0);
        assert_eq!(plan.spans()[1].region, 1);
        assert_eq!(plan.spans()[1].region_offset, 0);
    }

    #[test]
    fn mixed_workspace_frame_arena_is_streamed_in_bounded_copies_with_atomic_guard_rejection() {
        let arena = 0x810000;
        let stack_base = user_address_limit - 18 * page_size;
        let mapping = mapping_plan::new(
            &[
                (code, 32, false, true),
                (data, 8, true, false),
                (arena, 9, true, false),
                (stack_base, 16, true, false),
            ]
            .map(|(virtual_address, page_count, writable, executable)| {
                mapping_request {
                    virtual_address,
                    page_count,
                    permissions: page_permissions {
                        user: true,
                        writable,
                        executable,
                    },
                }
            }),
            &[
                (0, 512),
                (code - page_size, 1),
                (code + 32 * page_size, 1),
                (data - page_size, 1),
                (data + 8 * page_size, 8),
                (arena + 9 * page_size, 1),
                (stack_base - page_size, 1),
                (user_address_limit - 2 * page_size, 1),
            ]
            .map(|(virtual_address, page_count)| reserved_request {
                virtual_address,
                page_count,
            }),
            mapping_budget {
                data_pages: 65,
                table_frames: 8,
            },
        )
        .unwrap();
        for direction in [copy_direction::from_user, copy_direction::to_user] {
            let mut covered = 0;
            // Two exact 18KiB frames share page four, so the second starts at
            // a half-page offset. Every transport operation stays <=4096 bytes.
            for frame in 0..2 {
                let base = arena + frame * 18432;
                for offset in (0..18432).step_by(copy_byte_limit) {
                    let length = (18432 - offset).min(copy_byte_limit);
                    let plan =
                        copy_plan::new(&mapping, direction, base + offset as u64, length).unwrap();
                    assert!(plan.spans().iter().all(|span| span.region == 2));
                    assert_eq!(
                        plan.spans().iter().map(|span| span.length).sum::<usize>(),
                        length
                    );
                    assert_eq!(plan.spans()[0].region_offset, frame * 18432 + offset as u64);
                    covered += length;
                }
            }
            assert_eq!(covered, 9 * page_size as usize);
            for (address, length) in [
                (arena - 1, 2),
                (arena + 9 * page_size - 1, 2),
                (data + 8 * page_size - 1, 2),
                (stack_base - 1, 2),
                (user_address_limit - 2 * page_size - 1, 2),
            ] {
                assert_eq!(
                    copy_plan::new(&mapping, direction, address, length),
                    Err(user_copy_error::unmapped)
                );
            }
            for page in 0..8 {
                assert_eq!(
                    copy_plan::new(&mapping, direction, 0x808000 + page * page_size, 1),
                    Err(user_copy_error::unmapped)
                );
            }
            assert_eq!(
                copy_plan::new(&mapping, direction, arena, 18432),
                Err(user_copy_error::too_large)
            );
        }
        assert_eq!(
            copy_plan::new(&mapping, copy_direction::to_user, code, 1),
            Err(user_copy_error::permission_denied)
        );
    }

    fn live_pair() -> (
        task_table,
        circle_table,
        scheduler_table,
        [task_identity; 2],
        task_bounds,
    ) {
        live_group::<2>()
    }

    #[test]
    fn native_arena_copy_preserves_legacy_count_and_exact_live_identity_checks() {
        let (tasks, mut circles, scheduler, [current, peer], bounds) =
            live_group_with_circles(circle_table::new_native_arenas());
        assert_eq!(
            validate_arena_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Err(user_copy_error::circle_binding)
        );
        circles
            .attach_child(current.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            validate_arena_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Ok(())
        );
        assert_eq!(
            validate_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Err(user_copy_error::circle_binding)
        );
        for (retained, identity) in [(current, peer), (peer, current), (peer, peer)] {
            assert_eq!(
                validate_arena_copy_binding(
                    retained, bounds, &tasks, &circles, &scheduler, identity
                ),
                Err(user_copy_error::task_binding)
            );
        }
        let changed = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        assert_eq!(
            validate_arena_copy_binding(current, changed, &tasks, &circles, &scheduler, current),
            Err(user_copy_error::task_binding)
        );
        assert_eq!(
            circles.attach_child(current.circle(), circle_child_kind::mapping),
            Err(crate::circle::circle_error::child_limit_exceeded)
        );
        circles
            .attach_child(current.circle(), circle_child_kind::endpoint)
            .unwrap();
        assert_eq!(
            validate_arena_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Err(user_copy_error::circle_binding)
        );
        circles
            .detach_child(current.circle(), circle_child_kind::endpoint)
            .unwrap();
        circles
            .detach_child(current.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            validate_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Ok(())
        );
    }

    #[test]
    fn nine_mapping_copy_requires_explicit_profile_and_exact_current_context() {
        let (mut tasks, mut circles, mut scheduler, [current, peer], bounds) =
            live_group_with_circles(circle_table::new_multi_region());
        assert_eq!(
            validate_multi_region_copy_binding(
                current, bounds, &tasks, &circles, &scheduler, current
            ),
            Err(user_copy_error::circle_binding)
        );
        assert_eq!(
            validate_copy_binding(current, bounds, &tasks, &circles, &scheduler, current),
            Ok(())
        );
        for identity in [current, peer] {
            circles
                .attach_children(identity.circle(), circle_child_kind::mapping, 6)
                .unwrap();
        }
        assert_eq!(
            validate_multi_region_copy_binding(
                current, bounds, &tasks, &circles, &scheduler, current
            ),
            Ok(())
        );
        for validate in [validate_copy_binding, validate_arena_copy_binding] {
            assert_eq!(
                validate(current, bounds, &tasks, &circles, &scheduler, current),
                Err(user_copy_error::circle_binding)
            );
        }
        let stale = task_identity::new(
            task_id::try_from_parts(current.task().slot(), current.task().generation_value() + 1)
                .unwrap(),
            current.circle(),
        );
        for (retained, supplied) in [
            (current, peer),
            (peer, current),
            (peer, peer),
            (current, stale),
            (stale, stale),
            (current, task_identity::new(current.task(), peer.circle())),
        ] {
            assert_eq!(
                validate_multi_region_copy_binding(
                    retained, bounds, &tasks, &circles, &scheduler, supplied
                ),
                Err(user_copy_error::task_binding)
            );
        }
        let changed = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        assert_eq!(
            validate_multi_region_copy_binding(
                current, changed, &tasks, &circles, &scheduler, current
            ),
            Err(user_copy_error::task_binding)
        );
        // These pure checks establish scheduler/context binding. The separate
        // live copy owner additionally authenticates CR3, TSS and actual PTEs.
        let foreign_scheduler = scheduler_table::new(2).unwrap();
        assert_eq!(
            validate_multi_region_copy_binding(
                current,
                bounds,
                &tasks,
                &circles,
                &foreign_scheduler,
                current
            ),
            Err(user_copy_error::task_not_current)
        );
        for child in [circle_child_kind::mapping, circle_child_kind::task_context] {
            circles.detach_child(current.circle(), child).unwrap();
            assert_eq!(
                validate_multi_region_copy_binding(
                    current, bounds, &tasks, &circles, &scheduler, current
                ),
                Err(user_copy_error::circle_binding)
            );
            circles.attach_child(current.circle(), child).unwrap();
        }
        assert_eq!(
            circles.attach_child(current.circle(), circle_child_kind::mapping),
            Err(crate::circle::circle_error::child_limit_exceeded)
        );
        scheduler.fault_current(&mut tasks, current).unwrap();
        scheduler
            .withdraw_terminal_context(&mut circles, &mut tasks, current)
            .unwrap();
        assert!(!tasks.snapshot(current).unwrap().context_attached());
        assert_eq!(
            validate_multi_region_copy_binding(
                current, bounds, &tasks, &circles, &scheduler, current
            ),
            Err(user_copy_error::task_binding)
        );
        assert_eq!(
            validate_multi_region_copy_binding(peer, bounds, &tasks, &circles, &scheduler, peer),
            Ok(())
        );
    }

    #[test]
    fn nine_mapping_copy_rejects_retired_slot_generations_after_replacement() {
        let (mut tasks, mut circles, mut scheduler, [old], bounds) =
            live_group_with_circles(circle_table::new_multi_region());
        circles
            .attach_children(old.circle(), circle_child_kind::mapping, 6)
            .unwrap();
        scheduler.complete_current(&mut tasks, old).unwrap();
        scheduler
            .withdraw_terminal_context(&mut circles, &mut tasks, old)
            .unwrap();
        circles
            .detach_children(old.circle(), circle_child_kind::mapping, 9)
            .unwrap();
        circles
            .detach_child(old.circle(), circle_child_kind::address_space)
            .unwrap();
        tasks.destroy(&mut circles, old).unwrap();
        circles.destroy(old.circle()).unwrap();
        let circle = circles.create().unwrap();
        circles
            .attach_child(circle, circle_child_kind::address_space)
            .unwrap();
        circles
            .attach_children(circle, circle_child_kind::mapping, 9)
            .unwrap();
        let fresh = tasks
            .create(
                &mut circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(91).unwrap(),
                    synthetic_origin_metadata::machine_role_fixture,
                    bounds,
                ),
            )
            .unwrap();
        scheduler.admit(&mut circles, &mut tasks, fresh).unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        assert_eq!(fresh.task().slot(), old.task().slot());
        assert_eq!(fresh.circle().slot(), old.circle().slot());
        assert_ne!(fresh.task().generation(), old.task().generation());
        assert_ne!(fresh.circle().generation(), old.circle().generation());
        assert_eq!(
            validate_multi_region_copy_binding(fresh, bounds, &tasks, &circles, &scheduler, fresh),
            Ok(())
        );
        for (retained, supplied) in [
            (old, old),
            (old, fresh),
            (fresh, old),
            (fresh, task_identity::new(fresh.task(), old.circle())),
        ] {
            assert_eq!(
                validate_multi_region_copy_binding(
                    retained, bounds, &tasks, &circles, &scheduler, supplied
                ),
                Err(user_copy_error::task_binding)
            );
        }
    }

    #[test]
    fn arena_copy_geometry_spans_pages_but_never_either_guard_or_rx_write() {
        for arena in [0x804000, 0x814000] {
            let mapping = mapping_plan::new(
                &[
                    (code, 8, false, true),
                    (data, 2, true, false),
                    (arena, 8, true, false),
                    (stack, 2, true, false),
                ]
                .map(|(virtual_address, page_count, writable, executable)| {
                    mapping_request {
                        virtual_address,
                        page_count,
                        permissions: page_permissions {
                            user: true,
                            writable,
                            executable,
                        },
                    }
                }),
                &[
                    reserved_request {
                        virtual_address: arena - page_size,
                        page_count: 1,
                    },
                    reserved_request {
                        virtual_address: arena + 8 * page_size,
                        page_count: 1,
                    },
                ],
                mapping_budget {
                    data_pages: 20,
                    table_frames: 8,
                },
            )
            .unwrap();
            for direction in [copy_direction::from_user, copy_direction::to_user] {
                for page in 1..8 {
                    let plan =
                        copy_plan::new(&mapping, direction, arena + page * page_size - 7, 19)
                            .unwrap();
                    assert_eq!(plan.spans().len(), 2);
                    assert_eq!(plan.spans()[0].region, 2);
                    assert_eq!(plan.spans()[0].region_offset, page * page_size - 7);
                    assert_eq!(plan.spans()[0].length, 7);
                    assert_eq!(plan.spans()[1].region, 2);
                    assert_eq!(plan.spans()[1].region_offset, page * page_size);
                    assert_eq!(plan.spans()[1].length, 12);
                }
                for (address, length) in [
                    (arena - 1, 2),
                    (arena + 8 * page_size - 1, 2),
                    (arena + 8 * page_size, 1),
                ] {
                    assert_eq!(
                        copy_plan::new(&mapping, direction, address, length),
                        Err(user_copy_error::unmapped)
                    );
                }
                assert_eq!(
                    copy_plan::new(&mapping, direction, stack, 8)
                        .unwrap()
                        .spans()[0]
                        .region,
                    3
                );
            }
            assert_eq!(
                copy_plan::new(&mapping, copy_direction::to_user, code, 1),
                Err(user_copy_error::permission_denied)
            );
        }
    }

    fn live_group<const count: usize>() -> (
        task_table,
        circle_table,
        scheduler_table,
        [task_identity; count],
        task_bounds,
    ) {
        live_group_with_circles(circle_table::new())
    }

    fn live_group_with_circles<const count: usize>(
        mut circles: circle_table,
    ) -> (
        task_table,
        circle_table,
        scheduler_table,
        [task_identity; count],
        task_bounds,
    ) {
        let mut tasks = task_table::new();
        let mut scheduler = scheduler_table::new(count).unwrap();
        let bounds = task_bounds::new(
            code,
            code + 128,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        let identities = core::array::from_fn(|index| {
            let circle = circles.create().unwrap();
            circles
                .attach_child(circle, circle_child_kind::address_space)
                .unwrap();
            circles
                .attach_children(circle, circle_child_kind::mapping, 3)
                .unwrap();
            let task = tasks
                .create(
                    &mut circles,
                    task_binding::new(
                        circle,
                        fixture_principal_id::new(index as u64 + 1).unwrap(),
                        synthetic_origin_metadata::machine_role_fixture,
                        bounds,
                    ),
                )
                .unwrap();
            scheduler.admit(&mut circles, &mut tasks, task).unwrap();
            task
        });
        scheduler.start_next(&mut tasks).unwrap();
        (tasks, circles, scheduler, identities, bounds)
    }

    #[test]
    fn suspended_copy_requires_sealed_call_and_both_retained_live_bindings() {
        let (mut tasks, mut circles, mut scheduler, [caller, service, veil], bounds) = live_group();
        let mut pending = None;
        let mut waiting = None;
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let authorization = pending.as_ref().unwrap();
        assert_eq!(
            validate_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                authorization,
            ),
            Ok(())
        );
        // The ordinary current-copy path still admits only the running veil.
        assert!(
            validate_copy_binding(caller, bounds, &tasks, &circles, &scheduler, caller).is_err()
        );
        assert!(validate_copy_binding(veil, bounds, &tasks, &circles, &scheduler, veil).is_ok());
        for (retained_caller, retained_veil) in [(service, veil), (caller, service), (veil, caller)]
        {
            assert_eq!(
                validate_suspended_copy_binding(
                    (retained_caller, bounds),
                    (retained_veil, bounds),
                    &tasks,
                    &circles,
                    &scheduler,
                    authorization,
                ),
                Err(user_copy_error::task_binding)
            );
        }
        let different = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        for (caller_bounds, veil_bounds) in [(different, bounds), (bounds, different)] {
            assert_eq!(
                validate_suspended_copy_binding(
                    (caller, caller_bounds),
                    (veil, veil_bounds),
                    &tasks,
                    &circles,
                    &scheduler,
                    authorization,
                ),
                Err(user_copy_error::task_binding)
            );
        }
        for identity in [caller, veil] {
            circles
                .detach_child(identity.circle(), circle_child_kind::mapping)
                .unwrap();
            assert_eq!(
                validate_suspended_copy_binding(
                    (caller, bounds),
                    (veil, bounds),
                    &tasks,
                    &circles,
                    &scheduler,
                    authorization,
                ),
                Err(user_copy_error::circle_binding)
            );
            circles
                .attach_child(identity.circle(), circle_child_kind::mapping)
                .unwrap();
        }
        // Preflighted reply holds the same immutable live authorization until
        // the architecture has finished checked copy and commits the wake.
        let reply = scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap();
        assert_eq!(
            validate_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                reply.tasks(),
                &circles,
                reply.scheduler(),
                reply.authorization(),
            ),
            Ok(())
        );
        reply.commit();
        assert!(pending.is_none());
        assert!(
            validate_copy_binding(caller, bounds, &tasks, &circles, &scheduler, caller).is_ok()
        );
    }

    #[test]
    fn suspended_copy_rejects_authority_from_recreated_tables_with_identical_task_ids() {
        let (mut tasks, circles, mut scheduler, [caller, _, veil], bounds) = live_group();
        let (mut other_tasks, other_circles, mut other_scheduler, [other_caller, _, other_veil], _) =
            live_group();
        assert_eq!((caller, veil), (other_caller, other_veil));
        let (mut waiting, mut pending, mut other_waiting, mut other_pending) =
            (None, None, None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        other_scheduler
            .prepare_call_handoff(
                &mut other_tasks,
                other_caller,
                other_veil,
                &mut other_waiting,
                &mut other_pending,
            )
            .unwrap()
            .commit();
        let authorization = pending.as_ref().unwrap();
        assert_eq!(
            validate_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                authorization,
            ),
            Ok(())
        );
        assert_eq!(
            validate_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &other_tasks,
                &other_circles,
                &other_scheduler,
                authorization,
            ),
            Err(user_copy_error::task_binding)
        );
    }

    #[test]
    fn copy_binding_requires_exact_live_current_identity_bounds_and_circle_children() {
        let (mut tasks, mut circles, mut scheduler, [first, second], bounds) = live_pair();
        assert_eq!(
            validate_copy_binding(first, bounds, &tasks, &circles, &scheduler, first),
            Ok(())
        );
        for (retained, supplied) in [
            (first, second),
            (second, second),
            (first, task_identity::new(first.task(), second.circle())),
            (
                first,
                task_identity::new(
                    task_id::try_from_parts(
                        first.task().slot(),
                        first.task().generation().get() + 1,
                    )
                    .unwrap(),
                    first.circle(),
                ),
            ),
        ] {
            assert!(
                validate_copy_binding(retained, bounds, &tasks, &circles, &scheduler, supplied)
                    .is_err()
            );
        }
        let different = task_bounds::new(
            code,
            code + 129,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        assert_eq!(
            validate_copy_binding(first, different, &tasks, &circles, &scheduler, first),
            Err(user_copy_error::task_binding)
        );
        circles
            .detach_child(first.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            validate_copy_binding(first, bounds, &tasks, &circles, &scheduler, first),
            Err(user_copy_error::circle_binding)
        );
        circles
            .attach_child(first.circle(), circle_child_kind::mapping)
            .unwrap();
        scheduler.fault_current(&mut tasks, first).unwrap();
        assert!(validate_copy_binding(first, bounds, &tasks, &circles, &scheduler, first).is_err());
        assert!(
            validate_copy_binding(second, bounds, &tasks, &circles, &scheduler, second).is_ok()
        );
        scheduler.complete_current(&mut tasks, second).unwrap();
        assert!(
            validate_copy_binding(second, bounds, &tasks, &circles, &scheduler, second).is_err()
        );
    }

    #[test]
    fn copy_binding_rejects_retired_generations_after_task_and_circle_slot_reuse() {
        let (mut tasks, mut circles, mut scheduler, [first, second], bounds) = live_pair();
        scheduler.fault_current(&mut tasks, first).unwrap();
        scheduler.complete_current(&mut tasks, second).unwrap();
        for identity in [first, second] {
            scheduler
                .withdraw_terminal_context(&mut circles, &mut tasks, identity)
                .unwrap();
            circles
                .detach_children(identity.circle(), circle_child_kind::mapping, 3)
                .unwrap();
            circles
                .detach_child(identity.circle(), circle_child_kind::address_space)
                .unwrap();
            tasks.destroy(&mut circles, identity).unwrap();
            circles.destroy(identity.circle()).unwrap();
        }
        let circle = circles.create().unwrap();
        circles
            .attach_child(circle, circle_child_kind::address_space)
            .unwrap();
        circles
            .attach_children(circle, circle_child_kind::mapping, 3)
            .unwrap();
        let fresh = tasks
            .create(
                &mut circles,
                task_binding::new(
                    circle,
                    fixture_principal_id::new(9).unwrap(),
                    synthetic_origin_metadata::machine_role_fixture,
                    bounds,
                ),
            )
            .unwrap();
        scheduler.admit(&mut circles, &mut tasks, fresh).unwrap();
        scheduler.start_next(&mut tasks).unwrap();
        assert_eq!(fresh.task().slot(), first.task().slot());
        assert_eq!(fresh.circle().slot(), first.circle().slot());
        assert_ne!(fresh.task().generation(), first.task().generation());
        assert_ne!(fresh.circle().generation(), first.circle().generation());
        assert!(validate_copy_binding(fresh, bounds, &tasks, &circles, &scheduler, fresh).is_ok());
        for (retained, supplied) in [
            (first, first),
            (first, fresh),
            (fresh, first),
            (fresh, task_identity::new(fresh.task(), first.circle())),
        ] {
            assert!(
                validate_copy_binding(retained, bounds, &tasks, &circles, &scheduler, supplied)
                    .is_err()
            );
        }
    }

    #[test]
    fn loader_suspended_copy_requires_exact_selected_profile_and_retains_legacy() {
        for caller_mappings in [3, 4, 8, 9, 10] {
            for veil_mappings in [3, 4, 9, 10] {
                let (mut tasks, mut circles, mut scheduler, [caller, _, veil], bounds) =
                    live_group_with_circles(circle_table::new_loader_runtime());
                for (identity, count) in [(caller, caller_mappings), (veil, veil_mappings)] {
                    for _ in 3..count {
                        circles
                            .attach_child(identity.circle(), circle_child_kind::mapping)
                            .unwrap();
                    }
                }
                let (mut pending, mut waiting) = (None, None);
                scheduler
                    .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
                    .unwrap()
                    .commit();
                let authorization = pending.as_ref().unwrap();
                let validate = |profile| {
                    let function = match profile {
                        9 => validate_loader_suspended_copy_binding,
                        10 => validate_loader_mapping_suspended_copy_binding,
                        _ => validate_suspended_copy_binding,
                    };
                    function(
                        (caller, bounds),
                        (veil, bounds),
                        &tasks,
                        &circles,
                        &scheduler,
                        authorization,
                    )
                };
                assert_eq!(
                    validate(9),
                    if caller_mappings == 9 && veil_mappings == 3 {
                        Ok(())
                    } else {
                        Err(user_copy_error::circle_binding)
                    }
                );
                assert_eq!(
                    validate(3),
                    if caller_mappings == 3 && veil_mappings == 3 {
                        Ok(())
                    } else {
                        Err(user_copy_error::circle_binding)
                    }
                );
                assert_eq!(
                    validate(10),
                    if caller_mappings == 10 && veil_mappings == 3 {
                        Ok(())
                    } else {
                        Err(user_copy_error::circle_binding)
                    }
                );
            }
        }
    }

    #[test]
    fn loader_runtime_copy_requires_exact_bindings_scheduler_and_no_extra_resources() {
        let (mut tasks, mut circles, mut scheduler, [caller, peer, veil], bounds) =
            live_group_with_circles(circle_table::new_loader_runtime());
        circles
            .attach_children(caller.circle(), circle_child_kind::mapping, 7)
            .unwrap();
        let (mut pending, mut waiting) = (None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let authority = pending.as_ref().unwrap();
        let stale = task_identity::new(
            task_id::try_from_parts(caller.task().slot(), caller.task().generation().get() + 1)
                .unwrap(),
            caller.circle(),
        );
        let changed = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        for (retained_caller, retained_veil) in [
            ((peer, bounds), (veil, bounds)),
            ((caller, bounds), (peer, bounds)),
            ((veil, bounds), (caller, bounds)),
            ((stale, bounds), (veil, bounds)),
            ((caller, changed), (veil, bounds)),
            ((caller, bounds), (veil, changed)),
        ] {
            assert_eq!(
                validate_loader_mapping_suspended_copy_binding(
                    retained_caller,
                    retained_veil,
                    &tasks,
                    &circles,
                    &scheduler,
                    authority,
                ),
                Err(user_copy_error::task_binding)
            );
        }
        let (mut other_tasks, mut other_circles, mut other_scheduler, other, _) =
            live_group_with_circles::<3>(circle_table::new_loader_runtime());
        assert_eq!((caller, veil), (other[0], other[2]));
        other_circles
            .attach_children(other[0].circle(), circle_child_kind::mapping, 7)
            .unwrap();
        let (mut other_pending, mut other_waiting) = (None, None);
        other_scheduler
            .prepare_call_handoff(
                &mut other_tasks,
                other[0],
                other[2],
                &mut other_waiting,
                &mut other_pending,
            )
            .unwrap()
            .commit();
        assert_eq!(
            validate_loader_mapping_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &other_tasks,
                &other_circles,
                &other_scheduler,
                authority,
            ),
            Err(user_copy_error::task_binding)
        );
        for identity in [caller, veil] {
            circles
                .attach_child(identity.circle(), circle_child_kind::endpoint)
                .unwrap();
            assert_eq!(
                validate_loader_mapping_suspended_copy_binding(
                    (caller, bounds),
                    (veil, bounds),
                    &tasks,
                    &circles,
                    &scheduler,
                    authority,
                ),
                Err(user_copy_error::circle_binding)
            );
            circles
                .detach_child(identity.circle(), circle_child_kind::endpoint)
                .unwrap();
        }
        assert_eq!(
            validate_loader_mapping_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                authority,
            ),
            Ok(())
        );
        circles
            .detach_child(caller.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            validate_loader_mapping_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                authority,
            ),
            Err(user_copy_error::circle_binding)
        );
    }

    #[test]
    fn retained_loader_copy_tracks_current_reply_and_fresh_next_call_authority() {
        let (mut tasks, mut circles, mut scheduler, [caller, peer, veil], bounds) =
            live_group_with_circles(circle_table::new_loader_runtime());
        circles
            .attach_children(caller.circle(), circle_child_kind::mapping, 7)
            .unwrap();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Ok(())
        );
        assert_eq!(
            validate_multi_region_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::circle_binding)
        );
        for retained in [peer, veil] {
            assert_eq!(
                validate_loader_mapping_copy_binding(
                    retained, bounds, &tasks, &circles, &scheduler, caller,
                ),
                Err(user_copy_error::task_binding)
            );
        }
        let changed = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, changed, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::task_binding)
        );
        circles
            .attach_child(caller.circle(), circle_child_kind::endpoint)
            .unwrap();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::circle_binding)
        );
        circles
            .detach_child(caller.circle(), circle_child_kind::endpoint)
            .unwrap();
        circles
            .detach_child(caller.circle(), circle_child_kind::mapping)
            .unwrap();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::circle_binding)
        );
        circles
            .attach_child(caller.circle(), circle_child_kind::mapping)
            .unwrap();
        let (mut pending, mut waiting) = (None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let creation = pending.as_ref().unwrap().correlation_words();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::task_binding)
        );
        assert_eq!(
            validate_loader_mapping_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                pending.as_ref().unwrap(),
            ),
            Ok(())
        );
        assert_eq!(
            validate_copy_binding(veil, bounds, &tasks, &circles, &scheduler, veil),
            Ok(())
        );
        assert_eq!(
            validate_loader_mapping_copy_binding(veil, bounds, &tasks, &circles, &scheduler, veil),
            Err(user_copy_error::circle_binding)
        );
        scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap()
            .commit();
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Ok(())
        );
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        // The existing owner can support this newly sealed invocation, but a
        // creation-bound lifetime handle cannot match its new correlation.
        assert_ne!(pending.as_ref().unwrap().correlation_words(), creation);
        assert_eq!(
            validate_loader_mapping_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                pending.as_ref().unwrap(),
            ),
            Ok(())
        );
        assert_eq!(
            validate_loader_mapping_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Err(user_copy_error::task_binding)
        );
    }

    #[test]
    fn loader_suspended_copy_rejects_foreign_roles_bounds_and_extra_children() {
        let (mut tasks, mut circles, mut scheduler, [caller, peer, veil], bounds) =
            live_group_with_circles(circle_table::new_multi_region());
        circles
            .attach_children(caller.circle(), circle_child_kind::mapping, 6)
            .unwrap();
        let (mut pending, mut waiting) = (None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        let authority = pending.as_ref().unwrap();
        let stale = task_identity::new(
            task_id::try_from_parts(caller.task().slot(), caller.task().generation().get() + 1)
                .unwrap(),
            caller.circle(),
        );
        let changed = task_bounds::new(
            code,
            code + 64,
            code,
            stack,
            stack + 2 * page_size,
            stack + 2 * page_size,
        )
        .unwrap();
        for (retained_caller, retained_veil) in [
            ((peer, bounds), (veil, bounds)),
            ((caller, bounds), (peer, bounds)),
            ((veil, bounds), (caller, bounds)),
            ((stale, bounds), (veil, bounds)),
            ((caller, changed), (veil, bounds)),
            ((caller, bounds), (veil, changed)),
        ] {
            assert_eq!(
                validate_loader_suspended_copy_binding(
                    retained_caller,
                    retained_veil,
                    &tasks,
                    &circles,
                    &scheduler,
                    authority,
                ),
                Err(user_copy_error::task_binding)
            );
        }
        for identity in [caller, veil] {
            circles
                .attach_child(identity.circle(), circle_child_kind::endpoint)
                .unwrap();
            assert_eq!(
                validate_loader_suspended_copy_binding(
                    (caller, bounds),
                    (veil, bounds),
                    &tasks,
                    &circles,
                    &scheduler,
                    authority,
                ),
                Err(user_copy_error::circle_binding)
            );
            circles
                .detach_child(identity.circle(), circle_child_kind::endpoint)
                .unwrap();
        }
        let reply = scheduler
            .prepare_call_reply(&mut tasks, &mut pending, &mut waiting)
            .unwrap();
        assert_eq!(
            validate_loader_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                reply.tasks(),
                &circles,
                reply.scheduler(),
                reply.authorization(),
            ),
            Ok(())
        );
        reply.commit();
        assert!(pending.is_none() && waiting.is_some());
        assert_eq!(
            validate_multi_region_copy_binding(
                caller, bounds, &tasks, &circles, &scheduler, caller
            ),
            Ok(())
        );
        assert_eq!(
            validate_copy_binding(caller, bounds, &tasks, &circles, &scheduler, caller),
            Err(user_copy_error::circle_binding)
        );
    }

    #[test]
    fn loader_suspended_copy_rejects_equal_identity_foreign_scheduler_authority() {
        let (mut tasks, mut circles, mut scheduler, [caller, _, veil], bounds) =
            live_group_with_circles(circle_table::new_multi_region());
        let (mut other_tasks, mut other_circles, mut other_scheduler, other, _) =
            live_group_with_circles::<3>(circle_table::new_multi_region());
        assert_eq!((caller, veil), (other[0], other[2]));
        circles
            .attach_children(caller.circle(), circle_child_kind::mapping, 6)
            .unwrap();
        other_circles
            .attach_children(other[0].circle(), circle_child_kind::mapping, 6)
            .unwrap();
        let (mut pending, mut waiting, mut other_pending, mut other_waiting) =
            (None, None, None, None);
        scheduler
            .prepare_call_handoff(&mut tasks, caller, veil, &mut waiting, &mut pending)
            .unwrap()
            .commit();
        other_scheduler
            .prepare_call_handoff(
                &mut other_tasks,
                other[0],
                other[2],
                &mut other_waiting,
                &mut other_pending,
            )
            .unwrap()
            .commit();
        assert_eq!(
            validate_loader_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &other_tasks,
                &other_circles,
                &other_scheduler,
                pending.as_ref().unwrap(),
            ),
            Err(user_copy_error::task_binding)
        );
        assert_eq!(
            validate_loader_suspended_copy_binding(
                (caller, bounds),
                (veil, bounds),
                &tasks,
                &circles,
                &scheduler,
                pending.as_ref().unwrap(),
            ),
            Ok(())
        );
    }
}
