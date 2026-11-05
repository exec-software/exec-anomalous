.set workspace_kernel_data_selector, 0x10
.set workspace_user_data_selector, 0x2b
.set workspace_user_code_selector, 0x33
.set workspace_rite_gate_vector, 0xf3
.set workspace_frame_code_segment, 144
.set workspace_frame_stack_segment, 168
.text
.p2align 4

.extern x86_64_workspace_rite_dispatch
.global x86_64_workspace_rite_gate_entry
.type x86_64_workspace_rite_gate_entry, @function
# A CPL-3 interrupt frame, two synthetic words, and every GPR form the shared
# 22-qword scheduler frame. Rust returns only the exact validated caller.
# Withdrawal restores the bootstrap continuation and never returns here.
x86_64_workspace_rite_gate_entry:
    cld
    pushq $0
    pushq $workspace_rite_gate_vector
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

    movw $workspace_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_workspace_rite_dispatch
    testq %rax, %rax
    jz .Lworkspace_rite_gate_invalid
    movq %rax, %rsp

    cmpq $workspace_user_code_selector, workspace_frame_code_segment(%rsp)
    jne .Lworkspace_rite_gate_invalid
    cmpq $workspace_user_data_selector, workspace_frame_stack_segment(%rsp)
    jne .Lworkspace_rite_gate_invalid
    movw $workspace_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $workspace_kernel_data_selector, %ax
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

.Lworkspace_rite_gate_invalid:
    cli
    ud2
.size x86_64_workspace_rite_gate_entry, . - x86_64_workspace_rite_gate_entry


.section .note.GNU-stack,"",@progbits
