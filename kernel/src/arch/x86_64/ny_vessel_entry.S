.set ny_vessel_kernel_data_selector, 0x10
.set ny_vessel_user_data_selector, 0x2b
.set ny_vessel_rite_gate_vector, 0xf3
.set ny_vessel_sanitized_flags, 0x02

.text
.p2align 4

.global x86_64_enter_ny_vessel_task
.type x86_64_enter_ny_vessel_task, @function
# System V inputs are RDI = user RIP, RSI = user RSP, RDX = user data/stack
# selector, RCX = user code selector, and R8 = saved-kernel-RSP pointer. The
# function preserves the caller's callee-saved registers on its original stack.
# Exact task exit returns through x86_64_resume_ny_vessel_task; ordinary return
# from the vessel is impossible.
x86_64_enter_ny_vessel_task:
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
    pushq $ny_vessel_sanitized_flags
    pushq %rcx
    pushq %rdi

    # The vessel has no floating-point ABI. Its Ny object and the freestanding
    # runtime it links are both general-register only, which is checked at
    # build time, so TS may deny inherited x87, MMX and SIMD state outright.
    # Clearing every GPR leaves only mapped code and the vessel's own writable
    # pages as feature-local inputs.
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
.size x86_64_enter_ny_vessel_task, . - x86_64_enter_ny_vessel_task

.global x86_64_resume_ny_vessel_task
.type x86_64_resume_ny_vessel_task, @function
# RDI is the fixed completion status, RSI is the single-use saved kernel stack,
# and RDX is the original CR0.TS bit. The routine abandons the TSS privilege
# stack and returns through the original Rust call frame. It is valid only on
# the bootstrap processor while IF and NMI remain masked.
x86_64_resume_ny_vessel_task:
    testq %rdx, %rdx
    jnz .Lny_vessel_task_switched_restored
    clts
.Lny_vessel_task_switched_restored:
    movw $ny_vessel_kernel_data_selector, %ax
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
.size x86_64_resume_ny_vessel_task, . - x86_64_resume_ny_vessel_task

.global x86_64_ny_vessel_gate_entry
.type x86_64_ny_vessel_gate_entry, @function
# The DPL-3 interrupt gate supplies RIP, CS, RFLAGS, RSP, and SS. Synthetic
# vector and error words make a 22-qword frame. R12 retains its pre-alignment
# address across Rust; returning calls restore every user register. A valid
# task-exit dispatch resumes the saved kernel stack nonlocally and never
# returns.
x86_64_ny_vessel_gate_entry:
    cld
    pushq $0
    pushq $ny_vessel_rite_gate_vector
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
    movw $ny_vessel_kernel_data_selector, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movq %rsp, %r12
    movq %rsp, %rdi
    andq $-16, %rsp
    callq x86_64_ny_vessel_gate_dispatch
    movq %r12, %rsp
    movw $ny_vessel_user_data_selector, %ax
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
.size x86_64_ny_vessel_gate_entry, . - x86_64_ny_vessel_gate_entry

.section .note.GNU-stack,"",@progbits
