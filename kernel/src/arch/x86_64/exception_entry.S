.text
.p2align 4

.set kernel_code_selector, 0x08
.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set user_code_selector, 0x33

.global x86_64_load_descriptor_tables
.type x86_64_load_descriptor_tables, @function
# System V inputs are RDI = packed GDTR pointer, RSI = code selector,
# RDX = data selector, and RCX = TSS selector. The routine requires CPL 0,
# temporarily uses RAX, reloads every visible segment selector, and balances
# its far-return stack frame before returning to Rust.
x86_64_load_descriptor_tables:
    lgdt (%rdi)
    pushq %rsi
    leaq descriptor_tables_loaded(%rip), %rax
    pushq %rax
    lretq
descriptor_tables_loaded:
    movw %dx, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movw %cx, %ax
    ltr %ax
    retq
.size x86_64_load_descriptor_tables, . - x86_64_load_descriptor_tables

.macro exception_without_error vector
.global x86_64_exception_stub_\vector
.type x86_64_exception_stub_\vector, @function
x86_64_exception_stub_\vector:
    pushq $0
    pushq $\vector
    jmp x86_64_exception_common
.size x86_64_exception_stub_\vector, . - x86_64_exception_stub_\vector
.endm

# The processor supplies an error code only for the vectors assigned to the
# second macro. Every entry leaves vector,error,RIP,CS,RFLAGS at one stable
# prefix; long mode always appends the hardware-saved RSP and SS.
.macro exception_with_error vector
.global x86_64_exception_stub_\vector
.type x86_64_exception_stub_\vector, @function
x86_64_exception_stub_\vector:
    pushq $\vector
    jmp x86_64_exception_common
.size x86_64_exception_stub_\vector, . - x86_64_exception_stub_\vector
.endm

exception_without_error 0
exception_without_error 1
exception_without_error 2
exception_without_error 3
exception_without_error 4
exception_without_error 5
exception_without_error 6
exception_without_error 7
exception_with_error 8
exception_without_error 9
exception_with_error 10
exception_with_error 11
exception_with_error 12
exception_with_error 13
exception_with_error 14
exception_without_error 15
exception_without_error 16
exception_with_error 17
exception_without_error 18
exception_without_error 19
exception_without_error 20
exception_with_error 21
exception_without_error 22
exception_without_error 23
exception_without_error 24
exception_without_error 25
exception_without_error 26
exception_without_error 27
exception_without_error 28
exception_with_error 29
exception_with_error 30
exception_without_error 31

.type x86_64_exception_common, @function
# The target disables SIMD, so the general registers form the complete execution
# prefix. The push order is mirrored by exception_frame, RDI receives its base,
# and RSP is aligned to the System V call boundary. Fatal dispatch never returns;
# an exactly contained user fault returns a validated frame on another live task
# stack. Interrupt gates already cleared IF; CLD restores the ABI invariant.
x86_64_exception_common:
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
    callq x86_64_exception_dispatch
    movq %rax, %rsp

    cmpq $user_code_selector, 144(%rsp)
    je .Lexception_return_user
    cmpq $kernel_code_selector, 144(%rsp)
    jne .Lexception_return_invalid
.Lexception_return_kernel:
    movw $kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    jmp .Lexception_restore
.Lexception_return_user:
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
.Lexception_restore:
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
.Lexception_return_invalid:
    cli
    ud2
.size x86_64_exception_common, . - x86_64_exception_common

.section .note.GNU-stack,"",@progbits
