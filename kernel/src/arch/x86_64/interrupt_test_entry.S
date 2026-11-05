.text
.p2align 4

.global x86_64_interrupt_test_register_probe
.type x86_64_interrupt_test_register_probe, @function
# Load every general register other than RSP plus known CF/IF/DF state, halt
# until one real external interrupt returns through IRETQ, and compare them.
# Clear DF again and preserve the System V callee-save set before returning.
x86_64_interrupt_test_register_probe:
    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq $0x101, %rax
    movq $0x102, %rbx
    movq $0x103, %rcx
    movq $0x104, %rdx
    movq $0x105, %rbp
    movq $0x106, %rsi
    movq $0x107, %rdi
    movq $0x108, %r8
    movq $0x109, %r9
    movq $0x10a, %r10
    movq $0x10b, %r11
    movq $0x10c, %r12
    movq $0x10d, %r13
    movq $0x10e, %r14
    movq $0x10f, %r15
    std
    stc
    hlt
    pushfq
    testq $0x001, (%rsp)
    jz interrupt_flags_probe_failed
    testq $0x200, (%rsp)
    jz interrupt_flags_probe_failed
    testq $0x400, (%rsp)
    jz interrupt_flags_probe_failed
    addq $8, %rsp
    cld
    cmpq $0x101, %rax
    jne interrupt_register_probe_failed
    cmpq $0x102, %rbx
    jne interrupt_register_probe_failed
    cmpq $0x103, %rcx
    jne interrupt_register_probe_failed
    cmpq $0x104, %rdx
    jne interrupt_register_probe_failed
    cmpq $0x105, %rbp
    jne interrupt_register_probe_failed
    cmpq $0x106, %rsi
    jne interrupt_register_probe_failed
    cmpq $0x107, %rdi
    jne interrupt_register_probe_failed
    cmpq $0x108, %r8
    jne interrupt_register_probe_failed
    cmpq $0x109, %r9
    jne interrupt_register_probe_failed
    cmpq $0x10a, %r10
    jne interrupt_register_probe_failed
    cmpq $0x10b, %r11
    jne interrupt_register_probe_failed
    cmpq $0x10c, %r12
    jne interrupt_register_probe_failed
    cmpq $0x10d, %r13
    jne interrupt_register_probe_failed
    cmpq $0x10e, %r14
    jne interrupt_register_probe_failed
    cmpq $0x10f, %r15
    jne interrupt_register_probe_failed
    movl $1, %eax
    jmp interrupt_register_probe_restore
interrupt_flags_probe_failed:
    addq $8, %rsp
    cld
interrupt_register_probe_failed:
    xorl %eax, %eax
interrupt_register_probe_restore:
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    retq
.size x86_64_interrupt_test_register_probe, . - x86_64_interrupt_test_register_probe

.section .note.GNU-stack,"",@progbits
