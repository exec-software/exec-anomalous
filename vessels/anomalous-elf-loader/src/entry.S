.section .text.elf_loader_entry,"ax",@progbits
.p2align 4
.global anomalous_elf_loader_entry
.type anomalous_elf_loader_entry, @function
anomalous_elf_loader_entry:
    # Startup admission observes the untouched initial RFLAGS and stack top.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_elf_loader_startup_return
anomalous_elf_loader_startup_return:
    testq %rax, %rax
    jne anomalous_elf_loader_fault_instruction
    testq %rcx, %rcx
    jne anomalous_elf_loader_fault_instruction
    testq %rdx, %rdx
    jne anomalous_elf_loader_fault_instruction
    callq anomalous_elf_loader_run
    ud2
.size anomalous_elf_loader_entry, . - anomalous_elf_loader_entry

.section .text.elf_loader_gate,"ax",@progbits
.p2align 4
.global anomalous_elf_loader_call
.type anomalous_elf_loader_call, @function
anomalous_elf_loader_call:
    # The sole pointer dereferenced here names a private live repr(C) record.
    # INTf4 preserves SysV callee-saved registers and all non-result registers.
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    int $0xf4
.global anomalous_elf_loader_syscall_return
anomalous_elf_loader_syscall_return:
    movq %rax, 40(%rbx)
    movq %rcx, 48(%rbx)
    movq %rdx, 56(%rbx)
    popq %rbx
    retq
.size anomalous_elf_loader_call, . - anomalous_elf_loader_call

.p2align 4
.global anomalous_elf_loader_fault_instruction
.type anomalous_elf_loader_fault_instruction, @function
anomalous_elf_loader_fault_instruction:
    # No loader fault is expected or armed; reaching this marker fails the proof.
    ud2
.size anomalous_elf_loader_fault_instruction, . - anomalous_elf_loader_fault_instruction
.section .note.GNU-stack,"",@progbits

.section .data.elf_loader_bootargs,"aw",@progbits
.p2align 4
.zero 128
