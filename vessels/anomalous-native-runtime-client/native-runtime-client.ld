OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_native_runtime_client_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x400000;
    anomalous_native_runtime_client_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.native_runtime_client_entry))
        KEEP(*(.text.native_runtime_client_gate))
        *(.text .text.*)
    } :image =0x90909090
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_native_runtime_client_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_native_runtime_client_image_end = .;
    . = 0x800000;
    anomalous_native_runtime_client_data_start = .;
    .data : ALIGN(16) {
        KEEP(*(.data.native_runtime_client_bootargs))
        . = 0x801000;
        anomalous_native_runtime_client_startup = .;
        anomalous_native_runtime_client_descriptor_start = .;
        KEEP(*(.data.native_runtime_client_descriptor))
        anomalous_native_runtime_client_descriptor_end = .;
        *(.data .data.*)
    } :data
    anomalous_native_runtime_client_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_native_runtime_client_bss_start = .;
        *(.bss .bss.* COMMON)
        anomalous_native_runtime_client_bss_end = .;
    } :data
    anomalous_native_runtime_client_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_native_runtime_client_entry == anomalous_native_runtime_client_image_start, "native runtime entry moved")
ASSERT(anomalous_native_runtime_client_image_end - anomalous_native_runtime_client_image_start <= 32768, "native runtime RX too large")
ASSERT(anomalous_native_runtime_client_descriptor_start == 0x801000, "native descriptor moved")
ASSERT(anomalous_native_runtime_client_descriptor_end - anomalous_native_runtime_client_descriptor_start == 128, "native descriptor size")
ASSERT(anomalous_native_runtime_client_data_end - anomalous_native_runtime_client_data_start <= 8192, "native runtime RW too large")
ASSERT(SIZEOF(.forbidden) == 0, "native runtime TLS/GOT/PLT forbidden")
