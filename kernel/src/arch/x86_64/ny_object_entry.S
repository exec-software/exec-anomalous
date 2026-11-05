.set ny_object_kernel_data_selector, 0x10
.set ny_object_user_data_selector, 0x2b
.set ny_object_user_code_selector, 0x33
.set ny_object_gate_vector, 0xf3
.text
.p2align 4

.extern x86_64_ny_object_gate_dispatch
.global x86_64_ny_object_gate_entry
.type x86_64_ny_object_gate_entry, @function
# Only the normalized GPR frame is built here. The Rust dispatcher owns eager
# legacy CPU capture/restore and every returned privilege-stack frame.
x86_64_ny_object_gate_entry:
    cld
    pushq $0
    pushq $ny_object_gate_vector
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
    movw $ny_object_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_ny_object_gate_dispatch
    testq %rax, %rax
    jz .Lny_object_invalid
    movq %rax, %rsp
    cmpq $ny_object_user_code_selector, 144(%rsp)
    jne .Lny_object_invalid
    cmpq $ny_object_user_data_selector, 168(%rsp)
    jne .Lny_object_invalid
    movw $ny_object_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $ny_object_kernel_data_selector, %ax
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
.Lny_object_invalid:
    cli
    ud2
.size x86_64_ny_object_gate_entry, . - x86_64_ny_object_gate_entry

.section .note.GNU-stack,"",@progbits
