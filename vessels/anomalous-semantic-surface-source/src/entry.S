.set source_rite_gate_vector, {rite_gate_vector}
.set source_user_mailbox, {user_mailbox_address}
.set source_initial_user_flags, {initial_user_flags}

.macro source_save_rust_registers
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
.endm

.macro source_restore_rust_registers
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
.endm

.macro source_zero_gate_registers
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
.endm

.macro source_rite function_name, return_name
    .p2align 4
    .global \function_name
    .type \function_name, @function
\function_name:
    source_save_rust_registers
    source_zero_gate_registers
    movabsq $source_user_mailbox, %r15
    pushq $source_initial_user_flags
    popfq
    int $source_rite_gate_vector
    .global \return_name
\return_name:
    source_restore_rust_registers
    retq
    .size \function_name, . - \function_name
.endm

.section .text.semantic_surface_source_entry,"ax",@progbits
.p2align 4
.global anomalous_semantic_surface_source_entry
.type anomalous_semantic_surface_source_entry, @function
anomalous_semantic_surface_source_entry:
    cld
    movabsq $source_user_mailbox, %rdi
    movl $512, %ecx
    xorq %rax, %rax
    rep stosq
    callq anomalous_semantic_surface_source_run
.global anomalous_semantic_surface_source_failed
anomalous_semantic_surface_source_failed:
    ud2
    jmp anomalous_semantic_surface_source_failed
.size anomalous_semantic_surface_source_entry, . - anomalous_semantic_surface_source_entry

.section .text.semantic_surface_source_rite,"ax",@progbits
source_rite anomalous_semantic_surface_source_read_denied_rite, anomalous_semantic_surface_source_read_denied_return
source_rite anomalous_semantic_surface_source_malformed_submit_rite, anomalous_semantic_surface_source_malformed_submit_return
source_rite anomalous_semantic_surface_source_freshness_denied_rite, anomalous_semantic_surface_source_freshness_denied_return
source_rite anomalous_semantic_surface_source_submit_rite, anomalous_semantic_surface_source_submit_return
source_rite anomalous_semantic_surface_source_backpressured_rite, anomalous_semantic_surface_source_backpressured_return
source_rite anomalous_semantic_surface_source_exit_rite, anomalous_semantic_surface_source_exit_return

.section .note.GNU-stack,"",@progbits
