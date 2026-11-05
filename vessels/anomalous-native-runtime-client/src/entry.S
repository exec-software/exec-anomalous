.section .text.native_runtime_client_entry,"ax",@progbits
.p2align 4
.global anomalous_native_runtime_client_entry
.type anomalous_native_runtime_client_entry, @function
anomalous_native_runtime_client_entry:
    # RDI/RSI retain the exact copied descriptor address/length through startup.
    # Admission sees initial RFLAGS=2 and the unchanged user stack top.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_native_runtime_client_startup_return
anomalous_native_runtime_client_startup_return:
    callq anomalous_native_runtime_client_run
    ud2
.size anomalous_native_runtime_client_entry, . - anomalous_native_runtime_client_entry

.section .text.native_runtime_client_gate,"ax",@progbits
.p2align 4
.global anomalous_native_runtime_client_call
.type anomalous_native_runtime_client_call, @function
anomalous_native_runtime_client_call:
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    int $0xf4
.global anomalous_native_runtime_client_syscall_return
anomalous_native_runtime_client_syscall_return:
    movq %rax, 40(%rbx)
    movq %rcx, 48(%rbx)
    movq %rdx, 56(%rbx)
    popq %rbx
    retq
.size anomalous_native_runtime_client_call, . - anomalous_native_runtime_client_call

.p2align 4
.global anomalous_native_runtime_client_arm_guard
.type anomalous_native_runtime_client_arm_guard, @function
anomalous_native_runtime_client_arm_guard:
    movq %rdi, %rsi
    movq $2, %rdi
    movq $8, %rax
    int $0xf4
.global anomalous_native_runtime_client_guard_fault_instruction
anomalous_native_runtime_client_guard_fault_instruction:
    movq $0x47554152, (%rsi)
    ud2
.size anomalous_native_runtime_client_arm_guard, . - anomalous_native_runtime_client_arm_guard

.p2align 4
.global anomalous_native_runtime_client_arm_nx
.type anomalous_native_runtime_client_arm_nx, @function
anomalous_native_runtime_client_arm_nx:
    movq %rdi, %rsi
    movq $3, %rdi
    movq $8, %rax
    int $0xf4
.global anomalous_native_runtime_client_nx_fault_instruction
anomalous_native_runtime_client_nx_fault_instruction:
    jmpq *%rsi
.global anomalous_native_runtime_client_fault_instruction
anomalous_native_runtime_client_fault_instruction:
    ud2
.size anomalous_native_runtime_client_arm_nx, . - anomalous_native_runtime_client_arm_nx
.section .note.GNU-stack,"",@progbits

.section .data.native_runtime_client_bootargs,"aw",@progbits
.p2align 4
.zero 128
.section .data.native_runtime_client_descriptor,"aw",@progbits
.p2align 4
.zero 128
