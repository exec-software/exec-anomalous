#![cfg_attr(not(test), allow(dead_code))]
#![forbid(unsafe_code)]

//! Exact startup fault predicates for the removable workspace admission proof.
//! These observations do not admit a task or validate ownership of its frame.

use super::{descriptors, exceptions::exception_snapshot};

pub(super) fn classify_probe(
    scenario: u64,
    snapshot: &exception_snapshot,
    guard_instruction: u64,
    stack_top: u64,
) -> Option<&'static str> {
    if snapshot.vector != 14
        || snapshot.code_segment != u64::from(descriptors::user_code_selector)
        || snapshot.stack_segment != Some(u64::from(descriptors::user_data_selector))
        || snapshot.stack_pointer != stack_top
        || snapshot.flags & ((3 << 12) | (1 << 14) | (1 << 10)) != 0
        || snapshot.flags & 0x202 != 0x202
    {
        return None;
    }
    match scenario {
        1 if snapshot.error_code == 6
            && snapshot.instruction_pointer == guard_instruction
            && snapshot.fault_address == Some(0x808000) =>
        {
            Some("guard_fault")
        }
        2 if snapshot.error_code == 21
            && snapshot.instruction_pointer == 0x807000
            && snapshot.fault_address == Some(0x807000) =>
        {
            Some("nx_fault")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const guard: u64 = 0x40009a;
    const top: u64 = (1 << 47) - 8192;

    fn expected(scenario: u64) -> exception_snapshot {
        exception_snapshot {
            accumulator: 0,
            vector: 14,
            error_code: if scenario == 1 { 6 } else { 21 },
            instruction_pointer: if scenario == 1 { guard } else { 0x807000 },
            code_segment: u64::from(descriptors::user_code_selector),
            flags: 0x202,
            stack_pointer: top,
            stack_segment: Some(u64::from(descriptors::user_data_selector)),
            fault_address: Some(if scenario == 1 { 0x808000 } else { 0x807000 }),
        }
    }

    #[test]
    fn accepts_exact_page_fault_errors_without_gate_zero_error_rule() {
        assert_eq!(
            classify_probe(1, &expected(1), guard, top),
            Some("guard_fault")
        );
        assert_eq!(
            classify_probe(2, &expected(2), guard, top),
            Some("nx_fault")
        );
        for scenario in [1, 2] {
            let mut snapshot = expected(scenario);
            snapshot.flags |= 1 << 16; // CPU may set RF on a fault frame.
            assert!(classify_probe(scenario, &snapshot, guard, top).is_some());
        }
    }

    #[test]
    fn rejects_nearby_faults_privilege_flags_and_foreign_scenarios() {
        for scenario in [1, 2] {
            for mutation in 0..15 {
                let mut snapshot = expected(scenario);
                match mutation {
                    0 => snapshot.vector = 6,
                    1 => snapshot.error_code = 0,
                    2 => snapshot.error_code ^= 1,
                    3 => snapshot.instruction_pointer += 1,
                    4 => snapshot.fault_address = None,
                    5 => snapshot.fault_address = Some(0x809000),
                    6 => snapshot.code_segment = u64::from(descriptors::kernel_code_selector),
                    7 => snapshot.stack_segment = None,
                    8 => snapshot.stack_segment = Some(0x10),
                    9 => snapshot.stack_pointer -= 8,
                    10 => snapshot.flags &= !2,
                    11 => snapshot.flags &= !(1 << 9),
                    12 => snapshot.flags |= 1 << 10,
                    13 => snapshot.flags |= 1 << 12,
                    _ => snapshot.flags |= 1 << 14,
                }
                assert_eq!(classify_probe(scenario, &snapshot, guard, top), None);
            }
            for other in [0, 3, u64::MAX, 3 - scenario] {
                assert_eq!(classify_probe(other, &expected(scenario), guard, top), None);
            }
        }
    }
}
