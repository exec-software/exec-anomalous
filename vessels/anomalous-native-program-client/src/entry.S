.section .text.native_program_client_entry,"ax",@progbits
.p2align 4
.global anomalous_native_program_client_entry
.type anomalous_native_program_client_entry, @function
anomalous_native_program_client_entry:
    # RDI/RSI retain the exact copied descriptor address/length through startup.
    # Admission sees initial RFLAGS=2 and the unchanged user stack top.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_native_program_client_startup_return
anomalous_native_program_client_startup_return:
    callq anomalous_native_program_client_run
    ud2
.size anomalous_native_program_client_entry, . - anomalous_native_program_client_entry

.section .text.native_program_client_gate,"ax",@progbits
.p2align 4
.global anomalous_native_program_client_call
.type anomalous_native_program_client_call, @function
anomalous_native_program_client_call:
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    int $0xf4
.global anomalous_native_program_client_syscall_return
anomalous_native_program_client_syscall_return:
    movq %rax, 40(%rbx)
    movq %rcx, 48(%rbx)
    movq %rdx, 56(%rbx)
    popq %rbx
    retq
.size anomalous_native_program_client_call, . - anomalous_native_program_client_call

.global anomalous_native_program_client_fault_instruction
anomalous_native_program_client_fault_instruction:
    ud2
.section .note.GNU-stack,"",@progbits

.section .data.native_program_client_descriptor,"aw",@progbits
.p2align 4
# A program-bearing launch is an ANPG descriptor of exactly 256 bytes.
.zero 256
