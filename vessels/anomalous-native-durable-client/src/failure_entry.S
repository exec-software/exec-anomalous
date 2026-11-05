.section .text.native_durable_client_cut,"ax",@progbits
.global anomalous_native_durable_client_cut_fault_instruction
.type anomalous_native_durable_client_cut_fault_instruction,@function
anomalous_native_durable_client_cut_fault_instruction:
    ud2
.size anomalous_native_durable_client_cut_fault_instruction, .-anomalous_native_durable_client_cut_fault_instruction
.section .note.GNU-stack,"",@progbits
