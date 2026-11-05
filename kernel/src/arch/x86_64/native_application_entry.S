.set native_application_kernel_data_selector, 0x10
.set native_application_user_data_selector, 0x2b
.set native_application_user_code_selector, 0x33
.set native_application_gate_vector, 0xf4
.text
.p2align 4

.extern x86_64_native_application_gate_dispatch
.global x86_64_native_application_gate_entry
.type x86_64_native_application_gate_entry, @function
# Only the normalized GPR frame is built here. The Rust dispatcher owns eager
# legacy CPU capture/restore and every returned privilege-stack frame.
x86_64_native_application_gate_entry:
    cld
    pushq $0
    pushq $native_application_gate_vector
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
.if {owned_tls_enabled}
    movq %rsp, %rdi
    callq x86_64_task_tls_capture_entry
.endif
    movw $native_application_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_native_application_gate_dispatch
    testq %rax, %rax
    jz .Lnative_application_invalid
    movq %rax, %rsp
    cmpq $native_application_user_code_selector, 144(%rsp)
    jne .Lnative_application_invalid
    cmpq $native_application_user_data_selector, 168(%rsp)
    jne .Lnative_application_invalid
    movw $native_application_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $native_application_kernel_data_selector, %ax
    movw %ax, %ss
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs
.if {owned_tls_enabled}
    movq %rsp, %rdi
    callq x86_64_task_tls_restore_user
.endif
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
.Lnative_application_invalid:
    cli
    ud2
.size x86_64_native_application_gate_entry, . - x86_64_native_application_gate_entry

.section .note.GNU-stack,"",@progbits
