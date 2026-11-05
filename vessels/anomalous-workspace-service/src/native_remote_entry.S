.set workspace_mailbox, @workspace_mailbox@
.set workspace_gate, 0xf3
.section .text.workspace_entry,"ax",@progbits
.p2align 4
.global anomalous_workspace_service_entry
.type anomalous_workspace_service_entry, @function
anomalous_workspace_service_entry:
    # RDI/RSI identify the exact retained startup descriptor. The first gate
    # validates that immutable installation before enabling ordinary calls.
    cld
    # Preserve admitted RFLAGS=2 until the first gate, as all native roles do.
    movq $0, %rax
    int $0xf4
.global anomalous_workspace_service_startup_return
anomalous_workspace_service_startup_return:
    callq anomalous_workspace_service_run
    ud2
.size anomalous_workspace_service_entry, . - anomalous_workspace_service_entry

.section .text.workspace_rite,"ax",@progbits
.p2align 4
.global anomalous_workspace_service_rite
.type anomalous_workspace_service_rite, @function
anomalous_workspace_service_rite:
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    xorq %rax, %rax
    xorq %rbx, %rbx
    xorq %rcx, %rcx
    xorq %rdx, %rdx
    xorq %rbp, %rbp
    xorq %rsi, %rsi
    xorq %rdi, %rdi
    xorq %r8, %r8
    xorq %r9, %r9
    xorq %r10, %r10
    xorq %r11, %r11
    xorq %r12, %r12
    xorq %r13, %r13
    xorq %r14, %r14
    movabsq $workspace_mailbox, %r15
    pushq $0x202
    popfq
    int $workspace_gate
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    retq
.size anomalous_workspace_service_rite, . - anomalous_workspace_service_rite

.p2align 4
.global anomalous_workspace_service_controlled_fault
.type anomalous_workspace_service_controlled_fault, @function
anomalous_workspace_service_controlled_fault:
.global anomalous_workspace_service_fault_instruction
anomalous_workspace_service_fault_instruction:
    ud2
.size anomalous_workspace_service_controlled_fault, . - anomalous_workspace_service_controlled_fault

# User-controlled invalid state must withdraw this owner, never halt the kernel.
.p2align 4
.global anomalous_workspace_service_bad_flags
.type anomalous_workspace_service_bad_flags, @function
anomalous_workspace_service_bad_flags:
    std
    ud2
.size anomalous_workspace_service_bad_flags, . - anomalous_workspace_service_bad_flags
.p2align 4
.global anomalous_workspace_service_bad_mailbox
.type anomalous_workspace_service_bad_mailbox, @function
anomalous_workspace_service_bad_mailbox:
    xorq %r15, %r15
    int $workspace_gate
    ud2
.size anomalous_workspace_service_bad_mailbox, . - anomalous_workspace_service_bad_mailbox

.section .note.GNU-stack,"",@progbits

.section .text.workspace_native_gate,"ax",@progbits
.p2align 4
.global anomalous_workspace_native_call
.type anomalous_workspace_native_call, @function
anomalous_workspace_native_call:
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    int $0xf4
.global anomalous_workspace_native_call_return
anomalous_workspace_native_call_return:
    movq %rax, 40(%rbx)
    movq %rcx, 48(%rbx)
    movq %rdx, 56(%rbx)
    popq %rbx
    retq
.size anomalous_workspace_native_call, . - anomalous_workspace_native_call

.section .data.native_workspace_descriptor,"aw",@progbits
.p2align 4
.zero 192
