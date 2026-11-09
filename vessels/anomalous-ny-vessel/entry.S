# The Anomalous-owned half of a Ny vessel.
#
# Ny supplies a freestanding object that publishes __ny_entry and imports the
# native runtime hooks the shipped archive satisfies, plus one symbol this file
# defines. It knows nothing about mailboxes or interrupt vectors, and under ADR
# 0019 it should not: that ABI is ours and is unstable pre-alpha. This file is
# the whole of the binding.
#
# What changed from the first vessel: Ny now issues the rite itself. The
# console write is a symbol Ny declares with @interop("host") and calls from
# inside its own control flow, so the vessel decides when to speak and can read
# the result back and act on it. The surrounding entry installs the region,
# runs Ny, and exits with what Ny returned.
#
# Register discipline: the gate requires every GPR but R15 to be zero, so the
# stub saves the System V callee-saved registers before zeroing them and
# restores them afterwards. Nothing survives a submission in a register, so
# anything needed across one lives in the vessel's own writable cell.

.set vessel_rite_gate_vector, 0xf3
.set vessel_mailbox_address, 0x0000600000004000
.set vessel_arena_address, 0x0000600000003000
.set vessel_arena_size, 0x1000

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
.set rite_operation_task_exit, 3

.set rite_payload_none, 0
.set rite_payload_utf8_text, 1
.set rite_payload_exit_status, 4

.set rite_right_console_write, 0x01
.set rite_right_task_exit, 0x04

.set rite_status_ok, 0

.set rite_transport_payload_limit, 4056
.set rite_sanitized_flags, 0x02

.set vessel_console_trace, 0x4e59564553534c31
.set vessel_exit_trace, 0x4e59564553534c32

# Every GPR but R15 is zero at the gate, so the dispatcher validates a
# complete frame rather than trusting ambient register state.
.macro vessel_submit
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
    int $vessel_rite_gate_vector
.endm

.macro vessel_validate_reply status_value, trace_word_value
    cmpw $rite_reply_size, rite_reply_header_size(%r15)
    jne .Lvessel_invalid
    cmpw $\status_value, rite_reply_status(%r15)
    jne .Lvessel_invalid
    cmpw $rite_payload_none, rite_reply_payload_kind(%r15)
    jne .Lvessel_invalid
    cmpw $0, rite_reply_reserved(%r15)
    jne .Lvessel_invalid
    cmpl $0, rite_reply_payload_size(%r15)
    jne .Lvessel_invalid
    cmpl $0, rite_reply_reserved_2(%r15)
    jne .Lvessel_invalid
    movabsq $\trace_word_value, %rax
    cmpq %rax, rite_reply_trace_id(%r15)
    jne .Lvessel_invalid
    cmpq %rax, rite_reply_trace_id + 8(%r15)
    jne .Lvessel_invalid
.endm

.section .text.ny_vessel_rite,"ax",@progbits
.p2align 4

# long anomalous_rite_console_write(const char *message)
#
# The symbol Ny declares with @interop("host"). It takes a pointer to
# NUL-terminated bytes -- which is what a Ny `text` is natively -- copies them
# into the mailbox payload through the runtime rather than libc, submits one
# console_write, and returns the number of bytes committed so the vessel can
# check its own rite succeeded.
.global anomalous_rite_console_write
.type anomalous_rite_console_write, @function
anomalous_rite_console_write:
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    # One more qword so the stack is 16-byte aligned at the calls below; the
    # entry offset plus six saved registers leaves it eight short.
    pushq %rax

    testq %rdi, %rdi
    jz .Lvessel_invalid
    movq %rdi, %r12
    callq nyrt_str_length
    # Refuse an empty or oversized message here rather than submitting one the
    # kernel would have to reject.
    testq %rax, %rax
    jz .Lvessel_invalid
    cmpq $rite_transport_payload_limit, %rax
    ja .Lvessel_invalid
    movq %rax, anomalous_ny_vessel_length(%rip)

    movabsq $vessel_mailbox_address, %r15
    leaq rite_payload(%r15), %rdi
    movq %r12, %rsi
    movq %rax, %rdx
    callq nyrt_mem_copy

    movabsq $vessel_mailbox_address, %r15
    movq anomalous_ny_vessel_length(%rip), %rcx
    movw $rite_call_size, rite_call_header_size(%r15)
    movw $rite_operation_console_write, rite_call_operation(%r15)
    movw $rite_payload_utf8_text, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq $rite_right_console_write, rite_call_rights_present(%r15)
    movl %ecx, rite_call_payload_size(%r15)
    movl %ecx, rite_call_payload_limit(%r15)
    movabsq $vessel_console_trace, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
    vessel_submit
.global anomalous_ny_vessel_console_return
anomalous_ny_vessel_console_return:
    vessel_validate_reply rite_status_ok, vessel_console_trace
    # The committed length comes back from the vessel's own cell, because the
    # submission cleared every register that could have carried it.
    movq anomalous_ny_vessel_length(%rip), %rax
    movq %rax, %rbx
    popq %rax
    movq %rbx, %rax
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    retq
.size anomalous_rite_console_write, . - anomalous_rite_console_write

.section .text.ny_vessel_entry,"ax",@progbits
.p2align 4
.global anomalous_ny_vessel_entry
.type anomalous_ny_vessel_entry, @function
anomalous_ny_vessel_entry:
    cld
    # The kernel enters on a 16-byte boundary; keep it so every System V
    # callee below observes the ordinary 8-byte entry offset.
    movq %rsp, %rax
    andl $15, %eax
    jnz .Lvessel_invalid

    # Install the region the vessel allocates from. The region is the host's:
    # Ny receives no address and cannot name it.
    movabsq $vessel_arena_address, %rdi
    movq $vessel_arena_size, %rsi
    callq nyrt_heap_init

    # Runtime ABI check, context begin, globals, Ny main, context end. Ny
    # issues its own console rites from inside this call.
    callq __ny_entry
    # Ny returns i64; this vessel's exit status is its low 32 bits.
    movl %eax, %eax
    movq %rax, anomalous_ny_vessel_status(%rip)

    movabsq $vessel_mailbox_address, %r15
    movq anomalous_ny_vessel_status(%rip), %rax
    movl %eax, rite_payload(%r15)
    movl $0, rite_payload + 4(%r15)
    movw $rite_call_size, rite_call_header_size(%r15)
    movw $rite_operation_task_exit, rite_call_operation(%r15)
    movw $rite_payload_exit_status, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq $rite_right_task_exit, rite_call_rights_present(%r15)
    movl $8, rite_call_payload_size(%r15)
    movl $8, rite_call_payload_limit(%r15)
    movabsq $vessel_exit_trace, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
    vessel_submit
# Exit commits terminally: the kernel consumes the saved continuation and no
# reply is observable, so this address must never execute.
.global anomalous_ny_vessel_exit_return
anomalous_ny_vessel_exit_return:
.Lvessel_invalid:
.global anomalous_ny_vessel_failed
anomalous_ny_vessel_failed:
    ud2
.size anomalous_ny_vessel_entry, . - anomalous_ny_vessel_entry

# The vessel's own writable state. Nothing survives a submission in a
# register, so the committed length and the exit status cross in memory.
.section .bss.ny_vessel_state,"aw",@nobits
.p2align 3
.global anomalous_ny_vessel_status
anomalous_ny_vessel_status:
    .zero 8
.global anomalous_ny_vessel_length
anomalous_ny_vessel_length:
    .zero 8

.section .note.GNU-stack,"",@progbits
