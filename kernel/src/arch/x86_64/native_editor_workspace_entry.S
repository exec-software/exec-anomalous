.text
.p2align 4
.macro native_editor_workspace_entry vector
.global x86_64_native_editor_workspace_gate_f\vector
.type x86_64_native_editor_workspace_gate_f\vector, @function
x86_64_native_editor_workspace_gate_f\vector:
    cld
    pushq $0
    pushq $(0xf0 + \vector)
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
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_native_editor_workspace_dispatch
    testq %rax, %rax
    jz .Lnative_editor_workspace_invalid_\vector
    movq %rax, %rsp
    cmpq $0x33, 144(%rsp)
    jne .Lnative_editor_workspace_invalid_\vector
    cmpq $0x2b, 168(%rsp)
    jne .Lnative_editor_workspace_invalid_\vector
    movw $0x2b, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $0x10, %ax
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
.Lnative_editor_workspace_invalid_\vector:
    cli
    ud2
.size x86_64_native_editor_workspace_gate_f\vector, . - x86_64_native_editor_workspace_gate_f\vector
.endm
native_editor_workspace_entry 3
native_editor_workspace_entry 4


.section .note.GNU-stack,"",@progbits
