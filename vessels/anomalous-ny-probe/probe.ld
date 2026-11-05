OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(anomalous_ny_probe_entry)
PHDRS {
    image PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0x400000;
    anomalous_ny_probe_image_start = .;
    .text : ALIGN(16) {
        KEEP(*(.text.ny_probe_entry))
        KEEP(*(.text.ny_probe_shim))
        . = ALIGN(16);
        anomalous_ny_probe_compiledcode_start = .;
        *(.text .text.*)
        anomalous_ny_probe_compiledcode_end = .;
    } :image
    .rodata : ALIGN(16) { *(.rodata .rodata.*) } :image
    anomalous_ny_probe_image_end = .;

    . = 0x800000;
    anomalous_ny_probe_data_start = .;
    .data : ALIGN(8) { KEEP(*(.data.ny_probe_mailbox)) } :data
    anomalous_ny_probe_data_end = .;
    . = 0x800100;
    .bss (NOLOAD) : ALIGN(16) {
        anomalous_ny_probe_bss_start = .;
        *(.bss .bss.* COMMON)
        anomalous_ny_probe_bss_end = .;
    } :data

    .forbidden (NOLOAD) : {
        *(.data .data.* .data.rel.ro .data.rel.ro.*)
        *(.tdata .tdata.* .tbss .tbss.*)
        *(.got .got.* .plt .plt.* .iplt .igot.*)
        *(.init .fini .init_array .init_array.* .fini_array .fini_array.*)
        *(.preinit_array .preinit_array.* .ctors .dtors)
        *(.interp .dynamic .dynsym .dynstr .hash .gnu.hash)
        *(.rela .rela.* .rel .rel.*)
    }
    /DISCARD/ : { *(.comment .eh_frame .eh_frame.* .eh_frame_hdr .note.GNU-stack) }
}
ASSERT(anomalous_ny_probe_entry == 0x400000, "Ny probe entry moved")
ASSERT(anomalous_ny_probe_image_end - anomalous_ny_probe_image_start <= 0x1000,
       "Ny probe RX exceeds one page")
ASSERT(anomalous_ny_probe_compiledcode_end > anomalous_ny_probe_compiledcode_start,
       "Ny probe compiled code is absent")
ASSERT(anomalous_ny_probe_data_end - anomalous_ny_probe_data_start == 128,
       "Ny probe mailbox must contain exactly 128 initialized bytes")
ASSERT(anomalous_ny_probe_bss_start == 0x800100, "Ny probe BSS moved")
ASSERT(anomalous_ny_probe_bss_end - anomalous_ny_probe_bss_start > 0 &&
       anomalous_ny_probe_bss_end - anomalous_ny_probe_bss_start <= 256,
       "Ny probe BSS exceeds its bounded global-state extent")
ASSERT(SIZEOF(.forbidden) == 0, "Ny probe has unsupported sections")
