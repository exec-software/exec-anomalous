.section .text.native_client_entry,"ax",@progbits
.p2align 4
.global anomalous_native_client_entry
.type anomalous_native_client_entry, @function
anomalous_native_client_entry:
    # Preserve the initial RFLAGS=2 and top-of-stack value for startup admission.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_native_client_startup_return
anomalous_native_client_startup_return:
    callq anomalous_native_client_run
    ud2
.size anomalous_native_client_entry, . - anomalous_native_client_entry

.section .text.native_client_gate,"ax",@progbits
.p2align 4
.global anomalous_native_client_call
.type anomalous_native_client_call, @function
anomalous_native_client_call:
    # RDI points to one private, live repr(C) request. INTf4 preserves every
    # register except its explicit RAX/RCX/RDX results, including saved RBX.
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    int $0xf4
.global anomalous_native_client_syscall_return
anomalous_native_client_syscall_return:
    movq %rax, 40(%rbx)
    movq %rcx, 48(%rbx)
    movq %rdx, 56(%rbx)
    popq %rbx
    retq
.size anomalous_native_client_call, . - anomalous_native_client_call

.p2align 4
.global anomalous_native_client_arm_fault
.type anomalous_native_client_arm_fault, @function
anomalous_native_client_arm_fault:
    movq $8, %rax
    int $0xf4
.global anomalous_native_client_arm_fault_return
.global anomalous_native_client_fault_instruction
anomalous_native_client_arm_fault_return:
anomalous_native_client_fault_instruction:
    ud2
.size anomalous_native_client_arm_fault, . - anomalous_native_client_arm_fault
.section .note.GNU-stack,"",@progbits

# Explicit writable zeroed storage for the kernel-owned startup arguments.
.section .data.native_client_bootargs,"aw",@progbits
.p2align 4
.zero 128
