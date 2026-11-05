.set renderer_rite_gate_vector, {rite_gate_vector}
.set renderer_user_mailbox, {user_mailbox_address}
.set renderer_initial_user_flags, {initial_user_flags}

.macro renderer_save_rust_registers
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
.endm

.macro renderer_restore_rust_registers
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
.endm

.macro renderer_zero_gate_registers
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

.macro renderer_rite function_name, return_name
    .p2align 4
    .global \function_name
    .type \function_name, @function
\function_name:
    renderer_save_rust_registers
    renderer_zero_gate_registers
    movabsq $renderer_user_mailbox, %r15
    pushq $renderer_initial_user_flags
    popfq
    int $renderer_rite_gate_vector
    .global \return_name
\return_name:
    renderer_restore_rust_registers
    retq
    .size \function_name, . - \function_name
.endm

.section .text.semantic_surface_renderer_entry,"ax",@progbits
.p2align 4
.global anomalous_semantic_surface_renderer_entry
.type anomalous_semantic_surface_renderer_entry, @function
anomalous_semantic_surface_renderer_entry:
    cld
    movabsq $renderer_user_mailbox, %rdi
    movl $512, %ecx
    xorq %rax, %rax
    rep stosq
    callq anomalous_semantic_surface_renderer_run
.global anomalous_semantic_surface_renderer_failed
anomalous_semantic_surface_renderer_failed:
    ud2
    jmp anomalous_semantic_surface_renderer_failed
.size anomalous_semantic_surface_renderer_entry, . - anomalous_semantic_surface_renderer_entry

.section .text.semantic_surface_renderer_rite,"ax",@progbits
renderer_rite anomalous_semantic_surface_renderer_submit_denied_rite, anomalous_semantic_surface_renderer_submit_denied_return
renderer_rite anomalous_semantic_surface_renderer_delivery_rite, anomalous_semantic_surface_renderer_delivery_return
renderer_rite anomalous_semantic_surface_renderer_display_rite, anomalous_semantic_surface_renderer_display_return
renderer_rite anomalous_semantic_surface_renderer_exit_rite, anomalous_semantic_surface_renderer_exit_return

.section .note.GNU-stack,"",@progbits
