.set owned_dispatcher_kernel_data_selector, 0x10
.set owned_dispatcher_user_data_selector, 0x2b
.set owned_dispatcher_user_code_selector, 0x33
.set owned_dispatcher_gate_vector, 0xf2
.set owned_dispatcher_mailbox, 0x0000000000800000
.set owned_dispatcher_canary_a, 0x61a7c0def17e0001
.set owned_dispatcher_canary_b, 0x9ee7c0deba5e0002
.text
.p2align 4

.extern x86_64_owned_dispatcher_gate_dispatch
.global x86_64_owned_dispatcher_gate_entry
.type x86_64_owned_dispatcher_gate_entry, @function
# The ordinary saved integer context. The dispatcher owns every selected frame;
# termination abandons this stack only through the bootstrap continuation.
x86_64_owned_dispatcher_gate_entry:
    cld
    pushq $0
    pushq $owned_dispatcher_gate_vector
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
    movw $owned_dispatcher_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_owned_dispatcher_gate_dispatch
    testq %rax, %rax
    jz .Lowned_dispatcher_invalid
    movq %rax, %rsp
    cmpq $owned_dispatcher_user_code_selector, 144(%rsp)
    jne .Lowned_dispatcher_invalid
    cmpq $owned_dispatcher_user_data_selector, 168(%rsp)
    jne .Lowned_dispatcher_invalid
    movw $owned_dispatcher_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $owned_dispatcher_kernel_data_selector, %ax
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
.Lowned_dispatcher_invalid:
    cli
    ud2
.size x86_64_owned_dispatcher_gate_entry, . - x86_64_owned_dispatcher_gate_entry

# Each relocatable template fits in one RX page. The same virtual addresses
# resolve to different owned physical pages. No FPU, SIMD, TLS or syscall ABI.
.macro owned_dispatcher_payload name, role
.section .rodata.owned_dispatcher_test.\name,"a",@progbits
.p2align 4
.global x86_64_owned_dispatcher_\name\()_start
.type x86_64_owned_dispatcher_\name\()_start, @function
x86_64_owned_dispatcher_\name\()_start:
    call .Lowned_dispatcher_\name\()_outer
    ud2
.Lowned_dispatcher_\name\()_outer:
    subq $8, %rsp
    call .Lowned_dispatcher_\name\()_worker
    ud2
.Lowned_dispatcher_\name\()_worker:
    # Both return addresses remain on the owned user stack. Every live timer,
    # fault and gate below observes initial RSP minus 24, inside nested calls.
    movabsq $owned_dispatcher_mailbox, %r15
    movabsq $(owned_dispatcher_canary_a + \role), %r12
    movabsq $(owned_dispatcher_canary_b + \role), %r13
    movq %r12, 0(%r15)
    movq %r13, 8(%r15)
    movq %r12, -32(%rsp)
    movq $1, %r14
    movq %r14, 24(%r15)
    movl $1, %eax
    pushq $0x2
    popfq
    int $owned_dispatcher_gate_vector
.global x86_64_owned_dispatcher_\name\()_ready_return
x86_64_owned_dispatcher_\name\()_ready_return:
.Lowned_dispatcher_\name\()_loop:
    cmpq %r12, 0(%r15)
    jne .Lowned_dispatcher_\name\()_failed
    cmpq %r13, 8(%r15)
    jne .Lowned_dispatcher_\name\()_failed
    cmpq %r12, -32(%rsp)
    jne .Lowned_dispatcher_\name\()_failed
    cmpq $0, 16(%r15)
    jne .Lowned_dispatcher_\name\()_armed
    incq %r14
    jz .Lowned_dispatcher_\name\()_failed
    movq %r14, 24(%r15)
    pause
    jmp .Lowned_dispatcher_\name\()_loop
.Lowned_dispatcher_\name\()_armed:
    cmpq $1, 16(%r15)
    jne .Lowned_dispatcher_\name\()_failed
    # The successful compare supplies RFLAGS 0x246. Keep the two live return
    # addresses and noninitial RSP unchanged after the ready gate enables IF.
.if \role == 0
.global x86_64_owned_dispatcher_fault_instruction
x86_64_owned_dispatcher_fault_instruction:
    ud2
.else
    movl $2, %eax
    int $owned_dispatcher_gate_vector
.global x86_64_owned_dispatcher_peer_completion_return
x86_64_owned_dispatcher_peer_completion_return:
.endif
.Lowned_dispatcher_\name\()_failed:
    ud2
.global x86_64_owned_dispatcher_\name\()_end
x86_64_owned_dispatcher_\name\()_end:
.size x86_64_owned_dispatcher_\name\()_start, . - x86_64_owned_dispatcher_\name\()_start
.endm

owned_dispatcher_payload faulting, 0
owned_dispatcher_payload peer, 1
.section .note.GNU-stack,"",@progbits
