# The Anomalous-owned half of a Ny vessel.
#
# Ny supplies a freestanding object that publishes __ny_entry and imports the
# native runtime hooks the shipped archive satisfies, plus the two symbols this
# file defines. It knows nothing about mailboxes or interrupt vectors, and under
# ADR 0019 it should not: that ABI is ours and is unstable pre-alpha. This file
# is the whole of the binding.
#
# What changed from the first vessel: Ny now issues the rite itself. The
# console write is a symbol Ny declares with @interop("host") and calls from
# inside its own control flow, so the vessel decides when to speak and can read
# the result back and act on it. The surrounding entry installs the region,
# runs Ny, and exits with what Ny returned.
#
# What changed again: a second operation. The input read is the first rite
# whose reply carries a payload, and the first to bring Ny data the vessel
# could not have contained -- Ny asks for a line, is given one, composes an
# answer around it in the region, and speaks that. Of the three writes, two
# carry bytes Ny built in the region and one carries a constant from its own
# image, which is what the fixture's region checks distinguish.
#
# And again: Ny can now fail. `__ny_host_fail` here replaces the archive's
# weak no-op, so an allocation this vessel cannot satisfy leaves as a rite
# rather than as a user task spinning where nothing can see it.
#
# Register discipline: the gate requires every GPR but R15 to be zero, so the
# stub saves the System V callee-saved registers before zeroing them and
# restores them afterwards. Nothing survives a submission in a register, so
# anything needed across one lives in the vessel's own writable cell.

.set vessel_rite_gate_vector, 0xf3
.set vessel_mailbox_address, 0x0000600000005000
.set vessel_arena_address, 0x0000600000004000
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
.set rite_operation_session_input_read, 6
.set rite_operation_task_yield, 4

.set rite_payload_none, 0
.set rite_payload_utf8_text, 1
.set rite_payload_exit_status, 4
.set rite_payload_session_input_request, 6
.set rite_payload_session_input_delivery, 7

.set rite_right_console_write, 0x01
.set rite_right_task_exit, 0x04
.set rite_right_session_input_read, 0x20
.set rite_right_task_yield, 0x08

.set rite_status_ok, 0

# The status this vessel exits with when Ny fails inside it, and when it
# fails with nothing to say. Neither can collide with a value Ny returns.
.set vessel_failure_status, 70
.set vessel_silent_failure_status, 71

# The session-input request and delivery bodies, version 1. Their field
# offsets are stated here rather than derived, because this file is the only
# place in the vessel that knows the ABI has a shape at all.
.set vessel_input_version_1, 1
.set vessel_input_wait_if_empty, 1
.set vessel_input_action_insert_text, 1
.set vessel_input_origin_human_text, 1
.set vessel_input_text_capacity, 256
.set vessel_input_request_size, 16
.set vessel_input_delivery_size, 280

.set vessel_input_request_structure_size, rite_payload + 0
.set vessel_input_request_version, rite_payload + 2
.set vessel_input_request_flags, rite_payload + 4
.set vessel_input_request_reply_capacity, rite_payload + 8
.set vessel_input_request_reserved, rite_payload + 12

.set vessel_input_delivery_structure_size, rite_payload + 0
.set vessel_input_delivery_version, rite_payload + 2
.set vessel_input_delivery_action, rite_payload + 4
.set vessel_input_delivery_origin, rite_payload + 6
.set vessel_input_delivery_text_size, rite_payload + 8
.set vessel_input_delivery_erase_count, rite_payload + 10
.set vessel_input_delivery_reserved, rite_payload + 12
.set vessel_input_delivery_sequence, rite_payload + 16
.set vessel_input_delivery_text, rite_payload + 24

.set rite_transport_payload_limit, 4056
.set rite_sanitized_flags, 0x02

.set vessel_console_trace, 0x4e59564553534c31
.set vessel_exit_trace, 0x4e59564553534c32
.set vessel_input_trace, 0x4e59564553534c33
.set vessel_yield_trace, 0x4e59564553534c34

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

# const char *anomalous_rite_input_read(void)
#
# The second symbol Ny declares with @interop("host"), and the first rite whose
# reply carries a payload. It submits one session_input_read with wait_if_empty
# set -- this vessel has nothing to do until it is answered, and polling would
# burn a quantum to learn nothing -- validates the delivery, copies the private
# text into the vessel's own cell, terminates it, and returns a pointer to that
# cell, which is what a Ny `text` is natively.
#
# Ny never sees the mailbox. The bytes are copied out of it before the pointer
# Ny receives exists, so the delivery can be answered and the mailbox reused
# without anything Ny holds becoming stale.
.global anomalous_rite_input_read
.type anomalous_rite_input_read, @function
anomalous_rite_input_read:
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    # One more qword so the stack is 16-byte aligned at the call below; the
    # entry offset plus six saved registers leaves it eight short.
    pushq %rax

    movabsq $vessel_mailbox_address, %r15
    movw $rite_call_size, rite_call_header_size(%r15)
    movw $rite_operation_session_input_read, rite_call_operation(%r15)
    movw $rite_payload_session_input_request, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq $rite_right_session_input_read, rite_call_rights_present(%r15)
    movl $vessel_input_request_size, rite_call_payload_size(%r15)
    movl $vessel_input_request_size, rite_call_payload_limit(%r15)
    movabsq $vessel_input_trace, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
    movw $vessel_input_request_size, vessel_input_request_structure_size(%r15)
    movw $vessel_input_version_1, vessel_input_request_version(%r15)
    movl $vessel_input_wait_if_empty, vessel_input_request_flags(%r15)
    movl $vessel_input_delivery_size, vessel_input_request_reply_capacity(%r15)
    movl $0, vessel_input_request_reserved(%r15)
    vessel_submit
.global anomalous_ny_vessel_input_return
anomalous_ny_vessel_input_return:
    # The reply envelope. A console write is acknowledged with no payload; this
    # one must carry a delivery, so the payload-free shape is a failure here.
    cmpw $rite_reply_size, rite_reply_header_size(%r15)
    jne .Lvessel_invalid
    cmpw $rite_status_ok, rite_reply_status(%r15)
    jne .Lvessel_invalid
    cmpw $rite_payload_session_input_delivery, rite_reply_payload_kind(%r15)
    jne .Lvessel_invalid
    cmpw $0, rite_reply_reserved(%r15)
    jne .Lvessel_invalid
    cmpl $vessel_input_delivery_size, rite_reply_payload_size(%r15)
    jne .Lvessel_invalid
    cmpl $0, rite_reply_reserved_2(%r15)
    jne .Lvessel_invalid
    movabsq $vessel_input_trace, %rax
    cmpq %rax, rite_reply_trace_id(%r15)
    jne .Lvessel_invalid
    cmpq %rax, rite_reply_trace_id + 8(%r15)
    jne .Lvessel_invalid

    # The delivery body. This vessel understands inserted text from a human
    # session and nothing else; every other action, origin or version is a
    # reply it was not written to read, not a reply it should guess at.
    cmpw $vessel_input_delivery_size, vessel_input_delivery_structure_size(%r15)
    jne .Lvessel_invalid
    cmpw $vessel_input_version_1, vessel_input_delivery_version(%r15)
    jne .Lvessel_invalid
    cmpw $vessel_input_action_insert_text, vessel_input_delivery_action(%r15)
    jne .Lvessel_invalid
    cmpw $vessel_input_origin_human_text, vessel_input_delivery_origin(%r15)
    jne .Lvessel_invalid
    cmpw $0, vessel_input_delivery_erase_count(%r15)
    jne .Lvessel_invalid
    cmpl $0, vessel_input_delivery_reserved(%r15)
    jne .Lvessel_invalid
    cmpq $0, vessel_input_delivery_sequence(%r15)
    je .Lvessel_invalid

    # The delivery states a length; a Ny text is terminated. The copy and the
    # terminator are what turn one into the other.
    movzwl vessel_input_delivery_text_size(%r15), %ecx
    testl %ecx, %ecx
    jz .Lvessel_invalid
    cmpl $vessel_input_text_capacity, %ecx
    ja .Lvessel_invalid
    movq %rcx, anomalous_ny_vessel_line_length(%rip)
    leaq anomalous_ny_vessel_line(%rip), %rdi
    leaq vessel_input_delivery_text(%r15), %rsi
    movq %rcx, %rdx
    callq nyrt_mem_copy
    # The length comes back from the vessel's own cell: nyrt_mem_copy is free
    # to clobber every caller-saved register, RCX among them.
    movq anomalous_ny_vessel_line_length(%rip), %rcx
    leaq anomalous_ny_vessel_line(%rip), %rax
    movb $0, (%rax, %rcx)

    popq %rcx
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    # Computed after the restores so no saved register has to carry it.
    leaq anomalous_ny_vessel_line(%rip), %rax
    retq
.size anomalous_rite_input_read, . - anomalous_rite_input_read

# long anomalous_rite_yield(void)
#
# Gives up the processor in the middle of Ny's own work and returns how many
# times this vessel has done so. The kernel may run another task before this
# returns; nothing here can tell, and nothing here needs to -- what the vessel
# holds lives in its own pages and its own stack, and the gate restores every
# register from the frame it saved.
#
# No call is made between the saves and the submission, so unlike the two
# stubs above this one needs no alignment padding.
.global anomalous_rite_yield
.type anomalous_rite_yield, @function
anomalous_rite_yield:
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    movabsq $vessel_mailbox_address, %r15
    movw $rite_call_size, rite_call_header_size(%r15)
    movw $rite_operation_task_yield, rite_call_operation(%r15)
    movw $rite_payload_none, rite_call_payload_kind(%r15)
    movw $0, rite_call_reserved(%r15)
    movq $rite_right_task_yield, rite_call_rights_present(%r15)
    movl $0, rite_call_payload_size(%r15)
    movl $0, rite_call_payload_limit(%r15)
    movabsq $vessel_yield_trace, %rax
    movq %rax, rite_call_trace_id(%r15)
    movq %rax, rite_call_trace_id + 8(%r15)
    vessel_submit
.global anomalous_ny_vessel_yield_return
anomalous_ny_vessel_yield_return:
    vessel_validate_reply rite_status_ok, vessel_yield_trace
    incq anomalous_ny_vessel_yields(%rip)

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    movq anomalous_ny_vessel_yields(%rip), %rax
    retq
.size anomalous_rite_yield, . - anomalous_rite_yield

# long anomalous_vessel_mode(void)
#
# Not a rite. The host writes this cell before the vessel is entered, exactly
# as it installs the region, and this reads it back. It is how one image can
# be asked for the run that is meant to fail, so the failure path can be
# proved without a second vessel and without losing the evidence the ordinary
# run already banks.
.global anomalous_vessel_mode
.type anomalous_vessel_mode, @function
anomalous_vessel_mode:
    movq anomalous_ny_vessel_mode(%rip), %rax
    retq
.size anomalous_vessel_mode, . - anomalous_vessel_mode

# void __ny_host_fail(const char *message)
#
# The shipped runtime defines this as a weak no-op, and `nyrt_fail` spins
# forever once it returns. In a vessel that means a user task looping with no
# rite ever submitted: it does not fault, does not exit, and says nothing, so
# the dispatcher sees nothing at all and only a timeout would notice. This
# strong definition replaces it, so a Ny failure leaves by the same door as a
# success -- the message is spoken as a console_write, and the task exits with
# a status that says Ny failed rather than a value Ny chose.
#
# The message belongs to the runtime. It is read-only here and arrives with a
# trailing newline the console does not admit, so it is copied into the
# vessel's own cell and trimmed there rather than edited where it lies.
#
# This function never returns, so the System V callee-saved contract has no
# observer: the saves below are what keeps the stack aligned for the calls,
# and the matching restores would be unreachable.
.global __ny_host_fail
.type __ny_host_fail, @function
__ny_host_fail:
    pushq %rbx
    pushq %r12
    pushq %rax

    testq %rdi, %rdi
    jz .Lvessel_fail_unspoken
    movq %rdi, %r12
    callq nyrt_str_length
    testq %rax, %rax
    jz .Lvessel_fail_unspoken
    cmpq $vessel_input_text_capacity, %rax
    jbe .Lvessel_fail_bounded
    movq $vessel_input_text_capacity, %rax
.Lvessel_fail_bounded:
    movq %rax, %rbx
    leaq anomalous_ny_vessel_line(%rip), %rdi
    movq %r12, %rsi
    movq %rbx, %rdx
    callq nyrt_mem_copy

    # Trim the runtime's framing. The console admits printable bytes only, and
    # a message that is nothing but framing is one the vessel cannot speak.
    leaq anomalous_ny_vessel_line(%rip), %rax
    movq %rbx, %rcx
.Lvessel_fail_trim:
    testq %rcx, %rcx
    jz .Lvessel_fail_unspoken
    cmpb $0x20, -1(%rax, %rcx)
    ja .Lvessel_fail_trimmed
    decq %rcx
    jmp .Lvessel_fail_trim
.Lvessel_fail_trimmed:
    movb $0, (%rax, %rcx)
    movq %rcx, anomalous_ny_vessel_line_length(%rip)
    leaq anomalous_ny_vessel_line(%rip), %rdi
    callq anomalous_rite_console_write
    movq $vessel_failure_status, anomalous_ny_vessel_status(%rip)
    jmp .Lvessel_exit
.Lvessel_fail_unspoken:
    movq $vessel_silent_failure_status, anomalous_ny_vessel_status(%rip)
    jmp .Lvessel_exit
.size __ny_host_fail, . - __ny_host_fail

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

.Lvessel_exit:
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
.global anomalous_ny_vessel_line_length
anomalous_ny_vessel_line_length:
    .zero 8
# The delivered line, and room for the terminator the delivery does not carry.
# It is the vessel's own storage rather than the mailbox, so what Ny holds
# stays valid after the mailbox is answered again.
.global anomalous_ny_vessel_line
anomalous_ny_vessel_line:
    .zero vessel_input_text_capacity + 1
# How many times the vessel has yielded. Ny checks it, so a yield that did
# not complete is a value Ny can refuse rather than a silence.
.p2align 3
.global anomalous_ny_vessel_yields
anomalous_ny_vessel_yields:
    .zero 8
# The run the host asked for. The host writes it; the vessel only reads it.
.p2align 3
.global anomalous_ny_vessel_mode
anomalous_ny_vessel_mode:
    .zero 8

.section .note.GNU-stack,"",@progbits
