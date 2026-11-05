.set renderer_test_kernel_data_selector, 0x10
.set renderer_test_user_data_selector, 0x2b
.set renderer_test_user_code_selector, 0x33
.set renderer_test_scheduler_gate_vector, 0xf2
.set renderer_test_rite_gate_vector, 0xf3

.set renderer_test_supervisor_mailbox, 0x0000700000003000
.set renderer_test_frame_code_segment, 144
.set renderer_test_frame_stack_segment, 168

.set renderer_test_control_await_fault, 2
.set renderer_test_control_restart, 3
.set renderer_test_control_complete, 5
.set renderer_test_wake_fault, 0x41
.set renderer_test_wake_completion, 0x42

.set renderer_test_supervisor_marker_a, 0x196bf3a04d72c85e
.set renderer_test_supervisor_marker_b, 0xd4a70e5b832c61f9
.set renderer_test_supervisor_continuity, 0x5ec128a7f40d936b
.set renderer_test_supervisor_marker_a_offset, 0
.set renderer_test_supervisor_marker_b_offset, 8
.set renderer_test_supervisor_state_offset, 16

.macro renderer_test_prepare_control operation_value
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
    movabsq $renderer_test_supervisor_mailbox, %r15
    movl $\operation_value, %eax
    pushq $0x02
    popfq
.endm

.text
.p2align 4

.extern x86_64_supervised_renderer_rite_dispatch
.global x86_64_supervised_renderer_rite_gate_entry
.type x86_64_supervised_renderer_rite_gate_entry, @function
# A CPL-3 interrupt supplies RIP, CS, RFLAGS, RSP, and SS. Two synthetic
# words plus every GPR form the scheduler's 22-qword saved-task frame. Rust
# returns either this exact renderer frame or the awakened supervisor frame.
x86_64_supervised_renderer_rite_gate_entry:
    cld
    pushq $0
    pushq $renderer_test_rite_gate_vector
    pushq %r15
    pushq %r14
    pushq %r13
    pushq %r12
    pushq %r11
    pushq %r10
    pushq %r9
    pushq %r8
    pushq %rdi
    pushq %rsi
    pushq %rbp
    pushq %rdx
    pushq %rcx
    pushq %rbx
    pushq %rax
    movw $renderer_test_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_supervised_renderer_rite_dispatch
    testq %rax, %rax
    jz .Lrenderer_test_rite_gate_invalid
    movq %rax, %rsp

    cmpq $renderer_test_user_code_selector, renderer_test_frame_code_segment(%rsp)
    jne .Lrenderer_test_rite_gate_invalid
    cmpq $renderer_test_user_data_selector, renderer_test_frame_stack_segment(%rsp)
    jne .Lrenderer_test_rite_gate_invalid
    movw $renderer_test_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $renderer_test_kernel_data_selector, %ax
    movw %ax, %ss
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs
    popq %rax
    popq %rbx
    popq %rcx
    popq %rdx
    popq %rbp
    popq %rsi
    popq %rdi
    popq %r8
    popq %r9
    popq %r10
    popq %r11
    popq %r12
    popq %r13
    popq %r14
    popq %r15
    addq $16, %rsp
    iretq
.Lrenderer_test_rite_gate_invalid:
    cli
    ud2
.size x86_64_supervised_renderer_rite_gate_entry, . - x86_64_supervised_renderer_rite_gate_entry

.section .rodata.supervised_renderer_test.supervisor,"a",@progbits
.p2align 4
.global x86_64_supervised_renderer_supervisor_start
.type x86_64_supervised_renderer_supervisor_start, @function
x86_64_supervised_renderer_supervisor_start:
    movabsq $renderer_test_supervisor_mailbox, %r15
    movabsq $renderer_test_supervisor_marker_a, %rax
    movq %rax, renderer_test_supervisor_marker_a_offset(%r15)
    movabsq $renderer_test_supervisor_marker_b, %rax
    movq %rax, renderer_test_supervisor_marker_b_offset(%r15)

    renderer_test_prepare_control renderer_test_control_await_fault
    int $renderer_test_scheduler_gate_vector
.global x86_64_supervised_renderer_supervisor_fault_return
x86_64_supervised_renderer_supervisor_fault_return:
    cmpq $renderer_test_wake_fault, %rax
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_marker_a, %rax
    cmpq %rax, renderer_test_supervisor_marker_a_offset(%r15)
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_marker_b, %rax
    cmpq %rax, renderer_test_supervisor_marker_b_offset(%r15)
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_continuity, %rax
    movq %rax, renderer_test_supervisor_state_offset(%r15)

    renderer_test_prepare_control renderer_test_control_restart
    int $renderer_test_scheduler_gate_vector
.global x86_64_supervised_renderer_supervisor_completion_return
x86_64_supervised_renderer_supervisor_completion_return:
    cmpq $renderer_test_wake_completion, %rax
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_marker_a, %rax
    cmpq %rax, renderer_test_supervisor_marker_a_offset(%r15)
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_marker_b, %rax
    cmpq %rax, renderer_test_supervisor_marker_b_offset(%r15)
    jne .Lrenderer_test_supervisor_failed
    movabsq $renderer_test_supervisor_continuity, %rax
    cmpq %rax, renderer_test_supervisor_state_offset(%r15)
    jne .Lrenderer_test_supervisor_failed

    renderer_test_prepare_control renderer_test_control_complete
    int $renderer_test_scheduler_gate_vector
.global x86_64_supervised_renderer_supervisor_complete_return
x86_64_supervised_renderer_supervisor_complete_return:
.Lrenderer_test_supervisor_failed:
    ud2
    jmp .Lrenderer_test_supervisor_failed
.global x86_64_supervised_renderer_supervisor_end
x86_64_supervised_renderer_supervisor_end:
.size x86_64_supervised_renderer_supervisor_start, . - x86_64_supervised_renderer_supervisor_start

.section .note.GNU-stack,"",@progbits
