.text
.p2align 4

.set kernel_code_selector, 0x08
.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set user_code_selector, 0x33

.macro external_interrupt vector
.global x86_64_interrupt_stub_\vector
.type x86_64_interrupt_stub_\vector, @function
x86_64_interrupt_stub_\vector:
    pushq $0
    pushq $\vector
    jmp x86_64_interrupt_common
.size x86_64_interrupt_stub_\vector, . - x86_64_interrupt_stub_\vector
.endm

external_interrupt 32
external_interrupt 33
external_interrupt 34
external_interrupt 35
external_interrupt 36
external_interrupt 37
external_interrupt 38
external_interrupt 39
external_interrupt 40
external_interrupt 41
external_interrupt 42
external_interrupt 43
external_interrupt 44
external_interrupt 45
external_interrupt 46
external_interrupt 47

.type x86_64_interrupt_common, @function
# The x86_64-unknown-none target disables its red zone and all SIMD. Save every
# general register around the returning Rust dispatcher. Rust returns the exact
# validated frame to resume, which can belong to a different task stack. The
# hardware-pushed RFLAGS word restores the selected direction and interrupt
# flags through IRETQ; CLD establishes Rust's direction invariant.
x86_64_interrupt_common:
    cld
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
    movw $kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_interrupt_dispatch
    movq %rax, %rsp

    cmpq $user_code_selector, 144(%rsp)
    je .Linterrupt_return_user
    cmpq $kernel_code_selector, 144(%rsp)
    jne .Linterrupt_return_invalid
.Linterrupt_return_kernel:
    movw $kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    jmp .Linterrupt_restore
.Linterrupt_return_user:
    movw $user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $kernel_data_selector, %ax
    movw %ax, %ss
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs
.if {owned_tls_enabled}
    movq %rsp, %rdi
    callq x86_64_task_tls_restore_user
.endif
.Linterrupt_restore:
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
.Linterrupt_return_invalid:
    cli
    ud2
.size x86_64_interrupt_common, . - x86_64_interrupt_common

.section .note.GNU-stack,"",@progbits
