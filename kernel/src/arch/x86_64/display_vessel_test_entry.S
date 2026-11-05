.set kernel_data_selector, 0x10
.set user_data_selector, 0x2b
.set display_rite_gate_vector, 0xf3
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

.set display_structure_size, rite_payload + 0
.set display_version, rite_payload + 2
.set display_flags, rite_payload + 4
.set display_x, rite_payload + 8
.set display_y, rite_payload + 12
.set display_width, rite_payload + 16
.set display_height, rite_payload + 20
.set display_row_bytes, rite_payload + 24
.set display_pixel_format, rite_payload + 28
.set display_reserved, rite_payload + 30
.set display_pixel_byte_length, rite_payload + 32
.set display_reserved_2, rite_payload + 36
.set display_pixels, rite_payload + 40
.set display_header_size, 40
.set display_small_payload_size, 43
.set display_row_payload_size, 3112
.set display_row_pixel_bytes, 3072
.set display_width_pixels, 1024
.set display_height_pixels, 768
.set display_state_y, 3200

.set rite_operation_task_exit, 3
.set rite_operation_display_update, 5
.set rite_payload_none, 0
.set rite_payload_exit_status, 4
.set rite_payload_display_patch, 5
.set rite_right_console_write, 0x01
.set rite_right_task_exit, 0x04
.set rite_right_display_update, 0x10
.set rite_status_ok, 0
.set rite_status_denied, 2
.set rite_status_bad_payload, 4
.set rite_sanitized_flags, 0x02
.set valid_exit_code, 42

.macro display_prepare_call operation_value, payload_kind_value, rights_value, payload_size_value, trace_word_value
    movw $rite_call_size, rite_call_header_size(%r15)
    movw \operation_value, rite_call_operation(%r15)
    movw \payload_kind_value, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq \rights_value, rite_call_rights_present(%r15)
    movl \payload_size_value, rite_call_payload_size(%r15)
    movl \payload_size_value, rite_call_payload_limit(%r15)
    movabsq \trace_word_value, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
.endm

.macro display_submit
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
    int $display_rite_gate_vector
.endm

.macro display_validate_reply status_value, sequence_value, trace_word_value
    cmpw $rite_reply_size, rite_reply_header_size(%r15)
    jne x86_64_display_vessel_failed
    cmpw \status_value, rite_reply_status(%r15)
    jne x86_64_display_vessel_failed
    cmpw $rite_payload_none, rite_reply_payload_kind(%r15)
    jne x86_64_display_vessel_failed
    cmpw $0, rite_reply_reserved(%r15)
    jne x86_64_display_vessel_failed
    cmpl $0, rite_reply_payload_size(%r15)
    jne x86_64_display_vessel_failed
    cmpl $0, rite_reply_reserved_2(%r15)
    jne x86_64_display_vessel_failed
    cmpq \sequence_value, rite_reply_sequence(%r15)
    jne x86_64_display_vessel_failed
    movabsq \trace_word_value, %rax
    cmpq %rax, rite_reply_trace_id(%r15)
    jne x86_64_display_vessel_failed
    cmpq %rax, rite_reply_trace_id + 8(%r15)
    jne x86_64_display_vessel_failed
.endm

.macro display_patch_header x_value, y_value, width_value, height_value, row_bytes_value, pixel_length_value, flags_value
    movw $display_header_size, display_structure_size(%r15)
    movw $1, display_version(%r15)
    movl \flags_value, display_flags(%r15)
    movl \x_value, display_x(%r15)
    movl \y_value, display_y(%r15)
    movl \width_value, display_width(%r15)
    movl \height_value, display_height(%r15)
    movl \row_bytes_value, display_row_bytes(%r15)
    movw $1, display_pixel_format(%r15)
    movw $0, display_reserved(%r15)
    movl \pixel_length_value, display_pixel_byte_length(%r15)
    movl $0, display_reserved_2(%r15)
.endm

# Paint one ordered half-open rectangle into the current row when Y intersects
# it. Color immediates are encoded as 0x00BBGGRR so the helper writes RGB8.
.macro display_paint_if y0, y1, x0, width_value, color_value
    movl display_state_y(%r15), %eax
    cmpl $\y0, %eax
    jb .Ldisplay_skip_\@
    cmpl $\y1, %eax
    jae .Ldisplay_skip_\@
    leaq display_pixels + (\x0 * 3)(%r15), %rdi
    movl $\width_value, %ecx
    movl $\color_value, %edx
    callq x86_64_display_vessel_paint_span
.Ldisplay_skip_\@:
.endm

.text
.p2align 4

.global x86_64_display_vessel_enter_task
.type x86_64_display_vessel_enter_task, @function
x86_64_display_vessel_enter_task:
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
.size x86_64_display_vessel_enter_task, . - x86_64_display_vessel_enter_task

.global x86_64_display_vessel_resume_task
.type x86_64_display_vessel_resume_task, @function
x86_64_display_vessel_resume_task:
    testq %rdx, %rdx
    jnz .Ldisplay_task_switched_restored
    clts
.Ldisplay_task_switched_restored:
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
.size x86_64_display_vessel_resume_task, . - x86_64_display_vessel_resume_task

.global x86_64_display_vessel_gate_entry
.type x86_64_display_vessel_gate_entry, @function
x86_64_display_vessel_gate_entry:
    cld
    pushq $0
    pushq $display_rite_gate_vector
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
    callq x86_64_display_vessel_gate_dispatch
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
.size x86_64_display_vessel_gate_entry, . - x86_64_display_vessel_gate_entry

.section .rodata.display_vessel_test.payload,"a",@progbits
.p2align 4

.global x86_64_display_vessel_user_start
.type x86_64_display_vessel_user_start, @function
x86_64_display_vessel_user_start:
    movabsq $user_mailbox_address, %r15

    # 1. Declaring an ungranted console right cannot confer display authority.
    display_patch_header $0, $0, $1, $1, $3, $3, $0
    movb $0, display_pixels(%r15)
    movb $0, display_pixels + 1(%r15)
    movb $0, display_pixels + 2(%r15)
    display_prepare_call $rite_operation_display_update, $rite_payload_display_patch, $rite_right_console_write, $display_small_payload_size, $0x1111111111111111
    display_submit
.global x86_64_display_vessel_denied_return
x86_64_display_vessel_denied_return:
    display_validate_reply $rite_status_denied, $1, $0x1111111111111111

    # 2. A structurally admitted body with nonzero flags is malformed.
    display_patch_header $0, $0, $1, $1, $3, $3, $1
    display_prepare_call $rite_operation_display_update, $rite_payload_display_patch, $rite_right_display_update, $display_small_payload_size, $0x2222222222222222
    display_submit
.global x86_64_display_vessel_malformed_return
x86_64_display_vessel_malformed_return:
    display_validate_reply $rite_status_bad_payload, $2, $0x2222222222222222

    # 3. A canonical patch outside the declared 1024-wide output is rejected.
    display_patch_header $1024, $0, $1, $1, $3, $3, $0
    display_prepare_call $rite_operation_display_update, $rite_payload_display_patch, $rite_right_display_update, $display_small_payload_size, $0x3333333333333333
    display_submit
.global x86_64_display_vessel_bounds_return
x86_64_display_vessel_bounds_return:
    display_validate_reply $rite_status_bad_payload, $3, $0x3333333333333333

    # 4..771. Generate one complete public 1024x768 fixture as bounded rows.
    movl $0, display_state_y(%r15)
.Ldisplay_row_loop:
    movl display_state_y(%r15), %eax
    movw $display_header_size, display_structure_size(%r15)
    movw $1, display_version(%r15)
    movl $0, display_flags(%r15)
    movl $0, display_x(%r15)
    movl %eax, display_y(%r15)
    movl $display_width_pixels, display_width(%r15)
    movl $1, display_height(%r15)
    movl $display_row_pixel_bytes, display_row_bytes(%r15)
    movw $1, display_pixel_format(%r15)
    movw $0, display_reserved(%r15)
    movl $display_row_pixel_bytes, display_pixel_byte_length(%r15)
    movl $0, display_reserved_2(%r15)

    leaq display_pixels(%r15), %rdi
    movl $display_width_pixels, %ecx
    movl $0x00ede6de, %edx
    callq x86_64_display_vessel_paint_span

    display_paint_if 0, 72, 0, 1024, 0x00efe8e0
    display_paint_if 70, 72, 0, 1024, 0x006a5b4e
    display_paint_if 24, 48, 32, 24, 0x00f6465b
    display_paint_if 27, 33, 72, 152, 0x002b1f16
    display_paint_if 39, 43, 72, 96, 0x005c4c3d
    display_paint_if 111, 655, 55, 240, 0x00c4b8aa
    display_paint_if 101, 645, 45, 240, 0x00fefcfa
    display_paint_if 105, 641, 49, 232, 0x006a5b4e
    display_paint_if 107, 639, 51, 228, 0x00efe8e0
    display_paint_if 137, 143, 75, 104, 0x002b1f16
    display_paint_if 177, 221, 67, 196, 0x00fefcfa
    display_paint_if 177, 221, 67, 5, 0x00f6465b
    display_paint_if 192, 198, 83, 120, 0x002b1f16
    display_paint_if 249, 255, 75, 112, 0x005c4c3d
    display_paint_if 285, 291, 75, 136, 0x005c4c3d
    display_paint_if 321, 327, 75, 96, 0x005c4c3d
    display_paint_if 111, 655, 327, 650, 0x00c4b8aa
    display_paint_if 101, 645, 317, 650, 0x00fefcfa
    display_paint_if 105, 641, 321, 642, 0x006a5b4e
    display_paint_if 107, 639, 323, 638, 0x00efe8e0
    display_paint_if 137, 145, 355, 196, 0x002b1f16
    display_paint_if 157, 162, 355, 288, 0x005c4c3d
    display_paint_if 187, 447, 355, 574, 0x006a5b4e
    display_paint_if 191, 443, 359, 566, 0x002b1f16
    display_paint_if 219, 224, 383, 232, 0x00f7f1eb
    display_paint_if 243, 248, 383, 368, 0x00f7f1eb
    display_paint_if 267, 272, 383, 304, 0x00f7f1eb
    display_paint_if 291, 296, 383, 88, 0x00f6465b
    display_paint_if 291, 296, 479, 248, 0x00f7f1eb
    display_paint_if 479, 555, 367, 330, 0x00f6465b
    display_paint_if 482, 552, 370, 324, 0x00efe8e0
    display_paint_if 485, 549, 373, 318, 0x00f6465b
    display_paint_if 487, 547, 375, 314, 0x00fefcfa
    display_paint_if 513, 521, 463, 138, 0x002b1f16
    display_paint_if 591, 596, 355, 240, 0x005c4c3d
    display_paint_if 591, 601, 895, 10, 0x00f6465b

    display_prepare_call $rite_operation_display_update, $rite_payload_display_patch, $rite_right_display_update, $display_row_payload_size, $0x4444444444444444
    display_submit
.global x86_64_display_vessel_row_return
x86_64_display_vessel_row_return:
    cmpw $rite_reply_size, rite_reply_header_size(%r15)
    jne x86_64_display_vessel_failed
    cmpw $rite_status_ok, rite_reply_status(%r15)
    jne x86_64_display_vessel_failed
    cmpw $rite_payload_none, rite_reply_payload_kind(%r15)
    jne x86_64_display_vessel_failed
    cmpw $0, rite_reply_reserved(%r15)
    jne x86_64_display_vessel_failed
    cmpl $0, rite_reply_payload_size(%r15)
    jne x86_64_display_vessel_failed
    cmpl $0, rite_reply_reserved_2(%r15)
    jne x86_64_display_vessel_failed
    movl display_state_y(%r15), %eax
    addq $4, %rax
    cmpq %rax, rite_reply_sequence(%r15)
    jne x86_64_display_vessel_failed
    movabsq $0x4444444444444444, %rax
    cmpq %rax, rite_reply_trace_id(%r15)
    jne x86_64_display_vessel_failed
    cmpq %rax, rite_reply_trace_id + 8(%r15)
    jne x86_64_display_vessel_failed
    movl display_state_y(%r15), %eax
    incl %eax
    movl %eax, display_state_y(%r15)
    cmpl $display_height_pixels, %eax
    jb .Ldisplay_row_loop

    # 772. Terminal exit is committed but never copied back to the dead task.
    movl $valid_exit_code, rite_payload(%r15)
    movl $0, rite_payload + 4(%r15)
    display_prepare_call $rite_operation_task_exit, $rite_payload_exit_status, $rite_right_task_exit, $8, $0x5555555555555555
    display_submit
.global x86_64_display_vessel_exit_return
x86_64_display_vessel_exit_return:
    ud2

x86_64_display_vessel_paint_span:
    testl %ecx, %ecx
    jz .Ldisplay_paint_done
.Ldisplay_paint_next:
    movl %edx, %eax
    movb %al, 0(%rdi)
    shrl $8, %eax
    movb %al, 1(%rdi)
    shrl $8, %eax
    movb %al, 2(%rdi)
    addq $3, %rdi
    decl %ecx
    jnz .Ldisplay_paint_next
.Ldisplay_paint_done:
    retq

x86_64_display_vessel_failed:
    ud2

.p2align 4
.global x86_64_display_vessel_feature_sentinel
.type x86_64_display_vessel_feature_sentinel, @object
x86_64_display_vessel_feature_sentinel:
.ascii "ANOM-DISP-V1-X64"
.size x86_64_display_vessel_feature_sentinel, . - x86_64_display_vessel_feature_sentinel

.global x86_64_display_vessel_user_end
x86_64_display_vessel_user_end:
.size x86_64_display_vessel_user_start, . - x86_64_display_vessel_user_start

.section .note.GNU-stack,"",@progbits
