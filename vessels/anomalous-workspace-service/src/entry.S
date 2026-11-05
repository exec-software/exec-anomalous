.set workspace_mailbox, @workspace_mailbox@
.set workspace_gate, 0xf3
.section .text.workspace_entry,"ax",@progbits
.p2align 4
.global anomalous_workspace_service_entry
.type anomalous_workspace_service_entry, @function
anomalous_workspace_service_entry:
    cld
.if @owned_admission@
    # Owned-only admission checks use reserved zero-tail and bottom-stack words.
    # RDI is the trusted initial scenario; no mailbox ABI is changed.
    movabsq $0x804000, %rdx
    cmpq $0, (%rdx)
    jne .Lowned_entry_failed
    movabsq $0x6f776e65645f7277, %rax
    movq %rax, (%rdx)
    movabsq $0x807ff8, %rdx
    cmpq $0, (%rdx)
    jne .Lowned_entry_failed
    movq %rax, (%rdx)
    movabsq $0x7ffffffee000, %rdx
    cmpq $0, (%rdx)
    jne .Lowned_entry_failed
    movabsq $0x6f776e65645f7374, %rax
    movq %rax, (%rdx)
    testq %rdi, %rdi
    je .Lowned_entry_normal
    cmpq $1, %rdi
    je .Lowned_entry_guard
    cmpq $2, %rdi
    jne .Lowned_entry_failed
    movabsq $0x807000, %rax
.global anomalous_workspace_service_nx_instruction
anomalous_workspace_service_nx_instruction:
    jmpq *%rax
.Lowned_entry_guard:
    movabsq $0x808000, %rdx
.global anomalous_workspace_service_guard_instruction
anomalous_workspace_service_guard_instruction:
    movq %rax, (%rdx)
.Lowned_entry_failed:
    ud2
.Lowned_entry_normal:
.endif
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
