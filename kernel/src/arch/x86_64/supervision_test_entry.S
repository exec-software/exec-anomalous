.set supervision_gate_vector, 0xf2

.set supervision_user_mailbox, 0x0000700000003000
.set supervision_mailbox_value_a, 0
.set supervision_mailbox_value_b, 8
.set supervision_mailbox_state, 16

.set supervision_operation_vessel_ready, 1
.set supervision_operation_supervisor_await_fault, 2
.set supervision_operation_supervisor_restart, 3
.set supervision_operation_vessel_complete, 4
.set supervision_operation_supervisor_complete, 5

.set supervision_wake_fault, 0x41
.set supervision_wake_completion, 0x42

.set supervision_generation_1_value_a, 0xa110ca1100010001
.set supervision_generation_1_value_b, 0xa110ca1100010002
.set supervision_generation_1_arm, 0xa110a11d00010001
.set supervision_supervisor_value_a, 0x5a9eca1100010001
.set supervision_supervisor_value_b, 0x5a9eca1100010002
.set supervision_supervisor_continuity, 0x5a9ec01700010001
.set supervision_generation_2_value_a, 0xa110ca1100020001
.set supervision_generation_2_value_b, 0xa110ca1100020002
.set supervision_generation_2_completion, 0xa110d09e00020001

.macro supervision_prepare_gate operation
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
    movabsq $supervision_user_mailbox, %r15
    movl $\operation, %eax
    pushq $0x02
    popfq
.endm

.section .rodata.supervision_test.vessel_generation_1,"a",@progbits
.p2align 4
.global x86_64_supervision_vessel_generation_1_start
.type x86_64_supervision_vessel_generation_1_start, @function
x86_64_supervision_vessel_generation_1_start:
    movabsq $supervision_user_mailbox, %r15
    movabsq $supervision_generation_1_value_a, %rax
    movq %rax, supervision_mailbox_value_a(%r15)
    movabsq $supervision_generation_1_value_b, %rax
    movq %rax, supervision_mailbox_value_b(%r15)

    supervision_prepare_gate supervision_operation_vessel_ready
    int $supervision_gate_vector
.global x86_64_supervision_vessel_generation_1_ready_return
x86_64_supervision_vessel_generation_1_ready_return:
.Lsupervision_generation_1_ready_return:
    pushq $0x202
    popfq
.Lsupervision_generation_1_wait:
    movabsq $supervision_generation_1_value_a, %rax
    cmpq %rax, supervision_mailbox_value_a(%r15)
    jne .Lsupervision_generation_1_failed
    movabsq $supervision_generation_1_value_b, %rax
    cmpq %rax, supervision_mailbox_value_b(%r15)
    jne .Lsupervision_generation_1_failed
    movq supervision_mailbox_state(%r15), %rax
    testq %rax, %rax
    jz .Lsupervision_generation_1_pause
    movabsq $supervision_generation_1_arm, %r14
    cmpq %r14, %rax
    jne .Lsupervision_generation_1_failed

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
    xorq %r15, %r15
    pushq $0x202
    popfq
.global x86_64_supervision_vessel_generation_1_fault_instruction
x86_64_supervision_vessel_generation_1_fault_instruction:
    ud2
.Lsupervision_generation_1_failed:
    ud2
    jmp .Lsupervision_generation_1_failed
.Lsupervision_generation_1_pause:
    pause
    jmp .Lsupervision_generation_1_wait
.global x86_64_supervision_vessel_generation_1_end
x86_64_supervision_vessel_generation_1_end:
.size x86_64_supervision_vessel_generation_1_start, . - x86_64_supervision_vessel_generation_1_start

.section .rodata.supervision_test.supervisor,"a",@progbits
.p2align 4
.global x86_64_supervision_supervisor_start
.type x86_64_supervision_supervisor_start, @function
x86_64_supervision_supervisor_start:
    movabsq $supervision_user_mailbox, %r15
    movabsq $supervision_supervisor_value_a, %rax
    movq %rax, supervision_mailbox_value_a(%r15)
    movabsq $supervision_supervisor_value_b, %rax
    movq %rax, supervision_mailbox_value_b(%r15)

    supervision_prepare_gate supervision_operation_supervisor_await_fault
    int $supervision_gate_vector
.global x86_64_supervision_supervisor_fault_return
x86_64_supervision_supervisor_fault_return:
    cmpq $supervision_wake_fault, %rax
    jne .Lsupervision_supervisor_failed
    movabsq $supervision_supervisor_value_a, %rax
    cmpq %rax, supervision_mailbox_value_a(%r15)
    jne .Lsupervision_supervisor_failed
    movabsq $supervision_supervisor_value_b, %rax
    cmpq %rax, supervision_mailbox_value_b(%r15)
    jne .Lsupervision_supervisor_failed
    jmp .Lsupervision_supervisor_fault_valid
.Lsupervision_supervisor_failed:
    ud2
    jmp .Lsupervision_supervisor_failed
.Lsupervision_supervisor_fault_valid:
    movabsq $supervision_supervisor_continuity, %rax
    movq %rax, supervision_mailbox_state(%r15)

    supervision_prepare_gate supervision_operation_supervisor_restart
    int $supervision_gate_vector
.global x86_64_supervision_supervisor_completion_return
x86_64_supervision_supervisor_completion_return:
    cmpq $supervision_wake_completion, %rax
    jne .Lsupervision_supervisor_failed
    movabsq $supervision_supervisor_value_a, %rax
    cmpq %rax, supervision_mailbox_value_a(%r15)
    jne .Lsupervision_supervisor_failed
    movabsq $supervision_supervisor_value_b, %rax
    cmpq %rax, supervision_mailbox_value_b(%r15)
    jne .Lsupervision_supervisor_failed
    movabsq $supervision_supervisor_continuity, %rax
    cmpq %rax, supervision_mailbox_state(%r15)
    jne .Lsupervision_supervisor_failed

    supervision_prepare_gate supervision_operation_supervisor_complete
    int $supervision_gate_vector
.global x86_64_supervision_supervisor_complete_return
x86_64_supervision_supervisor_complete_return:
    ud2
.global x86_64_supervision_supervisor_end
x86_64_supervision_supervisor_end:
.size x86_64_supervision_supervisor_start, . - x86_64_supervision_supervisor_start

.section .rodata.supervision_test.vessel_generation_2,"a",@progbits
.p2align 4
.global x86_64_supervision_vessel_generation_2_start
.type x86_64_supervision_vessel_generation_2_start, @function
x86_64_supervision_vessel_generation_2_start:
    movabsq $supervision_user_mailbox, %r15
    cmpq $0, supervision_mailbox_value_a(%r15)
    jne .Lsupervision_generation_2_failed
    cmpq $0, supervision_mailbox_value_b(%r15)
    jne .Lsupervision_generation_2_failed
    cmpq $0, supervision_mailbox_state(%r15)
    jne .Lsupervision_generation_2_failed
    movabsq $supervision_generation_2_value_a, %rax
    movq %rax, supervision_mailbox_value_a(%r15)
    movabsq $supervision_generation_2_value_b, %rax
    movq %rax, supervision_mailbox_value_b(%r15)
    movabsq $supervision_generation_2_completion, %rax
    movq %rax, supervision_mailbox_state(%r15)

    supervision_prepare_gate supervision_operation_vessel_complete
    int $supervision_gate_vector
.global x86_64_supervision_vessel_generation_2_completion_return
x86_64_supervision_vessel_generation_2_completion_return:
.Lsupervision_generation_2_failed:
    ud2
    jmp .Lsupervision_generation_2_failed
.global x86_64_supervision_vessel_generation_2_end
x86_64_supervision_vessel_generation_2_end:
.size x86_64_supervision_vessel_generation_2_start, . - x86_64_supervision_vessel_generation_2_start

.section .note.GNU-stack,"",@progbits
