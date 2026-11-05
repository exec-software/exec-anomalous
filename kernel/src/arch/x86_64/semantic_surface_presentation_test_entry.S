.set semantic_presentation_kernel_data_selector, 0x10
.set semantic_presentation_user_data_selector, 0x2b
.set semantic_presentation_user_code_selector, 0x33
.set semantic_presentation_scheduler_gate_vector, 0xf2
.set semantic_presentation_rite_gate_vector, 0xf3

.set semantic_presentation_supervisor_mailbox, 0x0000700000003000
.set semantic_presentation_frame_code_segment, 144
.set semantic_presentation_frame_stack_segment, 168

.set semantic_presentation_control_await_completion, 2
.set semantic_presentation_control_complete, 5
.set semantic_presentation_wake_completion, 0x42

.set semantic_presentation_supervisor_marker_a, 0x8d6b19e24af530c7
.set semantic_presentation_supervisor_marker_b, 0x31c7e8945db20fa6
.set semantic_presentation_supervisor_continuity, 0x6ab43d09f172c85e
.set semantic_presentation_supervisor_marker_a_offset, 0
.set semantic_presentation_supervisor_marker_b_offset, 8
.set semantic_presentation_supervisor_state_offset, 16

.macro semantic_presentation_prepare_control operation_value
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
    movabsq $semantic_presentation_supervisor_mailbox, %r15
    movl $\operation_value, %eax
    pushq $0x02
    popfq
.endm

.text
.p2align 4

.extern x86_64_semantic_surface_presentation_rite_dispatch
.global x86_64_semantic_surface_presentation_rite_gate_entry
.type x86_64_semantic_surface_presentation_rite_gate_entry, @function
# A CPL-3 interrupt frame, two synthetic words, and every GPR form the shared
# 22-qword scheduler frame. Rust returns only the exact validated caller or
# awakened-supervisor frame. Terminal exits never return to the retired task.
x86_64_semantic_surface_presentation_rite_gate_entry:
    cld
    pushq $0
    pushq $semantic_presentation_rite_gate_vector
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

    movw $semantic_presentation_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_semantic_surface_presentation_rite_dispatch
    testq %rax, %rax
    jz .Lsemantic_presentation_rite_gate_invalid
    movq %rax, %rsp

    cmpq $semantic_presentation_user_code_selector, semantic_presentation_frame_code_segment(%rsp)
    jne .Lsemantic_presentation_rite_gate_invalid
    cmpq $semantic_presentation_user_data_selector, semantic_presentation_frame_stack_segment(%rsp)
    jne .Lsemantic_presentation_rite_gate_invalid
    movw $semantic_presentation_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $semantic_presentation_kernel_data_selector, %ax
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

.Lsemantic_presentation_rite_gate_invalid:
    cli
    ud2
.size x86_64_semantic_surface_presentation_rite_gate_entry, . - x86_64_semantic_surface_presentation_rite_gate_entry

.section .rodata.semantic_surface_presentation_test.supervisor,"a",@progbits
.p2align 4
.global x86_64_semantic_surface_presentation_supervisor_start
.type x86_64_semantic_surface_presentation_supervisor_start, @function
x86_64_semantic_surface_presentation_supervisor_start:
    movabsq $semantic_presentation_supervisor_mailbox, %r15
    movabsq $semantic_presentation_supervisor_marker_a, %rax
    movq %rax, semantic_presentation_supervisor_marker_a_offset(%r15)
    movabsq $semantic_presentation_supervisor_marker_b, %rax
    movq %rax, semantic_presentation_supervisor_marker_b_offset(%r15)

    semantic_presentation_prepare_control semantic_presentation_control_await_completion
    int $semantic_presentation_scheduler_gate_vector
.global x86_64_semantic_surface_presentation_supervisor_completion_return
x86_64_semantic_surface_presentation_supervisor_completion_return:
    cmpq $semantic_presentation_wake_completion, %rax
    jne .Lsemantic_presentation_supervisor_failed
    movabsq $semantic_presentation_supervisor_marker_a, %rax
    cmpq %rax, semantic_presentation_supervisor_marker_a_offset(%r15)
    jne .Lsemantic_presentation_supervisor_failed
    movabsq $semantic_presentation_supervisor_marker_b, %rax
    cmpq %rax, semantic_presentation_supervisor_marker_b_offset(%r15)
    jne .Lsemantic_presentation_supervisor_failed
    movabsq $semantic_presentation_supervisor_continuity, %rax
    movq %rax, semantic_presentation_supervisor_state_offset(%r15)

    semantic_presentation_prepare_control semantic_presentation_control_complete
    int $semantic_presentation_scheduler_gate_vector
.global x86_64_semantic_surface_presentation_supervisor_complete_return
x86_64_semantic_surface_presentation_supervisor_complete_return:
.Lsemantic_presentation_supervisor_failed:
    ud2
    jmp .Lsemantic_presentation_supervisor_failed
.global x86_64_semantic_surface_presentation_supervisor_end
x86_64_semantic_surface_presentation_supervisor_end:
.size x86_64_semantic_surface_presentation_supervisor_start, . - x86_64_semantic_surface_presentation_supervisor_start

.section .note.GNU-stack,"",@progbits
