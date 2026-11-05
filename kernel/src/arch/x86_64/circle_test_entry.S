.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set circle_report_vector, 0xf0
.set user_data_address, 0x0000400000003000
.set user_stack_probe_address, 0x0000400000005000

.set report_generation_1_entered, 1
.set report_after_supervisor_read, 2
.set report_generation_2_entered, 3
.set report_after_nx_fetch, 4
.set report_generation_3_entered, 5
.set report_after_terminal, 6
.set report_dirty_state, 7

.text
.p2align 4

.global x86_64_enter_circle
.type x86_64_enter_circle, @function
# System V inputs are RDI = user RIP, RSI = user RSP, RDX = user data/stack
# selector, RCX = user code selector, and R8 = saved-kernel-RSP pointer. The
# function preserves the caller's callee-saved registers on its original stack.
# An exact contained fault enters the separate recovery routine; ordinary
# return from user mode is impossible.
x86_64_enter_circle:
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rsp, (%r8)

    movw %dx, %ax
    movw %ax, %ds
    movw %ax, %es
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs

    pushq %rdx
    pushq %rsi
    pushq $0x02
    pushq %rcx
    pushq %rdi

    # The fixed prototype has no floating-point ABI. TS denies access to all
    # inherited x87, MMX, and SIMD state; the recovery trampoline clears it
    # only after user state has been abandoned. General registers are zeroed.
    movq %cr0, %rax
    orq $0x08, %rax
    movq %rax, %cr0
    xorq %rax, %rax
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
.size x86_64_enter_circle, . - x86_64_enter_circle

.global x86_64_resume_circle
.type x86_64_resume_circle, @function
# RDI is the fixed containment status, RSI is the single-use saved kernel
# stack, and RDX is the original CR0.TS bit. This abandons the TSS privilege
# stack and returns through the original Rust call frame. It is valid only on
# the bootstrap processor with IF and NMI masked and is intentionally
# incompatible with a future shadow-stack ABI.
x86_64_resume_circle:
    testq %rdx, %rdx
    jnz circle_task_switched_restored
    clts
circle_task_switched_restored:
    movw $kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsi, %rsp
    movq %rdi, %rax
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    retq
.size x86_64_resume_circle, . - x86_64_resume_circle

.global x86_64_circle_report_entry
.type x86_64_circle_report_entry, @function
# The DPL-3 interrupt gate supplies RIP,CS,RFLAGS,RSP,SS. Synthetic vector and
# error words make the frame match exception_frame. R12 retains the pre-aligned
# frame pointer across the Rust call; all user general registers are restored
# before IRETQ for the two non-terminal reports.
x86_64_circle_report_entry:
    cld
    pushq $0
    pushq $circle_report_vector
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
    movw $kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movq %rsp, %r12
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_circle_report_dispatch
    movq %r12, %rsp
    movw $user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
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
.size x86_64_circle_report_entry, . - x86_64_circle_report_entry

.section .rodata.circle_test.payload,"a",@progbits
.p2align 4

.global x86_64_circle_generation_1_start
x86_64_circle_generation_1_start:
    movabsq $user_data_address, %rdi
    cmpq $0, (%rdi)
    jne circle_generation_1_dirty
    movabsq $0x1616161616161616, %rbx
    movq %rbx, (%rdi)
    pushq $0x02
    popfq
    movl $report_generation_1_entered, %eax
    int $circle_report_vector
.global x86_64_circle_generation_1_report_return
x86_64_circle_generation_1_report_return:
    movabsq $circle_kernel_canary, %rax
.global x86_64_circle_generation_1_denied_read
x86_64_circle_generation_1_denied_read:
    movq (%rax), %rax
    movl $report_after_supervisor_read, %eax
    int $circle_report_vector
    ud2
circle_generation_1_dirty:
    movl $report_dirty_state, %eax
    int $circle_report_vector
    ud2
.global x86_64_circle_generation_1_end
x86_64_circle_generation_1_end:

.p2align 4
.global x86_64_circle_generation_2_start
x86_64_circle_generation_2_start:
    movabsq $user_data_address, %rdi
    cmpq $0, (%rdi)
    jne circle_generation_2_dirty
    movabsq $0x2626262626262626, %rbx
    movq %rbx, (%rdi)
    pushq $0x02
    popfq
    movl $report_generation_2_entered, %eax
    int $circle_report_vector
.global x86_64_circle_generation_2_report_return
x86_64_circle_generation_2_report_return:
    movabsq $user_stack_probe_address, %rax
    jmpq *%rax
circle_generation_2_dirty:
    movl $report_dirty_state, %eax
    int $circle_report_vector
    ud2
.global x86_64_circle_generation_2_end
x86_64_circle_generation_2_end:

.p2align 4
.global x86_64_circle_stack_probe_start
x86_64_circle_stack_probe_start:
    movl $report_after_nx_fetch, %eax
    int $circle_report_vector
    ud2
.global x86_64_circle_stack_probe_end
x86_64_circle_stack_probe_end:

.p2align 4
.global x86_64_circle_generation_3_start
x86_64_circle_generation_3_start:
    movabsq $user_data_address, %rdi
    cmpq $0, (%rdi)
    jne circle_generation_3_dirty
    movabsq $0x3636363636363636, %rbx
    movq %rbx, (%rdi)
    pushq $0x02
    popfq
    movl $report_generation_3_entered, %eax
    int $circle_report_vector
.global x86_64_circle_generation_3_report_return
x86_64_circle_generation_3_report_return:
    movl $report_after_terminal, %eax
    int $circle_report_vector
    ud2
circle_generation_3_dirty:
    movl $report_dirty_state, %eax
    int $circle_report_vector
    ud2
.global x86_64_circle_generation_3_end
x86_64_circle_generation_3_end:

.section .note.GNU-stack,"",@progbits
