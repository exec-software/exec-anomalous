.text
.p2align 4

.global x86_64_fault_test_invalid_opcode
.type x86_64_fault_test_invalid_opcode, @function
# The exact address of this feature-local instruction is armed before entry.
# Returning would be treated as test failure rather than success.
x86_64_fault_test_invalid_opcode:
    ud2
    retq
.size x86_64_fault_test_invalid_opcode, . - x86_64_fault_test_invalid_opcode

.section .note.GNU-stack,"",@progbits
