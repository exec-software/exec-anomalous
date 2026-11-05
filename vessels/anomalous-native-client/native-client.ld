OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_native_client_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x400000;
    anomalous_native_client_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.native_client_entry))
        KEEP(*(.text.native_client_gate))
        *(.text .text.*)
    } :image =0x90909090
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_native_client_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_native_client_image_end = .;
    . = 0x800000;
    anomalous_native_client_data_start = .;
    /* The kernel owns these initial arguments; Rust globals cannot alias them. */
    .data : ALIGN(16) { KEEP(*(.data.native_client_bootargs)) *(.data .data.*) } :data
    anomalous_native_client_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_native_client_bss_start = .;
        *(.bss .bss.* COMMON)
        anomalous_native_client_bss_end = .;
    } :data
    anomalous_native_client_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_native_client_entry == anomalous_native_client_image_start, "native entry moved")
ASSERT(anomalous_native_client_image_end - anomalous_native_client_image_start <= 32768, "native RX too large")
ASSERT(anomalous_native_client_data_end - anomalous_native_client_data_start <= 8192, "native RW too large")
ASSERT(SIZEOF(.forbidden) == 0, "native TLS/GOT/PLT forbidden")
