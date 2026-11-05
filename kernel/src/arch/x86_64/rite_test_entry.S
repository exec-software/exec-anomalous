.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set rite_gate_vector, 0xf3
.set user_mailbox_address, 0x0000600000003000

.set rite_call_header_size, 0
.set rite_call_operation, 2
.set rite_call_payload_kind, 4
.set rite_call_reserved, 6
.set rite_call_rights_present, 8
.set rite_call_payload_size, 16
.set rite_call_payload_limit, 20
.set rite_call_trace_id, 24
.set rite_call_size, 40
.set rite_payload, rite_call_size

.set rite_reply_header_size, 0
.set rite_reply_status, 2
.set rite_reply_payload_kind, 4
.set rite_reply_reserved, 6
.set rite_reply_payload_size, 8
.set rite_reply_reserved_2, 12
.set rite_reply_sequence, 16
.set rite_reply_trace_id, 24
.set rite_reply_size, 40

.set rite_operation_console_write, 1
.set rite_operation_input_read, 2
.set rite_operation_task_exit, 3
.set rite_operation_display_update, 5
.set rite_operation_unknown, 0xffff

.set rite_payload_none, 0
.set rite_payload_utf8_text, 1
.set rite_payload_input_request, 3
.set rite_payload_exit_status, 4
.set rite_payload_display_patch, 5

.set rite_right_console_write, 0x01
.set rite_right_input_read, 0x02
.set rite_right_task_exit, 0x04
.set rite_right_display_update, 0x10

.set rite_status_ok, 0
.set rite_status_would_block, 1
.set rite_status_denied, 2
.set rite_status_bad_operation, 3
.set rite_status_bad_payload, 4
.set rite_status_limit_exceeded, 5

.set rite_transport_payload_limit, 4056
.set rite_transport_oversize, 4057
.set rite_kernel_payload_limit, 4096
.set rite_private_payload_size, 16
.set rite_valid_exit_code, 42
.set rite_sanitized_flags, 0x02

.macro rite_prepare_call operation_value, payload_kind_value, rights_value, payload_size_value, payload_limit_value, trace_word_value
    movw $rite_call_size, rite_call_header_size(%r15)
    movw $\operation_value, rite_call_operation(%r15)
    movw $\payload_kind_value, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq $\rights_value, rite_call_rights_present(%r15)
    movl $\payload_size_value, rite_call_payload_size(%r15)
    movl $\payload_limit_value, rite_call_payload_limit(%r15)
    movabsq $\trace_word_value, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
.endm

# The fixed gate carries no register arguments. R15 names the one mapped
# mailbox; every other GPR is zero so the Rust dispatcher can validate the
# complete user frame rather than trusting ambient register state.
.macro rite_submit
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
    pushq $rite_sanitized_flags
    popfq
    int $rite_gate_vector
.endm

.macro rite_validate_reply status_value, sequence_value, trace_word_value
    cmpw $rite_reply_size, rite_reply_header_size(%r15)
    jne x86_64_rite_test_failed
    cmpw $\status_value, rite_reply_status(%r15)
    jne x86_64_rite_test_failed
    cmpw $rite_payload_none, rite_reply_payload_kind(%r15)
    jne x86_64_rite_test_failed
    cmpw $0, rite_reply_reserved(%r15)
    jne x86_64_rite_test_failed
    cmpl $0, rite_reply_payload_size(%r15)
    jne x86_64_rite_test_failed
    cmpl $0, rite_reply_reserved_2(%r15)
    jne x86_64_rite_test_failed
    cmpq $\sequence_value, rite_reply_sequence(%r15)
    jne x86_64_rite_test_failed
    movabsq $\trace_word_value, %rax
    cmpq %rax, rite_reply_trace_id(%r15)
    jne x86_64_rite_test_failed
    cmpq %rax, rite_reply_trace_id + 8(%r15)
    jne x86_64_rite_test_failed
.endm

.text
.p2align 4

.global x86_64_enter_rite_task
.type x86_64_enter_rite_task, @function
# System V inputs are RDI = user RIP, RSI = user RSP, RDX = user data/stack
# selector, RCX = user code selector, and R8 = saved-kernel-RSP pointer. The
# function preserves the caller's callee-saved registers on its original stack.
# Exact task exit returns through x86_64_resume_rite_task; ordinary return from
# the user payload is impossible.
x86_64_enter_rite_task:
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
    pushq $rite_sanitized_flags
    pushq %rcx
    pushq %rdi

    # This fixture has no floating-point ABI. TS denies inherited x87, MMX, and
    # SIMD state, while clearing all GPRs leaves only mapped code and mailbox
    # state as feature-local inputs.
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
.size x86_64_enter_rite_task, . - x86_64_enter_rite_task

.global x86_64_resume_rite_task
.type x86_64_resume_rite_task, @function
# RDI is the fixed completion status, RSI is the single-use saved kernel stack,
# and RDX is the original CR0.TS bit. The routine abandons the TSS privilege
# stack and returns through the original Rust call frame. It is valid only on
# the bootstrap processor while IF and NMI remain masked.
x86_64_resume_rite_task:
    testq %rdx, %rdx
    jnz .Lrite_task_switched_restored
    clts
.Lrite_task_switched_restored:
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
.size x86_64_resume_rite_task, . - x86_64_resume_rite_task

.global x86_64_rite_gate_entry
.type x86_64_rite_gate_entry, @function
# The DPL-3 interrupt gate supplies RIP, CS, RFLAGS, RSP, and SS. Synthetic
# vector and error words make a 22-qword frame. R12 retains its pre-alignment
# address across Rust; returning calls restore every user register. A valid
# task-exit dispatch resumes the saved kernel stack nonlocally and never returns.
x86_64_rite_gate_entry:
    cld
    pushq $0
    pushq $rite_gate_vector
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
    callq x86_64_rite_gate_dispatch
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
.size x86_64_rite_gate_entry, . - x86_64_rite_gate_entry

.section .rodata.rite_test.payload,"a",@progbits
.p2align 4

# This exact byte range is copied into one private user RX page. The only
# writable address it names is the fixture's fixed RW+NX mailbox. Calls and
# canonical replies share the 40-byte prefix; request payload begins at +40.
.global x86_64_rite_test_user_start
.type x86_64_rite_test_user_start, @function
x86_64_rite_test_user_start:
    movabsq $user_mailbox_address, %r15

    # 1. An unknown operation fails before any payload interpretation.
    rite_prepare_call rite_operation_unknown, 0xffff, -1, -1, 0, 0x1111111111111111
    rite_submit
.global x86_64_rite_test_unknown_return
x86_64_rite_test_unknown_return:
    rite_validate_reply rite_status_bad_operation, 1, 0x1111111111111111

    # 2. A known console operation without its declared right is denied.
    movb $0x78, rite_payload(%r15)
    rite_prepare_call rite_operation_console_write, rite_payload_utf8_text, rite_right_input_read, 4097, 0, 0x2222222222222222
    rite_submit
.global x86_64_rite_test_missing_console_return
x86_64_rite_test_missing_console_return:
    rite_validate_reply rite_status_denied, 2, 0x2222222222222222

    # 3. Display authority declared by a shell fixture remains an overclaim.
    movb $0x01, rite_payload(%r15)
    rite_prepare_call rite_operation_display_update, rite_payload_display_patch, rite_right_display_update, 1, 1, 0x3333333333333333
    rite_submit
.global x86_64_rite_test_display_overclaim_return
x86_64_rite_test_display_overclaim_return:
    rite_validate_reply rite_status_denied, 3, 0x3333333333333333

    # 4. One byte beyond the fixed page envelope is rejected without copying.
    rite_prepare_call rite_operation_console_write, rite_payload_utf8_text, rite_right_console_write, rite_transport_oversize, rite_kernel_payload_limit, 0x4444444444444444
    rite_submit
.global x86_64_rite_test_transport_limit_return
x86_64_rite_test_transport_limit_return:
    rite_validate_reply rite_status_limit_exceeded, 4, 0x4444444444444444

    # 5. Structurally admitted console bytes still require valid UTF-8.
    movw $0x28c3, rite_payload(%r15)
    rite_prepare_call rite_operation_console_write, rite_payload_utf8_text, rite_right_console_write, 2, 2, 0x5555555555555555
    rite_submit
.global x86_64_rite_test_invalid_utf8_return
x86_64_rite_test_invalid_utf8_return:
    rite_validate_reply rite_status_bad_payload, 5, 0x5555555555555555

    # 6. The private printable sentinel is copied and committed exactly once.
    movabsq $0x344d395137584f4e, %rax
    movq %rax, rite_payload(%r15)
    movabsq $0x4b5a385036583252, %rax
    movq %rax, rite_payload + 8(%r15)
    rite_prepare_call rite_operation_console_write, rite_payload_utf8_text, rite_right_console_write, rite_private_payload_size, rite_private_payload_size, 0x6666666666666666
    rite_submit
.global x86_64_rite_test_console_return
x86_64_rite_test_console_return:
    rite_validate_reply rite_status_ok, 6, 0x6666666666666666

    # 7. An empty input source completes without side effects.
    rite_prepare_call rite_operation_input_read, rite_payload_input_request, rite_right_input_read, 0, 0, 0x7777777777777777
    rite_submit
.global x86_64_rite_test_input_return
x86_64_rite_test_input_return:
    rite_validate_reply rite_status_would_block, 7, 0x7777777777777777

    # 8. The exit body is copied only after admission, then its reservation
    # rejects the malformed body while leaving the vessel running.
    movl $rite_valid_exit_code, rite_payload(%r15)
    movl $1, rite_payload + 4(%r15)
    rite_prepare_call rite_operation_task_exit, rite_payload_exit_status, rite_right_task_exit, 8, 8, 0x8888888888888888
    rite_submit
.global x86_64_rite_test_malformed_exit_return
x86_64_rite_test_malformed_exit_return:
    rite_validate_reply rite_status_bad_payload, 8, 0x8888888888888888

    # 9. Exact exit commits terminally. Rust consumes the saved continuation;
    # no reply is observable and this IRET return address must never execute.
    movl $rite_valid_exit_code, rite_payload(%r15)
    movl $0, rite_payload + 4(%r15)
    rite_prepare_call rite_operation_task_exit, rite_payload_exit_status, rite_right_task_exit, 8, 8, 0x9999999999999999
    rite_submit
.global x86_64_rite_test_exit_return
x86_64_rite_test_exit_return:
x86_64_rite_test_failed:
    ud2
.global x86_64_rite_test_user_end
x86_64_rite_test_user_end:
.size x86_64_rite_test_user_start, . - x86_64_rite_test_user_start

.section .note.GNU-stack,"",@progbits
