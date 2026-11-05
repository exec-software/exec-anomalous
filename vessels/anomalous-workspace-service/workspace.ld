OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_workspace_service_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x0000600000001000;
    anomalous_workspace_service_image_start = .;
    .entry : ALIGN(16) { KEEP(*(.text.workspace_entry)) } :image
    .text : ALIGN(16) { KEEP(*(.text.workspace_rite)) *(.text .text.*) *(.ltext .ltext.*) } :image
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.lrodata .lrodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_workspace_service_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_workspace_service_image_end = .;
    . = 0x0000600000081000;
    anomalous_workspace_service_data_start = .;
    .data : ALIGN(16) { *(.data .data.*) *(.ldata .ldata.*) } :data
    anomalous_workspace_service_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) { *(.bss .bss.* COMMON) *(.lbss .lbss.*) } :data
    anomalous_workspace_service_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_workspace_service_entry == anomalous_workspace_service_image_start, "workspace entry moved")
ASSERT(anomalous_workspace_service_image_end - anomalous_workspace_service_image_start <= 0x40000, "workspace RX too large")
ASSERT(anomalous_workspace_service_data_end - anomalous_workspace_service_data_start <= 0x7000, "workspace data too large")
ASSERT(SIZEOF(.forbidden) == 0, "workspace TLS/GOT/PLT forbidden")
