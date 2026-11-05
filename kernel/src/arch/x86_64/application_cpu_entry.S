.set application_kernel_data_selector, 0x10
.set application_user_data_selector, 0x2b
.set application_user_code_selector, 0x33
.set application_frame_code_segment, 144
.set application_frame_stack_segment, 168

.text
.p2align 4

.global x86_64_application_enter
.type x86_64_application_enter, @function
# RDI = validated 22-qword user frame, RSI = retained kernel RSP cell.
# The eager CPU owner has already restored this task's complete legacy state.
# This boundary and its Rust caller use only GPRs; do not change CR0.TS here.
x86_64_application_enter:
    cli
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    movq %rsp, (%rsi)
    movq %rdi, %rsp
    cmpq $application_user_code_selector, application_frame_code_segment(%rsp)
    jne .Lapplication_enter_invalid
    cmpq $application_user_data_selector, application_frame_stack_segment(%rsp)
    jne .Lapplication_enter_invalid
    movw $application_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $application_kernel_data_selector, %ax
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
.Lapplication_enter_invalid:
    ud2
.size x86_64_application_enter, . - x86_64_application_enter

.global x86_64_application_resume_kernel
.type x86_64_application_resume_kernel, @function
# RDI = completion status, RSI = the single-use original kernel RSP.
# The CPU owner has restored exact original FP state and CR0/CR4/EFER and
# verified its storage wipe. Root/TSS restoration is owned by the caller.
x86_64_application_resume_kernel:
    cli
    movw $application_kernel_data_selector, %ax
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
.size x86_64_application_resume_kernel, . - x86_64_application_resume_kernel

.section .note.GNU-stack,"",@progbits
