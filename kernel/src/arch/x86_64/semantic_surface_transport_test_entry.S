.set semantic_surface_kernel_data_selector, 0x10
.set semantic_surface_user_data_selector, 0x2b
.set semantic_surface_user_code_selector, 0x33
.set semantic_surface_rite_gate_vector, 0xf3

.text
.p2align 4

.extern x86_64_semantic_surface_transport_rite_dispatch
.global x86_64_semantic_surface_transport_rite_gate_entry
.type x86_64_semantic_surface_transport_rite_gate_entry, @function
# A CPL-3 interrupt frame, two synthetic words, and every general register form
# the shared 22-qword scheduler frame. Ordinary Rite calls return the caller's
# frame. A terminal task-exit consumes the saved kernel continuation in Rust
# and therefore never returns through this assembly entry.
x86_64_semantic_surface_transport_rite_gate_entry:
    cld
    pushq $0
    pushq $semantic_surface_rite_gate_vector
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

    movw $semantic_surface_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_semantic_surface_transport_rite_dispatch
    testq %rax, %rax
    jz .Lsemantic_surface_rite_gate_invalid
    movq %rax, %rsp

    cmpq $semantic_surface_user_code_selector, 144(%rsp)
    jne .Lsemantic_surface_rite_gate_invalid
    cmpq $semantic_surface_user_data_selector, 168(%rsp)
    jne .Lsemantic_surface_rite_gate_invalid
    movw $semantic_surface_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $semantic_surface_kernel_data_selector, %ax
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

.Lsemantic_surface_rite_gate_invalid:
    cli
    ud2
.size x86_64_semantic_surface_transport_rite_gate_entry, . - x86_64_semantic_surface_transport_rite_gate_entry

.section .note.GNU-stack,"",@progbits
