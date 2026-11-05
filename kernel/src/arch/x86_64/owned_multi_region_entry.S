.set owned_multi_region_kernel_data_selector, 0x10
.set owned_multi_region_user_data_selector, 0x2b
.set owned_multi_region_user_code_selector, 0x33
.set owned_multi_region_gate_vector, 0xf2
.set owned_multi_region_mailbox, 0x0000000040007100
.set owned_multi_region_canary_a, 0x61a7c0def17e0001
.set owned_multi_region_canary_b, 0x9ee7c0deba5e0002
.text
.p2align 4

.extern x86_64_owned_multi_region_gate_dispatch
.global x86_64_owned_multi_region_gate_entry
.type x86_64_owned_multi_region_gate_entry, @function
# The ordinary saved integer context. The dispatcher owns every selected frame;
# termination abandons this stack only through the bootstrap continuation.
x86_64_owned_multi_region_gate_entry:
    cld
    pushq $0
    pushq $owned_multi_region_gate_vector
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
    movw $owned_multi_region_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_owned_multi_region_gate_dispatch
    testq %rax, %rax
    jz .Lowned_multi_region_invalid
    movq %rax, %rsp
    cmpq $owned_multi_region_user_code_selector, 144(%rsp)
    jne .Lowned_multi_region_invalid
    cmpq $owned_multi_region_user_data_selector, 168(%rsp)
    jne .Lowned_multi_region_invalid
    movw $owned_multi_region_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $owned_multi_region_kernel_data_selector, %ax
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
.Lowned_multi_region_invalid:
    cli
    ud2
.size x86_64_owned_multi_region_gate_entry, . - x86_64_owned_multi_region_gate_entry


# Native-only mechanism proof with two disjoint initialized RX intervals. Both
# blobs include non-executable leading file bytes; page slack is zeroed.
.set multi_main_entry, 0x40002037
.set multi_interpreter_base, 0x60001000
.if {owned_source_enabled}
# All requests execute with IF clear, on the same nested user stack. The
# returned status is checked by the producer before issuing the next request.
.macro source_request name, step, operation, grant, address, length, offset, result
    movl $\operation, %eax
    movl $\grant, %edi
    movl $\address, %esi
    movl $\length, %edx
    movl $\offset, %r8d
    pushq $2
    popfq
    int $owned_multi_region_gate_vector
.global x86_64_owned_source_\name\()_request_\step\()_return
x86_64_owned_source_\name\()_request_\step\()_return:
    cmpq $\result, %rax
    jne .Lmulti_\name\()_failed
.endm
.endif
.if {owned_tls_proof_enabled}
.set owned_tls_stack_base, 0x7fff00000000
.set owned_tls_canary_a, 0x745153aaf00d0001
.set owned_tls_canary_b, 0x745153bbface0001
# Each exact post-INT label belongs to the admitted initialized RX interval.
.macro tls_request name, step, operation, argument, result
    movl $\operation, %eax
    movabsq $\argument, %rdi
    pushq $2
    popfq
    int $owned_multi_region_gate_vector
.global x86_64_owned_tls_\name\()_request_\step\()_return
x86_64_owned_tls_\name\()_request_\step\()_return:
    movabsq $\result, %rcx
    cmpq %rcx, %rax
    jne .Lmulti_\name\()_failed
.endm
.macro tls_check_a name, role
    movabsq $(owned_tls_canary_a + \role), %rcx
    cmpq %rcx, %fs:0
    jne .Lmulti_\name\()_failed
.endm
.endif
.macro multi_payload name, role
.section .rodata.owned_multi_region_test.\name,"a",@progbits
.p2align 4
.global x86_64_owned_multi_region_\name\()_start
x86_64_owned_multi_region_\name\()_start:
    .fill 32, 1, 0x3c
    call .Lmulti_\name\()_outer
    ud2
.Lmulti_\name\()_outer:
    subq $8, %rsp
    movabsq $multi_main_entry, %rax
    callq *%rax
    ud2
.Lmulti_\name\()_ready:
.if {owned_source_enabled}
    cmpq $1, 32(%r15)
    je .Lsource_\name\()_consumer
    cmpq $0, 32(%r15)
    jne .Lmulti_\name\()_failed
    source_request \name, 0, 11, (0x5100+\role), 0, 0, 0, 1
    source_request \name, 1, 10, (0x5101+\role), 0x40002000, 1024, 0, 1
    source_request \name, 2, 10, (0x5100+\role), 0x40002000, 1024, 0, 0
    source_request \name, 3, 10, (0x5100+\role), 0x40002000, 1024, 0, 1
    source_request \name, 4, 10, (0x5100+\role), 0x60001000, 1024, 1024, 0
    source_request \name, 5, 10, (0x5100+\role), 0x6003efff, 2, 2048, 1
    source_request \name, 6, 10, (0x5100+\role), 0x6003b003, 4096, 2048, 0
    source_request \name, 7, 10, (0x5100+\role), 0x6003c003, 904, 6144, 0
    source_request \name, 8, 11, (0x5100+\role), 0, 0, 0, 0
    source_request \name, 9, 10, (0x5100+\role), 0x40002000, 1024, 0, 1
    xorl %eax, %eax
    movl $0x6003b003, %edi
    movl $5000, %ecx
    rep stosb
    movl $0x6003b003, %edi
    movl $5000, %ecx
    repe scasb
    jne .Lmulti_\name\()_failed
    jmp .Lsource_\name\()_verified
.Lsource_\name\()_consumer:
    movl $0x6003b003, %edi
    movl $5000, %ecx
    movl $3, %eax
.Lsource_\name\()_pattern:
    cmpb %al, (%rdi)
    jne .Lmulti_\name\()_failed
    addb $17, %al
    incq %rdi
    decl %ecx
    jnz .Lsource_\name\()_pattern
.Lsource_\name\()_verified:
    movq $1, 40(%r15)
.endif
.if {owned_tls_proof_enabled}
    tls_request \name, 0, 13, 0, 0
    tls_request \name, 1, 12, 0x0000800000000000, 1
    tls_request \name, 2, 12, 0xffff800000001000, 1
    tls_request \name, 3, 12, (owned_tls_stack_base + 0x100 + \role*0x1000), 0
    cmpq $0, %fs:0
    jne .Lmulti_\name\()_failed
    cmpq $0, %fs:8
    jne .Lmulti_\name\()_failed
    movabsq $(owned_tls_canary_a + \role), %rcx
    movq %rcx, %fs:0
    movq $1, %fs:8
    tls_request \name, 4, 13, 0, (owned_tls_stack_base + 0x100 + \role*0x1000)
    tls_check_a \name, \role
    tls_request \name, 5, 12, (owned_tls_stack_base + 0x500 + \role*0x1000), 0
    cmpq $0, %fs:0
    jne .Lmulti_\name\()_failed
    cmpq $0, %fs:8
    jne .Lmulti_\name\()_failed
    movabsq $(owned_tls_canary_b + \role), %rcx
    movq %rcx, %fs:0
    movq $1, %fs:8
    tls_request \name, 6, 12, 0, 0
    tls_request \name, 7, 13, 0, 0
    tls_request \name, 8, 12, (owned_tls_stack_base + 0x100 + \role*0x1000), 0
    tls_check_a \name, \role
    tls_request \name, 9, 13, 0, (owned_tls_stack_base + 0x100 + \role*0x1000)
    tls_check_a \name, \role
.endif
    movl $1, %eax
    pushq $0x2
    popfq
    int $owned_multi_region_gate_vector
.global x86_64_owned_multi_region_\name\()_ready_return
x86_64_owned_multi_region_\name\()_ready_return:
.Lmulti_\name\()_loop:
.if {owned_tls_proof_enabled}
    tls_check_a \name, \role
    cmpq %r14, %fs:8
    jne .Lmulti_\name\()_failed
.endif
    cmpq %r12, 0(%r15)
    jne .Lmulti_\name\()_failed
    cmpq %r13, 8(%r15)
    jne .Lmulti_\name\()_failed
    cmpq %r12, -32(%rsp)
    jne .Lmulti_\name\()_failed
    cmpq $0, 16(%r15)
    jne .Lmulti_\name\()_armed
    incq %r14
    jz .Lmulti_\name\()_failed
    movq %r14, 24(%r15)
.if {owned_tls_proof_enabled}
    incq %fs:8
    jz .Lmulti_\name\()_failed
.endif
    pause
    jmp .Lmulti_\name\()_loop
.Lmulti_\name\()_armed:
    cmpq $1, 16(%r15)
    jne .Lmulti_\name\()_failed
.if \role == 0
.global x86_64_owned_multi_region_fault_instruction
x86_64_owned_multi_region_fault_instruction:
    ud2
.else
    movl $2, %eax
    int $owned_multi_region_gate_vector
.global x86_64_owned_multi_region_peer_completion_return
x86_64_owned_multi_region_peer_completion_return:
.endif
.Lmulti_\name\()_failed:
    ud2
.global x86_64_owned_multi_region_\name\()_end
x86_64_owned_multi_region_\name\()_end:

.section .rodata.owned_multi_region_test.main_\name,"a",@progbits
.p2align 4
.global x86_64_owned_multi_region_main_\name\()_start
x86_64_owned_multi_region_main_\name\()_start:
    .fill 55, 1, 0xa5
    movabsq $owned_multi_region_mailbox, %r15
    movabsq $(owned_multi_region_canary_a + \role), %r12
    movabsq $(owned_multi_region_canary_b + \role), %r13
    movq %r12, 0(%r15)
    movq %r13, 8(%r15)
    movq %r12, -32(%rsp)
    movq $1, %r14
    movq %r14, 24(%r15)
    movl $3, %eax
    pushq $0x2
    popfq
    int $owned_multi_region_gate_vector
.global x86_64_owned_multi_region_main_\name\()_cross_return
x86_64_owned_multi_region_main_\name\()_cross_return:
    movabsq $(multi_interpreter_base + .Lmulti_\name\()_ready - x86_64_owned_multi_region_\name\()_start), %rax
    jmpq *%rax
.global x86_64_owned_multi_region_main_\name\()_end
x86_64_owned_multi_region_main_\name\()_end:
.endm
multi_payload faulting, 0
multi_payload peer, 1
.section .note.GNU-stack,"",@progbits
