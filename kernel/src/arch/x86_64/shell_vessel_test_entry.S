.set shell_vessel_kernel_data_selector, 0x10
.set shell_vessel_user_data_selector, 0x2b
.set shell_vessel_user_code_selector, 0x33
.set shell_vessel_scheduler_gate_vector, 0xf2
.set shell_vessel_rite_gate_vector, 0xf3

.set shell_vessel_user_mailbox, 0x0000700000003000
.set shell_vessel_user_stack, 0x0000700000005000
.set shell_vessel_stack_marker_a, 0x100
.set shell_vessel_stack_marker_b, 0x108
.set shell_vessel_stack_arm, 0x110

.set shell_vessel_frame_code_segment, 144
.set shell_vessel_frame_stack_segment, 168

.set shell_vessel_call_header_size, 0
.set shell_vessel_call_operation, 2
.set shell_vessel_call_payload_kind, 4
.set shell_vessel_call_reserved, 6
.set shell_vessel_call_rights_present, 8
.set shell_vessel_call_payload_size, 16
.set shell_vessel_call_payload_limit, 20
.set shell_vessel_call_trace_id, 24
.set shell_vessel_call_size, 40
.set shell_vessel_payload, shell_vessel_call_size

.set shell_vessel_reply_header_size, 0
.set shell_vessel_reply_status, 2
.set shell_vessel_reply_payload_kind, 4
.set shell_vessel_reply_reserved, 6
.set shell_vessel_reply_payload_size, 8
.set shell_vessel_reply_reserved_2, 12
.set shell_vessel_reply_sequence, 16
.set shell_vessel_reply_trace_id, 24
.set shell_vessel_reply_size, 40

.set shell_vessel_operation_console_write, 1
.set shell_vessel_operation_input_read, 2
.set shell_vessel_operation_task_exit, 3

.set shell_vessel_payload_none, 0
.set shell_vessel_payload_utf8_text, 1
.set shell_vessel_payload_input_request, 3
.set shell_vessel_payload_exit_status, 4

.set shell_vessel_right_console_write, 0x01
.set shell_vessel_right_input_read, 0x02
.set shell_vessel_right_task_exit, 0x04

.set shell_vessel_status_ok, 0
.set shell_vessel_status_would_block, 1

.set shell_vessel_console_size, 17
.set shell_vessel_exit_size, 8
.set shell_vessel_initial_flags, 0x02
.set shell_vessel_running_flags, 0x202

.set shell_vessel_control_vessel_ready, 1
.set shell_vessel_control_supervisor_await_fault, 2
.set shell_vessel_control_supervisor_restart, 3
.set shell_vessel_control_supervisor_complete, 5
.set shell_vessel_wake_fault, 0x41
.set shell_vessel_wake_completion, 0x42

.set shell_vessel_generation_1_marker_a, 0x8f12c6a47bd930e1
.set shell_vessel_generation_1_marker_b, 0x17e4b9c26a5d803f
.set shell_vessel_generation_1_arm, 0xa55ac33c96e17b4d
.set shell_vessel_supervisor_marker_a, 0x6d31a8f2c47b950e
.set shell_vessel_supervisor_marker_b, 0xe20c759a13bf486d
.set shell_vessel_supervisor_continuity, 0x4b9ed170a62cf835

.set shell_vessel_generation_1_console_trace_low, 0x91c47a2e6315b8d0
.set shell_vessel_generation_1_console_trace_high, 0x2f6da905c37e184b
.set shell_vessel_generation_1_input_trace_low, 0xa27d183f54c690e1
.set shell_vessel_generation_1_input_trace_high, 0x3b7eba16d48f295c
.set shell_vessel_generation_2_console_trace_low, 0xb38e294065d7a1f2
.set shell_vessel_generation_2_console_trace_high, 0x4c8fcb27e5903a6d
.set shell_vessel_generation_2_input_trace_low, 0xc49f3a5176e8b203
.set shell_vessel_generation_2_input_trace_high, 0x5da0dc38f6a14b7e
.set shell_vessel_generation_2_exit_trace_low, 0xd5a04b6287f9c314
.set shell_vessel_generation_2_exit_trace_high, 0x6eb1ed4907b25c8f

.set shell_vessel_supervisor_mailbox_marker_a, 0
.set shell_vessel_supervisor_mailbox_marker_b, 8
.set shell_vessel_supervisor_mailbox_state, 16

.macro shell_vessel_prepare_rite operation_value, payload_kind_value, rights_value, payload_size_value, payload_limit_value, trace_low_value, trace_high_value
    movw $shell_vessel_call_size, shell_vessel_call_header_size(%r15)
    movw $\operation_value, shell_vessel_call_operation(%r15)
    movw $\payload_kind_value, shell_vessel_call_payload_kind(%r15)
    movw $0, shell_vessel_call_reserved(%r15)
    movq $\rights_value, shell_vessel_call_rights_present(%r15)
    movl $\payload_size_value, shell_vessel_call_payload_size(%r15)
    movl $\payload_limit_value, shell_vessel_call_payload_limit(%r15)
    movabsq $\trace_low_value, %rax
    movq %rax, shell_vessel_call_trace_id(%r15)
    movabsq $\trace_high_value, %rax
    movq %rax, shell_vessel_call_trace_id + 8(%r15)
.endm

# The fixed rite gate takes no register arguments. R15 names the task's one
# RW+NX mailbox and every other GPR is zero so Rust can validate the complete
# scheduler-compatible frame. Rite entry always observes IF clear and IOPL zero.
.macro shell_vessel_submit_rite
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
    pushq $shell_vessel_initial_flags
    popfq
    int $shell_vessel_rite_gate_vector
.endm

.macro shell_vessel_validate_reply status_value, sequence_value, trace_low_value, trace_high_value, failure_label
    cmpw $shell_vessel_reply_size, shell_vessel_reply_header_size(%r15)
    jne \failure_label
    cmpw $\status_value, shell_vessel_reply_status(%r15)
    jne \failure_label
    cmpw $shell_vessel_payload_none, shell_vessel_reply_payload_kind(%r15)
    jne \failure_label
    cmpw $0, shell_vessel_reply_reserved(%r15)
    jne \failure_label
    cmpl $0, shell_vessel_reply_payload_size(%r15)
    jne \failure_label
    cmpl $0, shell_vessel_reply_reserved_2(%r15)
    jne \failure_label
    cmpq $\sequence_value, shell_vessel_reply_sequence(%r15)
    jne \failure_label
    movabsq $\trace_low_value, %rax
    cmpq %rax, shell_vessel_reply_trace_id(%r15)
    jne \failure_label
    movabsq $\trace_high_value, %rax
    cmpq %rax, shell_vessel_reply_trace_id + 8(%r15)
    jne \failure_label
.endm

.macro shell_vessel_prepare_control operation_value
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
    movabsq $shell_vessel_user_mailbox, %r15
    movl $\operation_value, %eax
    pushq $shell_vessel_initial_flags
    popfq
.endm

.text
.p2align 4

.extern x86_64_shell_vessel_rite_dispatch
.global x86_64_shell_vessel_rite_gate_entry
.type x86_64_shell_vessel_rite_gate_entry, @function
# A CPL-3 interrupt supplies RIP, CS, RFLAGS, RSP, and SS. Two synthetic words
# plus every GPR form the same 22-qword task frame used by the scheduler. Rust
# returns the fully validated frame to restore: the caller for ordinary replies,
# or the awakened supervisor after the terminal task-exit rite.
x86_64_shell_vessel_rite_gate_entry:
    cld
    pushq $0
    pushq $shell_vessel_rite_gate_vector
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
    movw $shell_vessel_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movw %ax, %fs
    movw %ax, %gs
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_shell_vessel_rite_dispatch
    testq %rax, %rax
    jz .Lshell_vessel_rite_gate_invalid
    movq %rax, %rsp

    cmpq $shell_vessel_user_code_selector, shell_vessel_frame_code_segment(%rsp)
    jne .Lshell_vessel_rite_gate_invalid
    cmpq $shell_vessel_user_data_selector, shell_vessel_frame_stack_segment(%rsp)
    jne .Lshell_vessel_rite_gate_invalid
    movw $shell_vessel_user_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw $shell_vessel_kernel_data_selector, %ax
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
.Lshell_vessel_rite_gate_invalid:
    cli
    ud2
.size x86_64_shell_vessel_rite_gate_entry, . - x86_64_shell_vessel_rite_gate_entry

.section .rodata.shell_vessel_test.generation_1,"a",@progbits
.p2align 4
.global x86_64_shell_vessel_generation_1_start
.type x86_64_shell_vessel_generation_1_start, @function
x86_64_shell_vessel_generation_1_start:
    cld
    movabsq $shell_vessel_user_mailbox, %r15
    movabsq $shell_vessel_user_stack, %r13
    cmpq $0, shell_vessel_stack_marker_a(%r13)
    jne .Lshell_vessel_generation_1_failed
    cmpq $0, shell_vessel_stack_marker_b(%r13)
    jne .Lshell_vessel_generation_1_failed
    cmpq $0, shell_vessel_stack_arm(%r13)
    jne .Lshell_vessel_generation_1_failed
    movabsq $shell_vessel_generation_1_marker_a, %rax
    movq %rax, shell_vessel_stack_marker_a(%r13)
    movabsq $shell_vessel_generation_1_marker_b, %rax
    movq %rax, shell_vessel_stack_marker_b(%r13)

    # Validate the exact compiled human projection before deriving its literal
    # argument. This is a fixed proof command, not a general parser or Canticle.
    leaq .Lshell_vessel_generation_1_source(%rip), %rsi
    movabsq $0x6f6e41206f686365, %rax
    cmpq %rax, 0(%rsi)
    jne .Lshell_vessel_generation_1_failed
    movabsq $0x722073756f6c616d, %rax
    cmpq %rax, 8(%rsi)
    jne .Lshell_vessel_generation_1_failed
    cmpl $0x79646165, 16(%rsi)
    jne .Lshell_vessel_generation_1_failed
    cmpw $0x0a2e, 20(%rsi)
    jne .Lshell_vessel_generation_1_failed

    addq $5, %rsi
    leaq shell_vessel_payload(%r15), %rdi
    movl $shell_vessel_console_size, %ecx
    rep movsb
    shell_vessel_prepare_rite shell_vessel_operation_console_write, shell_vessel_payload_utf8_text, shell_vessel_right_console_write, shell_vessel_console_size, shell_vessel_console_size, shell_vessel_generation_1_console_trace_low, shell_vessel_generation_1_console_trace_high
    shell_vessel_submit_rite
.global x86_64_shell_vessel_generation_1_console_return
x86_64_shell_vessel_generation_1_console_return:
    shell_vessel_validate_reply shell_vessel_status_ok, 1, shell_vessel_generation_1_console_trace_low, shell_vessel_generation_1_console_trace_high, .Lshell_vessel_generation_1_failed

    shell_vessel_prepare_rite shell_vessel_operation_input_read, shell_vessel_payload_input_request, shell_vessel_right_input_read, 0, 0, shell_vessel_generation_1_input_trace_low, shell_vessel_generation_1_input_trace_high
    shell_vessel_submit_rite
.global x86_64_shell_vessel_generation_1_input_return
x86_64_shell_vessel_generation_1_input_return:
    shell_vessel_validate_reply shell_vessel_status_would_block, 2, shell_vessel_generation_1_input_trace_low, shell_vessel_generation_1_input_trace_high, .Lshell_vessel_generation_1_failed

    shell_vessel_prepare_control shell_vessel_control_vessel_ready
    int $shell_vessel_scheduler_gate_vector
.global x86_64_shell_vessel_generation_1_ready_return
x86_64_shell_vessel_generation_1_ready_return:
    pushq $shell_vessel_running_flags
    popfq
    movabsq $shell_vessel_user_stack, %r13
.Lshell_vessel_generation_1_wait:
    movabsq $shell_vessel_generation_1_marker_a, %rax
    cmpq %rax, shell_vessel_stack_marker_a(%r13)
    jne .Lshell_vessel_generation_1_failed
    movabsq $shell_vessel_generation_1_marker_b, %rax
    cmpq %rax, shell_vessel_stack_marker_b(%r13)
    jne .Lshell_vessel_generation_1_failed
    movq shell_vessel_stack_arm(%r13), %rax
    testq %rax, %rax
    jz .Lshell_vessel_generation_1_pause
    movabsq $shell_vessel_generation_1_arm, %r14
    cmpq %r14, %rax
    jne .Lshell_vessel_generation_1_failed

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
    pushq $shell_vessel_running_flags
    popfq
.global x86_64_shell_vessel_generation_1_fault_instruction
x86_64_shell_vessel_generation_1_fault_instruction:
    ud2
.Lshell_vessel_generation_1_pause:
    pause
    jmp .Lshell_vessel_generation_1_wait
.Lshell_vessel_generation_1_failed:
    ud2
    jmp .Lshell_vessel_generation_1_failed
.Lshell_vessel_generation_1_source:
    .ascii "echo Anomalous ready.\n"
.Lshell_vessel_generation_1_source_end:
.if (.Lshell_vessel_generation_1_source_end - .Lshell_vessel_generation_1_source) != 22
    .error "shell vessel generation-1 source must remain exactly 22 bytes"
.endif
.global x86_64_shell_vessel_generation_1_end
x86_64_shell_vessel_generation_1_end:
.size x86_64_shell_vessel_generation_1_start, . - x86_64_shell_vessel_generation_1_start

.section .rodata.shell_vessel_test.generation_2,"a",@progbits
.p2align 4
.global x86_64_shell_vessel_generation_2_start
.type x86_64_shell_vessel_generation_2_start, @function
x86_64_shell_vessel_generation_2_start:
    cld
    movabsq $shell_vessel_user_mailbox, %r15
    movabsq $shell_vessel_user_stack, %r13
    cmpq $0, 0(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 8(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 16(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 24(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 32(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 40(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 48(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, 56(%r15)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, shell_vessel_stack_marker_a(%r13)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, shell_vessel_stack_marker_b(%r13)
    jne .Lshell_vessel_generation_2_failed
    cmpq $0, shell_vessel_stack_arm(%r13)
    jne .Lshell_vessel_generation_2_failed

    # The replacement models direct machine construction of the same semantic
    # console request. It introduces no machine AST or serialized shell format.
    movabsq $0x756f6c616d6f6e41, %rax
    movq %rax, shell_vessel_payload(%r15)
    movabsq $0x2e79646165722073, %rax
    movq %rax, shell_vessel_payload + 8(%r15)
    movb $0x0a, shell_vessel_payload + 16(%r15)
    shell_vessel_prepare_rite shell_vessel_operation_console_write, shell_vessel_payload_utf8_text, shell_vessel_right_console_write, shell_vessel_console_size, shell_vessel_console_size, shell_vessel_generation_2_console_trace_low, shell_vessel_generation_2_console_trace_high
    shell_vessel_submit_rite
.global x86_64_shell_vessel_generation_2_console_return
x86_64_shell_vessel_generation_2_console_return:
    shell_vessel_validate_reply shell_vessel_status_ok, 1, shell_vessel_generation_2_console_trace_low, shell_vessel_generation_2_console_trace_high, .Lshell_vessel_generation_2_failed

    shell_vessel_prepare_rite shell_vessel_operation_input_read, shell_vessel_payload_input_request, shell_vessel_right_input_read, 0, 0, shell_vessel_generation_2_input_trace_low, shell_vessel_generation_2_input_trace_high
    shell_vessel_submit_rite
.global x86_64_shell_vessel_generation_2_input_return
x86_64_shell_vessel_generation_2_input_return:
    shell_vessel_validate_reply shell_vessel_status_would_block, 2, shell_vessel_generation_2_input_trace_low, shell_vessel_generation_2_input_trace_high, .Lshell_vessel_generation_2_failed

    movl $0, shell_vessel_payload(%r15)
    movl $0, shell_vessel_payload + 4(%r15)
    shell_vessel_prepare_rite shell_vessel_operation_task_exit, shell_vessel_payload_exit_status, shell_vessel_right_task_exit, shell_vessel_exit_size, shell_vessel_exit_size, shell_vessel_generation_2_exit_trace_low, shell_vessel_generation_2_exit_trace_high
    shell_vessel_submit_rite
.global x86_64_shell_vessel_generation_2_exit_return
x86_64_shell_vessel_generation_2_exit_return:
.Lshell_vessel_generation_2_failed:
    ud2
    jmp .Lshell_vessel_generation_2_failed
.global x86_64_shell_vessel_generation_2_end
x86_64_shell_vessel_generation_2_end:
.size x86_64_shell_vessel_generation_2_start, . - x86_64_shell_vessel_generation_2_start

.section .rodata.shell_vessel_test.supervisor,"a",@progbits
.p2align 4
.global x86_64_shell_vessel_supervisor_start
.type x86_64_shell_vessel_supervisor_start, @function
x86_64_shell_vessel_supervisor_start:
    movabsq $shell_vessel_user_mailbox, %r15
    movabsq $shell_vessel_supervisor_marker_a, %rax
    movq %rax, shell_vessel_supervisor_mailbox_marker_a(%r15)
    movabsq $shell_vessel_supervisor_marker_b, %rax
    movq %rax, shell_vessel_supervisor_mailbox_marker_b(%r15)

    shell_vessel_prepare_control shell_vessel_control_supervisor_await_fault
    int $shell_vessel_scheduler_gate_vector
.global x86_64_shell_vessel_supervisor_fault_return
x86_64_shell_vessel_supervisor_fault_return:
    cmpq $shell_vessel_wake_fault, %rax
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_marker_a, %rax
    cmpq %rax, shell_vessel_supervisor_mailbox_marker_a(%r15)
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_marker_b, %rax
    cmpq %rax, shell_vessel_supervisor_mailbox_marker_b(%r15)
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_continuity, %rax
    movq %rax, shell_vessel_supervisor_mailbox_state(%r15)

    shell_vessel_prepare_control shell_vessel_control_supervisor_restart
    int $shell_vessel_scheduler_gate_vector
.global x86_64_shell_vessel_supervisor_completion_return
x86_64_shell_vessel_supervisor_completion_return:
    cmpq $shell_vessel_wake_completion, %rax
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_marker_a, %rax
    cmpq %rax, shell_vessel_supervisor_mailbox_marker_a(%r15)
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_marker_b, %rax
    cmpq %rax, shell_vessel_supervisor_mailbox_marker_b(%r15)
    jne .Lshell_vessel_supervisor_failed
    movabsq $shell_vessel_supervisor_continuity, %rax
    cmpq %rax, shell_vessel_supervisor_mailbox_state(%r15)
    jne .Lshell_vessel_supervisor_failed

    shell_vessel_prepare_control shell_vessel_control_supervisor_complete
    int $shell_vessel_scheduler_gate_vector
.global x86_64_shell_vessel_supervisor_complete_return
x86_64_shell_vessel_supervisor_complete_return:
.Lshell_vessel_supervisor_failed:
    ud2
    jmp .Lshell_vessel_supervisor_failed
.global x86_64_shell_vessel_supervisor_end
x86_64_shell_vessel_supervisor_end:
.size x86_64_shell_vessel_supervisor_start, . - x86_64_shell_vessel_supervisor_start

.section .note.GNU-stack,"",@progbits
