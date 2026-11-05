.section .text.linux_client_entry,"ax",@progbits
.p2align 4
.global anomalous_linux_client_entry
.type anomalous_linux_client_entry, @function
anomalous_linux_client_entry:
    # Capture the Linux process stack before a call or Rust prologue changes it.
    # The process entry RSP is 16-byte aligned; CALL establishes SysV alignment.
    movq %rsp, %rdi
    cld
    callq anomalous_linux_client_run
    ud2
.size anomalous_linux_client_entry, . - anomalous_linux_client_entry

.section .text.linux_client_syscall,"ax",@progbits
.p2align 4
.global anomalous_linux_client_syscall3
.type anomalous_linux_client_syscall3, @function
anomalous_linux_client_syscall3:
    # SysV (number, arg0, arg1, arg2) -> Linux x86-64 raw syscall registers.
    # SYSCALL clobbers RCX/R11, already caller-saved under the C ABI. Every
    # callee-saved register is untouched. Return signed byte count/negative errno.
    movq %rdi, %rax
    movq %rsi, %rdi
    movq %rdx, %rsi
    movq %rcx, %rdx
.global anomalous_linux_client_syscall_instruction
anomalous_linux_client_syscall_instruction:
    syscall
.global anomalous_linux_client_syscall_return
anomalous_linux_client_syscall_return:
    retq
.size anomalous_linux_client_syscall3, . - anomalous_linux_client_syscall3

.p2align 4
.global anomalous_linux_client_fault_instruction
.type anomalous_linux_client_fault_instruction, @function
anomalous_linux_client_fault_instruction:
    # The kernel independently recognizes this RIP and queued-first-packet
    # phase. No private syscall or INT gate arms the Linux fixture's fault.
    ud2
.size anomalous_linux_client_fault_instruction, . - anomalous_linux_client_fault_instruction

.p2align 4
.global anomalous_linux_client_entry_probe
.type anomalous_linux_client_entry_probe, @function
anomalous_linux_client_entry_probe:
    # Preserve the C caller, then establish a leaf red zone below the actual
    # SYSCALL RSP. No call or push touches it after the canaries are installed.
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    subq $8, %rsp
    movq %rsp, 0(%rsp)
    pushq $0x202
    popfq
    movabsq $0xfeedfacecafebabe, %rax
    movq %rax, -8(%rsp)
    movq %rax, -16(%rsp)
    movq %rax, -24(%rsp)
    movq %rax, -32(%rsp)
    movq %rax, -40(%rsp)
    movq %rax, -48(%rsp)
    movq %rax, -56(%rsp)
    movq %rax, -64(%rsp)
    movq %rax, -72(%rsp)
    movq %rax, -80(%rsp)
    movq %rax, -88(%rsp)
    movq %rax, -96(%rsp)
    movq %rax, -104(%rsp)
    movq %rax, -112(%rsp)
    movq %rax, -120(%rsp)
    movq %rax, -128(%rsp)
    movabsq $0x1122334400000001, %rbx
    movabsq $0x1122334400000002, %rbp
    movabsq $0x1122334400000003, %rdi
    movabsq $0x1122334400000004, %rsi
    movabsq $0x1122334400000005, %rdx
    movabsq $0x1122334400000006, %r8
    movabsq $0x1122334400000007, %r9
    movabsq $0x1122334400000008, %r10
    movabsq $0x1122334400000009, %r12
    movabsq $0x112233440000000a, %r13
    movabsq $0x112233440000000b, %r14
    movabsq $0x112233440000000c, %r15
    movq $-1, %rax
.global anomalous_linux_client_probe_syscall_instruction
anomalous_linux_client_probe_syscall_instruction:
    syscall
.global anomalous_linux_client_probe_syscall_return
anomalous_linux_client_probe_syscall_return:
    cmpq $-38, %rax
    jne .Llinux_probe_failed
    cmpq $0x202, %r11
    jne .Llinux_probe_failed
    leaq anomalous_linux_client_probe_syscall_return(%rip), %rax
    cmpq %rax, %rcx
    jne .Llinux_probe_failed
    cmpq %rsp, 0(%rsp)
    jne .Llinux_probe_failed
    movabsq $0x1122334400000001, %rax
    cmpq %rax, %rbx
    jne .Llinux_probe_failed
    movabsq $0x1122334400000002, %rax
    cmpq %rax, %rbp
    jne .Llinux_probe_failed
    movabsq $0x1122334400000003, %rax
    cmpq %rax, %rdi
    jne .Llinux_probe_failed
    movabsq $0x1122334400000004, %rax
    cmpq %rax, %rsi
    jne .Llinux_probe_failed
    movabsq $0x1122334400000005, %rax
    cmpq %rax, %rdx
    jne .Llinux_probe_failed
    movabsq $0x1122334400000006, %rax
    cmpq %rax, %r8
    jne .Llinux_probe_failed
    movabsq $0x1122334400000007, %rax
    cmpq %rax, %r9
    jne .Llinux_probe_failed
    movabsq $0x1122334400000008, %rax
    cmpq %rax, %r10
    jne .Llinux_probe_failed
    movabsq $0x1122334400000009, %rax
    cmpq %rax, %r12
    jne .Llinux_probe_failed
    movabsq $0x112233440000000a, %rax
    cmpq %rax, %r13
    jne .Llinux_probe_failed
    movabsq $0x112233440000000b, %rax
    cmpq %rax, %r14
    jne .Llinux_probe_failed
    movabsq $0x112233440000000c, %rax
    cmpq %rax, %r15
    jne .Llinux_probe_failed
    movabsq $0xfeedfacecafebabe, %rax
    cmpq %rax, -8(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -16(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -24(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -32(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -40(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -48(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -56(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -64(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -72(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -80(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -88(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -96(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -104(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -112(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -120(%rsp)
    jne .Llinux_probe_failed
    cmpq %rax, -128(%rsp)
    jne .Llinux_probe_failed
    movq $1, %rax
    addq $8, %rsp
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    retq
.Llinux_probe_failed:
    ud2
.size anomalous_linux_client_entry_probe, . - anomalous_linux_client_entry_probe

.section .data.linux_client_reserved,"aw",@progbits
.p2align 4
.zero 128
.section .note.GNU-stack,"",@progbits
