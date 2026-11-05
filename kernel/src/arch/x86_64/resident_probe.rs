//! Exact, single-use CPL0 permission probes for the resident-root feature proof.
//! This is not a general fault recovery mechanism or permission to activate a root.

use core::sync::atomic::{AtomicU8, Ordering};

use super::exceptions::exception_frame;
use super::paging;

const idle: u8 = 0;
const preparing: u8 = 1;
const armed: u8 = 2;
const consumed: u8 = 3;
const frame_bytes: u64 = 22 * 8;
const stack_headroom: u64 = 4096;
const permitted_flags: u64 = 0x0001_08d7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum probe_kind {
    read_only_write,
    guard_read,
    no_execute,
}

impl probe_kind {
    const fn error_code(self) -> u64 {
        match self {
            Self::read_only_write => 3,
            Self::guard_read => 0,
            Self::no_execute => 0x11,
        }
    }

    const fn return_bytes(self) -> u64 {
        match self {
            Self::no_execute => 16,
            Self::read_only_write | Self::guard_read => 8,
        }
    }
}

#[derive(Clone, Copy)]
struct expectation {
    kind: probe_kind,
    target: u64,
    root: u64,
    stack_start: u64,
    stack_end: u64,
    stack_pointer: u64,
    instruction_pointer: u64,
    resume: u64,
}

#[derive(Clone, Copy)]
struct observation {
    frame: u64,
    vector: u64,
    error_code: u64,
    instruction_pointer: u64,
    code_segment: u64,
    flags: u64,
    fault_address: Option<u64>,
    stack_pointer: u64,
    stack_segment: u64,
    root: u64,
}

fn complete_frame_is_resident(frame: u64, start: u64, end: u64) -> bool {
    start != 0
        && start < end
        && paging::is_canonical(start)
        && paging::is_canonical(end - 1)
        && start >> 47 == (end - 1) >> 47
        && frame.is_multiple_of(16)
        && frame >= start
        && frame
            .checked_add(frame_bytes)
            .is_some_and(|last| last <= end)
}

fn matches(expected: expectation, actual: observation) -> bool {
    complete_frame_is_resident(actual.frame, expected.stack_start, expected.stack_end)
        && expected.root != 0
        && expected.root.is_multiple_of(paging::page_size)
        && actual.root == expected.root
        && actual.vector == 14
        && actual.error_code == expected.kind.error_code()
        && actual.fault_address == Some(expected.target)
        && actual.instruction_pointer == expected.instruction_pointer
        && (expected.kind != probe_kind::no_execute
            || expected.instruction_pointer == expected.target)
        && actual.code_segment == u64::from(super::descriptors::kernel_code_selector)
        && actual.stack_segment == 0x10
        && actual.flags & 2 != 0
        && actual.flags & !permitted_flags == 0
        && actual.stack_pointer == expected.stack_pointer
        && expected.stack_start.checked_add(stack_headroom)
            .is_some_and(|minimum| actual.stack_pointer >= minimum)
        && actual.stack_pointer.checked_add(expected.kind.return_bytes())
            .is_some_and(|end| end <= expected.stack_end)
        // Long-mode hardware aligns the old stack before pushing SS:RSP and
        // the remaining interrupt frame, even when CPL and IST do not change.
        && actual.frame.checked_add(frame_bytes) == Some(actual.stack_pointer & !15)
        && expected.resume != 0
        && paging::is_canonical(expected.resume)
}

fn claim(phase: &AtomicU8, expected: expectation, actual: observation) -> Option<u64> {
    if phase.load(Ordering::Acquire) != armed || !matches(expected, actual) {
        return None;
    }
    phase
        .compare_exchange(armed, consumed, Ordering::AcqRel, Ordering::Acquire)
        .ok()?;
    Some(expected.resume)
}

/// # Safety
/// `frame` names the writable normalized exception prefix of the admitted live
/// hardware frame, exclusively owned by this returning exception handler.
unsafe fn replace_instruction_pointer(frame: *mut exception_frame, resume: u64) {
    // SAFETY: The caller grants this one field in the complete resident frame.
    unsafe {
        core::ptr::write(
            core::ptr::addr_of_mut!((*frame).instruction_pointer),
            resume,
        )
    };
}

#[cfg(not(test))]
mod native {
    use core::arch::{asm, global_asm};
    use core::sync::atomic::AtomicU64;

    use super::super::exceptions::exception_snapshot;
    use super::*;

    global_asm!(
        r#"
        .text
        .p2align 4
        .global x86_64_resident_probe_write
        .global x86_64_resident_probe_write_fault
        .global x86_64_resident_probe_write_resume
        .type x86_64_resident_probe_write,@function
    x86_64_resident_probe_write:
        movq %rsp, (%rsi)
        movabsq $0x724f70726f626531, %rax
    x86_64_resident_probe_write_fault:
        movq %rax, (%rdi)
        xorl %eax, %eax
        retq
    x86_64_resident_probe_write_resume:
        movl $1, %eax
        retq
        .size x86_64_resident_probe_write,.-x86_64_resident_probe_write

        .p2align 4
        .global x86_64_resident_probe_read
        .global x86_64_resident_probe_read_fault
        .global x86_64_resident_probe_read_resume
        .type x86_64_resident_probe_read,@function
    x86_64_resident_probe_read:
        movq %rsp, (%rsi)
    x86_64_resident_probe_read_fault:
        movq (%rdi), %rax
        xorl %eax, %eax
        retq
    x86_64_resident_probe_read_resume:
        movl $1, %eax
        retq
        .size x86_64_resident_probe_read,.-x86_64_resident_probe_read

        .p2align 4
        .global x86_64_resident_probe_execute
        .global x86_64_resident_probe_execute_resume
        .type x86_64_resident_probe_execute,@function
    x86_64_resident_probe_execute:
        leaq -8(%rsp), %rax
        movq %rax, (%rsi)
        callq *%rdi
        xorl %eax, %eax
        retq
    x86_64_resident_probe_execute_resume:
        addq $8, %rsp
        movl $1, %eax
        retq
        .size x86_64_resident_probe_execute,.-x86_64_resident_probe_execute
    "#,
        options(att_syntax)
    );

    unsafe extern "C" {
        fn x86_64_resident_probe_write(target: u64, saved_stack: *mut u64) -> u64;
        fn x86_64_resident_probe_read(target: u64, saved_stack: *mut u64) -> u64;
        fn x86_64_resident_probe_execute(target: u64, saved_stack: *mut u64) -> u64;
        fn x86_64_resident_probe_write_fault();
        fn x86_64_resident_probe_write_resume();
        fn x86_64_resident_probe_read_fault();
        fn x86_64_resident_probe_read_resume();
        fn x86_64_resident_probe_execute_resume();
    }

    static phase: AtomicU8 = AtomicU8::new(idle);
    static kind: AtomicU8 = AtomicU8::new(0);
    static target_address: AtomicU64 = AtomicU64::new(0);
    static root_address: AtomicU64 = AtomicU64::new(0);
    static stack_start_address: AtomicU64 = AtomicU64::new(0);
    static stack_end_address: AtomicU64 = AtomicU64::new(0);
    static saved_stack_pointer: AtomicU64 = AtomicU64::new(0);
    static fault_instruction: AtomicU64 = AtomicU64::new(0);
    static resume_instruction: AtomicU64 = AtomicU64::new(0);

    pub(super) unsafe fn run(
        requested: probe_kind,
        target: u64,
        expected_root: u64,
        stack_start: u64,
        stack_end: u64,
    ) -> bool {
        let flags: u64;
        let cr4: u64;
        let current_stack: u64;
        // SAFETY: The feature caller owns CPL0 on the BSP with a live stack.
        unsafe {
            asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
            asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
            asm!("mov {}, rsp", out(reg) current_stack, options(nomem, nostack, preserves_flags));
        }
        // Exclude controls which could make another protection mechanism look
        // like the intended WP, non-present, or NX failure, and unsupported CET.
        let unsupported_cr4 = (1 << 7)
            | (1 << 12)
            | (1 << 17)
            | (1 << 20)
            | (1 << 21)
            | (1 << 22)
            | (1 << 23)
            | (1 << 24);
        if flags & !permitted_flags != 0
            || cr4 & unsupported_cr4 != 0
            || !paging::protections_are_enabled()
            || paging::read_cr3() != expected_root
            || expected_root == 0
            || !expected_root.is_multiple_of(paging::page_size)
            || target == 0
            || !target.is_multiple_of(8)
            || !paging::is_canonical(target)
            || target
                .checked_add(7)
                .is_none_or(|end| !paging::is_canonical(end))
            || !stack_start.is_multiple_of(paging::page_size)
            || !stack_end.is_multiple_of(paging::page_size)
            || stack_start == 0
            || stack_start >= stack_end
            || !paging::is_canonical(stack_start)
            || !paging::is_canonical(stack_end - 1)
            || stack_start >> 47 != (stack_end - 1) >> 47
            || stack_start
                .checked_add(2 * stack_headroom)
                .is_none_or(|minimum| minimum > stack_end)
            || current_stack < stack_start + stack_headroom
            || current_stack >= stack_end
            || phase
                .compare_exchange(idle, preparing, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return false;
        }
        let (kind_value, fault, resume) = match requested {
            probe_kind::read_only_write => (
                1,
                x86_64_resident_probe_write_fault as *const () as u64,
                x86_64_resident_probe_write_resume as *const () as u64,
            ),
            probe_kind::guard_read => (
                2,
                x86_64_resident_probe_read_fault as *const () as u64,
                x86_64_resident_probe_read_resume as *const () as u64,
            ),
            probe_kind::no_execute => (
                3,
                target,
                x86_64_resident_probe_execute_resume as *const () as u64,
            ),
        };
        kind.store(kind_value, Ordering::Relaxed);
        target_address.store(target, Ordering::Relaxed);
        root_address.store(expected_root, Ordering::Relaxed);
        stack_start_address.store(stack_start, Ordering::Relaxed);
        stack_end_address.store(stack_end, Ordering::Relaxed);
        saved_stack_pointer.store(0, Ordering::Relaxed);
        fault_instruction.store(fault, Ordering::Relaxed);
        resume_instruction.store(resume, Ordering::Relaxed);
        phase.store(armed, Ordering::Release);
        // SAFETY: This isolated assembly records the exact fault-time RSP into
        // atomic backing storage before its one probe. IF/NMI/SMP exclusion
        // prevents concurrent access; the handler reads only after that store.
        let result = unsafe {
            match requested {
                probe_kind::read_only_write => {
                    x86_64_resident_probe_write(target, saved_stack_pointer.as_ptr())
                }
                probe_kind::guard_read => {
                    x86_64_resident_probe_read(target, saved_stack_pointer.as_ptr())
                }
                probe_kind::no_execute => {
                    x86_64_resident_probe_execute(target, saved_stack_pointer.as_ptr())
                }
            }
        };
        let accepted = phase.swap(preparing, Ordering::AcqRel) == consumed;
        for cell in [
            &target_address,
            &root_address,
            &stack_start_address,
            &stack_end_address,
            &saved_stack_pointer,
            &fault_instruction,
            &resume_instruction,
        ] {
            cell.store(0, Ordering::Relaxed);
        }
        kind.store(0, Ordering::Relaxed);
        phase.store(idle, Ordering::Release);
        result == 1 && accepted && paging::read_cr3() == expected_root
    }

    pub(super) unsafe fn contain(
        frame: *mut exception_frame,
        snapshot: exception_snapshot,
    ) -> bool {
        if phase.load(Ordering::Acquire) != armed {
            return false;
        }
        let requested = match kind.load(Ordering::Relaxed) {
            1 => probe_kind::read_only_write,
            2 => probe_kind::guard_read,
            3 => probe_kind::no_execute,
            _ => return false,
        };
        let start = stack_start_address.load(Ordering::Relaxed);
        let end = stack_end_address.load(Ordering::Relaxed);
        if !complete_frame_is_resident(frame as u64, start, end) {
            return false;
        }
        // SAFETY: The complete 176-byte normalized hardware frame was bounded
        // above. Long mode saves SS:RSP unconditionally, including CPL0 faults.
        let (saved_rsp, saved_ss) = unsafe {
            (
                core::ptr::read(frame.cast::<u64>().add(20)),
                core::ptr::read(frame.cast::<u64>().add(21)),
            )
        };
        let expected = expectation {
            kind: requested,
            target: target_address.load(Ordering::Relaxed),
            root: root_address.load(Ordering::Relaxed),
            stack_start: start,
            stack_end: end,
            stack_pointer: saved_stack_pointer.load(Ordering::Relaxed),
            instruction_pointer: fault_instruction.load(Ordering::Relaxed),
            resume: resume_instruction.load(Ordering::Relaxed),
        };
        let actual = observation {
            frame: frame as u64,
            vector: snapshot.vector,
            error_code: snapshot.error_code,
            instruction_pointer: snapshot.instruction_pointer,
            code_segment: snapshot.code_segment,
            flags: snapshot.flags,
            fault_address: snapshot.fault_address,
            stack_pointer: saved_rsp,
            stack_segment: saved_ss,
            root: paging::read_cr3(),
        };
        let Some(resume) = claim(&phase, expected, actual) else {
            return false;
        };
        // SAFETY: The exact single-use predicate admits this resident writable
        // frame and fixed continuation. Every other frame byte remains intact.
        unsafe { replace_instruction_pointer(frame, resume) };
        true
    }
}

/// # Safety
/// The BSP exclusively owns the admitted root and resident supervisor stack,
/// with IF clear and NMI masked. The target is the exact owned fixture/guard;
/// an unexpectedly successful write may change it and NX execution must contain
/// only the bounded fixture. No root switch or teardown may race this call.
#[cfg(not(test))]
pub(super) unsafe fn run(
    kind: probe_kind,
    target: u64,
    expected_root: u64,
    stack_start: u64,
    stack_end: u64,
) -> bool {
    // SAFETY: The caller grants the complete native probe contract.
    unsafe { native::run(kind, target, expected_root, stack_start, stack_end) }
}

/// # Safety
/// The common exception entry owns `frame` and supplies its matching snapshot.
/// The caller must clear its exception-active latch only if this returns true.
#[cfg(not(test))]
pub(super) unsafe fn contain(
    frame: *mut exception_frame,
    snapshot: super::exceptions::exception_snapshot,
) -> bool {
    // SAFETY: The native helper bounds the complete frame before reading its tail.
    unsafe { native::contain(frame, snapshot) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(kind: probe_kind) -> (expectation, observation) {
        let expected = expectation {
            kind,
            target: 0x400000,
            root: 0x180000,
            stack_start: 0x10000,
            stack_end: 0x20000,
            stack_pointer: 0x1fef8,
            instruction_pointer: if kind == probe_kind::no_execute {
                0x400000
            } else {
                0x110000
            },
            resume: 0x110010,
        };
        let actual = observation {
            frame: 0x1fe40,
            vector: 14,
            error_code: kind.error_code(),
            instruction_pointer: expected.instruction_pointer,
            code_segment: 8,
            flags: 0x10046,
            fault_address: Some(expected.target),
            stack_pointer: expected.stack_pointer,
            stack_segment: 0x10,
            root: expected.root,
        };
        (expected, actual)
    }

    #[test]
    fn exact_kernel_faults_are_single_use_and_reject_every_observation_mismatch() {
        for kind in [
            probe_kind::read_only_write,
            probe_kind::guard_read,
            probe_kind::no_execute,
        ] {
            let (expected, actual) = fixture(kind);
            let phase = AtomicU8::new(armed);
            assert_eq!(claim(&phase, expected, actual), Some(expected.resume));
            assert_eq!(claim(&phase, expected, actual), None);
            for mutation in 0..13 {
                let mut changed = actual;
                match mutation {
                    0 => changed.vector = 13,
                    1 => changed.error_code ^= 1,
                    2 => changed.instruction_pointer += 1,
                    3 => changed.code_segment = 0x33,
                    4 => changed.flags |= 1 << 9,
                    5 => changed.flags |= 1 << 10,
                    6 => changed.flags &= !2,
                    7 => changed.fault_address = Some(expected.target + 8),
                    8 => changed.fault_address = None,
                    9 => changed.stack_pointer -= 8,
                    10 => changed.stack_segment = 0x2b,
                    11 => changed.root += paging::page_size,
                    _ => changed.frame -= 16,
                }
                phase.store(armed, Ordering::Release);
                assert_eq!(
                    claim(&phase, expected, changed),
                    None,
                    "mutation {mutation}"
                );
                assert_eq!(phase.load(Ordering::Acquire), armed);
            }
            for state in [idle, preparing, consumed] {
                phase.store(state, Ordering::Release);
                assert_eq!(claim(&phase, expected, actual), None);
            }
        }
    }

    #[test]
    fn complete_frame_and_saved_stack_require_exact_resident_bounds_and_headroom() {
        let (expected, actual) = fixture(probe_kind::read_only_write);
        for mutation in 0..7 {
            let mut changed = expected;
            match mutation {
                0 => changed.stack_start = actual.frame + 16,
                1 => changed.stack_end = actual.frame + frame_bytes - 1,
                2 => changed.stack_start = actual.stack_pointer - stack_headroom + 1,
                3 => changed.stack_end = actual.stack_pointer + 7,
                4 => changed.root = 0,
                5 => changed.resume = 0,
                _ => changed.resume = 0x8000_0000_0000,
            }
            assert!(!matches(changed, actual), "mutation {mutation}");
        }
        assert!(!complete_frame_is_resident(u64::MAX - 15, 0x1000, u64::MAX));
        assert!(!complete_frame_is_resident(
            actual.frame + 1,
            0x10000,
            0x20000
        ));
    }

    #[test]
    fn containment_rewrites_only_the_instruction_pointer_in_the_full_frame() {
        let mut words = [0xa5a5_a5a5_a5a5_a5a5_u64; 22];
        let before = words;
        // SAFETY: The array owns the complete naturally aligned hardware frame.
        unsafe { replace_instruction_pointer(words.as_mut_ptr().cast(), 0x110010) };
        for index in 0..words.len() {
            assert_eq!(
                words[index],
                if index == 17 { 0x110010 } else { before[index] }
            );
        }
    }
}
