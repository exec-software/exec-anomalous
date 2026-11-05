.text
.p2align 4
.global x86_64_owned_user_enter
.type x86_64_owned_user_enter,@function
# RDI=entry, RSI=user RSP, RDX=saved kernel RSP cell, RCX=saved TS cell.
# The caller admits the root, code, both stacks, selectors and zero TLS bases.
# Only the exact terminal handler can consume this saved Rust continuation.
x86_64_owned_user_enter:
    cli
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rsp, (%rdx)
    movq %cr0, %rax
    movq %rax, %r8
    andq $8, %r8
    movq %r8, (%rcx)
    orq $8, %rax
    movq %rax, %cr0
    pushq $0x2b
    pushq %rsi
    pushq $2
    pushq $0x33
    pushq %rdi
    movw $0x2b, %ax
    movw %ax, %ds
    movw %ax, %es
    xorq %rax, %rax
    movw %ax, %fs
    movw %ax, %gs
    xorq %rbx, %rbx
    xorq %rcx, %rcx
    xorq %rdx, %rdx
    xorq %rbp, %rbp
    xorq %rsi, %rsi
    xorq %rdi, %rdi
    xorq %r8, %r8
    xorq %r9, %r9
    xorq %r10, %r10
    xorq %r11, %r11
    xorq %r12, %r12
    xorq %r13, %r13
    xorq %r14, %r14
    xorq %r15, %r15
    iretq
.size x86_64_owned_user_enter,.-x86_64_owned_user_enter

.global x86_64_owned_user_resume
.type x86_64_owned_user_resume,@function
# RDI=status, RSI=single-use saved kernel RSP, RDX=original TS bit.
x86_64_owned_user_resume:
    cli
    cmpq $0, %rdx
    je .Lowned_ts_valid
    cmpq $8, %rdx
    jne .Lowned_resume_invalid
.Lowned_ts_valid:
    movq %cr0, %rax
    andq $-9, %rax
    orq %rdx, %rax
    movq %rax, %cr0
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsi, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    movq %rdi, %rax
    retq
.Lowned_resume_invalid:
    ud2
.size x86_64_owned_user_resume,.-x86_64_owned_user_resume

# This copied fixture uses only relative branches and fixed admitted user data.
# UD2 sites form a private exact report protocol, not a user syscall interface.
.section .rodata.user_mapping.payload,"a",@progbits
.p2align 4
.global owned_user_payload_start
.global owned_user_report_one
.global owned_user_resume_one
.global owned_user_report_two
.global owned_user_resume_two
.global owned_user_denied_read
.global owned_user_exit
.global owned_user_payload_end
owned_user_payload_start:
    # Entry is a process-style 16-byte-aligned stack. Preserve a marker across
    # CALL/RET and both exception returns at different, noninitial RSP values.
    movabsq $0x800000, %rbx
    movq 0(%rbx), %r12
    movq 8(%rbx), %r13
    subq $16, %rsp
    movq %r13, 0(%rsp)
    callq .Lowned_nested
    cmpq %r13, 0(%rsp)
    jne .Lowned_payload_invalid
    movq $2, 16(%rbx)
    movq $2, %rax
owned_user_report_two:
    ud2
owned_user_resume_two:
    cmpq $1, %r12
    je .Lowned_fault
    movq $3, %rax
owned_user_exit:
    ud2
    jmp .Lowned_payload_invalid
.Lowned_fault:
    movabsq $0x100000, %rdi
owned_user_denied_read:
    movq (%rdi), %rax
.Lowned_payload_invalid:
    ud2
    jmp .Lowned_payload_invalid
.Lowned_nested:
    subq $8, %rsp
    movq %r13, 0(%rsp)
    movq $1, 16(%rbx)
    movq $1, %rax
owned_user_report_one:
    ud2
owned_user_resume_one:
    cmpq %r13, 0(%rsp)
    jne .Lowned_payload_invalid
    addq $8, %rsp
    retq
owned_user_payload_end:
.section .note.GNU-stack,"",@progbits
