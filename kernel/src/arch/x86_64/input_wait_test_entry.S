.set input_wait_kernel_data_selector, 0x10
.set input_wait_user_data_selector, 0x2b
.set input_wait_user_code_selector, 0x33
.set input_wait_scheduler_gate_vector, 0xf2
.set input_wait_rite_gate_vector, 0xf3

.set input_wait_user_mailbox, 0x0000700000003000
.set input_wait_frame_code_segment, 144
.set input_wait_frame_stack_segment, 168

.set input_wait_call_header_size, 0
.set input_wait_call_operation, 2
.set input_wait_call_payload_kind, 4
.set input_wait_call_reserved, 6
.set input_wait_call_rights_present, 8
.set input_wait_call_payload_size, 16
.set input_wait_call_payload_limit, 20
.set input_wait_call_trace_id, 24
.set input_wait_call_size, 40
.set input_wait_payload, input_wait_call_size

.set input_wait_reply_header_size, 0
.set input_wait_reply_status, 2
.set input_wait_reply_payload_kind, 4
.set input_wait_reply_reserved, 6
.set input_wait_reply_payload_size, 8
.set input_wait_reply_reserved_2, 12
.set input_wait_reply_sequence, 16
.set input_wait_reply_trace_id, 24
.set input_wait_reply_size, 40

.set input_wait_request_structure_size, input_wait_payload + 0
.set input_wait_request_version, input_wait_payload + 2
.set input_wait_request_flags, input_wait_payload + 4
.set input_wait_request_reply_capacity, input_wait_payload + 8
.set input_wait_request_reserved, input_wait_payload + 12
.set input_wait_request_size, 16

.set input_wait_delivery_structure_size, input_wait_payload + 0
.set input_wait_delivery_version, input_wait_payload + 2
.set input_wait_delivery_action, input_wait_payload + 4
.set input_wait_delivery_origin, input_wait_payload + 6
.set input_wait_delivery_text_size, input_wait_payload + 8
.set input_wait_delivery_erase_count, input_wait_payload + 10
.set input_wait_delivery_reserved, input_wait_payload + 12
.set input_wait_delivery_sequence, input_wait_payload + 16
.set input_wait_delivery_text, input_wait_payload + 24
.set input_wait_delivery_size, 280

.set input_wait_operation_session_input_read, 6
.set input_wait_payload_none, 0
.set input_wait_payload_session_input_request, 6
.set input_wait_payload_session_input_delivery, 7
.set input_wait_right_session_input_read, 0x20
.set input_wait_request_wait_if_empty, 1
.set input_wait_version_1, 1
.set input_wait_action_insert_text, 1
.set input_wait_origin_human_text, 1
.set input_wait_origin_machine_text, 2
.set input_wait_status_ok, 0
.set input_wait_status_interrupted, 6
.set input_wait_private_text_size, 5

.set input_wait_control_publish, 1
.set input_wait_control_preempt, 2
.set input_wait_control_close, 3
.set input_wait_control_complete, 4
.set input_wait_initial_flags, 0x02

.set input_wait_trace_1_low, 0x1716151413121110
.set input_wait_trace_1_high, 0x1f1e1d1c1b1a1918
.set input_wait_trace_2_low, 0x2726252423222120
.set input_wait_trace_2_high, 0x2f2e2d2c2b2a2928
.set input_wait_trace_3_low, 0x3736353433323130
.set input_wait_trace_3_high, 0x3f3e3d3c3b3a3938

.macro input_wait_prepare_request trace_low_value, trace_high_value
    movw $input_wait_call_size, input_wait_call_header_size(%r15)
    movw $input_wait_operation_session_input_read, input_wait_call_operation(%r15)
    movw $input_wait_payload_session_input_request, input_wait_call_payload_kind(%r15)
    movw $0, input_wait_call_reserved(%r15)
    movq $input_wait_right_session_input_read, input_wait_call_rights_present(%r15)
    movl $input_wait_request_size, input_wait_call_payload_size(%r15)
    movl $input_wait_request_size, input_wait_call_payload_limit(%r15)
    movabsq $\trace_low_value, %rax
    movq %rax, input_wait_call_trace_id(%r15)
    movabsq $\trace_high_value, %rax
    movq %rax, input_wait_call_trace_id + 8(%r15)
    movw $input_wait_request_size, input_wait_request_structure_size(%r15)
    movw $input_wait_version_1, input_wait_request_version(%r15)
    movl $input_wait_request_wait_if_empty, input_wait_request_flags(%r15)
    movl $input_wait_delivery_size, input_wait_request_reply_capacity(%r15)
    movl $0, input_wait_request_reserved(%r15)
.endm

# No register supplies authority to the rite gate. R15 names the fixed mailbox
# and every other GPR is zero so Rust can validate the entire saved frame.
.macro input_wait_submit_rite
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
    pushq $input_wait_initial_flags
    popfq
    int $input_wait_rite_gate_vector
.endm

.macro input_wait_validate_delivery origin_value, rite_sequence_value, delivery_sequence_value, trace_low_value, trace_high_value, failure_label
    cmpw $input_wait_reply_size, input_wait_reply_header_size(%r15)
    jne \failure_label
    cmpw $input_wait_status_ok, input_wait_reply_status(%r15)
    jne \failure_label
    cmpw $input_wait_payload_session_input_delivery, input_wait_reply_payload_kind(%r15)
    jne \failure_label
    cmpw $0, input_wait_reply_reserved(%r15)
    jne \failure_label
    cmpl $input_wait_delivery_size, input_wait_reply_payload_size(%r15)
    jne \failure_label
    cmpl $0, input_wait_reply_reserved_2(%r15)
    jne \failure_label
    cmpq $\rite_sequence_value, input_wait_reply_sequence(%r15)
    jne \failure_label
    movabsq $\trace_low_value, %rax
    cmpq %rax, input_wait_reply_trace_id(%r15)
    jne \failure_label
    movabsq $\trace_high_value, %rax
    cmpq %rax, input_wait_reply_trace_id + 8(%r15)
    jne \failure_label

    cmpw $input_wait_delivery_size, input_wait_delivery_structure_size(%r15)
    jne \failure_label
    cmpw $input_wait_version_1, input_wait_delivery_version(%r15)
    jne \failure_label
    cmpw $input_wait_action_insert_text, input_wait_delivery_action(%r15)
    jne \failure_label
    cmpw $\origin_value, input_wait_delivery_origin(%r15)
    jne \failure_label
    cmpw $input_wait_private_text_size, input_wait_delivery_text_size(%r15)
    jne \failure_label
    cmpw $0, input_wait_delivery_erase_count(%r15)
    jne \failure_label
    cmpl $0, input_wait_delivery_reserved(%r15)
    jne \failure_label
    cmpq $\delivery_sequence_value, input_wait_delivery_sequence(%r15)
    jne \failure_label
    cmpl $0x98e20a41, input_wait_delivery_text(%r15)
    jne \failure_label
    cmpb $0x83, input_wait_delivery_text + 4(%r15)
    jne \failure_label
    leaq input_wait_delivery_text + input_wait_private_text_size(%r15), %rdi
    movl $(input_wait_delivery_size - 24 - input_wait_private_text_size), %ecx
    xorl %eax, %eax
    repe scasb
    jne \failure_label
.endm

.macro input_wait_validate_interruption rite_sequence_value, trace_low_value, trace_high_value, failure_label
    cmpw $input_wait_reply_size, input_wait_reply_header_size(%r15)
    jne \failure_label
    cmpw $input_wait_status_interrupted, input_wait_reply_status(%r15)
    jne \failure_label
    cmpw $input_wait_payload_none, input_wait_reply_payload_kind(%r15)
    jne \failure_label
    cmpw $0, input_wait_reply_reserved(%r15)
    jne \failure_label
    cmpl $0, input_wait_reply_payload_size(%r15)
    jne \failure_label
    cmpl $0, input_wait_reply_reserved_2(%r15)
    jne \failure_label
    cmpq $\rite_sequence_value, input_wait_reply_sequence(%r15)
    jne \failure_label
    movabsq $\trace_low_value, %rax
    cmpq %rax, input_wait_reply_trace_id(%r15)
    jne \failure_label
    movabsq $\trace_high_value, %rax
    cmpq %rax, input_wait_reply_trace_id + 8(%r15)
    jne \failure_label
    leaq input_wait_payload(%r15), %rdi
    movl $input_wait_delivery_size, %ecx
    xorl %eax, %eax
    repe scasb
    jne \failure_label
.endm

.macro input_wait_prepare_control operation_value
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
    movabsq $input_wait_user_mailbox, %r15
    movl $\operation_value, %eax
    pushq $input_wait_initial_flags
    popfq
.endm

.text
.p2align 4

.extern x86_64_input_wait_rite_dispatch
.global x86_64_input_wait_rite_gate_entry
.type x86_64_input_wait_rite_gate_entry, @function
# A CPL-3 interrupt plus two synthetic words and every GPR form the shared
# 22-qword scheduler frame. Rust may return the same caller or a selected task.
x86_64_input_wait_rite_gate_entry:
    cld
    pushq $0
    pushq $input_wait_rite_gate_vector
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
    movw $input_wait_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_input_wait_rite_dispatch
    testq %rax, %rax
    jz .Linput_wait_rite_gate_invalid
    movq %rax, %rsp

    cmpq $input_wait_user_code_selector, input_wait_frame_code_segment(%rsp)
    jne .Linput_wait_rite_gate_invalid
    cmpq $input_wait_user_data_selector, input_wait_frame_stack_segment(%rsp)
    jne .Linput_wait_rite_gate_invalid
    movw $input_wait_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $input_wait_kernel_data_selector, %ax
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
.Linput_wait_rite_gate_invalid:
    cli
    ud2
.size x86_64_input_wait_rite_gate_entry, . - x86_64_input_wait_rite_gate_entry

.section .rodata.input_wait_test.consumer,"a",@progbits
.p2align 4
.global x86_64_input_wait_consumer_start
.type x86_64_input_wait_consumer_start, @function
x86_64_input_wait_consumer_start:
    cld
    movabsq $input_wait_user_mailbox, %r15

    input_wait_prepare_request input_wait_trace_1_low, input_wait_trace_1_high
    input_wait_submit_rite
.global x86_64_input_wait_consumer_immediate_return
x86_64_input_wait_consumer_immediate_return:
    input_wait_validate_delivery input_wait_origin_human_text, 1, 1, input_wait_trace_1_low, input_wait_trace_1_high, .Linput_wait_consumer_failed

    input_wait_prepare_request input_wait_trace_2_low, input_wait_trace_2_high
    input_wait_submit_rite
.global x86_64_input_wait_consumer_delivery_return
x86_64_input_wait_consumer_delivery_return:
    input_wait_validate_delivery input_wait_origin_machine_text, 2, 2, input_wait_trace_2_low, input_wait_trace_2_high, .Linput_wait_consumer_failed

    input_wait_prepare_request input_wait_trace_3_low, input_wait_trace_3_high
    input_wait_submit_rite
.global x86_64_input_wait_consumer_interrupted_return
x86_64_input_wait_consumer_interrupted_return:
    input_wait_validate_interruption 3, input_wait_trace_3_low, input_wait_trace_3_high, .Linput_wait_consumer_failed

    input_wait_prepare_control input_wait_control_complete
    int $input_wait_scheduler_gate_vector
.global x86_64_input_wait_consumer_completion_return
x86_64_input_wait_consumer_completion_return:
.Linput_wait_consumer_failed:
    ud2
    jmp .Linput_wait_consumer_failed
.global x86_64_input_wait_consumer_end
x86_64_input_wait_consumer_end:
.size x86_64_input_wait_consumer_start, . - x86_64_input_wait_consumer_start

.section .rodata.input_wait_test.peer,"a",@progbits
.p2align 4
.global x86_64_input_wait_peer_start
.type x86_64_input_wait_peer_start, @function
x86_64_input_wait_peer_start:
    input_wait_prepare_control input_wait_control_publish
    int $input_wait_scheduler_gate_vector
.global x86_64_input_wait_peer_publish_return
x86_64_input_wait_peer_publish_return:
    input_wait_prepare_control input_wait_control_preempt
    int $input_wait_scheduler_gate_vector
.global x86_64_input_wait_peer_preempt_return
x86_64_input_wait_peer_preempt_return:
    input_wait_prepare_control input_wait_control_close
    int $input_wait_scheduler_gate_vector
.global x86_64_input_wait_peer_close_return
x86_64_input_wait_peer_close_return:
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
    pushq $input_wait_initial_flags
    popfq
.global x86_64_input_wait_peer_fault_instruction
x86_64_input_wait_peer_fault_instruction:
    ud2
.Linput_wait_peer_failed:
    ud2
    jmp .Linput_wait_peer_failed
.global x86_64_input_wait_peer_end
x86_64_input_wait_peer_end:
.size x86_64_input_wait_peer_start, . - x86_64_input_wait_peer_start

.section .note.GNU-stack,"",@progbits
