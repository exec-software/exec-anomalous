.set scheduler_kernel_code_selector, 0x08
.set scheduler_kernel_data_selector, 0x10
.set scheduler_user_data_selector, 0x2b
.set scheduler_user_code_selector, 0x33
.set scheduler_gate_vector, 0xf2

.set scheduler_user_mailbox, 0x0000700000003000
.set scheduler_mailbox_canary_a, 0
.set scheduler_mailbox_canary_b, 8
.set scheduler_mailbox_arm, 16
.set scheduler_operation_ready, 1
.set scheduler_operation_complete, 2

.set scheduler_fault_canary_a, 0x51a7c0def17e0001
.set scheduler_fault_canary_b, 0x51a7c0def17e0002
.set scheduler_fault_arm, 0xf017aa55c0de0001
.set scheduler_peer_canary_a, 0x9ee7c0deba5e0001
.set scheduler_peer_canary_b, 0x9ee7c0deba5e0002
.set scheduler_peer_arm, 0x9ee7aa55c0de0002

.set scheduler_cr0_task_switched, 0x08
.set scheduler_frame_code_segment, 144
.set scheduler_frame_stack_segment, 168

.text
.p2align 4

.global x86_64_scheduler_enter
.type x86_64_scheduler_enter, @function
# System V inputs are RDI = synthesized 22-qword user frame, RSI = saved kernel
# RSP cell, and RDX = original CR0.TS cell. The caller owns both cells and has
# already selected the task address space and TSS RSP0. Completion returns only
# through x86_64_scheduler_resume_kernel.
x86_64_scheduler_enter:
    cli
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rsp, (%rsi)

    movq %cr0, %rax
    movq %rax, %rcx
    andq $scheduler_cr0_task_switched, %rcx
    movq %rcx, (%rdx)
    orq $scheduler_cr0_task_switched, %rax
    movq %rax, %cr0

    movq %rdi, %rsp
    cmpq $scheduler_user_code_selector, scheduler_frame_code_segment(%rsp)
    jne .Lscheduler_enter_invalid
    cmpq $scheduler_user_data_selector, scheduler_frame_stack_segment(%rsp)
    jne .Lscheduler_enter_invalid
    movw $scheduler_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $scheduler_kernel_data_selector, %ax
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
.Lscheduler_enter_invalid:
    ud2
.size x86_64_scheduler_enter, . - x86_64_scheduler_enter

.global x86_64_scheduler_resume_kernel
.type x86_64_scheduler_resume_kernel, @function
# System V inputs are RDI = completion status, RSI = the single-use saved kernel
# RSP, and RDX = the exact original CR0.TS bit. This abandons the task's current
# transition frame and returns through the preserved Rust call boundary.
x86_64_scheduler_resume_kernel:
    cli
    cmpq $0, %rdx
    je .Lscheduler_resume_ts_valid
    cmpq $scheduler_cr0_task_switched, %rdx
    jne .Lscheduler_resume_invalid
.Lscheduler_resume_ts_valid:
    movq %cr0, %rax
    andq $-9, %rax
    orq %rdx, %rax
    movq %rax, %cr0
    movw $scheduler_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
.if {owned_tls_enabled}
    callq x86_64_task_tls_resume_kernel
.endif
    movq %rsi, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    movq %rdi, %rax
    retq
.Lscheduler_resume_invalid:
    ud2
.size x86_64_scheduler_resume_kernel, . - x86_64_scheduler_resume_kernel

.global x86_64_scheduler_gate_entry
.type x86_64_scheduler_gate_entry, @function
# A CPL-3 interrupt supplies RIP, CS, RFLAGS, RSP, and SS. Two synthetic words
# plus every GPR form the same 22-qword task frame used by timer and
# exception return. Rust either returns a fully validated selected user frame
# or completes nonlocally through x86_64_scheduler_resume_kernel.
x86_64_scheduler_gate_entry:
    cld
    pushq $0
    pushq $scheduler_gate_vector
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
    movw $scheduler_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_scheduler_gate_dispatch
    movq %rax, %rsp

    cmpq $scheduler_user_code_selector, scheduler_frame_code_segment(%rsp)
    jne .Lscheduler_gate_invalid
    cmpq $scheduler_user_data_selector, scheduler_frame_stack_segment(%rsp)
    jne .Lscheduler_gate_invalid
    movw $scheduler_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $scheduler_kernel_data_selector, %ax
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
.Lscheduler_gate_invalid:
    cli
    ud2
.size x86_64_scheduler_gate_entry, . - x86_64_scheduler_gate_entry

.section .rodata.scheduler_test.faulting,"a",@progbits
.p2align 4
.global x86_64_scheduler_faulting_start
.type x86_64_scheduler_faulting_start, @function
x86_64_scheduler_faulting_start:
.Lscheduler_faulting_blob_start:
    movabsq $scheduler_user_mailbox, %r15
    movabsq $scheduler_fault_canary_a, %rax
    movq %rax, scheduler_mailbox_canary_a(%r15)
    movabsq $scheduler_fault_canary_b, %rax
    movq %rax, scheduler_mailbox_canary_b(%r15)
    movl $scheduler_operation_ready, %eax
    pushq $0x02
    popfq
    int $scheduler_gate_vector
.global x86_64_scheduler_fault_ready_return
x86_64_scheduler_fault_ready_return:
.Lscheduler_fault_wait:
    movabsq $scheduler_fault_canary_a, %rax
    cmpq %rax, scheduler_mailbox_canary_a(%r15)
    jne x86_64_scheduler_fault_failed_instruction
    movabsq $scheduler_fault_canary_b, %rax
    cmpq %rax, scheduler_mailbox_canary_b(%r15)
    jne x86_64_scheduler_fault_failed_instruction
    movq scheduler_mailbox_arm(%r15), %rax
    testq %rax, %rax
    jz .Lscheduler_fault_pause
    movabsq $scheduler_fault_arm, %r14
    cmpq %r14, %rax
    jne x86_64_scheduler_fault_failed_instruction

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
    pushq $0x202
    popfq
.global x86_64_scheduler_fault_instruction
x86_64_scheduler_fault_instruction:
    ud2
    jmp x86_64_scheduler_fault_failed_instruction
.Lscheduler_fault_pause:
    pause
    jmp .Lscheduler_fault_wait
.global x86_64_scheduler_fault_failed_instruction
x86_64_scheduler_fault_failed_instruction:
    ud2
.global x86_64_scheduler_faulting_end
x86_64_scheduler_faulting_end:
.Lscheduler_faulting_blob_end:
.size x86_64_scheduler_faulting_start, . - x86_64_scheduler_faulting_start

.section .rodata.scheduler_test.peer,"a",@progbits
.p2align 4
.global x86_64_scheduler_peer_start
.type x86_64_scheduler_peer_start, @function
x86_64_scheduler_peer_start:
.Lscheduler_peer_blob_start:
    movabsq $scheduler_user_mailbox, %r15
    movabsq $scheduler_peer_canary_a, %rax
    movq %rax, scheduler_mailbox_canary_a(%r15)
    movabsq $scheduler_peer_canary_b, %rax
    movq %rax, scheduler_mailbox_canary_b(%r15)
    movl $scheduler_operation_ready, %eax
    pushq $0x02
    popfq
    int $scheduler_gate_vector
.global x86_64_scheduler_peer_ready_return
x86_64_scheduler_peer_ready_return:
.Lscheduler_peer_wait:
    movabsq $scheduler_peer_canary_a, %rax
    cmpq %rax, scheduler_mailbox_canary_a(%r15)
    jne x86_64_scheduler_peer_failed_instruction
    movabsq $scheduler_peer_canary_b, %rax
    cmpq %rax, scheduler_mailbox_canary_b(%r15)
    jne x86_64_scheduler_peer_failed_instruction
    movq scheduler_mailbox_arm(%r15), %rax
    testq %rax, %rax
    jz .Lscheduler_peer_pause
    movabsq $scheduler_peer_arm, %r14
    cmpq %r14, %rax
    jne x86_64_scheduler_peer_failed_instruction

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
    movl $scheduler_operation_complete, %eax
    pushq $0x202
    popfq
    int $scheduler_gate_vector
.global x86_64_scheduler_peer_completion_return
x86_64_scheduler_peer_completion_return:
    ud2
    jmp x86_64_scheduler_peer_failed_instruction
.Lscheduler_peer_pause:
    pause
    jmp .Lscheduler_peer_wait
.global x86_64_scheduler_peer_failed_instruction
x86_64_scheduler_peer_failed_instruction:
    ud2
.global x86_64_scheduler_peer_end
x86_64_scheduler_peer_end:
.Lscheduler_peer_blob_end:
.size x86_64_scheduler_peer_start, . - x86_64_scheduler_peer_start

.section .note.GNU-stack,"",@progbits
