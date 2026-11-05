.set linux_syscall_kernel_data_selector, 0x10
.set linux_syscall_user_data_selector, 0x2b
.set linux_syscall_user_code_selector, 0x33
.set linux_syscall_synthetic_vector, 0x100
.text
.p2align 4

.extern x86_64_linux_syscall_dispatch
.extern x86_64_linux_syscall_stack_top
.extern x86_64_linux_syscall_user_stack
.global x86_64_linux_syscall_entry
.type x86_64_linux_syscall_entry, @function
# Hardware supplies next RIP in RCX and saved RFLAGS in R11, but leaves RSP
# untrusted. FMASK already cleared IF/DF/TF/AC. This sole-BSP entry touches only
# supervisor RIP-relative scratch before loading the retained privilege stack.
# No PUSH, CALL, stack-relative access, red-zone write, or SWAPGS precedes it.
x86_64_linux_syscall_entry:
    movq %rsp, x86_64_linux_syscall_user_stack(%rip)
    movq x86_64_linux_syscall_stack_top(%rip), %rsp
.global x86_64_linux_syscall_trusted_stack
x86_64_linux_syscall_trusted_stack:
    pushq $linux_syscall_user_data_selector
    pushq x86_64_linux_syscall_user_stack(%rip)
    pushq %r11
    pushq $linux_syscall_user_code_selector
    pushq %rcx
    pushq $0
    pushq $linux_syscall_synthetic_vector
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
    movq $0, x86_64_linux_syscall_user_stack(%rip)
.if {owned_tls_enabled}
    movq %rsp, %rdi
    callq x86_64_task_tls_capture_entry
.endif
    movw $linux_syscall_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_linux_syscall_dispatch
    testq %rax, %rax
    jz .Llinux_syscall_invalid
    movq %rax, %rsp
    cmpq $linux_syscall_user_code_selector, 144(%rsp)
    jne .Llinux_syscall_invalid
    cmpq $linux_syscall_user_data_selector, 168(%rsp)
    jne .Llinux_syscall_invalid
    movw $linux_syscall_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $linux_syscall_kernel_data_selector, %ax
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
.Llinux_syscall_invalid:
    cli
    ud2
.size x86_64_linux_syscall_entry, . - x86_64_linux_syscall_entry
.section .note.GNU-stack,"",@progbits
