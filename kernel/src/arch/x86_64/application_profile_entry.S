.set application_profile_kernel_data_selector, 0x10
.set application_profile_user_data_selector, 0x2b
.set application_profile_user_code_selector, 0x33
.set application_profile_gate_vector, 0xf2
.set application_profile_mailbox, 0x0000000000800000
.set application_profile_canary_a, 0x61a7c0def17e0001
.set application_profile_canary_b, 0x9ee7c0deba5e0002
.text
.p2align 4

.extern x86_64_application_profile_gate_dispatch
.global x86_64_application_profile_gate_entry
.type x86_64_application_profile_gate_entry, @function
# The ordinary saved integer context. The dispatcher owns every selected frame;
# termination abandons this stack only through the bootstrap continuation.
x86_64_application_profile_gate_entry:
    cld
    pushq $0
    pushq $application_profile_gate_vector
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
    movw $application_profile_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_application_profile_gate_dispatch
    testq %rax, %rax
    jz .Lapplication_profile_invalid
    movq %rax, %rsp
    cmpq $application_profile_user_code_selector, 144(%rsp)
    jne .Lapplication_profile_invalid
    cmpq $application_profile_user_data_selector, 168(%rsp)
    jne .Lapplication_profile_invalid
    movw $application_profile_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $application_profile_kernel_data_selector, %ax
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
.Lapplication_profile_invalid:
    cli
    ud2
.size x86_64_application_profile_gate_entry, . - x86_64_application_profile_gate_entry

# Each template stays inside its copied RX page. The CPU owner supplies clean
# x87/SSE2 state before entry; neither AVX nor TLS is part of this profile.
.macro application_profile_payload name, role
.section .rodata.application_profile_test.\name,"a",@progbits
.p2align 4
.global x86_64_application_profile_\name\()_start
.type x86_64_application_profile_\name\()_start, @function
x86_64_application_profile_\name\()_start:
    call .Lapplication_profile_\name\()_outer
    ud2
.Lapplication_profile_\name\()_outer:
    subq $8, %rsp
    call .Lapplication_profile_\name\()_worker
    ud2
.Lapplication_profile_\name\()_worker:
    movabsq $application_profile_mailbox, %r15
    movabsq $(application_profile_canary_a + \role), %r12
    movabsq $(application_profile_canary_b + \role), %r13
    movq %r12, 0(%r15)
    movq %r13, 8(%r15)
    movq %r12, -32(%rsp)
    movq $1, %r14
    movq %r14, 24(%r15)
    movl $1, %eax
    pushq $0x2
    popfq
    int $application_profile_gate_vector
.global x86_64_application_profile_\name\()_initial_return
x86_64_application_profile_\name\()_initial_return:
    movdqu .Lapplication_profile_\name\()_xmm + 0(%rip), %xmm0
    movdqu .Lapplication_profile_\name\()_xmm + 16(%rip), %xmm1
    movdqu .Lapplication_profile_\name\()_xmm + 32(%rip), %xmm2
    movdqu .Lapplication_profile_\name\()_xmm + 48(%rip), %xmm3
    movdqu .Lapplication_profile_\name\()_xmm + 64(%rip), %xmm4
    movdqu .Lapplication_profile_\name\()_xmm + 80(%rip), %xmm5
    movdqu .Lapplication_profile_\name\()_xmm + 96(%rip), %xmm6
    movdqu .Lapplication_profile_\name\()_xmm + 112(%rip), %xmm7
    movdqu .Lapplication_profile_\name\()_xmm + 128(%rip), %xmm8
    movdqu .Lapplication_profile_\name\()_xmm + 144(%rip), %xmm9
    movdqu .Lapplication_profile_\name\()_xmm + 160(%rip), %xmm10
    movdqu .Lapplication_profile_\name\()_xmm + 176(%rip), %xmm11
    movdqu .Lapplication_profile_\name\()_xmm + 192(%rip), %xmm12
    movdqu .Lapplication_profile_\name\()_xmm + 208(%rip), %xmm13
    movdqu .Lapplication_profile_\name\()_xmm + 224(%rip), %xmm14
    movdqu .Lapplication_profile_\name\()_xmm + 240(%rip), %xmm15
    fldcw .Lapplication_profile_\name\()_fcw(%rip)
    ldmxcsr .Lapplication_profile_\name\()_mxcsr(%rip)
    fldl .Lapplication_profile_\name\()_x87(%rip)
    movl $2, %eax
    int $application_profile_gate_vector
.global x86_64_application_profile_\name\()_ready_return
x86_64_application_profile_\name\()_ready_return:
.Lapplication_profile_\name\()_loop:
    cmpq %r12, 0(%r15)
    jne .Lapplication_profile_\name\()_failed
    cmpq %r13, 8(%r15)
    jne .Lapplication_profile_\name\()_failed
    cmpq %r12, -32(%rsp)
    jne .Lapplication_profile_\name\()_failed
    cmpq $0, 16(%r15)
    jne .Lapplication_profile_\name\()_armed
    incq %r14
    jz .Lapplication_profile_\name\()_failed
    movq %r14, 24(%r15)
    pause
    jmp .Lapplication_profile_\name\()_loop
.Lapplication_profile_\name\()_armed:
    cmpq $1, 16(%r15)
    jne .Lapplication_profile_\name\()_failed
.if \role == 0
    cmpq $0, 32(%r15)
    je .Lapplication_profile_\name\()_divide
    cmpq $6, 32(%r15)
    je .Lapplication_profile_\name\()_invalid
    cmpq $13, 32(%r15)
    je .Lapplication_profile_\name\()_protection
    cmpq $14, 32(%r15)
    jne .Lapplication_profile_\name\()_failed
    # Fetch from an absent page: the terminal RIP cannot pass return admission.
    leaq 8192(%r15), %rax
    jmp *%rax
    ud2
.Lapplication_profile_\name\()_divide:
    xorl %edx, %edx
    movl $1, %eax
    xorl %ecx, %ecx
    divq %rcx
    ud2
.Lapplication_profile_\name\()_protection:
    # Noncanonical for either 48-bit or 57-bit address translation.
    movabsq $0x0100000000000000, %rax
    movq (%rax), %rax
    ud2
.Lapplication_profile_\name\()_invalid:
.endif
.if \role == 1
    # Terminal fault attribution does not require a resumable user stack.
    xorq %rsp, %rsp
.endif
    # The peer terminates by a generic user fault after survivor progress.
    ud2
.Lapplication_profile_\name\()_failed:
    ud2
.p2align 4
.Lapplication_profile_\name\()_xmm:
    .quad (0x3141592600000000 + (\role << 16) + 0), (0x2718281800000000 + (\role << 16) + 0)
    .quad (0x3141592600000000 + (\role << 16) + 1), (0x2718281800000000 + (\role << 16) + 1)
    .quad (0x3141592600000000 + (\role << 16) + 2), (0x2718281800000000 + (\role << 16) + 2)
    .quad (0x3141592600000000 + (\role << 16) + 3), (0x2718281800000000 + (\role << 16) + 3)
    .quad (0x3141592600000000 + (\role << 16) + 4), (0x2718281800000000 + (\role << 16) + 4)
    .quad (0x3141592600000000 + (\role << 16) + 5), (0x2718281800000000 + (\role << 16) + 5)
    .quad (0x3141592600000000 + (\role << 16) + 6), (0x2718281800000000 + (\role << 16) + 6)
    .quad (0x3141592600000000 + (\role << 16) + 7), (0x2718281800000000 + (\role << 16) + 7)
    .quad (0x3141592600000000 + (\role << 16) + 8), (0x2718281800000000 + (\role << 16) + 8)
    .quad (0x3141592600000000 + (\role << 16) + 9), (0x2718281800000000 + (\role << 16) + 9)
    .quad (0x3141592600000000 + (\role << 16) + 10), (0x2718281800000000 + (\role << 16) + 10)
    .quad (0x3141592600000000 + (\role << 16) + 11), (0x2718281800000000 + (\role << 16) + 11)
    .quad (0x3141592600000000 + (\role << 16) + 12), (0x2718281800000000 + (\role << 16) + 12)
    .quad (0x3141592600000000 + (\role << 16) + 13), (0x2718281800000000 + (\role << 16) + 13)
    .quad (0x3141592600000000 + (\role << 16) + 14), (0x2718281800000000 + (\role << 16) + 14)
    .quad (0x3141592600000000 + (\role << 16) + 15), (0x2718281800000000 + (\role << 16) + 15)
.Lapplication_profile_\name\()_fcw:
    .short (0x037f + (\role << 10))
.p2align 2
.Lapplication_profile_\name\()_mxcsr:
    .long (0x1f80 + (\role << 13))
.p2align 3
.Lapplication_profile_\name\()_x87:
.if \role == 0
    .quad 0x3ff0000000000000
.else
    .quad 0x4000000000000000
.endif
.global x86_64_application_profile_\name\()_end
x86_64_application_profile_\name\()_end:
.size x86_64_application_profile_\name\()_start, . - x86_64_application_profile_\name\()_start
.endm

application_profile_payload faulting, 0
application_profile_payload peer, 1
.section .note.GNU-stack,"",@progbits
