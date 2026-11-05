.set ny_probe_gate_vector, 0xf3
.set ny_probe_mailbox, 0x800000
.set ny_probe_magic, 0x4e595f5354415254

# This shim implements only the inspected scalar fixture's generated C entry
# contract. It supplies no hosted runtime, allocator, text API, TLS or unwinder.
# The kernel owns role/scenario, copies initialized magic, and zeroes the BSS.
.section .text.ny_probe_entry,"ax",@progbits
.p2align 4
.global anomalous_ny_probe_entry
.type anomalous_ny_probe_entry, @function
anomalous_ny_probe_entry:
    cld
    movq %rsp, %rax
    andl $15, %eax
    jnz .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox
    ja .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox + 8
    ja .Lny_probe_invalid
    movq $ny_probe_mailbox + 16, %rsi
    movl $64, %ecx
.Lny_probe_mailbox_zero:
    cmpb $0, (%rsi)
    jne .Lny_probe_invalid
    incq %rsi
    decl %ecx
    jnz .Lny_probe_mailbox_zero
    movabsq $ny_probe_magic, %rax
    cmpq %rax, ny_probe_mailbox + 80
    jne .Lny_probe_invalid
    addq $8, %rsi
    movl $40, %ecx
.Lny_probe_reserved_zero:
    cmpb $0, (%rsi)
    jne .Lny_probe_invalid
    incq %rsi
    decl %ecx
    jnz .Lny_probe_reserved_zero
    leaq anomalous_ny_probe_bss_start(%rip), %rsi
    leaq anomalous_ny_probe_bss_end(%rip), %rdx
.Lny_probe_bss_zero:
    cmpq %rdx, %rsi
    je .Lny_probe_bss_clean
    cmpb $0, (%rsi)
    jne .Lny_probe_invalid
    incq %rsi
    jmp .Lny_probe_bss_zero
.Lny_probe_bss_clean:
    movq $1, ny_probe_mailbox + 16
    movq $1, ny_probe_mailbox + 64
    movq $1, ny_probe_mailbox + 72
    movl $1, %eax
    pushq $2
    popfq
.global anomalous_ny_probe_start_gate
anomalous_ny_probe_start_gate:
    int $ny_probe_gate_vector
.global anomalous_ny_probe_startgate_return
anomalous_ny_probe_startgate_return:
    # RSP is 16-aligned before CALL, hence 8 mod 16 at each SysV callee.
    xorl %edi, %edi
    xorl %esi, %esi
    call main
    movl %eax, %eax
    movq %rax, ny_probe_mailbox + 56
    cmpq $1, ny_probe_mailbox + 32
    jne .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox + 40
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 48
    jne .Lny_probe_invalid
    movl $ny_probe_mailbox, %edi
    movl $128, %esi
    movl $3, %eax
.global anomalous_ny_probe_exit_gate
anomalous_ny_probe_exit_gate:
    int $ny_probe_gate_vector
.global anomalous_ny_probe_exit_return
anomalous_ny_probe_exit_return:
    ud2
.size anomalous_ny_probe_entry, . - anomalous_ny_probe_entry

.section .text.ny_probe_shim,"ax",@progbits
.p2align 4
.macro ny_probe_check_alignment
    movq %rsp, %r11
    andl $15, %r11d
    cmpl $8, %r11d
    jne .Lny_probe_invalid
.endm
.macro ny_probe_check_started
    ny_probe_check_alignment
    cmpq $1, ny_probe_mailbox + 16
    jne .Lny_probe_invalid
    movabsq $ny_probe_magic, %r11
    cmpq %r11, ny_probe_mailbox + 80
    jne .Lny_probe_invalid
.endm

.global nyrt_abi_version
.type nyrt_abi_version, @function
nyrt_abi_version:
    ny_probe_check_started
    cmpq $0, ny_probe_mailbox + 24
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 32
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 40
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 48
    jne .Lny_probe_invalid
    incq ny_probe_mailbox + 24
    movl $1, %eax
    cmpq $0, ny_probe_mailbox
    jne .Lny_probe_abi_done
    cmpq $0, ny_probe_mailbox + 8
    jne .Lny_probe_abi_done
    xorl %eax, %eax
.Lny_probe_abi_done:
    ret
.size nyrt_abi_version, . - nyrt_abi_version

.global nyrt_context_begin
.type nyrt_context_begin, @function
nyrt_context_begin:
    ny_probe_check_started
    testl %edi, %edi
    jnz .Lny_probe_invalid
    testq %rsi, %rsi
    jnz .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox + 24
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 32
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 40
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 48
    jne .Lny_probe_invalid
    incq ny_probe_mailbox + 32
    ret
.size nyrt_context_begin, . - nyrt_context_begin

.global nyrt_context_end
.type nyrt_context_end, @function
nyrt_context_end:
    ny_probe_check_started
    cmpq $1, ny_probe_mailbox + 24
    jne .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox + 32
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 40
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 48
    jne .Lny_probe_invalid
    # The pinned generated C wrapper preserves Ny main's EAX result in EBX
    # across this call. Artifact inspection qualifies that exact sequence;
    # this register observation is private fixture machinery, not a Ny ABI.
    cmpl $50000000, %ebx
    jne .Lny_probe_invalid
    movl %ebx, %r11d
    movq %r11, ny_probe_mailbox + 56
    incq ny_probe_mailbox + 40
    cmpq $0, ny_probe_mailbox
    jne .Lny_probe_context_done
    cmpq $1, ny_probe_mailbox + 8
    jne .Lny_probe_context_done
    # Tail entry preserves the generated caller's SysV stack alignment.
    leaq anomalous_ny_probe_injected_message(%rip), %rdi
    jmp nyrt_fail
.Lny_probe_context_done:
    ret
.size nyrt_context_end, . - nyrt_context_end

.global nyrt_fail
.type nyrt_fail, @function
nyrt_fail:
    ny_probe_check_started
    cmpq $0, ny_probe_mailbox
    jne .Lny_probe_invalid
    cmpq $1, ny_probe_mailbox + 24
    jne .Lny_probe_invalid
    cmpq $0, ny_probe_mailbox + 48
    jne .Lny_probe_invalid
    movq ny_probe_mailbox + 8, %r11
    cmpq $1, %r11
    ja .Lny_probe_invalid
    cmpq %r11, ny_probe_mailbox + 32
    jne .Lny_probe_invalid
    cmpq %r11, ny_probe_mailbox + 40
    jne .Lny_probe_invalid
    incq ny_probe_mailbox + 48
    movl $2, %eax
.global anomalous_ny_probe_fail_gate
anomalous_ny_probe_fail_gate:
    int $ny_probe_gate_vector
.global anomalous_ny_probe_fail_ud2
anomalous_ny_probe_fail_ud2:
    # The kernel validated/captured RDI through checked copy and cleared IF.
    ud2
.size nyrt_fail, . - nyrt_fail

# Ny emits this helper's references even when scalar source never calls it.
# Keep the object intact and make every unsupported entry observably fatal.
.global strlen
.type strlen, @function
strlen:
    movl $1, %edi
    jmp .Lny_probe_unsupported
.size strlen, . - strlen
.global calloc
.type calloc, @function
calloc:
    movl $2, %edi
    jmp .Lny_probe_unsupported
.size calloc, . - calloc
.global memcpy
.type memcpy, @function
memcpy:
    movl $3, %edi
    jmp .Lny_probe_unsupported
.size memcpy, . - memcpy
.Lny_probe_invalid:
    movl $4, %edi
.Lny_probe_unsupported:
    movl $4, %eax
.global anomalous_ny_probe_unsupported_gate
anomalous_ny_probe_unsupported_gate:
    int $ny_probe_gate_vector
.global anomalous_ny_probe_unsupported_return
anomalous_ny_probe_unsupported_return:
    ud2

.section .rodata.ny_probe,"a",@progbits
.global anomalous_ny_probe_injected_message
anomalous_ny_probe_injected_message:
    .asciz "Ny Anomalous probe: injected context_end failure\n"
.global anomalous_ny_probe_injected_message_end
anomalous_ny_probe_injected_message_end:

.section .data.ny_probe_mailbox,"aw",@progbits
.p2align 3
    .zero 80
    .quad ny_probe_magic
    .zero 40
.section .note.GNU-stack,"",@progbits
