.section .text.linux_client_tls,"ax",@progbits
.p2align 4
.global anomalous_linux_client_tls_load
.type anomalous_linux_client_tls_load,@function
anomalous_linux_client_tls_load:
    fninit
    fldcw .Llinux_tls_control(%rip)
    ldmxcsr .Llinux_tls_mxcsr(%rip)
    fldl .Llinux_tls_x87(%rip)
    movdqu .Llinux_tls_xmm(%rip), %xmm15
    retq
.size anomalous_linux_client_tls_load,.-anomalous_linux_client_tls_load

.type anomalous_linux_client_tls_check,@function
anomalous_linux_client_tls_check:
    fnstcw .Llinux_tls_scratch(%rip)
    cmpw $0x077f, .Llinux_tls_scratch(%rip)
    jne .Llinux_tls_failed
    stmxcsr .Llinux_tls_scratch(%rip)
    cmpl $0x3f80, .Llinux_tls_scratch(%rip)
    jne .Llinux_tls_failed
    fstl .Llinux_tls_scratch(%rip)
    movabsq $0x3ff0000000000000, %rax
    cmpq %rax, .Llinux_tls_scratch(%rip)
    jne .Llinux_tls_failed
    movq %xmm15, %r10
    movabsq $0x31415926aabbccdd, %rax
    cmpq %rax, %r10
    jne .Llinux_tls_failed
    pshufd $0x4e, %xmm15, %xmm14
    movq %xmm14, %r10
    movabsq $0x27182818eeff0011, %rax
    cmpq %rax, %r10
    jne .Llinux_tls_failed
    retq
.size anomalous_linux_client_tls_check,.-anomalous_linux_client_tls_check

.global anomalous_linux_client_tls_syscall
.type anomalous_linux_client_tls_syscall,@function
anomalous_linux_client_tls_syscall:
    subq $8, %rsp
    callq anomalous_linux_client_tls_load
    callq anomalous_linux_client_syscall3
    movq %rax, 0(%rsp)
    callq anomalous_linux_client_tls_check
    movq 0(%rsp), %rax
    addq $8, %rsp
    retq
.size anomalous_linux_client_tls_syscall,.-anomalous_linux_client_tls_syscall

.global anomalous_linux_client_tls_probe
.type anomalous_linux_client_tls_probe,@function
anomalous_linux_client_tls_probe:
    subq $8, %rsp
    callq anomalous_linux_client_tls_load
    callq anomalous_linux_client_entry_probe
    movq %rax, 0(%rsp)
    callq anomalous_linux_client_tls_check
    movq 0(%rsp), %rax
    addq $8, %rsp
    retq
.size anomalous_linux_client_tls_probe,.-anomalous_linux_client_tls_probe

.global anomalous_linux_client_tls_canary
.type anomalous_linux_client_tls_canary,@function
anomalous_linux_client_tls_canary:
    cmpq $0, %fs:0
    jne .Llinux_tls_failed
    cmpq $0, %fs:8
    jne .Llinux_tls_failed
    movq %rdi, %fs:0
    movq $1, %fs:8
    cmpq %rdi, %fs:0
    jne .Llinux_tls_failed
    retq
.size anomalous_linux_client_tls_canary,.-anomalous_linux_client_tls_canary

.global anomalous_linux_client_tls_spin
.type anomalous_linux_client_tls_spin,@function
anomalous_linux_client_tls_spin:
    subq $8, %rsp
    callq anomalous_linux_client_tls_load
    movq $1, 0x800068
.Llinux_tls_spin:
    movabsq $0x745153aaf00d0001, %rax
    cmpq %rax, %fs:0
    jne .Llinux_tls_failed
    incq %fs:8
    jz .Llinux_tls_failed
    cmpq $1, 0x800060
    je .Llinux_tls_spin_done
    pause
    jmp .Llinux_tls_spin
.Llinux_tls_spin_done:
    callq anomalous_linux_client_tls_check
    addq $8, %rsp
    retq
.size anomalous_linux_client_tls_spin,.-anomalous_linux_client_tls_spin

.global anomalous_linux_client_tls_fault
.type anomalous_linux_client_tls_fault,@function
anomalous_linux_client_tls_fault:
    subq $8, %rsp
    callq anomalous_linux_client_tls_load
    jmp anomalous_linux_client_fault_instruction
.size anomalous_linux_client_tls_fault,.-anomalous_linux_client_tls_fault
.Llinux_tls_failed:
    ud2

.section .rodata.linux_client_tls,"a",@progbits
.p2align 4
.Llinux_tls_xmm:
    .quad 0x31415926aabbccdd, 0x27182818eeff0011
.Llinux_tls_x87:
    .quad 0x3ff0000000000000
.Llinux_tls_mxcsr:
    .long 0x3f80
.Llinux_tls_control:
    .short 0x077f
.section .bss.linux_client_tls,"aw",@nobits
.p2align 3
.Llinux_tls_scratch:
    .zero 8
.section .note.GNU-stack,"",@progbits
