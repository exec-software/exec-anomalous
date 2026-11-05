.set renderer_scheduler_gate_vector, {scheduler_gate_vector}
.set renderer_rite_gate_vector, {rite_gate_vector}
.set renderer_user_mailbox, {user_mailbox_address}
.set renderer_scheduler_ready_operation, {scheduler_ready_operation}
.set renderer_initial_user_flags, {initial_user_flags}
.set renderer_running_user_flags, {running_user_flags}
.set renderer_mailbox_canary_a, {mailbox_canary_a_offset}
.set renderer_mailbox_canary_b, {mailbox_canary_b_offset}
.set renderer_mailbox_arm, {mailbox_arm_offset}
.set renderer_generation_1_canary_a, {generation_1_canary_a}
.set renderer_generation_1_canary_b, {generation_1_canary_b}
.set renderer_generation_1_arm, {generation_1_arm}

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

.section .text.renderer_generation_1_entry,"ax",@progbits
.p2align 4
.global anomalous_supervised_renderer_generation_1_entry
.type anomalous_supervised_renderer_generation_1_entry, @function
anomalous_supervised_renderer_generation_1_entry:
    cld
    movabsq $renderer_user_mailbox, %r15
    cmpq $0, renderer_mailbox_canary_a(%r15)
    jne anomalous_supervised_renderer_generation_1_failed
    cmpq $0, renderer_mailbox_canary_b(%r15)
    jne anomalous_supervised_renderer_generation_1_failed
    cmpq $0, renderer_mailbox_arm(%r15)
    jne anomalous_supervised_renderer_generation_1_failed
    movabsq $renderer_generation_1_canary_a, %rax
    movq %rax, renderer_mailbox_canary_a(%r15)
    movabsq $renderer_generation_1_canary_b, %rax
    movq %rax, renderer_mailbox_canary_b(%r15)

    renderer_zero_gate_registers
    movabsq $renderer_user_mailbox, %r15
    movl $renderer_scheduler_ready_operation, %eax
    pushq $renderer_initial_user_flags
    popfq
    int $renderer_scheduler_gate_vector
.global anomalous_supervised_renderer_generation_1_ready_return
anomalous_supervised_renderer_generation_1_ready_return:
    pushq $renderer_running_user_flags
    popfq

.global anomalous_supervised_renderer_generation_1_preemption_wait
anomalous_supervised_renderer_generation_1_preemption_wait:
    movabsq $renderer_generation_1_canary_a, %rax
    cmpq %rax, renderer_mailbox_canary_a(%r15)
    jne anomalous_supervised_renderer_generation_1_failed
    movabsq $renderer_generation_1_canary_b, %rax
    cmpq %rax, renderer_mailbox_canary_b(%r15)
    jne anomalous_supervised_renderer_generation_1_failed
    movq renderer_mailbox_arm(%r15), %rax
    testq %rax, %rax
    jz .Lrenderer_generation_1_pause
    movabsq $renderer_generation_1_arm, %r14
    cmpq %r14, %rax
    jne anomalous_supervised_renderer_generation_1_failed

.global anomalous_supervised_renderer_generation_1_preemption_resume
anomalous_supervised_renderer_generation_1_preemption_resume:
    movabsq $renderer_user_mailbox, %rdi
    movl $512, %ecx
    xorq %rax, %rax
    rep stosq
    callq anomalous_supervised_renderer_generation_1_run
    testq %rax, %rax
    jne anomalous_supervised_renderer_generation_1_failed
.global anomalous_supervised_renderer_generation_1_fault
anomalous_supervised_renderer_generation_1_fault:
    ud2
    jmp anomalous_supervised_renderer_generation_1_failed

.Lrenderer_generation_1_pause:
    pause
    jmp anomalous_supervised_renderer_generation_1_preemption_wait

.global anomalous_supervised_renderer_generation_1_failed
anomalous_supervised_renderer_generation_1_failed:
    ud2
    jmp anomalous_supervised_renderer_generation_1_failed
.size anomalous_supervised_renderer_generation_1_entry, . - anomalous_supervised_renderer_generation_1_entry

.section .text.renderer_generation_2_entry,"ax",@progbits
.p2align 4
.global anomalous_supervised_renderer_generation_2_entry
.type anomalous_supervised_renderer_generation_2_entry, @function
anomalous_supervised_renderer_generation_2_entry:
    cld
    movabsq $renderer_user_mailbox, %rsi
    movl $512, %ecx
    xorq %rax, %rax
.Lrenderer_generation_2_zero_check:
    orq (%rsi), %rax
    addq $8, %rsi
    loop .Lrenderer_generation_2_zero_check
    testq %rax, %rax
    jne anomalous_supervised_renderer_generation_2_failed
    callq anomalous_supervised_renderer_generation_2_run
.global anomalous_supervised_renderer_generation_2_exit_return
anomalous_supervised_renderer_generation_2_exit_return:
.global anomalous_supervised_renderer_generation_2_failed
anomalous_supervised_renderer_generation_2_failed:
    ud2
    jmp anomalous_supervised_renderer_generation_2_failed
.size anomalous_supervised_renderer_generation_2_entry, . - anomalous_supervised_renderer_generation_2_entry

.section .text.renderer_rite,"ax",@progbits
.p2align 4
.global anomalous_supervised_renderer_submit_display_rite
.type anomalous_supervised_renderer_submit_display_rite, @function
anomalous_supervised_renderer_submit_display_rite:
    renderer_save_rust_registers
    renderer_zero_gate_registers
    movabsq $renderer_user_mailbox, %r15
    pushq $renderer_initial_user_flags
    popfq
    int $renderer_rite_gate_vector
.global anomalous_supervised_renderer_display_rite_return
anomalous_supervised_renderer_display_rite_return:
    renderer_restore_rust_registers
    retq
.size anomalous_supervised_renderer_submit_display_rite, . - anomalous_supervised_renderer_submit_display_rite

.p2align 4
.global anomalous_supervised_renderer_submit_exit_rite
.type anomalous_supervised_renderer_submit_exit_rite, @function
anomalous_supervised_renderer_submit_exit_rite:
    renderer_save_rust_registers
    renderer_zero_gate_registers
    movabsq $renderer_user_mailbox, %r15
    pushq $renderer_initial_user_flags
    popfq
    int $renderer_rite_gate_vector
.global anomalous_supervised_renderer_task_exit_rite_return
anomalous_supervised_renderer_task_exit_rite_return:
    renderer_restore_rust_registers
    retq
.size anomalous_supervised_renderer_submit_exit_rite, . - anomalous_supervised_renderer_submit_exit_rite

.section .note.GNU-stack,"",@progbits
