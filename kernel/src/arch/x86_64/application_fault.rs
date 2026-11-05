//! Selected task-local faults for the internal legacy application CPU profile.
//!
//! This is classification only: the caller must first prove that the hardware
//! frame belongs to the current retained task and is not nested. Invalid user
//! RIP/RSP are reasons to terminate that task, not failed return admission.
use super::{descriptors, exceptions::exception_snapshot};

pub(super) fn classify_user_fault(snapshot: exception_snapshot) -> bool {
    // Architectural status/control flags admitted by this bounded profile.
    const permitted_flags: u64 = 0x0000_0000_0025_0fd7;
    if snapshot.code_segment != u64::from(descriptors::user_code_selector)
        || snapshot.stack_segment != Some(u64::from(descriptors::user_data_selector))
        || snapshot.flags & 2 == 0
        || snapshot.flags & !permitted_flags != 0
    {
        return false;
    }
    match snapshot.vector {
        0 | 6 => snapshot.error_code == 0 && snapshot.fault_address.is_none(),
        // A local #GP can carry a selector index. External-event errors and
        // reserved upper bits cannot be attributed to this executing user.
        13 => snapshot.error_code & !0xfffe == 0 && snapshot.fault_address.is_none(),
        // Only P/W/U/I are admitted. Reserved-bit, PK, shadow-stack, enclave
        // and unknown machine-profile errors remain fatal kernel diagnostics.
        14 => {
            snapshot.error_code & 4 != 0
                && snapshot.error_code & !0x17 == 0
                && snapshot.fault_address.is_some()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_fault(vector: u64, error_code: u64) -> exception_snapshot {
        exception_snapshot {
            accumulator: 0,
            vector,
            error_code,
            instruction_pointer: u64::MAX,
            code_segment: u64::from(descriptors::user_code_selector),
            flags: 0x10246,
            stack_pointer: 0,
            stack_segment: Some(u64::from(descriptors::user_data_selector)),
            fault_address: (vector == 14).then_some(u64::MAX),
        }
    }

    #[test]
    fn selected_user_faults_do_not_require_returnable_instruction_or_stack() {
        for (vector, error) in [
            (0, 0),
            (6, 0),
            (13, 0),
            (13, 0xfffe),
            (14, 4),
            (14, 7),
            (14, 0x15),
        ] {
            let fault = user_fault(vector, error);
            assert!(classify_user_fault(fault));
            for flags in [2, 0x202, 0x10246, 0x40602, 0x200202] {
                assert!(classify_user_fault(exception_snapshot { flags, ..fault }));
            }
        }
    }

    #[test]
    fn privileged_unsupported_nested_profile_and_malformed_errors_are_rejected() {
        for vector in 0..32 {
            if !matches!(vector, 0 | 6 | 13 | 14) {
                assert!(!classify_user_fault(user_fault(vector, 0)));
            }
        }
        for vector in [0, 6, 13, 14] {
            let valid = user_fault(vector, if vector == 14 { 4 } else { 0 });
            for code_segment in [0, 8, 0x30, 0x3b] {
                assert!(!classify_user_fault(exception_snapshot {
                    code_segment,
                    ..valid
                }));
            }
            for stack_segment in [None, Some(0), Some(0x10)] {
                assert!(!classify_user_fault(exception_snapshot {
                    stack_segment,
                    ..valid
                }));
            }
            for flags in [0, 0x3002, 0x4002, 0x20002, 0x80002, 0x100002, 1 << 63] {
                assert!(!classify_user_fault(exception_snapshot { flags, ..valid }));
            }
        }
        for error in [0, 1, 2, 3, 0xc, 0x24, 0x44, 0x8004, u64::MAX] {
            assert!(!classify_user_fault(user_fault(14, error)));
        }
        for error in [1, 0x10000, u64::MAX] {
            assert!(!classify_user_fault(user_fault(13, error)));
        }
        for vector in [0, 6] {
            assert!(!classify_user_fault(user_fault(vector, 1)));
            assert!(!classify_user_fault(exception_snapshot {
                fault_address: Some(0),
                ..user_fault(vector, 0)
            }));
        }
        assert!(!classify_user_fault(exception_snapshot {
            fault_address: None,
            ..user_fault(14, 4)
        }));
    }
}
