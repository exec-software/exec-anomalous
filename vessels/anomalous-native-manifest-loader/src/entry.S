.section .text.native_manifest_loader_entry,"ax",@progbits
.p2align 4
.global anomalous_native_manifest_loader_entry
.type anomalous_native_manifest_loader_entry, @function
anomalous_native_manifest_loader_entry:
    # Preserve the exact copied ANPG pointer/length through startup admission.
    cld
    movq $0, %rax
    int $0xf4
.global anomalous_native_manifest_loader_startup_return
anomalous_native_manifest_loader_startup_return:
    callq anomalous_native_manifest_loader_run
    ud2
.size anomalous_native_manifest_loader_entry, . - anomalous_native_manifest_loader_entry

.section .text.native_manifest_loader_gate,"ax",@progbits
.p2align 4
.global anomalous_native_manifest_loader_call
.type anomalous_native_manifest_loader_call, @function
anomalous_native_manifest_loader_call:
    pushq %rbx
    movq %rdi, %rbx
    movq 0(%rbx), %rax
    movq 8(%rbx), %rdi
    movq 16(%rbx), %rsi
    movq 24(%rbx), %rdx
    movq 32(%rbx), %rcx
    movq 40(%rbx), %r8
    int $0xf4
.global anomalous_native_manifest_loader_syscall_return
anomalous_native_manifest_loader_syscall_return:
    movq %rax, 48(%rbx)
    movq %rcx, 56(%rbx)
    movq %rdx, 64(%rbx)
    popq %rbx
    retq
.size anomalous_native_manifest_loader_call, . - anomalous_native_manifest_loader_call

.global anomalous_native_manifest_loader_fault_instruction
anomalous_native_manifest_loader_fault_instruction:
    ud2
.section .note.GNU-stack,"",@progbits

.section .data.native_manifest_loader_descriptor,"aw",@progbits
.p2align 4
.zero 256
