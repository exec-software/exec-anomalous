.text
.p2align 4
.macro mixed_entry vector
.global x86_64_mixed_workspace_gate_f\vector
.type x86_64_mixed_workspace_gate_f\vector, @function
x86_64_mixed_workspace_gate_f\vector:
    cld
    pushq $0
    pushq $(0xf0 + \vector)
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
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_mixed_workspace_dispatch
    testq %rax, %rax
    jz .Lmixed_invalid_\vector
    movq %rax, %rsp
    cmpq $0x33, 144(%rsp)
    jne .Lmixed_invalid_\vector
    cmpq $0x2b, 168(%rsp)
    jne .Lmixed_invalid_\vector
    movw $0x2b, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $0x10, %ax
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
.Lmixed_invalid_\vector:
    cli
    ud2
.size x86_64_mixed_workspace_gate_f\vector, . - x86_64_mixed_workspace_gate_f\vector
.endm
mixed_entry 3
mixed_entry 4

# These fixtures are relocatable, stackless instruction streams copied to
# 0x400000 in three independent roots. All branch targets remain in their stream.
.macro mixed_call name, operation, handle=0, address=0, length=0, sequence=0, status=0, vector=0xf4
    movq $\operation, %rax
    movq $\handle, %rdi
    movq $\address, %rsi
    movq $\length, %rdx
    movq $\sequence, %rcx
    int $\vector
    cmpq $\status, %rax
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    movq %xmm0, %r10
    cmpq %r13, %r10
    jne x86_64_mixed_workspace_\name\()_failed_instruction
.endm
.macro mixed_receive name, route, arena, value, sequence=1
    mixed_call \name, 2, \route, (\arena + 256), 8
    cmpq $\sequence, %rcx
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    cmpq $8, %rdx
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    movq $\value, %r10
    cmpq %r10, 256(%r14)
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    mixed_call \name, 3, \route, 0, 0, \sequence
.endm
.macro mixed_send name, route, arena, value, sequence=1
    movq $\value, 64(%r14)
    mixed_call \name, 1, \route, (\arena + 64), 8, \sequence
    cmpq $\sequence, %rcx
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    cmpq $8, %rdx
    jne x86_64_mixed_workspace_\name\()_failed_instruction
.endm
.macro mixed_fill words
    movq %r14, %rdi
    movq %r13, %rax
    movq $\words, %rcx
    cld
    rep stosq
.endm
.macro mixed_payload name, role, arena, words
.section .rodata.mixed_workspace.\name,"a",@progbits
.p2align 4
.global x86_64_mixed_workspace_\name\()_start
.type x86_64_mixed_workspace_\name\()_start, @function
x86_64_mixed_workspace_\name\()_start:
    movq $\role, %r12
    movabsq $(0x71716d6978656400 + \role), %r13
    movq $\arena, %r14
    movq $0, %rax
    movq $\role, %rdi
    movq $0, %rsi
    movq $0, %rdx
    movq $0, %rcx
    int $0xf4
    testq %rax, %rax
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    fninit
.if \role == 0
    fld1
.elseif \role == 1
    fldz
.else
    fldpi
.endif
    movq %r13, %xmm0
    mixed_call \name, 7
.Lmixed_\name\()_ticks:
    cmpq $2, 0x800200
    jae .Lmixed_\name\()_ready
    pause
    jmp .Lmixed_\name\()_ticks
.Lmixed_\name\()_ready:
    mixed_fill \words
.if \role == 0
    mixed_call \name, 1, 0, 0, 0, 0, 0, 0xf3
    mixed_call \name, 8, 0, 0, 0, 0, 2
    mixed_call \name, 1, 3, (\arena + 64), 8, 1, 2
    mixed_call \name, 1, 1, 0x80f000, 8, 1, 3
    mixed_call \name, 1, 1, 0x819000, 8, 1, 3
    mixed_call \name, 1, 1, 0x818ffc, 8, 1, 3
    mixed_send \name, 1, \arena, 0x574f524b
    mixed_call \name, 4
    mixed_call \name, 2, 2, 0x818ffc, 8, 0, 3
    cmpq %r13, 0x818ff8
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    mixed_receive \name, 2, \arena, 0x45444954
    mixed_call \name, 6, 1, 0, 0, 1
    mixed_call \name, 4
    mixed_send \name, 1, \arena, 0x574f5232, 2
    mixed_call \name, 4
    mixed_receive \name, 2, \arena, 0x45444932, 2
    mixed_call \name, 6, 1, 0, 0, 2
    mixed_call \name, 1, 0, 0, 0, 0, 0, 0xf3
    mixed_call \name, 4
    mixed_call \name, 5
.elseif \role == 1
    mixed_call \name, 1, 0, 0, 0, 0, 2, 0xf3
    mixed_call \name, 8, 0, 0, 0, 0, 2
    mixed_call \name, 2, 2, (\arena + 256), 8, 0, 2
    mixed_call \name, 2, 1, 0x80bffc, 8, 0, 3
    cmpq %r13, 0x80bff8
    jne x86_64_mixed_workspace_\name\()_failed_instruction
    mixed_receive \name, 1, \arena, 0x574f524b
    mixed_send \name, 2, \arena, 0x45444954
    mixed_send \name, 3, \arena, 0x444f4351
    mixed_call \name, 4
    mixed_call \name, 6, 2, 0, 0, 1
    mixed_call \name, 6, 3, 0, 0, 1
    mixed_receive \name, 4, \arena, 0x444f4352
    mixed_call \name, 4
    mixed_call \name, 1, 3, (\arena + 64), 8, 2, 5
    mixed_call \name, 2, 4, (\arena + 256), 8, 0, 5
    mixed_receive \name, 1, \arena, 0x574f5232, 2
    mixed_send \name, 2, \arena, 0x45444932, 2
    mixed_call \name, 4
    mixed_call \name, 6, 2, 0, 0, 2
    mixed_call \name, 5
.else
    mixed_call \name, 1, 0, 0, 0, 0, 2, 0xf3
    mixed_call \name, 8
    mixed_receive \name, 3, \arena, 0x444f4351
    mixed_send \name, 4, \arena, 0x444f4352
    mixed_call \name, 4
    mixed_call \name, 6, 4, 0, 0, 1
.global x86_64_mixed_workspace_service_cut_instruction
x86_64_mixed_workspace_service_cut_instruction:
    ud2
.endif
.global x86_64_mixed_workspace_\name\()_failed_instruction
x86_64_mixed_workspace_\name\()_failed_instruction:
    ud2
.global x86_64_mixed_workspace_\name\()_end
x86_64_mixed_workspace_\name\()_end:
.size x86_64_mixed_workspace_\name\()_start, . - x86_64_mixed_workspace_\name\()_start
.endm
mixed_payload workspace, 0, 0x810000, 4608
mixed_payload editor, 1, 0x804000, 4096
mixed_payload service, 2, 0x814000, 4096
.section .note.GNU-stack,"",@progbits
