OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_linux_veil_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x400000;
    anomalous_linux_veil_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.linux_veil_entry))
        KEEP(*(.text.linux_veil_gate))
        *(.text .text.*)
    } :image =0x90909090
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_linux_veil_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_linux_veil_image_end = .;
    . = 0x800000;
    anomalous_linux_veil_data_start = .;
    .data : ALIGN(16) {
        KEEP(*(.data.linux_veil_reserved))
        anomalous_linux_veil_data_canary_start = .;
        KEEP(*(.data.elf_canary))
        anomalous_linux_veil_data_canary_end = .;
        anomalous_linux_veil_file_source_binding_start = .;
        KEEP(*(.data.file_source))
        anomalous_linux_veil_file_source_binding_end = .;
        *(.data .data.*)
    } :data
    anomalous_linux_veil_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_linux_veil_bss_start = .;
        anomalous_linux_veil_file_source_report_start = .;
        KEEP(*(.bss.file_source_report))
        anomalous_linux_veil_file_source_report_end = .;
        anomalous_linux_veil_file_source_buffer_start = .;
        KEEP(*(.bss.file_source_buffer))
        anomalous_linux_veil_file_source_buffer_end = .;
        anomalous_linux_veil_file_service_report_start = .;
        KEEP(*(.bss.file_service_report))
        anomalous_linux_veil_file_service_report_end = .;
        *(.bss .bss.* COMMON)
        anomalous_linux_veil_bss_end = .;
    } :data
    anomalous_linux_veil_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_linux_veil_entry == anomalous_linux_veil_image_start, "Linux veil entry moved")
ASSERT(anomalous_linux_veil_image_end - anomalous_linux_veil_image_start <= 32768, "Linux veil RX too large")
ASSERT(anomalous_linux_veil_data_end - anomalous_linux_veil_data_start <= 8192, "Linux veil RW too large")
ASSERT(anomalous_linux_veil_data_canary_start == 0x800080, "Linux veil reserved prefix changed")
ASSERT(anomalous_linux_veil_data_canary_end - anomalous_linux_veil_data_canary_start == 8, "Linux veil canary changed")
ASSERT(anomalous_linux_veil_file_source_binding_start == 0x800088, "Linux veil file source binding moved")
ASSERT(anomalous_linux_veil_file_source_binding_end - anomalous_linux_veil_file_source_binding_start == (DEFINED(anomalous_linux_veil_file_source) ? 32 : 0), "Linux veil file source binding size changed")
ASSERT(anomalous_linux_veil_data_file_end - anomalous_linux_veil_data_start == (DEFINED(anomalous_linux_veil_file_source) ? 168 : 136), "Linux veil initialized data changed")
ASSERT(anomalous_linux_veil_file_source_report_end - anomalous_linux_veil_file_source_report_start == (DEFINED(anomalous_linux_veil_file_source) ? 32 : 0), "Linux veil file source report size changed")
ASSERT(!DEFINED(anomalous_linux_veil_file_source) || anomalous_linux_veil_file_source_report_start == 0x8000b0, "Linux veil file source report moved")
ASSERT(anomalous_linux_veil_file_source_buffer_end - anomalous_linux_veil_file_source_buffer_start == (DEFINED(anomalous_linux_veil_file_source) ? 4096 : 0), "Linux veil file source buffer size changed")
ASSERT(!DEFINED(anomalous_linux_veil_file_source) || anomalous_linux_veil_file_source_buffer_start == 0x8000d0, "Linux veil file source buffer moved")
ASSERT(anomalous_linux_veil_file_service_report_end - anomalous_linux_veil_file_service_report_start == (DEFINED(anomalous_linux_veil_file_service_report) ? 80 : 0), "Linux veil file service report size changed")
ASSERT(!DEFINED(anomalous_linux_veil_file_service_report) || anomalous_linux_veil_file_service_report_start == 0x8010d0, "Linux veil file service report moved")
ASSERT(!DEFINED(anomalous_linux_veil_file_source) || anomalous_linux_veil_data_end <= 0x801ff8, "Linux veil overlaps source boundary witness")
ASSERT(SIZEOF(.forbidden) == 0, "Linux veil TLS/GOT/PLT forbidden")
