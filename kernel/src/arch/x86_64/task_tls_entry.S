# Feature-private FS ownership. No stack frame offsets change. Caller GPRs are
# already saved for capture/return; helpers use only integer registers and never
# consult user memory, SWAPGS, or FS/GS-relative addressing.
.text
.p2align 4

.macro tls_read_msr number
    movl $\number, %ecx
    rdmsr
    shlq $32, %rdx
    orq %rdx, %rax
.endm

# Always load the zero-base kernel descriptors before explicit base writes.
# This removes CPU-specific null-selector/base behavior from the contract.
.type x86_64_task_tls_clean_bases, @function
x86_64_task_tls_clean_bases:
    movw $0x10, %ax
    movw %ax, %fs
    movw %ax, %gs
    xorl %eax, %eax
    xorl %edx, %edx
    movl $0xc0000100, %ecx
    wrmsr
    movl $0xc0000101, %ecx
    wrmsr
    movl $0xc0000102, %ecx
    wrmsr
    retq
.size x86_64_task_tls_clean_bases, . - x86_64_task_tls_clean_bases

# Return EAX=1 only for explicit zero bases. Selector policy is checked by the
# caller because return-to-user already loaded null selectors.
.type x86_64_task_tls_zero_bases, @function
x86_64_task_tls_zero_bases:
    tls_read_msr 0xc0000100
    testq %rax, %rax
    jne .Ltls_not_zero
    tls_read_msr 0xc0000101
    testq %rax, %rax
    jne .Ltls_not_zero
    tls_read_msr 0xc0000102
    testq %rax, %rax
    jne .Ltls_not_zero
    movl $1, %eax
    retq
.Ltls_not_zero:
    xorl %eax, %eax
    retq
.size x86_64_task_tls_zero_bases, . - x86_64_task_tls_zero_bases

.global x86_64_task_tls_capture_entry
.type x86_64_task_tls_capture_entry, @function
# RDI=hardware-created normalized frame on the trusted supervisor stack.
# Inactive means the legacy selected runtime: leave its behavior unchanged.
x86_64_task_tls_capture_entry:
    cmpq ${phase_inactive}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    je .Ltls_capture_done
    movq %cr4, %rax
    testq $0x10000, %rax
    jne .Ltls_capture_failed
    pushfq
    popq %rax
    testq $0x200, %rax
    jne .Ltls_capture_failed
    movq 144(%rdi), %rax
    andq $3, %rax
    cmpq $3, %rax
    je .Ltls_capture_user
    testq %rax, %rax
    jne .Ltls_capture_failed
    # Kernel-origin entry must never consume or replace a user's capsule.
    cmpq ${phase_clean}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jne .Ltls_capture_failed
    movw %fs, %ax
    cmpw $0x10, %ax
    jne .Ltls_capture_failed
    movw %gs, %ax
    cmpw $0x10, %ax
    jne .Ltls_capture_failed
    callq x86_64_task_tls_zero_bases
    testl %eax, %eax
    jz .Ltls_capture_failed
    retq
.Ltls_capture_user:
    cmpq ${phase_user}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jne .Ltls_capture_failed
    cmpq %rdi, x86_64_task_tls_capsule+{capsule_frame}(%rip)
    jne .Ltls_capture_failed
    cmpq $1, x86_64_task_tls_capsule+{capsule_slot}(%rip)
    jb .Ltls_capture_failed
    movq x86_64_task_tls_active_slots(%rip), %rax
    cmpq $2, %rax
    jb .Ltls_capture_failed
    cmpq ${max_task_count}, %rax
    ja .Ltls_capture_failed
    cmpq %rax, x86_64_task_tls_capsule+{capsule_slot}(%rip)
    ja .Ltls_capture_failed
    xorl %eax, %eax
    movw %fs, %ax
    movq %rax, x86_64_task_tls_capsule+{capsule_fs_selector}(%rip)
    xorl %eax, %eax
    movw %gs, %ax
    movq %rax, x86_64_task_tls_capsule+{capsule_gs_selector}(%rip)
    tls_read_msr 0xc0000100
    movq %rax, x86_64_task_tls_capsule+{capsule_fs}(%rip)
    tls_read_msr 0xc0000101
    movq %rax, x86_64_task_tls_capsule+{capsule_gs}(%rip)
    tls_read_msr 0xc0000102
    movq %rax, x86_64_task_tls_capsule+{capsule_kernel_gs}(%rip)
    movq ${phase_captured}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jmp x86_64_task_tls_clean_bases
.Ltls_capture_failed:
    # The normal Rust dispatch receives clean hardware and a poisoned capsule;
    # it will reject the phase. Do not recursively fault before Rust's latch.
    movq ${phase_failed}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jmp x86_64_task_tls_clean_bases
.Ltls_capture_done:
    retq
.size x86_64_task_tls_capture_entry, . - x86_64_task_tls_capture_entry

.global x86_64_task_tls_restore_user
.type x86_64_task_tls_restore_user, @function
# RDI=exact selected frame, after null FS/GS selectors but before GPR pops.
x86_64_task_tls_restore_user:
    cmpq ${phase_inactive}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    je .Ltls_return_done
    cmpq ${phase_staged}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jne .Ltls_return_failed
    cmpq %rdi, x86_64_task_tls_capsule+{capsule_frame}(%rip)
    jne .Ltls_return_failed
    cmpq $1, x86_64_task_tls_capsule+{capsule_slot}(%rip)
    jb .Ltls_return_failed
    movq x86_64_task_tls_active_slots(%rip), %rax
    cmpq $2, %rax
    jb .Ltls_return_failed
    cmpq ${max_task_count}, %rax
    ja .Ltls_return_failed
    cmpq %rax, x86_64_task_tls_capsule+{capsule_slot}(%rip)
    ja .Ltls_return_failed
    cmpq $0x33, 144(%rdi)
    jne .Ltls_return_failed
    cmpq $0x2b, 168(%rdi)
    jne .Ltls_return_failed
    movq %cr4, %rax
    testq $0x10000, %rax
    jne .Ltls_return_failed
    pushfq
    popq %rax
    testq $0x200, %rax
    jne .Ltls_return_failed
    movw %fs, %ax
    testw %ax, %ax
    jne .Ltls_return_failed
    movw %gs, %ax
    testw %ax, %ax
    jne .Ltls_return_failed
    callq x86_64_task_tls_zero_bases
    testl %eax, %eax
    jz .Ltls_return_failed
    movq x86_64_task_tls_capsule+{capsule_return_fs}(%rip), %r8
    movq %r8, %rax
    shrq $47, %rax
    jnz .Ltls_return_failed
    movq %r8, %rax
    movq %r8, %rdx
    shrq $32, %rdx
    movl $0xc0000100, %ecx
    wrmsr
    tls_read_msr 0xc0000100
    cmpq %r8, %rax
    jne .Ltls_return_failed
    movq ${phase_user}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
.Ltls_return_done:
    retq
.Ltls_return_failed:
    movq ${phase_failed}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    callq x86_64_task_tls_clean_bases
    # A CPL0 exception is fatal; its capture helper leaves hardware clean and
    # returns to the common fatal dispatcher rather than recursively faulting.
    ud2
.size x86_64_task_tls_restore_user, . - x86_64_task_tls_restore_user

.global x86_64_task_tls_resume_kernel
.type x86_64_task_tls_resume_kernel, @function
# Bootstrap resume preserves RDI/RSI/RDX inputs and all ABI callee-saved GPRs.
# The session remains active/clean until finish executes on bootstrap RSP.
x86_64_task_tls_resume_kernel:
    cmpq ${phase_inactive}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    je .Ltls_resume_done
    pushq %rax
    pushq %rcx
    pushq %rdx
    cmpq ${phase_clean}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    jne .Ltls_resume_failed
    movq %cr4, %rax
    testq $0x10000, %rax
    jne .Ltls_resume_failed
    pushfq
    popq %rax
    testq $0x200, %rax
    jne .Ltls_resume_failed
    movw %fs, %ax
    cmpw $0x10, %ax
    jne .Ltls_resume_failed
    movw %gs, %ax
    cmpw $0x10, %ax
    jne .Ltls_resume_failed
    callq x86_64_task_tls_zero_bases
    testl %eax, %eax
    jz .Ltls_resume_failed
    callq x86_64_task_tls_clean_bases
    popq %rdx
    popq %rcx
    popq %rax
.Ltls_resume_done:
    retq
.Ltls_resume_failed:
    movq ${phase_failed}, x86_64_task_tls_capsule+{capsule_phase}(%rip)
    callq x86_64_task_tls_clean_bases
    ud2
.size x86_64_task_tls_resume_kernel, . - x86_64_task_tls_resume_kernel
.section .note.GNU-stack,"",@progbits
