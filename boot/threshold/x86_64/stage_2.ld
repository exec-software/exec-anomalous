OUTPUT_FORMAT(elf64-x86-64)
ENTRY(threshold_entry)

PHDRS {
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS {
    . = 0x10000;
    threshold_image_start = .;

    .text : ALIGN(16) {
        KEEP(*(.text.threshold))
        *(.text .text.*)
    } :text

    .rodata : ALIGN(16) {
        KEEP(*(.rodata.threshold))
        *(.rodata .rodata.*)
    } :text

    .data : ALIGN(16) {
        *(.data .data.*)
        *(.data.rel.ro .data.rel.ro.*)
        *(.got .got.*)
    } :data
    threshold_file_end = .;

    .relocation_guard : {
        *(.rel .rel.*)
        *(.rela .rela.*)
        *(.iplt .plt .plt.*)
        *(.igot .igot.*)
    } :data

    .boot_workspace (NOLOAD) : ALIGN(16) {
        *(.boot_workspace)
    } :data

    .bss (NOLOAD) : ALIGN(16) {
        *(COMMON)
        *(.bss .bss.*)
    } :data

    .debug_abbrev 0 : { *(.debug_abbrev) }
    .debug_aranges 0 : { *(.debug_aranges) }
    .debug_frame 0 : { *(.debug_frame) }
    .debug_info 0 : { *(.debug_info) }
    .debug_line 0 : { *(.debug_line) }
    .debug_loc 0 : { *(.debug_loc) }
    .debug_ranges 0 : { *(.debug_ranges) }
    .debug_str 0 : { *(.debug_str) }

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame*)
        *(.note*)
    }
}

ASSERT(threshold_entry == 0x10000, "Threshold stage 2 must load at 0x10000")
ASSERT(threshold_file_end > threshold_image_start, "Threshold stage 2 must not be empty")
ASSERT(SIZEOF(.relocation_guard) == 0, "Threshold stage 2 must be fully static")
ASSERT(
    threshold_file_end - threshold_image_start <= 127 * 512,
    "Threshold stage 2 exceeds its one-transfer load window"
)
ASSERT(
    ADDR(.boot_workspace) + SIZEOF(.boot_workspace) - threshold_entry <= 64K,
    "Threshold BIOS workspace must remain within one real-mode segment"
)
ASSERT(
    ADDR(.bss) + SIZEOF(.bss) <= 0x30000,
    "Threshold runtime memory overlaps kernel staging"
)
