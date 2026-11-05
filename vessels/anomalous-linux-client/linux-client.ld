OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_linux_client_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x400000;
    anomalous_linux_client_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.linux_client_entry))
        KEEP(*(.text.linux_client_syscall))
        *(.text .text.*)
    } :image =0x90909090
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_linux_client_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_linux_client_image_end = .;
    . = 0x800000;
    anomalous_linux_client_data_start = .;
    .data : ALIGN(16) {
        KEEP(*(.data.linux_client_reserved))
        anomalous_linux_client_data_canary_start = .;
        KEEP(*(.data.elf_canary))
        anomalous_linux_client_data_canary_end = .;
        *(.data .data.*)
    } :data
    anomalous_linux_client_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_linux_client_bss_start = .;
        *(.bss .bss.* COMMON)
        anomalous_linux_client_bss_objects_end = .;
        . = ABSOLUTE(0x801ff8);
        anomalous_linux_client_poll_tail = .;
        . += 8;
        anomalous_linux_client_bss_end = .;
    } :data
    anomalous_linux_client_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_linux_client_entry == anomalous_linux_client_image_start, "Linux entry moved")
ASSERT(anomalous_linux_client_image_end - anomalous_linux_client_image_start <= 32768, "Linux RX too large")
ASSERT(anomalous_linux_client_data_end - anomalous_linux_client_data_start <= 8192, "Linux RW too large")
ASSERT(anomalous_linux_client_data_canary_start == anomalous_linux_client_data_start + 128, "Linux canary moved")
ASSERT(anomalous_linux_client_data_canary_end - anomalous_linux_client_data_canary_start == 8, "Linux canary size changed")
ASSERT(SIZEOF(.forbidden) == 0, "Linux TLS/GOT/PLT forbidden")
ASSERT(anomalous_linux_client_bss_objects_end <= anomalous_linux_client_poll_tail, "Linux BSS overlaps poll tail probe")
ASSERT(anomalous_linux_client_poll_tail == 0x801ff8, "Linux poll tail probe moved")
