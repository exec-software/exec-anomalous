OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_elf_loader_entry)
PHDRS { image PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
    . = 0x400000;
    anomalous_elf_loader_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.elf_loader_entry))
        KEEP(*(.text.elf_loader_gate))
        *(.text .text.*)
    } :image =0x90909090
    .rodata : ALIGN(16) { *(.rodata .rodata.*) *(.data.rel.ro .data.rel.ro.*) } :image
    anomalous_elf_loader_image_payload_end = .;
    . = ALIGN(0x1000);
    anomalous_elf_loader_image_end = .;
    . = 0x800000;
    anomalous_elf_loader_data_start = .;
    .data : ALIGN(16) { KEEP(*(.data.elf_loader_bootargs)) *(.data .data.*) } :data
    anomalous_elf_loader_data_file_end = .;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_elf_loader_bss_start = .;
        *(.bss .bss.* COMMON)
        anomalous_elf_loader_bss_end = .;
    } :data
    anomalous_elf_loader_data_end = .;
    .forbidden (NOLOAD) : { *(.tdata .tdata.*) *(.tbss .tbss.*) *(.got .got.*) *(.plt .plt.*) }
    /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note*) }
}
ASSERT(anomalous_elf_loader_entry == anomalous_elf_loader_image_start, "ELF loader entry moved")
ASSERT(anomalous_elf_loader_image_end - anomalous_elf_loader_image_start <= 32768, "ELF loader RX too large")
ASSERT(anomalous_elf_loader_data_end - anomalous_elf_loader_data_start <= 8192, "ELF loader RW too large")
ASSERT(anomalous_elf_loader_data_file_end - anomalous_elf_loader_data_start >= 128, "ELF loader bootargs missing")
ASSERT(SIZEOF(.forbidden) == 0, "ELF loader TLS/GOT/PLT forbidden")
