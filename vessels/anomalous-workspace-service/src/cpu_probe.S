
.section .text.workspace_cpu_probe,"ax",@progbits
.p2align 4
.global anomalous_workspace_cpu_probe_start
anomalous_workspace_cpu_probe_start:
.global anomalous_workspace_cpu_initialize
.type anomalous_workspace_cpu_initialize, @function
anomalous_workspace_cpu_initialize:
    # Observe actual initial registers, not the kernel's intended initial image.
    fxsave64 0x802000
    cmpw $0x037f, 0x802000
    jne .Lcpu_failed
    cmpw $0, 0x802002
    jne .Lcpu_failed
    cmpb $0, 0x802004
    jne .Lcpu_failed
    cmpw $0, 0x802006
    jne .Lcpu_failed
    cmpq $0, 0x802008
    jne .Lcpu_failed
    cmpq $0, 0x802010
    jne .Lcpu_failed
    cmpl $0x1f80, 0x802018
    jne .Lcpu_failed
    movq $0x802020, %rsi
    movl $8, %ecx
.Lcpu_initial_x87:
    cmpq $0, (%rsi)
    jne .Lcpu_failed
    cmpw $0, 8(%rsi)
    jne .Lcpu_failed
    addq $16, %rsi
    decl %ecx
    jne .Lcpu_initial_x87
    movl $32, %ecx
.Lcpu_initial_xmm:
    cmpq $0, (%rsi)
    jne .Lcpu_failed
    addq $8, %rsi
    decl %ecx
    jne .Lcpu_initial_xmm
    movdqu .Lcpu_xmm+0(%rip), %xmm0
    movdqu .Lcpu_xmm+16(%rip), %xmm1
    movdqu .Lcpu_xmm+32(%rip), %xmm2
    movdqu .Lcpu_xmm+48(%rip), %xmm3
    movdqu .Lcpu_xmm+64(%rip), %xmm4
    movdqu .Lcpu_xmm+80(%rip), %xmm5
    movdqu .Lcpu_xmm+96(%rip), %xmm6
    movdqu .Lcpu_xmm+112(%rip), %xmm7
    movdqu .Lcpu_xmm+128(%rip), %xmm8
    movdqu .Lcpu_xmm+144(%rip), %xmm9
    movdqu .Lcpu_xmm+160(%rip), %xmm10
    movdqu .Lcpu_xmm+176(%rip), %xmm11
    movdqu .Lcpu_xmm+192(%rip), %xmm12
    movdqu .Lcpu_xmm+208(%rip), %xmm13
    movdqu .Lcpu_xmm+224(%rip), %xmm14
    movdqu .Lcpu_xmm+240(%rip), %xmm15
    fldcw .Lcpu_x87_control(%rip)
    ldmxcsr .Lcpu_x87_mxcsr(%rip)
    fld1
    movl $1, %edi
    callq anomalous_workspace_cpu_phase
.Lcpu_wait_x87:
    pause
    cmpq $2, 0x802200
    jb .Lcpu_wait_x87
    callq anomalous_workspace_cpu_check_x87
    # Enter the transition phase before changing any live architectural byte.
    movl $2, %edi
    callq anomalous_workspace_cpu_phase
    fninit
    # Seed every aliased x87 exponent explicitly. Hardware MMX loads set these
    # bits to ones; TCG can retain prior exponent bits. Both paths now start
    # with the exact same complete 80-bit values, without relaxing observation.
    # Eight quiet-NaN loads fill the physical register file and return TOP to 0.
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    fldt .Lcpu_mmx_upper(%rip)
    # Reset flags/tags/TOP while retaining the explicitly seeded register bits.
    fninit
    fldcw .Lcpu_mmx_control(%rip)
    ldmxcsr .Lcpu_mmx_mxcsr(%rip)
    movq .Lcpu_mmx+0(%rip), %mm0
    movq .Lcpu_mmx+8(%rip), %mm1
    movq .Lcpu_mmx+16(%rip), %mm2
    movq .Lcpu_mmx+24(%rip), %mm3
    movq .Lcpu_mmx+32(%rip), %mm4
    movq .Lcpu_mmx+40(%rip), %mm5
    movq .Lcpu_mmx+48(%rip), %mm6
    movq .Lcpu_mmx+56(%rip), %mm7
    movl $3, %edi
    callq anomalous_workspace_cpu_phase
.Lcpu_wait_mmx:
    pause
    cmpq $2, 0x802200
    jb .Lcpu_wait_mmx
    callq anomalous_workspace_cpu_check_mmx
    movl $4, %edi
    callq anomalous_workspace_cpu_phase
    retq
.size anomalous_workspace_cpu_initialize, . - anomalous_workspace_cpu_initialize

.p2align 4
.type anomalous_workspace_cpu_phase, @function
anomalous_workspace_cpu_phase:
    pushq %r15
    movabsq $workspace_mailbox, %r15
    movabsq $0x6370755f70726f62, %rax
    pushq $0x202
    popfq
    int $workspace_gate
.global anomalous_workspace_cpu_probe_return
anomalous_workspace_cpu_probe_return:
    testq %rax, %rax
    jne .Lcpu_failed
    popq %r15
    retq
.size anomalous_workspace_cpu_phase, . - anomalous_workspace_cpu_phase

.p2align 4
.global anomalous_workspace_cpu_check_x87
.type anomalous_workspace_cpu_check_x87, @function
anomalous_workspace_cpu_check_x87:
    fxsave64 0x802000
    cmpw $0x077f, 0x802000
    jne .Lcpu_failed
    cmpw $0x3800, 0x802002
    jne .Lcpu_failed
    cmpb $0x80, 0x802004
    jne .Lcpu_failed
    cmpl $0x3f80, 0x802018
    jne .Lcpu_failed
    movabsq $0x8000000000000000, %rax
    cmpq %rax, 0x802020
    jne .Lcpu_failed
    cmpw $0x3fff, 0x802028
    jne .Lcpu_failed
    jmp anomalous_workspace_cpu_check_xmm
.size anomalous_workspace_cpu_check_x87, . - anomalous_workspace_cpu_check_x87

.p2align 4
.global anomalous_workspace_cpu_check_mmx
.type anomalous_workspace_cpu_check_mmx, @function
anomalous_workspace_cpu_check_mmx:
    fxsave64 0x802000
    cmpw $0x0b7f, 0x802000
    jne .Lcpu_failed
    cmpw $0, 0x802002
    jne .Lcpu_failed
    cmpb $0xff, 0x802004
    jne .Lcpu_failed
    cmpl $0x5f80, 0x802018
    jne .Lcpu_failed
    movq $0x802020, %rsi
    leaq .Lcpu_mmx(%rip), %rdx
    movl $8, %ecx
.Lcpu_check_mmx:
    movq (%rdx), %rax
    cmpq %rax, (%rsi)
    jne .Lcpu_failed
    cmpw $0xffff, 8(%rsi)
    jne .Lcpu_failed
    addq $8, %rdx
    addq $16, %rsi
    decl %ecx
    jne .Lcpu_check_mmx
    jmp anomalous_workspace_cpu_check_xmm
.size anomalous_workspace_cpu_check_mmx, . - anomalous_workspace_cpu_check_mmx

.p2align 4
.type anomalous_workspace_cpu_check_xmm, @function
anomalous_workspace_cpu_check_xmm:
    movq $0x8020a0, %rsi
    leaq .Lcpu_xmm(%rip), %rdx
    movl $32, %ecx
.Lcpu_check_xmm:
    movq (%rdx), %rax
    cmpq %rax, (%rsi)
    jne .Lcpu_failed
    addq $8, %rdx
    addq $8, %rsi
    decl %ecx
    jne .Lcpu_check_xmm
    retq
.size anomalous_workspace_cpu_check_xmm, . - anomalous_workspace_cpu_check_xmm
.Lcpu_failed:
    ud2
.global anomalous_workspace_cpu_probe_end
anomalous_workspace_cpu_probe_end:

.section .rodata.workspace_cpu_probe,"a",@progbits
.p2align 4
.Lcpu_xmm:
    .quad 0x31a7c0de00000000, 0x91eec0de00000000
    .quad 0x31a7c0de00000001, 0x91eec0de00000001
    .quad 0x31a7c0de00000002, 0x91eec0de00000002
    .quad 0x31a7c0de00000003, 0x91eec0de00000003
    .quad 0x31a7c0de00000004, 0x91eec0de00000004
    .quad 0x31a7c0de00000005, 0x91eec0de00000005
    .quad 0x31a7c0de00000006, 0x91eec0de00000006
    .quad 0x31a7c0de00000007, 0x91eec0de00000007
    .quad 0x31a7c0de00000008, 0x91eec0de00000008
    .quad 0x31a7c0de00000009, 0x91eec0de00000009
    .quad 0x31a7c0de0000000a, 0x91eec0de0000000a
    .quad 0x31a7c0de0000000b, 0x91eec0de0000000b
    .quad 0x31a7c0de0000000c, 0x91eec0de0000000c
    .quad 0x31a7c0de0000000d, 0x91eec0de0000000d
    .quad 0x31a7c0de0000000e, 0x91eec0de0000000e
    .quad 0x31a7c0de0000000f, 0x91eec0de0000000f
.Lcpu_mmx:
    .quad 0x6d6d780000000100
    .quad 0x6d6d780000000101
    .quad 0x6d6d780000000102
    .quad 0x6d6d780000000103
    .quad 0x6d6d780000000104
    .quad 0x6d6d780000000105
    .quad 0x6d6d780000000106
    .quad 0x6d6d780000000107
# Negative quiet NaN in the x87 80-bit memory format.
.Lcpu_mmx_upper: .quad 0xc000000000000000
    .word 0xffff
.Lcpu_x87_control: .word 0x077f
.Lcpu_mmx_control: .word 0x0b7f
.p2align 2
.Lcpu_x87_mxcsr: .long 0x3f80
.Lcpu_mmx_mxcsr: .long 0x5f80
.section .note.GNU-stack,"",@progbits
