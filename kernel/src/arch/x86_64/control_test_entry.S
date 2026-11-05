.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set control_gate_vector, 0xf1
.set user_mailbox_address, 0x0000600000003000

.set operation_probe, 0x10
.set operation_send, 0x11
.set operation_receive, 0x12
.set operation_complete, 0x13

.set status_accepted, 0
.set status_denied, 1
.set status_would_block, 2

.set mailbox_valid_slot, 0
.set mailbox_valid_generation, 8
.set mailbox_valid_tag, 16
.set mailbox_copied_slot, 24
.set mailbox_copied_generation, 32
.set mailbox_copied_tag, 40
.set mailbox_expected_first, 64
.set mailbox_expected_second, 136
.set mailbox_transfer, 256
.set message_word_count, 9

.set private_payload_first, 0x9ad4c1e76b208f35
.set private_payload_second, 0x53b8f02da6914ce7

.text
.p2align 4

.global x86_64_enter_control_task
.type x86_64_enter_control_task, @function
# System V inputs are RDI = user RIP, RSI = user RSP, RDX = user data/stack
# selector, RCX = user code selector, and R8 = saved-kernel-RSP pointer. The
# function preserves the caller's callee-saved registers on its original stack.
# The exact completion operation returns through x86_64_resume_control_task;
# ordinary return from the user payload is impossible.
x86_64_enter_control_task:
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

    # This fixed prototype has no floating-point ABI. TS denies access to
    # inherited x87, MMX, and SIMD state. General registers are cleared so only
    # the task's mapped code and mailbox supply feature-local inputs.
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
.size x86_64_enter_control_task, . - x86_64_enter_control_task

.global x86_64_resume_control_task
.type x86_64_resume_control_task, @function
# RDI is the fixed completion status, RSI is the single-use saved kernel stack,
# and RDX is the original CR0.TS bit. The routine abandons the TSS privilege
# stack and returns through the original Rust call frame. It is valid only on
# the bootstrap processor while IF and NMI remain masked.
x86_64_resume_control_task:
    testq %rdx, %rdx
    jnz control_task_switched_restored
    clts
control_task_switched_restored:
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
.size x86_64_resume_control_task, . - x86_64_resume_control_task

.global x86_64_control_gate_entry
.type x86_64_control_gate_entry, @function
# The DPL-3 interrupt gate supplies RIP, CS, RFLAGS, RSP, and SS. Synthetic
# vector and error words make the frame explicit. R12 retains the pre-aligned
# frame pointer across Rust; nonterminal calls restore all user registers after
# the dispatcher writes its bounded result fields into the saved frame.
x86_64_control_gate_entry:
    cld
    pushq $0
    pushq $control_gate_vector
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
    callq x86_64_control_gate_dispatch
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
.size x86_64_control_gate_entry, . - x86_64_control_gate_entry

.section .rodata.control_test.payload,"a",@progbits
.p2align 4

# Both producer fixtures copy this exact byte range into separately allocated
# RX pages. The task-local reference comes from its private mailbox namespace;
# zero reference fields deliberately exercise denial before the valid send.
.global x86_64_control_producer_start
x86_64_control_producer_start:
    movabsq $user_mailbox_address, %r15
    movabsq $private_payload_first, %r8
    movq %r8, mailbox_transfer(%r15)
    movabsq $private_payload_second, %r8
    movq %r8, mailbox_transfer + 8(%r15)
    xorq %r8, %r8
    xorq %rbx, %rbx
    xorq %rcx, %rcx
    xorq %rdx, %rdx
    movl $operation_probe, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_producer_probe_return
x86_64_control_producer_probe_return:
    cmpq $status_denied, %rax
    jne control_producer_failed
    movq mailbox_valid_slot(%r15), %rbx
    movq mailbox_valid_generation(%r15), %rcx
    movq mailbox_valid_tag(%r15), %rdx
    movl $operation_send, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_producer_send_return
x86_64_control_producer_send_return:
    cmpq $status_accepted, %rax
    jne control_producer_failed
    movl $operation_complete, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_producer_complete_return
x86_64_control_producer_complete_return:
control_producer_failed:
    ud2
.global x86_64_control_producer_end
x86_64_control_producer_end:

.p2align 4
.global x86_64_control_receiver_start
x86_64_control_receiver_start:
    movabsq $user_mailbox_address, %r15
    movq mailbox_copied_slot(%r15), %rbx
    movq mailbox_copied_generation(%r15), %rcx
    movq mailbox_copied_tag(%r15), %rdx
    movl $operation_probe, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_receiver_probe_return
x86_64_control_receiver_probe_return:
    cmpq $status_denied, %rax
    jne control_receiver_failed

    movq mailbox_valid_slot(%r15), %rbx
    movq mailbox_valid_generation(%r15), %rcx
    movq mailbox_valid_tag(%r15), %rdx
    movl $operation_receive, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_receiver_first_receive_return
x86_64_control_receiver_first_receive_return:
    cmpq $status_accepted, %rax
    jne control_receiver_failed
    leaq mailbox_expected_first(%r15), %rsi
    leaq mailbox_transfer(%r15), %rdi
    movl $message_word_count, %ecx
    repe cmpsq
    jne control_receiver_failed

    movq mailbox_valid_slot(%r15), %rbx
    movq mailbox_valid_generation(%r15), %rcx
    movq mailbox_valid_tag(%r15), %rdx
    movl $operation_receive, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_receiver_second_receive_return
x86_64_control_receiver_second_receive_return:
    cmpq $status_accepted, %rax
    jne control_receiver_failed
    leaq mailbox_expected_second(%r15), %rsi
    leaq mailbox_transfer(%r15), %rdi
    movl $message_word_count, %ecx
    repe cmpsq
    jne control_receiver_failed

    movq mailbox_valid_slot(%r15), %rbx
    movq mailbox_valid_generation(%r15), %rcx
    movq mailbox_valid_tag(%r15), %rdx
    movl $operation_receive, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_receiver_empty_return
x86_64_control_receiver_empty_return:
    cmpq $status_would_block, %rax
    jne control_receiver_failed
    movl $operation_complete, %eax
    pushq $0x02
    popfq
    int $control_gate_vector
.global x86_64_control_receiver_complete_return
x86_64_control_receiver_complete_return:
control_receiver_failed:
    ud2
.global x86_64_control_receiver_end
x86_64_control_receiver_end:

.section .note.GNU-stack,"",@progbits
