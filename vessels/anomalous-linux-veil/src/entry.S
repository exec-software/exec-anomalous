.section .text.linux_veil_entry,"ax",@progbits
.p2align 4
.global anomalous_linux_veil_entry
.type anomalous_linux_veil_entry, @function
anomalous_linux_veil_entry:
    # Native startup observes the pristine stack and flags before Rust runs.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_linux_veil_startup_return
anomalous_linux_veil_startup_return:
    testq %rax, %rax
    jne anomalous_linux_veil_failure_instruction
    movq %rdx, %rdi
    movq %rcx, %rsi
    callq anomalous_linux_veil_run
    ud2
.size anomalous_linux_veil_entry, . - anomalous_linux_veil_entry

.section .text.linux_veil_gate,"ax",@progbits
.p2align 4
.global anomalous_linux_veil_call
.type anomalous_linux_veil_call, @function
anomalous_linux_veil_call:
    # This private repr(C) record is not the cross-task wire format. The sole
    # pointer names this task's live stack storage, never a foreign user frame.
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %r10
    movq 40(%rbx), %r8
    movq 48(%rbx), %r9
    int $0xf4
.global anomalous_linux_veil_syscall_return
anomalous_linux_veil_syscall_return:
    movq %rax, 56(%rbx)
    movq %rdx, 64(%rbx)
    movq %rcx, 72(%rbx)
    popq %rbx
    retq
.size anomalous_linux_veil_call, . - anomalous_linux_veil_call

.p2align 4
.global anomalous_linux_veil_fault_instruction
.type anomalous_linux_veil_fault_instruction, @function
anomalous_linux_veil_fault_instruction:
    # Scenario 2 reaches this only with a successful but uncommitted submit.
    ud2
.size anomalous_linux_veil_fault_instruction, . - anomalous_linux_veil_fault_instruction

.global anomalous_linux_veil_failure_instruction
.type anomalous_linux_veil_failure_instruction, @function
anomalous_linux_veil_failure_instruction:
    # Distinct unexpected-failure marker; never accepted as the fault probe.
    ud2
.size anomalous_linux_veil_failure_instruction, . - anomalous_linux_veil_failure_instruction
.section .note.GNU-stack,"",@progbits

.section .data.linux_veil_reserved,"aw",@progbits
.p2align 4
.zero 128
