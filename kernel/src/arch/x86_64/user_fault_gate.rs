#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Exact observations for the bounded owned-mapping ring-3 fixture.
//!
//! A match does not consume a phase, identify a live task, or authorize a
//! return. The caller retains the task, root, transition stack, and kernel
//! continuation and separately consumes the expected phase exactly once.

use super::exceptions::exception_snapshot;
use super::paging;
use super::user_entry_plan::user_entry_plan;

const frame_bytes: u64 = 22 * 8;
const permitted_flags: u64 = 0x0001_08d7;
const data_base: u64 = 0x0080_0000;
const denied_address: u64 = 0x0010_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum user_phase {
    first_report,
    second_report,
    terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum user_action {
    resume(u64),
    exit,
    fault,
}

#[derive(Clone, Copy)]
pub(super) struct user_expectation {
    pub phase: user_phase,
    pub user: user_entry_plan,
    pub root: u64,
    pub mode: u64,
    pub marker: u64,
    pub privilege_start: u64,
    pub privilege_end: u64,
    pub report_one: u64,
    pub resume_one: u64,
    pub report_two: u64,
    pub resume_two: u64,
    pub exit: u64,
    pub denied_read: u64,
}

#[derive(Clone, Copy)]
pub(super) struct user_observation {
    pub snapshot: exception_snapshot,
    pub frame: u64,
    pub root: u64,
    pub task_switched: bool,
    pub data_base_register: u64,
    pub mode_register: u64,
    pub marker_register: u64,
}

pub(super) fn match_exception(
    expected: user_expectation,
    actual: user_observation,
) -> Option<user_action> {
    let initial_rsp = expected.user.bounds().initial_stack_pointer();
    let expected_frame = expected.privilege_end.checked_sub(frame_bytes)?;
    let snapshot = actual.snapshot;
    if !profile_is_valid(expected, initial_rsp)
        || actual.frame != expected_frame
        || !actual.frame.is_multiple_of(16)
        || actual.frame < expected.privilege_start
        || actual.frame.checked_add(frame_bytes) != Some(expected.privilege_end)
        || actual.root != expected.root
        || !actual.task_switched
        || snapshot.code_segment != 0x33
        || snapshot.stack_segment != Some(0x2b)
        || snapshot.flags & 2 == 0
        || snapshot.flags & !permitted_flags != 0
        || !expected
            .user
            .accepts_return(snapshot.instruction_pointer, snapshot.stack_pointer)
        || actual.data_base_register != data_base
        || actual.mode_register != expected.mode
        || actual.marker_register != expected.marker
    {
        return None;
    }

    let (vector, error_code, fault_address, accumulator, rip, stack_used, action) =
        match expected.phase {
            user_phase::first_report => (
                6,
                0,
                None,
                1,
                expected.report_one,
                32,
                user_action::resume(expected.resume_one),
            ),
            user_phase::second_report => (
                6,
                0,
                None,
                2,
                expected.report_two,
                16,
                user_action::resume(expected.resume_two),
            ),
            user_phase::terminal if expected.mode == 0 => {
                (6, 0, None, 3, expected.exit, 16, user_action::exit)
            }
            user_phase::terminal => (
                14,
                5,
                Some(denied_address),
                2,
                expected.denied_read,
                16,
                user_action::fault,
            ),
        };
    (snapshot.vector == vector
        && snapshot.error_code == error_code
        && snapshot.fault_address == fault_address
        && snapshot.accumulator == accumulator
        && snapshot.instruction_pointer == rip
        && Some(snapshot.stack_pointer) == initial_rsp.checked_sub(stack_used))
    .then_some(action)
}

fn profile_is_valid(expected: user_expectation, initial_rsp: u64) -> bool {
    expected.root != 0
        && expected.root < paging::identity_map_limit
        && expected.root.is_multiple_of(paging::page_size)
        && matches!(expected.mode, 0 | 1)
        && expected.privilege_start != 0
        && expected.privilege_start < expected.privilege_end
        && paging::is_canonical(expected.privilege_start)
        && paging::is_canonical(expected.privilege_end - 1)
        && expected.privilege_start >> 47 == (expected.privilege_end - 1) >> 47
        && expected
            .user
            .accepts_return(expected.user.bounds().entry(), initial_rsp)
        && expected.report_one.checked_add(2) == Some(expected.resume_one)
        && expected.report_two.checked_add(2) == Some(expected.resume_two)
        && [
            expected.report_one,
            expected.resume_one,
            expected.report_two,
            expected.resume_two,
            expected.exit,
            expected.denied_read,
        ]
        .into_iter()
        .all(|site| expected.user.accepts_return(site, initial_rsp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::x86_64::mapping_plan::{
        mapping_budget, mapping_plan, mapping_request, reserved_request,
    };
    use crate::arch::x86_64::paging::{page_permissions, page_size};

    const code_start: u64 = 0x0040_0000;
    const payload_bytes: u64 = 0x100;
    const stack_start: u64 = (1 << 47) - 4 * page_size;
    const stack_end: u64 = stack_start + 2 * page_size;

    fn fixture(mode: u64, phase: user_phase) -> (user_expectation, user_observation, user_action) {
        let mappings = mapping_plan::new(
            &[
                mapping_request {
                    virtual_address: code_start,
                    page_count: 1,
                    permissions: page_permissions {
                        user: true,
                        writable: false,
                        executable: true,
                    },
                },
                mapping_request {
                    virtual_address: stack_start,
                    page_count: 2,
                    permissions: page_permissions {
                        user: true,
                        writable: true,
                        executable: false,
                    },
                },
            ],
            &[
                reserved_request {
                    virtual_address: stack_start - page_size,
                    page_count: 1,
                },
                reserved_request {
                    virtual_address: stack_end,
                    page_count: 1,
                },
            ],
            mapping_budget {
                data_pages: 3,
                table_frames: 32,
            },
        )
        .expect("fixture mapping geometry");
        let user = user_entry_plan::new(&mappings, 0, payload_bytes, code_start, 1, stack_end)
            .expect("fixture user admission");
        let expected = user_expectation {
            phase,
            user,
            root: 0x20_0000,
            mode,
            marker: 0xabc0 + mode,
            privilege_start: 0x18_000,
            privilege_end: 0x28_000,
            report_one: code_start + 0x20,
            resume_one: code_start + 0x22,
            report_two: code_start + 0x60,
            resume_two: code_start + 0x62,
            exit: code_start + 0xa0,
            denied_read: code_start + 0xb0,
        };
        let mut actual = user_observation {
            snapshot: exception_snapshot {
                accumulator: 1,
                vector: 6,
                error_code: 0,
                instruction_pointer: expected.report_one,
                code_segment: 0x33,
                flags: 2,
                stack_pointer: stack_end - 32,
                stack_segment: Some(0x2b),
                fault_address: None,
            },
            frame: expected.privilege_end - frame_bytes,
            root: expected.root,
            task_switched: true,
            data_base_register: data_base,
            mode_register: mode,
            marker_register: expected.marker,
        };
        let action = match phase {
            user_phase::first_report => user_action::resume(expected.resume_one),
            user_phase::second_report => {
                actual.snapshot.accumulator = 2;
                actual.snapshot.instruction_pointer = expected.report_two;
                actual.snapshot.stack_pointer = stack_end - 16;
                user_action::resume(expected.resume_two)
            }
            user_phase::terminal if mode == 0 => {
                actual.snapshot.accumulator = 3;
                actual.snapshot.instruction_pointer = expected.exit;
                actual.snapshot.stack_pointer = stack_end - 16;
                user_action::exit
            }
            user_phase::terminal => {
                actual.snapshot.accumulator = 2;
                actual.snapshot.vector = 14;
                actual.snapshot.error_code = 5;
                actual.snapshot.instruction_pointer = expected.denied_read;
                actual.snapshot.stack_pointer = stack_end - 16;
                actual.snapshot.fault_address = Some(denied_address);
                user_action::fault
            }
        };
        (expected, actual, action)
    }

    #[test]
    fn both_modes_require_their_complete_cpl3_phase_evidence() {
        for mode in [0, 1] {
            for phase in [
                user_phase::first_report,
                user_phase::second_report,
                user_phase::terminal,
            ] {
                let (expected, actual, action) = fixture(mode, phase);
                assert_eq!(match_exception(expected, actual), Some(action));
                for flag in 0..64 {
                    let mut changed = actual;
                    changed.snapshot.flags ^= 1 << flag;
                    let allowed = (1 << flag) & permitted_flags != 0 && flag != 1;
                    assert_eq!(
                        match_exception(expected, changed).is_some(),
                        allowed,
                        "flag {flag}"
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_near_miss_frames_registers_roots_selectors_and_fault_causes() {
        for mode in [0, 1] {
            for phase in [
                user_phase::first_report,
                user_phase::second_report,
                user_phase::terminal,
            ] {
                let (expected, actual, _) = fixture(mode, phase);
                for mutation in 0..26 {
                    let mut changed = actual;
                    match mutation {
                        0 => changed.frame -= 16,
                        1 => changed.frame += 16,
                        2 => changed.frame += 1,
                        3 => changed.frame = u64::MAX - 7,
                        4 => changed.root += page_size,
                        5 => changed.root += 1,
                        6 => changed.task_switched = false,
                        7 => changed.data_base_register += 8,
                        8 => changed.mode_register ^= 1,
                        9 => changed.marker_register ^= 1,
                        10 => changed.snapshot.code_segment = 0x08,
                        11 => changed.snapshot.code_segment |= 1 << 32,
                        12 => changed.snapshot.stack_segment = None,
                        13 => changed.snapshot.stack_segment = Some(0x10),
                        14 => changed.snapshot.stack_segment = Some((1 << 32) | 0x2b),
                        15 => changed.snapshot.stack_pointer -= 8,
                        16 => changed.snapshot.stack_pointer += 8,
                        17 => changed.snapshot.stack_pointer = stack_start - 8,
                        18 => changed.snapshot.instruction_pointer -= 1,
                        19 => changed.snapshot.instruction_pointer += 1,
                        20 => changed.snapshot.instruction_pointer = code_start + payload_bytes,
                        21 => changed.snapshot.vector ^= 1,
                        22 => changed.snapshot.error_code ^= 1,
                        23 => changed.snapshot.fault_address = Some(denied_address + 1),
                        24 => changed.snapshot.accumulator ^= 1,
                        25 => changed.snapshot.error_code |= 1 << 32,
                        _ => unreachable!(),
                    }
                    assert_eq!(
                        match_exception(expected, changed),
                        None,
                        "mutation {mutation}"
                    );
                }
                let mut changed = actual;
                changed.snapshot.fault_address = if actual.snapshot.fault_address.is_some() {
                    None
                } else {
                    Some(denied_address)
                };
                assert_eq!(match_exception(expected, changed), None);
            }
        }
    }

    #[test]
    fn a_report_or_terminal_observation_cannot_match_another_phase() {
        for mode in [0, 1] {
            for phase in [
                user_phase::first_report,
                user_phase::second_report,
                user_phase::terminal,
            ] {
                let (expected, actual, _) = fixture(mode, phase);
                for other in [
                    user_phase::first_report,
                    user_phase::second_report,
                    user_phase::terminal,
                ] {
                    if phase != other {
                        assert_eq!(
                            match_exception(
                                user_expectation {
                                    phase: other,
                                    ..expected
                                },
                                actual
                            ),
                            None
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn malformed_expected_context_cannot_authorize_a_matching_observation() {
        let (expected, actual, _) = fixture(0, user_phase::first_report);
        for mutation in 0..12 {
            let mut changed = expected;
            let mut observed = actual;
            match mutation {
                0 => {
                    changed.root = 0;
                    observed.root = 0;
                }
                1 => {
                    changed.root += 1;
                    observed.root = changed.root;
                }
                2 => {
                    changed.root = paging::identity_map_limit;
                    observed.root = changed.root;
                }
                3 => {
                    changed.mode = 2;
                    observed.mode_register = 2;
                }
                4 => changed.privilege_start = changed.privilege_end,
                5 => changed.privilege_start = observed.frame + 1,
                6 => changed.privilege_start = 0,
                7 => {
                    changed.privilege_end = 0;
                    observed.frame = u64::MAX - frame_bytes + 1;
                }
                8 => {
                    changed.privilege_end = u64::MAX;
                    observed.frame = u64::MAX - frame_bytes;
                }
                9 => {
                    changed.privilege_end = 1 << 48;
                    observed.frame = changed.privilege_end - frame_bytes;
                }
                10 => {
                    changed.privilege_start = 0x0000_8000_0000_0000;
                    changed.privilege_end = 0xffff_8000_0000_0000;
                    observed.frame = changed.privilege_end - frame_bytes;
                }
                11 => {
                    changed.privilege_end += 1;
                    observed.frame += 1;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                match_exception(changed, observed),
                None,
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn all_fixture_sites_and_both_exact_resume_offsets_require_initialized_code() {
        let (expected, actual, _) = fixture(0, user_phase::first_report);
        for mutation in 0..10 {
            let mut changed = expected;
            match mutation {
                0 => changed.resume_one += 1,
                1 => changed.resume_two += 1,
                2 => changed.report_one = u64::MAX,
                3 => changed.report_two = u64::MAX,
                4 => {
                    changed.report_one = code_start + payload_bytes - 2;
                    changed.resume_one = code_start + payload_bytes;
                }
                5 => {
                    changed.report_two = code_start + payload_bytes - 2;
                    changed.resume_two = code_start + payload_bytes;
                }
                6 => changed.exit = code_start - 1,
                7 => changed.denied_read = code_start + payload_bytes,
                8 => changed.exit = stack_start - page_size,
                9 => {
                    changed.report_one = code_start - 2;
                    changed.resume_one = code_start;
                }
                _ => unreachable!(),
            }
            assert_eq!(
                match_exception(changed, actual),
                None,
                "mutation {mutation}"
            );
        }
    }
}
